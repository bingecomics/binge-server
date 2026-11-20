//! Pages the caller kept while reading, to find them again.

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
        .routes(routes!(keep, let_go))
        .routes(routes!(kept_in))
        .routes(routes!(kept))
}

/// A page the caller kept, with the book it is a page of.
///
/// It belongs to the book's `identity` and the page's number, like
/// progress: it stays with the page when the book's file is moved or
/// renamed. Everyone has their own.
#[derive(Debug, Serialize, ToSchema)]
pub struct KeptPage {
    pub book: Book,
    /// The page, from 0. Its picture is at `/pages/{identity}/{number}`,
    /// and a small one at `/pages/{identity}/{number}/thumbnail`.
    pub page: i64,
    /// When it was kept, in seconds since 1970.
    pub kept_at: i64,
}

/// A part of the pages the caller kept.
#[derive(Debug, Serialize, ToSchema)]
pub struct PagedKeptPages {
    pub pages: Vec<KeptPage>,
    /// How many pages the whole list holds.
    pub total: i64,
}

/// Keeps a page for the caller. A page already kept stays as it was.
#[utoipa::path(
    put,
    path = "/pages/{identity}/{number}/kept",
    tag = "reading",
    operation_id = "keepPage",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses(
        (status = NO_CONTENT, description = "The page is kept"),
        (status = BAD_REQUEST, description = "The caller keeps 5,000 pages already", body = ApiError),
        (status = NOT_FOUND, description = "No such page", body = ApiError),
    )
)]
async fn keep(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .keep_page(caller.user.id, &identity, number)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lets go of a page the caller kept. A page that was not kept is no error.
#[utoipa::path(
    delete,
    path = "/pages/{identity}/{number}/kept",
    tag = "reading",
    operation_id = "letPageGo",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses((status = NO_CONTENT, description = "The page is not kept"))
)]
async fn let_go(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .let_page_go(caller.user.id, &identity, number)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the pages of a book that the caller kept, by their numbers from 0,
/// in reading order.
#[utoipa::path(
    get,
    path = "/books/{id}/kept-pages",
    tag = "reading",
    operation_id = "listKeptPagesOfBook",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = [i64]), (status = NOT_FOUND, body = ApiError))
)]
async fn kept_in(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<i64>>, ApiError> {
    Ok(Json(state.catalog.kept_in(caller.user.id, id).await?))
}

/// Lists a part of the pages the caller kept, with how many there are in
/// all: series by series in the order of their names, and inside a series
/// book by book as they are read. The pages of books in no series come
/// last. A page of a book that is not on the server now is left out, and is
/// listed again when its book is back.
#[utoipa::path(
    get,
    path = "/me/kept-pages",
    tag = "reading",
    operation_id = "listKeptPages",
    params(PartQuery),
    responses((status = OK, body = PagedKeptPages))
)]
async fn kept(
    State(state): State<AppState>,
    caller: Caller,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedKeptPages>, ApiError> {
    let part = state
        .catalog
        .kept_pages(caller.user.id, part.span())
        .await?;
    Ok(Json(PagedKeptPages {
        pages: part
            .items
            .into_iter()
            .map(|kept| KeptPage {
                book: kept.book.into(),
                page: kept.page,
                kept_at: kept.kept_at,
            })
            .collect(),
        total: part.total,
    }))
}
