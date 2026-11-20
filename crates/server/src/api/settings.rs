//! What the caller chose in an app, kept by the server so that it follows
//! them to another browser or device.

use std::collections::BTreeMap;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list))
        .routes(routes!(keep, forget))
}

/// Everything the caller chose.
#[derive(Debug, Serialize, ToSchema)]
pub struct Settings {
    /// Each value under its key. The server keeps a value and does not read
    /// it: the app that wrote it knows what it means.
    pub settings: BTreeMap<String, String>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SettingValue {
    /// 8 KB at most.
    pub value: String,
}

/// Lists everything the caller chose.
#[utoipa::path(
    get,
    path = "/me/settings",
    tag = "account",
    operation_id = "listSettings",
    responses((status = OK, body = Settings))
)]
async fn list(State(state): State<AppState>, caller: Caller) -> Result<Json<Settings>, ApiError> {
    let settings = state.accounts.settings(caller.user.id).await?;
    Ok(Json(Settings {
        settings: settings.into_iter().collect(),
    }))
}

/// Keeps a value under a key for the caller, in place of the one before.
#[utoipa::path(
    put,
    path = "/me/settings/{key}",
    tag = "account",
    operation_id = "keepSetting",
    params(("key" = String, Path, description = "1 to 64 lower-case letters, digits, dots, dashes or underscores")),
    request_body = SettingValue,
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The key is not written as one, the value is too long, or the caller keeps too many", body = ApiError),
    )
)]
async fn keep(
    State(state): State<AppState>,
    caller: Caller,
    Path(key): Path<String>,
    Json(new): Json<SettingValue>,
) -> Result<StatusCode, ApiError> {
    state
        .accounts
        .keep_setting(caller.user.id, &key, &new.value)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Forgets what the caller kept under a key. Nothing kept there is no error.
#[utoipa::path(
    delete,
    path = "/me/settings/{key}",
    tag = "account",
    operation_id = "forgetSetting",
    params(("key" = String, Path, description = "The setting's key")),
    responses((status = NO_CONTENT), (status = BAD_REQUEST, body = ApiError))
)]
async fn forget(
    State(state): State<AppState>,
    caller: Caller,
    Path(key): Path<String>,
) -> Result<StatusCode, ApiError> {
    state.accounts.forget_setting(caller.user.id, &key).await?;
    Ok(StatusCode::NO_CONTENT)
}
