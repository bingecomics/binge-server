//! The server's own updates: whether a newer version is published, and
//! bringing the server to it.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::error::ApiError;
use crate::app::AppState;
use crate::update::UpdateStatus;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(status, set_looks, start))
        .routes(routes!(look))
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateLooks {
    /// Whether the server looks for a newer version by itself, once a day.
    pub looks: bool,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct UpdateTo {
    /// The version to bring the server to: the newer one the last look
    /// found, as `newer.version` says it.
    pub version: String,
}

/// Says where the server stands with its own updates: the version that
/// runs, the newer one the last look found, and how far an update that was
/// asked for is. Only the owner can ask.
#[utoipa::path(
    get,
    path = "/server/update",
    tag = "server",
    operation_id = "getUpdateStatus",
    responses(
        (status = OK, body = UpdateStatus),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn status(State(state): State<AppState>, _: Owner) -> Result<Json<UpdateStatus>, ApiError> {
    Ok(Json(state.updates.status().await?))
}

/// Turns the daily look for a newer version on or off. Turning it on looks
/// at once. A look asks GitHub, where the versions are published, and sends
/// nothing but the server's version. Only the owner can.
#[utoipa::path(
    patch,
    path = "/server/update",
    tag = "server",
    operation_id = "setUpdateLooks",
    request_body = UpdateLooks,
    responses(
        (status = OK, body = UpdateStatus),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "The server was started with updates turned off", body = ApiError),
    )
)]
async fn set_looks(
    State(state): State<AppState>,
    _: Owner,
    Json(asked): Json<UpdateLooks>,
) -> Result<Json<UpdateStatus>, ApiError> {
    state.updates.set_looks(asked.looks).await?;
    Ok(Json(state.updates.status().await?))
}

/// Looks now whether a newer version is published. The answer says what
/// was found in `newer`, or in `problem` why the server could not find out:
/// a look that fails is not an error of this request. Only the owner can.
#[utoipa::path(
    post,
    path = "/server/update/look",
    tag = "server",
    operation_id = "lookForUpdate",
    responses(
        (status = OK, body = UpdateStatus),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "The server was started with updates turned off", body = ApiError),
    )
)]
async fn look(State(state): State<AppState>, _: Owner) -> Result<Json<UpdateStatus>, ApiError> {
    state.updates.look().await?;
    Ok(Json(state.updates.status().await?))
}

/// Starts bringing the server to the newer version the last look found,
/// and answers at once. The version is named, so that the server does not
/// fetch another one that came out since. The server fetches it, makes sure
/// Binge signed it, keeps it in its data directory and starts again as it;
/// requests fail for a moment then. `GET /server/update` says how far it
/// is, and in `problem` why it stopped when it could not be done. Only the
/// owner can.
#[utoipa::path(
    post,
    path = "/server/update",
    tag = "server",
    operation_id = "startUpdate",
    request_body = UpdateTo,
    responses(
        (status = ACCEPTED, body = UpdateStatus),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "An update is under way already, the version is not the newer one the last look found, or this server does not update itself", body = ApiError),
    )
)]
async fn start(
    State(state): State<AppState>,
    _: Owner,
    Json(asked): Json<UpdateTo>,
) -> Result<(StatusCode, Json<UpdateStatus>), ApiError> {
    state.updates.begin(&asked.version)?;
    Ok((StatusCode::ACCEPTED, Json(state.updates.status().await?)))
}
