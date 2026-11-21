//! Favorite pages: pages the caller marked while reading, to find them again.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use serde::Serialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::browse::{Book, PartQuery};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(add, remove))
        .routes(routes!(favorites_in))
        .routes(routes!(favorites))
}

/// A favorite page of the caller's, with the book it is a page of.
///
/// It belongs to the book's `identity` and the page's number, like
/// progress: it stays with the page when the book's file is moved or
/// renamed. Everyone has their own.
#[derive(Debug, Serialize, ToSchema)]
pub struct FavoritePage {
    pub book: Book,
    /// The page, from 0. Its picture is at `/pages/{identity}/{number}`,
    /// and a small one at `/pages/{identity}/{number}/thumbnail`.
    pub page: i64,
    /// When it became a favorite, in seconds since 1970.
    pub added_at: i64,
}

/// A part of the caller's favorite pages.
#[derive(Debug, Serialize, ToSchema)]
pub struct PagedFavoritePages {
    pub pages: Vec<FavoritePage>,
    /// How many pages the whole list holds.
    pub total: i64,
}

/// Makes a page a favorite of the caller's. A page that already is one
/// stays as it was.
#[utoipa::path(
    put,
    path = "/pages/{identity}/{number}/favorite",
    tag = "reading",
    operation_id = "addFavoritePage",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses(
        (status = NO_CONTENT, description = "The page is a favorite"),
        (status = BAD_REQUEST, description = "The caller has 5,000 favorite pages already", body = ApiError),
        (status = NOT_FOUND, description = "No such page", body = ApiError),
    )
)]
async fn add(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .add_favorite_page(caller.user.id, &identity, number)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Makes a page no longer a favorite of the caller's. A page that was not
/// one is no error.
#[utoipa::path(
    delete,
    path = "/pages/{identity}/{number}/favorite",
    tag = "reading",
    operation_id = "removeFavoritePage",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses((status = NO_CONTENT, description = "The page is not a favorite"))
)]
async fn remove(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .remove_favorite_page(caller.user.id, &identity, number)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the pages of a book that are favorites of the caller's, by their
/// numbers from 0, in reading order.
#[utoipa::path(
    get,
    path = "/books/{id}/favorite-pages",
    tag = "reading",
    operation_id = "listFavoritePagesOfBook",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = [i64]), (status = NOT_FOUND, body = ApiError))
)]
async fn favorites_in(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<i64>>, ApiError> {
    Ok(Json(
        state.catalog.favorite_pages_in(caller.user.id, id).await?,
    ))
}

/// Lists a part of the caller's favorite pages, with how many there are in
/// all: series by series in the order of their names, and inside a series
/// book by book as they are read. The pages of books in no series come
/// last. A page of a book that is not on the server now is left out, and is
/// listed again when its book is back.
#[utoipa::path(
    get,
    path = "/me/favorite-pages",
    tag = "reading",
    operation_id = "listFavoritePages",
    params(PartQuery),
    responses((status = OK, body = PagedFavoritePages))
)]
async fn favorites(
    State(state): State<AppState>,
    caller: Caller,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedFavoritePages>, ApiError> {
    let part = state
        .catalog
        .favorite_pages(caller.user.id, part.span())
        .await?;
    Ok(Json(PagedFavoritePages {
        pages: part
            .items
            .into_iter()
            .map(|favorite| FavoritePage {
                book: favorite.book.into(),
                page: favorite.page,
                added_at: favorite.added_at,
            })
            .collect(),
        total: part.total,
    }))
}
