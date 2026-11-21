//! Want to read: the series a person means to read.
//!
//! It is each person's own: nobody else sees it, and there is nothing to
//! share. `wanted` on a series says whether the caller put it there.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::browse::{PagedSeries, PartQuery};
use super::collections::{SeriesAdded, SeriesNamed};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(want_to_read, want))
        .routes(routes!(want_no_more))
}

/// Lists a part of the series the caller wants to read, the ones put there
/// last first.
///
/// A series they have read to the end is left out. It is still theirs to
/// take off, and it is listed again by itself once it has a book they have
/// not read.
#[utoipa::path(
    get,
    path = "/want-to-read",
    tag = "reading",
    operation_id = "listWantToRead",
    params(PartQuery),
    responses((status = OK, body = PagedSeries))
)]
async fn want_to_read(
    State(state): State<AppState>,
    caller: Caller,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedSeries>, ApiError> {
    let wanted = state
        .catalog
        .want_to_read(caller.user.id, part.span())
        .await?;
    Ok(Json(wanted.into()))
}

/// Puts the series named on the caller's want to read. A series already on
/// it stays where it was.
///
/// Every series named must be on the server: one that is not refuses the
/// request, and nothing is added. One request names 10,000 at most.
#[utoipa::path(
    post,
    path = "/want-to-read",
    tag = "reading",
    operation_id = "addToWantToRead",
    request_body = SeriesNamed,
    responses(
        (status = OK, body = SeriesAdded),
        (status = BAD_REQUEST, description = "A series named is not on the server", body = ApiError),
    )
)]
async fn want(
    State(state): State<AppState>,
    caller: Caller,
    Json(named): Json<SeriesNamed>,
) -> Result<Json<SeriesAdded>, ApiError> {
    let series = state.catalog.want(caller.user.id, &named.series).await?;
    Ok(Json(SeriesAdded { series }))
}

/// Takes a series off the caller's want to read.
#[utoipa::path(
    delete,
    path = "/want-to-read/{series_id}",
    tag = "reading",
    operation_id = "takeOffWantToRead",
    params(("series_id" = i64, Path, description = "The series")),
    responses(
        (status = NO_CONTENT, description = "The series is off it"),
        (status = NOT_FOUND, description = "No such series, or it is not on the caller's want to read", body = ApiError),
    )
)]
async fn want_no_more(
    State(state): State<AppState>,
    caller: Caller,
    Path(series_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .want_no_more(caller.user.id, series_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}
