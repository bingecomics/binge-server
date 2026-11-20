//! Copies of the database, kept in the data directory.

use axum::Json;
use axum::extract::State;
use axum::http::StatusCode;
use binge_core::COPIES_KEPT;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new().routes(routes!(list, make, schedule))
}

/// One copy of the database: the whole of it as it was at one moment, in a
/// file that opens like the database itself.
#[derive(Debug, Serialize, ToSchema)]
pub struct DatabaseCopy {
    /// The file's name in the folder of copies.
    pub name: String,
    /// In bytes.
    pub size: u64,
    /// When it was made, in seconds since 1970.
    pub made_at: i64,
}

impl From<binge_core::DatabaseCopy> for DatabaseCopy {
    fn from(copy: binge_core::DatabaseCopy) -> Self {
        Self {
            name: copy.name,
            size: copy.size,
            made_at: copy.made_at,
        }
    }
}

/// The copies of the database and how they are made.
#[derive(Debug, Serialize, ToSchema)]
pub struct DatabaseCopies {
    /// Whether the server makes a copy every day.
    pub scheduled: bool,
    /// How many copies are kept. Making one more removes the oldest.
    pub kept: usize,
    /// The folder the copies are in, as a full path on the server.
    pub folder: String,
    /// The latest first.
    pub copies: Vec<DatabaseCopy>,
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct CopySchedule {
    /// Whether the server makes a copy every day.
    pub scheduled: bool,
}

async fn described(state: &AppState) -> Result<Json<DatabaseCopies>, ApiError> {
    let copies = state.backups.copies().await?;
    Ok(Json(DatabaseCopies {
        scheduled: state.backups.scheduled().await?,
        kept: COPIES_KEPT,
        folder: state.backups.dir().display().to_string(),
        copies: copies.into_iter().map(DatabaseCopy::from).collect(),
    }))
}

/// Lists the copies of the database the server keeps in its data
/// directory, and says whether it makes one every day. Only the owner can
/// ask.
#[utoipa::path(
    get,
    path = "/server/backups",
    tag = "server",
    operation_id = "listDatabaseCopies",
    responses(
        (status = OK, body = DatabaseCopies),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn list(State(state): State<AppState>, _: Owner) -> Result<Json<DatabaseCopies>, ApiError> {
    described(&state).await
}

/// Makes a copy of the database now, and removes the oldest when there are
/// more than are kept. Only the owner can.
#[utoipa::path(
    post,
    path = "/server/backups",
    tag = "server",
    operation_id = "makeDatabaseCopy",
    responses(
        (status = CREATED, body = DatabaseCopy),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = INTERNAL_SERVER_ERROR, description = "The copy could not be written", body = ApiError),
    )
)]
async fn make(
    State(state): State<AppState>,
    _: Owner,
) -> Result<(StatusCode, Json<DatabaseCopy>), ApiError> {
    let copy = state.backups.make().await?;
    Ok((StatusCode::CREATED, Json(copy.into())))
}

/// Turns the daily copy on or off. The copies there are stay. Only the
/// owner can.
#[utoipa::path(
    patch,
    path = "/server/backups",
    tag = "server",
    operation_id = "scheduleDatabaseCopies",
    request_body = CopySchedule,
    responses(
        (status = OK, body = DatabaseCopies),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn schedule(
    State(state): State<AppState>,
    _: Owner,
    Json(asked): Json<CopySchedule>,
) -> Result<Json<DatabaseCopies>, ApiError> {
    state.backups.schedule(asked.scheduled).await?;
    described(&state).await
}
