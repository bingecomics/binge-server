//! The people who may use the server, managed by its owner. Only for an
//! owner who signed in with a password.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use binge_core::UserId;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::error::ApiError;
use super::session::{Password, User};
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, add))
        .routes(routes!(change, remove))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewUser {
    /// The name to sign in with. Upper and lower case are the same.
    pub name: String,
    /// At least 8 characters. The owner tells it to the person, who can
    /// change it once signed in.
    #[schema(value_type = String, format = Password)]
    pub password: Password,
    /// Ids of the libraries the user may read. None when left out.
    #[serde(default)]
    pub libraries: Vec<i64>,
}

/// What to change about a user. What is left out stays as it is.
#[derive(Debug, Deserialize, ToSchema)]
pub struct UserChange {
    /// A new password. It ends the user's sessions; their tokens stay.
    #[schema(value_type = Option<String>, format = Password)]
    pub password: Option<Password>,
    /// Ids of the libraries the user may read from now on.
    pub libraries: Option<Vec<i64>>,
}

/// Lists everyone who may use the server: the owner, then the others by
/// name.
#[utoipa::path(
    get,
    path = "/users",
    tag = "account",
    operation_id = "listUsers",
    responses((status = OK, body = [User]), (status = FORBIDDEN, body = ApiError))
)]
async fn list(State(state): State<AppState>, _: Owner) -> Result<Json<Vec<User>>, ApiError> {
    let accounts = state.accounts.accounts().await?;
    Ok(Json(accounts.into_iter().map(User::from).collect()))
}

/// Adds a user who may read the given libraries.
#[utoipa::path(
    post,
    path = "/users",
    tag = "account",
    operation_id = "addUser",
    request_body = NewUser,
    responses(
        (status = CREATED, body = User),
        (status = BAD_REQUEST, description = "The name, the password or a library will not do", body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = CONFLICT, description = "Someone already has that name", body = ApiError),
    )
)]
async fn add(
    State(state): State<AppState>,
    _: Owner,
    Json(new): Json<NewUser>,
) -> Result<(StatusCode, Json<User>), ApiError> {
    let account = state
        .accounts
        .create_user(&new.name, new.password.as_str(), &new.libraries)
        .await?;
    Ok((StatusCode::CREATED, Json(account.into())))
}

/// Gives a user a new password, other libraries to read, or both. Not for
/// the owner, who reads everything and changes their own password with
/// `PUT /me/password`.
#[utoipa::path(
    patch,
    path = "/users/{id}",
    tag = "account",
    operation_id = "changeUser",
    params(("id" = i64, Path, description = "The user")),
    request_body = UserChange,
    responses(
        (status = OK, body = User),
        (status = BAD_REQUEST, description = "The password or a library will not do, or the user is the owner", body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn change(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
    Json(change): Json<UserChange>,
) -> Result<Json<User>, ApiError> {
    let account = state
        .accounts
        .change_user(
            UserId(id),
            change.password.as_ref().map(Password::as_str),
            change.libraries.as_deref(),
        )
        .await?;
    Ok(Json(account.into()))
}

/// Removes a user, with their sessions, tokens and reading progress. The
/// owner cannot be removed.
#[utoipa::path(
    delete,
    path = "/users/{id}",
    tag = "account",
    operation_id = "removeUser",
    params(("id" = i64, Path, description = "The user")),
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The user is the owner", body = ApiError),
        (status = FORBIDDEN, body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn remove(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.accounts.remove_user(UserId(id)).await?;
    Ok(StatusCode::NO_CONTENT)
}
