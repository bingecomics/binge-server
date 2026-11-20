//! Collections: series someone picked by hand, from any library.
//!
//! A collection belongs to the person who made it: anyone makes one, only
//! they change it, and they share it with the people they choose or with
//! everyone. Each person sees of a collection the series they may read. A
//! change does exactly what was asked or is refused with the reason.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::browse::Series;
use super::error::ApiError;
use super::lists::Sharing;
use super::session::UserName;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(collections, create))
        .routes(routes!(collection, change, delete))
        .routes(routes!(share))
        .routes(routes!(series, add_series))
        .routes(routes!(remove_series))
        .routes(routes!(collections_of_series))
}

/// A collection: series someone picked by hand, from any library, in no
/// order of their own. It belongs to the person who made it. A series stays
/// in it under a new name, and leaves it when the series is deleted.
#[derive(Debug, Serialize, ToSchema)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    /// How many of its series the caller may read.
    pub series_count: i64,
    /// How many books those series hold.
    pub book_count: i64,
    /// How many of those books the caller has not read to the end.
    pub unread_count: i64,
    /// Name of the collection's cover, for `/covers/{cover}`: that of its
    /// first series, by name, that has one.
    pub cover: Option<String>,
    /// The covers to show together as one: those of its first series, four
    /// different ones at most.
    pub covers: Vec<String>,
    /// Whose collection it is: the person who made it.
    pub made_by: UserName,
    /// Whether it is the caller's own. Only then can they change it, and
    /// say who it is shared with.
    pub mine: bool,
    /// Whether everyone on the server sees it.
    pub shared_with_all: bool,
    /// The people it is shared with one by one, in name order. Only the one
    /// whose collection it is is told: it is empty for anyone else.
    pub shared_with: Vec<UserName>,
}

impl From<binge_core::Collection> for Collection {
    fn from(collection: binge_core::Collection) -> Self {
        Self {
            id: collection.id,
            name: collection.name,
            summary: collection.summary,
            series_count: collection.series_count,
            book_count: collection.book_count,
            unread_count: collection.unread_count,
            cover: collection.cover,
            covers: collection.covers,
            made_by: collection.made_by.into(),
            mine: collection.mine,
            shared_with_all: collection.shared_with_all,
            shared_with: collection
                .shared_with
                .into_iter()
                .map(UserName::from)
                .collect(),
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCollection {
    /// At most 200 characters. No other collection of the caller's may have
    /// it: names that differ only in capitals, accents, spaces or
    /// punctuation count as the same.
    pub name: String,
    /// At most 4,000 characters.
    pub summary: Option<String>,
    /// The series the collection starts with.
    #[serde(default)]
    pub series: Vec<i64>,
}

/// What a collection is called and said to be from now on. It is all of
/// it: without a summary, the collection has none.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CollectionDetails {
    /// At most 200 characters. No other collection of the caller's may have
    /// it.
    pub name: String,
    /// At most 4,000 characters.
    pub summary: Option<String>,
}

/// Some series, by their ids.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct SeriesNamed {
    pub series: Vec<i64>,
}

/// What adding series to a collection changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct SeriesAdded {
    /// How many series the collection gained. The ones it already held are
    /// not counted.
    pub series: u64,
}

/// Lists the collections the caller sees, in name order: their own, and the
/// ones shared with them or with everyone. A collection of someone else's
/// that holds no series the caller may read is left out.
#[utoipa::path(
    get,
    path = "/collections",
    tag = "collections",
    operation_id = "listCollections",
    responses((status = OK, body = [Collection]))
)]
async fn collections(
    State(state): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<Collection>>, ApiError> {
    let collections = state.catalog.collections(caller.user.id).await?;
    Ok(Json(
        collections.into_iter().map(Collection::from).collect(),
    ))
}

/// Makes a collection of the caller's own, with the series named in it from
/// the start. Nobody else sees it until it is shared.
///
/// Every series named must be on the server: one that is not refuses the
/// request, and nothing is made. A series named twice is in the collection
/// once. One request names 10,000 at most.
#[utoipa::path(
    post,
    path = "/collections",
    tag = "collections",
    operation_id = "createCollection",
    request_body = NewCollection,
    responses(
        (status = CREATED, body = Collection),
        (status = BAD_REQUEST, description = "The name or the summary cannot be used, or a series named is not on the server", body = ApiError),
        (status = CONFLICT, description = "Another collection of the caller's has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn create(
    State(state): State<AppState>,
    caller: Caller,
    Json(new): Json<NewCollection>,
) -> Result<(StatusCode, Json<Collection>), ApiError> {
    let collection = state
        .catalog
        .create_collection(
            caller.user.id,
            &new.name,
            new.summary.as_deref(),
            &new.series,
        )
        .await?;
    Ok((StatusCode::CREATED, Json(collection.into())))
}

/// Returns one collection.
#[utoipa::path(
    get,
    path = "/collections/{id}",
    tag = "collections",
    operation_id = "getCollection",
    params(("id" = i64, Path, description = "The collection")),
    responses((status = OK, body = Collection), (status = NOT_FOUND, body = ApiError))
)]
async fn collection(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Collection>, ApiError> {
    let collection = state.catalog.collection(caller.user.id, id).await?;
    Ok(Json(collection.into()))
}

/// Gives a collection another name and summary. Its series stay. Only the
/// one whose collection it is can.
#[utoipa::path(
    patch,
    path = "/collections/{id}",
    tag = "collections",
    operation_id = "changeCollection",
    params(("id" = i64, Path, description = "The collection")),
    request_body = CollectionDetails,
    responses(
        (status = OK, body = Collection),
        (status = BAD_REQUEST, description = "The name or the summary cannot be used", body = ApiError),
        (status = FORBIDDEN, description = "The collection is someone else's", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "Another collection of the caller's has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn change(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Json(details): Json<CollectionDetails>,
) -> Result<Json<Collection>, ApiError> {
    let collection = state
        .catalog
        .change_collection(
            caller.user.id,
            id,
            &details.name,
            details.summary.as_deref(),
        )
        .await?;
    Ok(Json(collection.into()))
}

/// Says who a collection is shared with from now on: everyone, the people
/// named, or nobody. Only the one whose collection it is can. The people it
/// is shared with see it, with the series they may read, and cannot change
/// it. Someone who may read none of its series does not see it.
///
/// Everyone named must be on the server: someone who is not refuses the
/// request, and nothing changes. Naming the one whose collection it is
/// changes nothing.
#[utoipa::path(
    put,
    path = "/collections/{id}/sharing",
    tag = "collections",
    operation_id = "shareCollection",
    params(("id" = i64, Path, description = "The collection")),
    request_body = Sharing,
    responses(
        (status = OK, body = Collection),
        (status = BAD_REQUEST, description = "Someone named is not on the server", body = ApiError),
        (status = FORBIDDEN, description = "The collection is someone else's", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn share(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Json(sharing): Json<Sharing>,
) -> Result<Json<Collection>, ApiError> {
    let collection = state
        .catalog
        .share_collection(caller.user.id, id, &sharing.into())
        .await?;
    Ok(Json(collection.into()))
}

/// Deletes a collection. Its series are not touched. Only the one whose
/// collection it is can.
#[utoipa::path(
    delete,
    path = "/collections/{id}",
    tag = "collections",
    operation_id = "deleteCollection",
    params(("id" = i64, Path, description = "The collection")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The collection is someone else's", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn delete(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.delete_collection(caller.user.id, id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the series of a collection the caller may read, in name order.
#[utoipa::path(
    get,
    path = "/collections/{id}/series",
    tag = "collections",
    operation_id = "listCollectionSeries",
    params(("id" = i64, Path, description = "The collection")),
    responses((status = OK, body = [Series]), (status = NOT_FOUND, body = ApiError))
)]
async fn series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Series>>, ApiError> {
    let series = state
        .catalog
        .series_in_collection(caller.user.id, id)
        .await?;
    Ok(Json(series.into_iter().map(Series::from).collect()))
}

/// Puts the series named in a collection. A series it already holds stays.
/// Only the one whose collection it is can.
///
/// Every series named must be on the server: one that is not refuses the
/// request, and nothing is added. One request names 10,000 at most.
#[utoipa::path(
    post,
    path = "/collections/{id}/series",
    tag = "collections",
    operation_id = "addToCollection",
    params(("id" = i64, Path, description = "The collection")),
    request_body = SeriesNamed,
    responses(
        (status = OK, body = SeriesAdded),
        (status = BAD_REQUEST, description = "A series named is not on the server", body = ApiError),
        (status = FORBIDDEN, description = "The collection is someone else's", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn add_series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Json(named): Json<SeriesNamed>,
) -> Result<Json<SeriesAdded>, ApiError> {
    let series = state
        .catalog
        .add_to_collection(caller.user.id, id, &named.series)
        .await?;
    Ok(Json(SeriesAdded { series }))
}

/// Takes a series out of a collection. Only the one whose collection it is
/// can.
#[utoipa::path(
    delete,
    path = "/collections/{id}/series/{series_id}",
    tag = "collections",
    operation_id = "removeFromCollection",
    params(
        ("id" = i64, Path, description = "The collection"),
        ("series_id" = i64, Path, description = "The series"),
    ),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The collection is someone else's", body = ApiError),
        (status = NOT_FOUND, description = "No such collection, or the series is not in it", body = ApiError),
    )
)]
async fn remove_series(
    State(state): State<AppState>,
    caller: Caller,
    Path((id, series_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .remove_from_collection(caller.user.id, id, series_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the collections the caller sees that a series is in, in name
/// order.
#[utoipa::path(
    get,
    path = "/series/{id}/collections",
    tag = "collections",
    operation_id = "listCollectionsOfSeries",
    params(("id" = i64, Path, description = "The series")),
    responses((status = OK, body = [Collection]), (status = NOT_FOUND, body = ApiError))
)]
async fn collections_of_series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Collection>>, ApiError> {
    let collections = state
        .catalog
        .collections_of_series(caller.user.id, id)
        .await?;
    Ok(Json(
        collections.into_iter().map(Collection::from).collect(),
    ))
}
