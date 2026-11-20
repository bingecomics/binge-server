//! Reading: one book, its pages, progress, and the lists on the home page.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::browse::{Book, Series};
use super::error::ApiError;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(book))
        .routes(routes!(book_after))
        .routes(routes!(pages))
        .routes(routes!(save_progress))
        .routes(routes!(mark_read))
        .routes(routes!(page_image))
        .routes(routes!(page_thumbnail))
        .routes(routes!(continue_reading))
        .routes(routes!(recently_added))
        .routes(routes!(up_next))
        .routes(routes!(take_off_up_next))
        .routes(routes!(recently_updated))
}

/// One page of a book.
#[derive(Debug, Serialize, ToSchema)]
pub struct Page {
    /// Place in reading order, from 0.
    pub number: i64,
    /// Size of the image in bytes. For a page of a PDF that the server
    /// draws, the size of what the page is made from, not of the picture
    /// sent.
    pub byte_size: i64,
    /// Width in pixels. Missing until a scan has read it, and for an image
    /// the server cannot understand.
    pub width: Option<i64>,
    /// Height in pixels. Missing when `width` is.
    pub height: Option<i64>,
}

impl From<binge_core::Page> for Page {
    fn from(page: binge_core::Page) -> Self {
        Self {
            number: page.number,
            byte_size: page.byte_size,
            width: page.width,
            height: page.height,
        }
    }
}

/// How far the caller has read a book. Everyone has their own.
///
/// It belongs to the book's `identity`, so it stays with the book when its
/// file is moved or renamed, and two files with the same pages share it.
#[derive(Debug, Serialize, ToSchema)]
pub struct Progress {
    /// The last page shown, from 0.
    pub page: i64,
    /// Whether that was the book's last page.
    pub completed: bool,
    /// When this was saved, in seconds since 1970.
    pub updated_at: i64,
}

impl From<binge_core::Progress> for Progress {
    fn from(progress: binge_core::Progress) -> Self {
        Self {
            page: progress.page,
            completed: progress.completed,
            updated_at: progress.updated_at,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct SaveProgress {
    /// The last page shown, from 0. With two pages on screen, the later one.
    pub page: i64,
}

/// Which books to mark, and how.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadMark {
    /// `true` marks the books as read to the end, `false` as never opened.
    pub read: bool,
    /// Books to mark.
    #[serde(default)]
    pub books: Vec<i64>,
    /// Groups to mark every book of.
    #[serde(default)]
    pub groups: Vec<i64>,
    /// Series to mark every book of.
    #[serde(default)]
    pub series: Vec<i64>,
    /// Reading lists to mark every book of.
    #[serde(default)]
    pub lists: Vec<i64>,
}

/// What a mark changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct Marked {
    /// How many books the mark changed. The ones that already were as asked
    /// are not counted.
    pub books: u64,
}

/// Entries in a list when the request does not say how many.
const DEFAULT_LIMIT: u32 = 24;
const MAX_LIMIT: u32 = 100;

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ListLimit {
    /// How many entries to return at most. 24 when left out, never more
    /// than 100.
    #[param(minimum = 1, maximum = 100)]
    pub limit: Option<u32>,
}

impl ListLimit {
    fn get(&self) -> u32 {
        self.limit.unwrap_or(DEFAULT_LIMIT).clamp(1, MAX_LIMIT)
    }
}

/// Returns one book.
#[utoipa::path(
    get,
    path = "/books/{id}",
    tag = "reading",
    operation_id = "getBook",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = Book), (status = NOT_FOUND, body = ApiError))
)]
async fn book(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Book>, ApiError> {
    Ok(Json(state.catalog.book(caller.user.id, id).await?.into()))
}

/// Where a book is being read, which says what comes after it.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct ReadFrom {
    /// The reading list the book was opened from.
    pub list: Option<i64>,
    /// Whether the book was opened from its group of its series. Counts
    /// for nothing when `list` is given, or when the book is in no group.
    pub group: Option<bool>,
}

/// What a book is read as a part of.
#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum Followed {
    /// A reading list.
    List,
    /// The book's group of its series.
    Group,
    /// The book's series, from one group to the next.
    Series,
}

impl From<binge_core::Followed> for Followed {
    fn from(followed: binge_core::Followed) -> Self {
        match followed {
            binge_core::Followed::List => Self::List,
            binge_core::Followed::Group => Self::Group,
            binge_core::Followed::Series => Self::Series,
        }
    }
}

/// What comes after a book where it is read.
#[derive(Debug, Serialize, ToSchema)]
pub struct BookAfter {
    /// What the book is read as a part of. Missing when it is a part of
    /// nothing: it is in no series and was opened from no list, or it is
    /// not in the list named.
    pub followed: Option<Followed>,
    /// The book to go on to: the first one after it there that can be
    /// read. Missing at the end, and whenever nothing is followed.
    pub next: Option<Book>,
}

/// Says what comes after a book where the caller reads it, for a reader to
/// go on to at the book's last page.
///
/// Reading follows where the book was opened from: the reading list named
/// by `list`; with `group`, the book's group of its series, which ends
/// with the group; and otherwise its series, on from one group to the
/// next.
#[utoipa::path(
    get,
    path = "/books/{id}/next",
    tag = "reading",
    operation_id = "getBookAfter",
    params(("id" = i64, Path, description = "The book"), ReadFrom),
    responses(
        (status = OK, body = BookAfter),
        (status = NOT_FOUND, description = "No such book, or no such reading list", body = ApiError),
    )
)]
async fn book_after(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Query(from): Query<ReadFrom>,
) -> Result<Json<BookAfter>, ApiError> {
    let after = state
        .catalog
        .book_after(caller.user.id, id, from.list, from.group.unwrap_or(false))
        .await?;
    Ok(Json(BookAfter {
        followed: after.followed.map(Followed::from),
        next: after.next.map(Book::from),
    }))
}

/// Lists the pages of a book in reading order, with the sizes a reader needs
/// to lay them out. The images are at `/pages/{identity}/{number}`.
#[utoipa::path(
    get,
    path = "/books/{id}/pages",
    tag = "reading",
    operation_id = "listPages",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = [Page]), (status = NOT_FOUND, body = ApiError))
)]
async fn pages(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Page>>, ApiError> {
    let pages = state.catalog.pages(caller.user.id, id).await?;
    Ok(Json(pages.into_iter().map(Page::from).collect()))
}

/// Records how far the caller has read a book. Reaching the last page completes
/// the book; reading it again from an earlier page takes that back.
///
/// A reader saves each page as it is shown. The saves are also the record
/// of when the caller read: pages of one book saved less than half an hour
/// apart count as one sitting.
#[utoipa::path(
    put,
    path = "/books/{id}/progress",
    tag = "reading",
    operation_id = "saveProgress",
    params(("id" = i64, Path, description = "The book")),
    request_body = SaveProgress,
    responses(
        (status = OK, body = Progress),
        (status = BAD_REQUEST, description = "The page is not in the book", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn save_progress(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Json(progress): Json<SaveProgress>,
) -> Result<Json<Progress>, ApiError> {
    let saved = state
        .catalog
        .save_progress(caller.user.id, id, progress.page)
        .await?;
    Ok(Json(saved.into()))
}

/// Marks books as read by the caller, or as unread: the books named, and
/// every book of the groups, series and reading lists named.
///
/// A book marked read is as if the caller had reached its last page now; one
/// they had read already keeps the time they read it. A book marked unread
/// is as if they had never opened it. What is not on the server, what the
/// caller may not read, and files that could not be read are passed over, so
/// the answer says how many books changed. One request names 10,000 books,
/// groups, series and lists at most.
#[utoipa::path(
    post,
    path = "/read-marks",
    tag = "reading",
    operation_id = "markRead",
    request_body = ReadMark,
    responses(
        (status = OK, body = Marked),
        (status = BAD_REQUEST, description = "More than 10,000 were named", body = ApiError),
    )
)]
async fn mark_read(
    State(state): State<AppState>,
    caller: Caller,
    Json(mark): Json<ReadMark>,
) -> Result<Json<Marked>, ApiError> {
    let which = binge_core::BooksToMark {
        books: mark.books,
        groups: mark.groups,
        series: mark.series,
        lists: mark.lists,
    };
    let books = state
        .catalog
        .mark_read(caller.user.id, &which, mark.read)
        .await?;
    Ok(Json(Marked { books }))
}

/// Returns a page image exactly as the book's file stores it.
///
/// The address is made of the book's `identity` and the page's `number`, so
/// the image under it never changes: it can be cached for good, and stays
/// valid when the book's file is moved or renamed.
#[utoipa::path(
    get,
    path = "/pages/{identity}/{number}",
    tag = "reading",
    operation_id = "getPageImage",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses(
        (status = OK, description = "The image as stored", content_type = "image/*", body = Vec<u8>),
        (status = NOT_MODIFIED, description = "The copy named by `If-None-Match` is still good"),
        (status = NOT_FOUND, description = "No such page, or the book's file is gone", body = ApiError),
        (status = CONFLICT, description = "The book's file changed since it was scanned", body = ApiError),
    )
)]
async fn page_image(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
    request: HeaderMap,
) -> Result<Response, ApiError> {
    page_as(&state, &caller, &request, &identity, number, Size::Stored).await
}

/// Returns a page as a small picture: a JPEG that fits 320 by 480 pixels,
/// for a strip of pages to jump by or a card that shows one page.
///
/// Like the page itself, it never changes under its address and can be
/// cached for good. The server makes it each time it is asked and keeps no
/// copy.
#[utoipa::path(
    get,
    path = "/pages/{identity}/{number}/thumbnail",
    tag = "reading",
    operation_id = "getPageThumbnail",
    params(
        ("identity" = String, Path, description = "The `identity` of a book"),
        ("number" = i64, Path, description = "The page, from 0"),
    ),
    responses(
        (status = OK, description = "The small picture", content_type = "image/jpeg", body = Vec<u8>),
        (status = NOT_MODIFIED, description = "The copy named by `If-None-Match` is still good"),
        (status = BAD_REQUEST, description = "The page is no picture the server can make smaller", body = ApiError),
        (status = NOT_FOUND, description = "No such page, or the book's file is gone", body = ApiError),
        (status = CONFLICT, description = "The book's file changed since it was scanned", body = ApiError),
    )
)]
async fn page_thumbnail(
    State(state): State<AppState>,
    caller: Caller,
    Path((identity, number)): Path<(String, i64)>,
    request: HeaderMap,
) -> Result<Response, ApiError> {
    page_as(&state, &caller, &request, &identity, number, Size::Small).await
}

/// The two pictures a page is sent as.
#[derive(Clone, Copy)]
enum Size {
    /// As the book's file stores it.
    Stored,
    /// Scaled down.
    Small,
}

/// Answers with a page in one of its sizes, or says that the copy the
/// request names is still good.
async fn page_as(
    state: &AppState,
    caller: &Caller,
    request: &HeaderMap,
    identity: &str,
    number: i64,
    size: Size,
) -> Result<Response, ApiError> {
    let tag = match size {
        Size::Stored => format!("\"{identity}-{number}\""),
        Size::Small => format!("\"{identity}-{number}-small\""),
    };
    // Text from the address that cannot go in a header names no book.
    let etag = HeaderValue::from_str(&tag)
        .map_err(|_| ApiError::from(binge_core::CatalogError::NoSuchPage))?;
    let cached = [
        (
            header::CACHE_CONTROL,
            HeaderValue::from_static(super::KEPT_FOR_GOOD),
        ),
        (header::ETAG, etag.clone()),
    ];

    // Whoever holds the image of a page the catalog knows holds the right
    // one: the book's file does not have to be opened to say so.
    let user = caller.user.id;
    if names_etag(request, &etag) && state.catalog.has_page(user, identity, number).await? {
        return Ok((StatusCode::NOT_MODIFIED, cached).into_response());
    }
    let image = match size {
        Size::Stored => state.catalog.page_image(user, identity, number).await?,
        Size::Small => state.catalog.small_page(user, identity, number).await?,
    };
    let kind = [(header::CONTENT_TYPE, image.media_type)];
    Ok((cached, kind, image.bytes).into_response())
}

/// Whether the request's `If-None-Match` lists this entity tag. `*` is not
/// taken for it: that asks whether the page exists, which only a look can
/// say.
fn names_etag(request: &HeaderMap, etag: &HeaderValue) -> bool {
    request
        .get_all(header::IF_NONE_MATCH)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|value| value.split(','))
        .map(|tag| tag.trim().trim_start_matches("W/"))
        .any(|tag| tag.as_bytes() == etag.as_bytes())
}

/// Lists the books the caller started and did not finish, the one read last
/// first.
#[utoipa::path(
    get,
    path = "/continue-reading",
    tag = "reading",
    operation_id = "listContinueReading",
    params(ListLimit),
    responses((status = OK, body = [Book]))
)]
async fn continue_reading(
    State(state): State<AppState>,
    caller: Caller,
    Query(limit): Query<ListLimit>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state
        .catalog
        .continue_reading(caller.user.id, limit.get())
        .await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Lists the books the server saw last, newest first. Files that could not
/// be read are left out.
#[utoipa::path(
    get,
    path = "/recently-added",
    tag = "reading",
    operation_id = "listRecentlyAdded",
    params(ListLimit),
    responses((status = OK, body = [Book]))
)]
async fn recently_added(
    State(state): State<AppState>,
    caller: Caller,
    Query(limit): Query<ListLimit>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state
        .catalog
        .recently_added(caller.user.id, limit.get())
        .await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Lists, for each series the caller finished a book of and opened none
/// since, the book that follows, the series read last first. A series the
/// caller has not read from for some weeks is left out, until it gets a
/// book, and so is one they took off the list, until they read from it
/// again.
#[utoipa::path(
    get,
    path = "/up-next",
    tag = "reading",
    operation_id = "listUpNext",
    params(ListLimit),
    responses((status = OK, body = [Book]))
)]
async fn up_next(
    State(state): State<AppState>,
    caller: Caller,
    Query(limit): Query<ListLimit>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state.catalog.up_next(caller.user.id, limit.get()).await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Takes a series off the caller's list of `GET /up-next`. It stays off,
/// also when it gets a book, until the caller reads from it again.
#[utoipa::path(
    delete,
    path = "/up-next/{series_id}",
    tag = "reading",
    operation_id = "takeOffUpNext",
    params(("series_id" = i64, Path, description = "The series")),
    responses(
        (status = NO_CONTENT, description = "The series is off the list"),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn take_off_up_next(
    State(state): State<AppState>,
    caller: Caller,
    Path(series_id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .take_off_up_next(caller.user.id, series_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the series that hold books in the libraries the caller may read,
/// the one given a book last first.
#[utoipa::path(
    get,
    path = "/recently-updated",
    tag = "reading",
    operation_id = "listRecentlyUpdated",
    params(ListLimit),
    responses((status = OK, body = [Series]))
)]
async fn recently_updated(
    State(state): State<AppState>,
    caller: Caller,
    Query(limit): Query<ListLimit>,
) -> Result<Json<Vec<Series>>, ApiError> {
    let series = state
        .catalog
        .recently_updated(caller.user.id, limit.get())
        .await?;
    Ok(Json(series.into_iter().map(Series::from).collect()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn if_none_match_is_read_as_a_list() {
        let etag = HeaderValue::from_static("\"abc-3\"");
        let matches = |sent: &'static str| {
            let mut request = HeaderMap::new();
            request.insert(header::IF_NONE_MATCH, HeaderValue::from_static(sent));
            names_etag(&request, &etag)
        };
        assert!(matches("\"abc-3\""));
        assert!(matches("W/\"abc-3\""));
        assert!(matches("\"other\", \"abc-3\""));
        assert!(!matches("*"));
        assert!(!matches("\"abc-4\""));
        assert!(!matches("abc-3"));
        assert!(!names_etag(&HeaderMap::new(), &etag));
    }
}
