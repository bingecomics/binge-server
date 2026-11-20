//! Changing the organization: series, their groups, and where books sit.
//!
//! Every route here is for the owner, and does exactly what was asked or
//! refuses with the reason. The server decides nothing on the side.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::browse::{Book, Group, GroupKind, Series};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(create_series))
        .routes(routes!(rename_series, delete_series))
        .routes(routes!(create_group))
        .routes(routes!(rename_group, delete_group))
        .routes(routes!(place_book))
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesName {
    /// At most 200 characters. No other series of the library may have it:
    /// names that differ only in capitals, accents, spaces or punctuation
    /// count as the same.
    pub name: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewGroup {
    pub kind: GroupKind,
    /// A number to know it by, from 1 to 65535: a volume's, or for annuals
    /// that of the volume they are of. Any kind may have one.
    pub number: Option<i64>,
    /// A name of its own, of at most 200 characters: a limited series'
    /// title, or the other title a group of appearances is in. Any kind may
    /// have one.
    pub name: Option<String>,
}

/// What a group is known by from now on. It is all of it: what is left out,
/// the group does not have.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct GroupName {
    /// A number to know it by, from 1 to 65535: a volume's, or for annuals
    /// that of the volume they are of. Any kind may have one.
    pub number: Option<i64>,
    /// A name of its own, of at most 200 characters: a limited series'
    /// title, or the other title a group of appearances is in. Any kind may
    /// have one.
    pub name: Option<String>,
}

/// Where a book is to sit. It is the whole place: what is left out, the book
/// does not have.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct Place {
    /// A series of the library the book's file is in.
    pub series_id: i64,
    /// A group of that series. Without one the book sits directly in its
    /// series.
    pub group_id: Option<i64>,
    /// The issue's number, such as `12` or `0.5`. At most 32 characters.
    pub number: Option<String>,
    /// The date published, written `2016`, `2016-07` or `2016-07-21`.
    pub published: Option<String>,
}

impl From<Place> for binge_core::Place {
    fn from(place: Place) -> Self {
        Self {
            series_id: place.series_id,
            group_id: place.group_id,
            number: place.number,
            published: place.published,
        }
    }
}

/// Adds a series with no books to a library. Only the owner can.
#[utoipa::path(
    post,
    path = "/libraries/{id}/series",
    tag = "layout",
    operation_id = "createSeries",
    params(("id" = i64, Path, description = "The library")),
    request_body = SeriesName,
    responses(
        (status = CREATED, body = Series),
        (status = BAD_REQUEST, description = "The name cannot be used", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "Another series of the library has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn create_series(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(new): Json<SeriesName>,
) -> Result<(StatusCode, Json<Series>), ApiError> {
    let series = state.catalog.create_series(owner.id, id, &new.name).await?;
    Ok((StatusCode::CREATED, Json(series.into())))
}

/// Gives a series another name. Its books stay in it. A name another series
/// has is refused: two series are never merged by renaming one. Only the
/// owner can.
#[utoipa::path(
    patch,
    path = "/series/{id}",
    tag = "layout",
    operation_id = "renameSeries",
    params(("id" = i64, Path, description = "The series")),
    request_body = SeriesName,
    responses(
        (status = OK, body = Series),
        (status = BAD_REQUEST, description = "The name cannot be used", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "Another series of the library has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn rename_series(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(new): Json<SeriesName>,
) -> Result<Json<Series>, ApiError> {
    let series = state.catalog.rename_series(owner.id, id, &new.name).await?;
    Ok(Json(series.into()))
}

/// Deletes a series that holds no books, with its groups. One that holds
/// books is refused: move them first. Only the owner can.
#[utoipa::path(
    delete,
    path = "/series/{id}",
    tag = "layout",
    operation_id = "deleteSeries",
    params(("id" = i64, Path, description = "The series")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "The series still holds books", body = ApiError),
    )
)]
async fn delete_series(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.delete_series(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Adds a group with no books to a series. Only the owner can.
#[utoipa::path(
    post,
    path = "/series/{id}/groups",
    tag = "layout",
    operation_id = "createGroup",
    params(("id" = i64, Path, description = "The series")),
    request_body = NewGroup,
    responses(
        (status = CREATED, body = Group),
        (status = BAD_REQUEST, description = "The number or name is not one a group may have", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "The series has a group of that kind, number and name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn create_group(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(new): Json<NewGroup>,
) -> Result<(StatusCode, Json<Group>), ApiError> {
    let group = state
        .catalog
        .create_group(
            owner.id,
            id,
            new.kind.into(),
            new.number,
            new.name.as_deref(),
        )
        .await?;
    Ok((StatusCode::CREATED, Json(group.into())))
}

/// Gives a group another number or name. Its kind and its books stay. Only
/// the owner can.
#[utoipa::path(
    patch,
    path = "/groups/{id}",
    tag = "layout",
    operation_id = "renameGroup",
    params(("id" = i64, Path, description = "The group")),
    request_body = GroupName,
    responses(
        (status = OK, body = Group),
        (status = BAD_REQUEST, description = "The number or name is not one a group may have", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "The series has a group of that kind, number and name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn rename_group(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(new): Json<GroupName>,
) -> Result<Json<Group>, ApiError> {
    let group = state
        .catalog
        .rename_group(owner.id, id, new.number, new.name.as_deref())
        .await?;
    Ok(Json(group.into()))
}

/// Deletes a group that holds no books. One that holds books is refused:
/// move them first. Only the owner can.
#[utoipa::path(
    delete,
    path = "/groups/{id}",
    tag = "layout",
    operation_id = "deleteGroup",
    params(("id" = i64, Path, description = "The group")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "The group still holds books", body = ApiError),
    )
)]
async fn delete_group(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.delete_group(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Puts a book where the body says, and answers with the book as it is then.
/// The series and the group must be there already: this makes neither. Only
/// the owner can.
#[utoipa::path(
    put,
    path = "/books/{id}/place",
    tag = "layout",
    operation_id = "placeBook",
    params(("id" = i64, Path, description = "The book")),
    request_body = Place,
    responses(
        (status = OK, body = Book),
        (status = BAD_REQUEST, description = "There is no such series or group, the group is in another series, the series is in another library, the date is not written as asked, or the file could not be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn place_book(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(place): Json<Place>,
) -> Result<Json<Book>, ApiError> {
    let book = state
        .catalog
        .place_book(owner.id, id, &place.into())
        .await?;
    Ok(Json(book.into()))
}
