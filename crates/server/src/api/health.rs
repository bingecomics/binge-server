//! The server's own status.

use axum::Json;
use axum::extract::State;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use crate::app::AppState;
use crate::built;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(health))
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum DatabaseStatus {
    Ok,
    Unavailable,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct Health {
    /// Product name of the server.
    pub name: &'static str,
    /// Server version.
    pub version: &'static str,
    /// The commit the server was built from, with a `+` after it when files
    /// differed from that commit. Missing, like `built_at`, in a build made
    /// without the build scripts.
    pub commit: Option<&'static str>,
    /// When the server was built, in seconds since 1970. With `commit`, it
    /// tells one build from another of the same version.
    pub built_at: Option<i64>,
    pub database: DatabaseStatus,
    /// Whether the server still waits for its owner to be made with
    /// `POST /setup`. Until then nobody can sign in.
    pub needs_setup: bool,
}

/// Reports the server's version and build, whether its database responds, and whether
/// first-run setup is still to do. Open to anyone: it is how a client finds
/// out that this is a Binge server before it has signed in.
#[utoipa::path(
    get,
    path = "/health",
    tag = "server",
    operation_id = "getHealth",
    responses((status = OK, description = "The server is running", body = Health))
)]
async fn health(State(state): State<AppState>) -> Json<Health> {
    let database = if state.db.is_healthy().await {
        DatabaseStatus::Ok
    } else {
        DatabaseStatus::Unavailable
    };
    Json(Health {
        name: "Binge Server",
        version: binge_core::VERSION,
        commit: built::commit(),
        built_at: built::at(),
        database,
        // A database that does not answer is what `database` reports.
        needs_setup: state.accounts.needs_setup().await.unwrap_or(false),
    })
}
