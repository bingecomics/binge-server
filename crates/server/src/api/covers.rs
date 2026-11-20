//! Choosing covers: the cover of a book, of a series or of a reading list
//! can be a page of a book or a picture, in place of the one the server
//! shows by itself.
//!
//! Every route here is for the owner. What is chosen is kept in the data
//! directory; nothing is written to a library.

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use binge_core::{CoverFrom, CoverOf};
use serde::Deserialize;
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Owner;
use super::error::{ApiError, whole};
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    // A picture is more than any other route takes in a request.
    let pictures = OpenApiRouter::new()
        .routes(routes!(give_book_cover))
        .routes(routes!(give_series_cover))
        .routes(routes!(give_list_cover))
        .layer(DefaultBodyLimit::max(CoverFrom::BIGGEST_PICTURE));
    OpenApiRouter::new()
        .routes(routes!(choose_book_cover, forget_book_cover))
        .routes(routes!(choose_series_cover, forget_series_cover))
        .routes(routes!(choose_list_cover, forget_list_cover))
        .merge(pictures)
}

/// A page of a book, to make a cover from.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CoverPage {
    /// The book the page is in. It need not be the book, or a book of the
    /// series, that the cover is for.
    pub book_id: i64,
    /// The page, from 0.
    pub page: i64,
}

impl From<CoverPage> for CoverFrom {
    fn from(page: CoverPage) -> Self {
        Self::Page {
            book_id: page.book_id,
            page: page.page,
        }
    }
}

/// Makes a page of a book the cover of a book, in place of its first page.
/// Only the owner can.
///
/// The cover goes with the book's identity: every copy of the book shows
/// it, and it is still the book's when its file is moved or renamed. The
/// book's `cover` names the new image.
#[utoipa::path(
    put,
    path = "/books/{id}/cover",
    tag = "covers",
    operation_id = "chooseBookCover",
    params(("id" = i64, Path, description = "The book the cover is for")),
    request_body = CoverPage,
    responses(
        (status = NO_CONTENT, description = "The book has the cover"),
        (status = BAD_REQUEST, description = "The page is no picture the server can read, or the book's file could not be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such book, or no such page", body = ApiError),
    )
)]
async fn choose_book_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(page): Json<CoverPage>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::Book(id), page.into()).await
}

/// Makes a picture the cover of a book: the request's body is a JPEG, PNG,
/// WebP or GIF file of 20 MB at most. Only the owner can.
#[utoipa::path(
    put,
    path = "/books/{id}/cover/picture",
    tag = "covers",
    operation_id = "giveBookCover",
    params(("id" = i64, Path, description = "The book the cover is for")),
    request_body(content = Vec<u8>, content_type = "image/*", description = "The picture's file"),
    responses(
        (status = NO_CONTENT, description = "The book has the cover"),
        (status = BAD_REQUEST, description = "Not a picture the server can read, one of more than 20 MB, or the book's file could not be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn give_book_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    picture: Result<Bytes, BytesRejection>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::Book(id), given(picture)?).await
}

/// Takes back the cover chosen for a book, which shows its first page
/// again. Nothing happens when none was chosen. Only the owner can.
#[utoipa::path(
    delete,
    path = "/books/{id}/cover",
    tag = "covers",
    operation_id = "forgetBookCover",
    params(("id" = i64, Path, description = "The book")),
    responses(
        (status = NO_CONTENT, description = "The book shows its first page"),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn forget_book_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    forget(&state, &owner, CoverOf::Book(id)).await
}

/// Makes a page of a book the cover of a series, in place of its first
/// book's cover. Only the owner can. The series' `cover` names the new
/// image.
#[utoipa::path(
    put,
    path = "/series/{id}/cover",
    tag = "covers",
    operation_id = "chooseSeriesCover",
    params(("id" = i64, Path, description = "The series the cover is for")),
    request_body = CoverPage,
    responses(
        (status = NO_CONTENT, description = "The series has the cover"),
        (status = BAD_REQUEST, description = "The page is no picture the server can read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such series, no such book, or no such page", body = ApiError),
    )
)]
async fn choose_series_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(page): Json<CoverPage>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::Series(id), page.into()).await
}

/// Makes a picture the cover of a series: the request's body is a JPEG,
/// PNG, WebP or GIF file of 20 MB at most. Only the owner can.
#[utoipa::path(
    put,
    path = "/series/{id}/cover/picture",
    tag = "covers",
    operation_id = "giveSeriesCover",
    params(("id" = i64, Path, description = "The series the cover is for")),
    request_body(content = Vec<u8>, content_type = "image/*", description = "The picture's file"),
    responses(
        (status = NO_CONTENT, description = "The series has the cover"),
        (status = BAD_REQUEST, description = "Not a picture the server can read, or one of more than 20 MB", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn give_series_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    picture: Result<Bytes, BytesRejection>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::Series(id), given(picture)?).await
}

/// Takes back the cover chosen for a series, which shows its first book's
/// cover again. Nothing happens when none was chosen. Only the owner can.
#[utoipa::path(
    delete,
    path = "/series/{id}/cover",
    tag = "covers",
    operation_id = "forgetSeriesCover",
    params(("id" = i64, Path, description = "The series")),
    responses(
        (status = NO_CONTENT, description = "The series shows its first book's cover"),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn forget_series_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    forget(&state, &owner, CoverOf::Series(id)).await
}

/// Makes a page of a book the cover of a reading list, in place of its
/// first book's cover. Only the owner can. The list's `cover` names the new
/// image.
#[utoipa::path(
    put,
    path = "/reading-lists/{id}/cover",
    tag = "covers",
    operation_id = "chooseReadingListCover",
    params(("id" = i64, Path, description = "The reading list the cover is for")),
    request_body = CoverPage,
    responses(
        (status = NO_CONTENT, description = "The list has the cover"),
        (status = BAD_REQUEST, description = "The page is no picture the server can read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such list, no such book, or no such page", body = ApiError),
    )
)]
async fn choose_list_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(page): Json<CoverPage>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::List(id), page.into()).await
}

/// Makes a picture the cover of a reading list: the request's body is a
/// JPEG, PNG, WebP or GIF file of 20 MB at most. Only the owner can.
#[utoipa::path(
    put,
    path = "/reading-lists/{id}/cover/picture",
    tag = "covers",
    operation_id = "giveReadingListCover",
    params(("id" = i64, Path, description = "The reading list the cover is for")),
    request_body(content = Vec<u8>, content_type = "image/*", description = "The picture's file"),
    responses(
        (status = NO_CONTENT, description = "The list has the cover"),
        (status = BAD_REQUEST, description = "Not a picture the server can read, or one of more than 20 MB", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn give_list_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    picture: Result<Bytes, BytesRejection>,
) -> Result<StatusCode, ApiError> {
    choose(&state, &owner, CoverOf::List(id), given(picture)?).await
}

/// Takes back the cover chosen for a reading list, which shows its first
/// book's cover again. Nothing happens when none was chosen. Only the owner
/// can.
#[utoipa::path(
    delete,
    path = "/reading-lists/{id}/cover",
    tag = "covers",
    operation_id = "forgetReadingListCover",
    params(("id" = i64, Path, description = "The reading list")),
    responses(
        (status = NO_CONTENT, description = "The list shows its first book's cover"),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn forget_list_cover(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    forget(&state, &owner, CoverOf::List(id)).await
}

/// The picture a request brought, unless it is more than the server takes.
fn given(picture: Result<Bytes, BytesRejection>) -> Result<CoverFrom, ApiError> {
    let picture = whole(picture, CoverFrom::too_big, "picture")?;
    Ok(CoverFrom::Picture(picture.into()))
}

async fn choose(
    state: &AppState,
    owner: &binge_core::User,
    of: CoverOf,
    from: CoverFrom,
) -> Result<StatusCode, ApiError> {
    state.catalog.choose_cover(owner.id, of, from).await?;
    Ok(StatusCode::NO_CONTENT)
}

async fn forget(
    state: &AppState,
    owner: &binge_core::User,
    of: CoverOf,
) -> Result<StatusCode, ApiError> {
    state.catalog.forget_cover(owner.id, of).await?;
    Ok(StatusCode::NO_CONTENT)
}
