//! The HTTP API under `/api` and its OpenAPI description.

mod account;
mod auth;
mod backups;
mod browse;
mod collections;
mod covers;
mod error;
mod health;
mod history;
mod kept;
mod layout;
mod libraries;
mod lists;
mod people;
mod reading;
mod scans;
mod session;
mod settings;
mod updates;
mod users;
mod views;

use axum::body::Bytes;
use axum::http::header;
use axum::routing::{any, get};
use axum::{Router, middleware};
use utoipa::openapi::security::{
    ApiKey, ApiKeyValue, HttpAuthScheme, HttpBuilder, SecurityRequirement, SecurityScheme,
};
use utoipa::{Modify, OpenApi};
use utoipa_axum::router::OpenApiRouter;

pub use self::error::ApiError;
use crate::app::AppState;

/// How an image that never changes under its address is cached: for good.
const KEPT_FOR_GOOD: &str = "private, max-age=31536000, immutable";

const ROOT: &str = "/api";
const V1: &str = "/v1";

#[derive(OpenApi)]
#[openapi(
    info(
        title = "Binge Server API",
        description = "HTTP API of the Binge comic server.\n\nEvery route except `/health`, `/setup` and `/sign-in` takes a session cookie or an API token, and answers 401 with the code `unauthorized` without a good one. Routes that manage accounts (`/me/password`, `/tokens`, `/users`) take a session only."
    ),
    components(schemas(ApiError)),
    modifiers(&HowToSignIn),
    // What every route asks for unless it says otherwise: one of the two.
    security(("token" = []), ("session" = [])),
    tags(
        (name = "server", description = "The server itself"),
        (name = "account", description = "Setup, signing in, tokens and users"),
        (name = "libraries", description = "Folders of comics and their scans"),
        (name = "browse", description = "Series, books and covers"),
        (name = "layout", description = "Changing series, their groups and where books sit"),
        (name = "covers", description = "Choosing the cover of a book, a series or a reading list"),
        (name = "reading", description = "Pages, progress, and what to read next"),
        (name = "lists", description = "Reading lists, and making one from a CBL file"),
        (name = "collections", description = "Collections: series picked by hand, from any library"),
        (name = "people", description = "Who the books name, and the names they go by"),
        (name = "views", description = "A library narrowed and ordered, kept under a name"),
    )
)]
struct ApiDoc;

/// Describes the two ways a request shows who makes it.
struct HowToSignIn;

impl Modify for HowToSignIn {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.get_or_insert_default();
        let token = HttpBuilder::new()
            .scheme(HttpAuthScheme::Bearer)
            .description(Some(
                "An API token, made with `POST /tokens`. What apps use.",
            ))
            .build();
        components.add_security_scheme("token", SecurityScheme::Http(token));
        let session = ApiKeyValue::with_description(
            auth::SESSION_COOKIE,
            "The cookie that signing in sets. What the web UI uses.",
        );
        components.add_security_scheme("session", SecurityScheme::ApiKey(ApiKey::Cookie(session)));
    }
}

// The routes of version 1, relative to `/api/v1`, in three rings. A ring
// decides what its routes ask for, in the server and in the description
// alike: a route says nothing about it itself.

/// What anyone may call: the server's status, first-run setup, signing in.
fn open() -> OpenApiRouter<AppState> {
    let routes = health::routes().merge(session::open_routes());
    asking(routes, SecurityRequirement::default())
}

/// What takes a session or a token.
fn guarded() -> OpenApiRouter<AppState> {
    session::routes()
        .merge(libraries::routes())
        .merge(browse::routes())
        .merge(layout::routes())
        .merge(covers::routes())
        .merge(reading::routes())
        .merge(lists::routes())
        .merge(collections::routes())
        .merge(people::routes())
        .merge(views::routes())
        .merge(settings::routes())
        .merge(history::routes())
        .merge(kept::routes())
        .merge(scans::routes())
        .merge(backups::routes())
        .merge(updates::routes())
}

/// What takes a session: managing accounts is not for tokens.
fn in_person() -> OpenApiRouter<AppState> {
    let routes = account::routes().merge(users::routes());
    asking(routes, SecurityRequirement::new("session", [""; 0]))
}

/// Writes into the description what the routes of a ring ask for, where
/// that is not what [`ApiDoc`] says of every route.
fn asking(
    mut ring: OpenApiRouter<AppState>,
    proof: SecurityRequirement,
) -> OpenApiRouter<AppState> {
    for path in ring.get_openapi_mut().paths.paths.values_mut() {
        let operations = [
            &mut path.get,
            &mut path.put,
            &mut path.post,
            &mut path.delete,
            &mut path.patch,
            &mut path.head,
            &mut path.options,
            &mut path.trace,
        ];
        for operation in operations.into_iter().flatten() {
            operation.security = Some(vec![proof.clone()]);
        }
    }
    ring
}

/// The OpenAPI description, generated from the routes.
pub fn description() -> utoipa::openapi::OpenApi {
    let v1 = open().merge(guarded()).merge(in_person());
    let api = OpenApiRouter::new().nest(V1, v1);
    OpenApiRouter::with_openapi(ApiDoc::openapi())
        .nest(ROOT, api)
        .into_openapi()
}

/// Adds the API to the application under `/api`: the routes, the check of
/// who calls them, the description of them, and an [`ApiError`] for every
/// request that fails there, unknown paths included.
pub fn mount(app: Router<AppState>, state: &AppState) -> Router<AppState> {
    // The description never changes while the server runs, so render it once.
    let description = description()
        .to_json()
        .expect("the API description is plain data and always serializes");
    let description = Bytes::from(description);

    let (open, _) = open().split_for_parts();
    let (guarded, _) = guarded().split_for_parts();
    let (in_person, _) = in_person().split_for_parts();
    let guarded = guarded
        .route(
            "/openapi.json",
            get(move || {
                let description = description.clone();
                async move { ([(header::CONTENT_TYPE, "application/json")], description) }
            }),
        )
        .merge(in_person.layer(middleware::from_fn(auth::in_person)))
        // Added last, so it is asked first: nothing above runs for a
        // request that does not show who makes it.
        .layer(middleware::from_fn_with_state(
            state.accounts.clone(),
            auth::signed_in,
        ));
    let api = Router::new()
        .nest(V1, open.merge(guarded))
        .fallback(not_found)
        .layer(middleware::from_fn(auth::same_origin))
        .layer(middleware::map_response(error::json_errors));

    // A nested router answers `/api` and everything below it, but not the
    // bare `/api/`.
    app.nest(ROOT, api)
        .route(&format!("{ROOT}/"), any(not_found))
}

async fn not_found() -> ApiError {
    ApiError::not_found("No such API route.")
}
