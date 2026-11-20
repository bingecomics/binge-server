//! Reading lists: books in an order someone chose, across series.
//!
//! A list belongs to the server. Everyone reads the lists, each seeing the
//! books they may read; making and changing them is the owner's, and does
//! exactly what was asked or is refused with the reason.

use axum::Json;
use axum::body::Bytes;
use axum::extract::rejection::BytesRejection;
use axum::extract::{DefaultBodyLimit, Path, State};
use axum::http::StatusCode;
use binge_core::CblRuleFor;
use binge_core::cbl::{self, BIGGEST_FILE};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{Caller, Owner};
use super::browse::{Book, BooksOverview};
use super::error::{ApiError, ErrorCode, whole};
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    // A CBL file is more than the other routes take in a request.
    let files = OpenApiRouter::new()
        .routes(routes!(plan_import))
        .layer(DefaultBodyLimit::max(BIGGEST_FILE));
    OpenApiRouter::new()
        .routes(routes!(lists, create))
        .routes(routes!(overview, change, delete))
        .routes(routes!(books, add_books))
        .routes(routes!(remove_book))
        .routes(routes!(move_book))
        .routes(routes!(lists_of_book))
        .routes(routes!(keep_rule))
        .routes(routes!(forget_rule))
        .merge(files)
}

/// A reading list: books in an order someone chose, across series. It
/// belongs to the server, and holds its books by their `identity`: a book
/// stays in it when its file is moved or renamed, and comes back into it
/// when a file that was gone is seen again.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingList {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    /// How many of its books are on the server now and the caller may read.
    pub book_count: i64,
    /// How many of those the caller has not read to the end.
    pub unread_count: i64,
    /// Name of the list's cover, for `/covers/{cover}`: the one chosen for
    /// it, or else the cover of its first book that has one.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the list.
    pub cover_chosen: bool,
    /// The covers to show together as one: those of its first books, four
    /// different ones at most. None when a cover was chosen for the list.
    pub covers: Vec<String>,
    /// The earliest year its books came out.
    pub first_year: Option<i64>,
    /// The latest year its books came out.
    pub last_year: Option<i64>,
}

impl From<binge_core::ReadingList> for ReadingList {
    fn from(list: binge_core::ReadingList) -> Self {
        Self {
            id: list.id,
            name: list.name,
            summary: list.summary,
            book_count: list.book_count,
            unread_count: list.unread_count,
            cover: list.cover,
            cover_chosen: list.cover_chosen,
            covers: list.covers,
            first_year: list.first_year,
            last_year: list.last_year,
        }
    }
}

/// A reading list seen whole, for the page that presents it: what its books
/// say together, and how far the caller is through it. What to open next
/// follows what they did last among its books, whatever they read
/// elsewhere.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadingListOverview {
    pub list: ReadingList,
    #[serde(flatten)]
    pub about: BooksOverview,
}

/// Some books, named by what holds them. In a list they go in this order:
/// the books as named, then the books of the groups, of the series and of
/// the reading lists, each in the order it is read.
#[derive(Debug, Default, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct BooksNamed {
    #[serde(default)]
    pub books: Vec<i64>,
    /// Groups to take every book of.
    #[serde(default)]
    pub groups: Vec<i64>,
    /// Series to take every book of.
    #[serde(default)]
    pub series: Vec<i64>,
    /// Reading lists to take every book of.
    #[serde(default)]
    pub lists: Vec<i64>,
}

impl From<BooksNamed> for binge_core::BooksToMark {
    fn from(named: BooksNamed) -> Self {
        Self {
            books: named.books,
            groups: named.groups,
            series: named.series,
            lists: named.lists,
        }
    }
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewReadingList {
    /// At most 200 characters. No other list may have it: names that differ
    /// only in capitals, accents, spaces or punctuation count as the same.
    pub name: String,
    /// At most 4,000 characters.
    pub summary: Option<String>,
    /// The books the list starts with, in this order: these books as named,
    /// then the books of the groups, of the series and of the reading lists
    /// named, each in the order it is read.
    #[serde(default)]
    pub books: Vec<i64>,
    /// Groups to take every book of.
    #[serde(default)]
    pub groups: Vec<i64>,
    /// Series to take every book of.
    #[serde(default)]
    pub series: Vec<i64>,
    /// Reading lists to take every book of.
    #[serde(default)]
    pub lists: Vec<i64>,
}

/// What a list is called and said to be from now on. It is all of it:
/// without a summary, the list has none.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ReadingListDetails {
    /// At most 200 characters. No other list may have it.
    pub name: String,
    /// At most 4,000 characters.
    pub summary: Option<String>,
}

/// What adding books to a list changed.
#[derive(Debug, Serialize, ToSchema)]
pub struct Added {
    /// How many books the list gained. The ones it already held are not
    /// counted, and stay where they were.
    pub books: u64,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct ListPosition {
    /// Where the book goes among the list's books as `GET
    /// /reading-lists/{id}/books` gives them, from 0. Past the end is the
    /// end.
    pub position: i64,
}

/// Lists the reading lists in name order. A list that holds no book the
/// caller may read is left out, except for the owner.
#[utoipa::path(
    get,
    path = "/reading-lists",
    tag = "lists",
    operation_id = "listReadingLists",
    responses((status = OK, body = [ReadingList]))
)]
async fn lists(
    State(state): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<ReadingList>>, ApiError> {
    let lists = state.catalog.reading_lists(caller.user.id).await?;
    Ok(Json(lists.into_iter().map(ReadingList::from).collect()))
}

/// Makes a reading list, with the books named in it from the start. Only
/// the owner can.
///
/// Every book, group, series and list named must be on the server: one that
/// is not, or a file that could not be read, refuses the request, and
/// nothing is made. A book named twice is in the list once. One request
/// names 10,000 at most.
#[utoipa::path(
    post,
    path = "/reading-lists",
    tag = "lists",
    operation_id = "createReadingList",
    request_body = NewReadingList,
    responses(
        (status = CREATED, body = ReadingList),
        (status = BAD_REQUEST, description = "The name or the summary cannot be used, or something named is not on the server", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "Another list has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn create(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(new): Json<NewReadingList>,
) -> Result<(StatusCode, Json<ReadingList>), ApiError> {
    let books = binge_core::BooksToMark {
        books: new.books,
        groups: new.groups,
        series: new.series,
        lists: new.lists,
    };
    let list = state
        .catalog
        .create_list(owner.id, &new.name, new.summary.as_deref(), &books)
        .await?;
    Ok((StatusCode::CREATED, Json(list.into())))
}

/// Returns one reading list seen whole, for the page that presents it: the
/// list, what its books say together, how far the caller is through it, and
/// the book to open next.
#[utoipa::path(
    get,
    path = "/reading-lists/{id}",
    tag = "lists",
    operation_id = "getReadingList",
    params(("id" = i64, Path, description = "The reading list")),
    responses((status = OK, body = ReadingListOverview), (status = NOT_FOUND, body = ApiError))
)]
async fn overview(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<ReadingListOverview>, ApiError> {
    let overview = state
        .catalog
        .reading_list_overview(caller.user.id, id)
        .await?;
    Ok(Json(ReadingListOverview {
        list: overview.list.into(),
        about: overview.about.into(),
    }))
}

/// Gives a reading list another name and summary. Its books stay. Only the
/// owner can.
#[utoipa::path(
    patch,
    path = "/reading-lists/{id}",
    tag = "lists",
    operation_id = "changeReadingList",
    params(("id" = i64, Path, description = "The reading list")),
    request_body = ReadingListDetails,
    responses(
        (status = OK, body = ReadingList),
        (status = BAD_REQUEST, description = "The name or the summary cannot be used", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
        (status = CONFLICT, description = "Another list has the name; `conflicts_with` is its id", body = ApiError),
    )
)]
async fn change(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(details): Json<ReadingListDetails>,
) -> Result<Json<ReadingList>, ApiError> {
    let list = state
        .catalog
        .change_list(owner.id, id, &details.name, details.summary.as_deref())
        .await?;
    Ok(Json(list.into()))
}

/// Deletes a reading list. Its books are not touched, nor is anyone's
/// progress in them. Only the owner can.
#[utoipa::path(
    delete,
    path = "/reading-lists/{id}",
    tag = "lists",
    operation_id = "deleteReadingList",
    params(("id" = i64, Path, description = "The reading list")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn delete(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.delete_list(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the books of a reading list in the list's order: the ones that are
/// on the server now and the caller may read. A book whose pages the server
/// holds in two files is listed once.
#[utoipa::path(
    get,
    path = "/reading-lists/{id}/books",
    tag = "lists",
    operation_id = "listReadingListBooks",
    params(("id" = i64, Path, description = "The reading list")),
    responses((status = OK, body = [Book]), (status = NOT_FOUND, body = ApiError))
)]
async fn books(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state.catalog.books_in_list(caller.user.id, id).await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Puts the books named at the end of a reading list, in the order they are
/// named. A book the list already holds stays where it is. Only the owner
/// can.
///
/// Every book, group, series and list named must be on the server: one that
/// is not, or a file that could not be read, refuses the request, and
/// nothing is added. One request names 10,000 at most.
#[utoipa::path(
    post,
    path = "/reading-lists/{id}/books",
    tag = "lists",
    operation_id = "addToReadingList",
    params(("id" = i64, Path, description = "The reading list")),
    request_body = BooksNamed,
    responses(
        (status = OK, body = Added),
        (status = BAD_REQUEST, description = "Something named is not on the server, or is a file that could not be read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn add_books(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(named): Json<BooksNamed>,
) -> Result<Json<Added>, ApiError> {
    let books = state
        .catalog
        .add_to_list(owner.id, id, &named.into())
        .await?;
    Ok(Json(Added { books }))
}

/// Takes a book out of a reading list. The others keep their order. Only
/// the owner can.
#[utoipa::path(
    delete,
    path = "/reading-lists/{id}/books/{book_id}",
    tag = "lists",
    operation_id = "removeFromReadingList",
    params(
        ("id" = i64, Path, description = "The reading list"),
        ("book_id" = i64, Path, description = "The book"),
    ),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such list or book, or the book is not in the list", body = ApiError),
    )
)]
async fn remove_book(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path((id, book_id)): Path<(i64, i64)>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .remove_from_list(owner.id, id, book_id)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Moves a book of a reading list to another position in it. The other
/// books keep their order. Only the owner can.
#[utoipa::path(
    put,
    path = "/reading-lists/{id}/books/{book_id}/position",
    tag = "lists",
    operation_id = "moveInReadingList",
    params(
        ("id" = i64, Path, description = "The reading list"),
        ("book_id" = i64, Path, description = "The book"),
    ),
    request_body = ListPosition,
    responses(
        (status = NO_CONTENT),
        (status = BAD_REQUEST, description = "The position is below 0", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, description = "No such list or book, or the book is not in the list", body = ApiError),
    )
)]
async fn move_book(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path((id, book_id)): Path<(i64, i64)>,
    Json(to): Json<ListPosition>,
) -> Result<StatusCode, ApiError> {
    state
        .catalog
        .move_in_list(owner.id, id, book_id, to.position)
        .await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Lists the reading lists a book is in, in name order.
#[utoipa::path(
    get,
    path = "/books/{id}/reading-lists",
    tag = "lists",
    operation_id = "listReadingListsOfBook",
    params(("id" = i64, Path, description = "The book")),
    responses((status = OK, body = [ReadingList]), (status = NOT_FOUND, body = ApiError))
)]
async fn lists_of_book(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<ReadingList>>, ApiError> {
    let lists = state.catalog.lists_of_book(caller.user.id, id).await?;
    Ok(Json(lists.into_iter().map(ReadingList::from).collect()))
}

/// One book as a CBL file names it.
#[derive(Debug, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct CblEntry {
    pub series: String,
    /// The issue's number, as the file writes it.
    pub number: Option<String>,
    /// The volume, as the file writes it. Most files give the year the
    /// series started.
    pub volume: Option<String>,
    /// The year the issue came out.
    pub year: Option<i64>,
}

impl From<CblEntry> for binge_core::CblEntry {
    fn from(entry: CblEntry) -> Self {
        Self {
            series: entry.series,
            number: entry.number,
            volume: entry.volume,
            year: entry.year,
        }
    }
}

/// What was found for an entry of a CBL file.
#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CblFound {
    /// One book: `book`.
    Book,
    /// No series goes by the entry's name.
    NoSeries,
    /// The series is there, and no book of it has the number.
    NoIssue,
    /// Several books fit, and nothing in the entry tells them apart:
    /// `candidates`.
    Several,
}

/// How the series of an entry was found. Each way is tried in this order
/// until one finds the issue.
#[derive(Clone, Copy, Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CblHow {
    /// By what the owner chose for it before.
    Rule,
    /// A series has exactly that name.
    Name,
    /// A series is called by the name and the volume: "Batman (2016)".
    NameAndVolume,
    /// The names are the same without "The", "A" or "An" in front.
    WithoutArticle,
    /// The names are the same without what is in brackets and without the
    /// words "Annual", "TPB", "HC", "GN" and "Omnibus".
    WithoutWords,
}

/// One entry of a CBL file, with what the server found for it.
#[derive(Debug, Serialize, ToSchema)]
pub struct CblLine {
    pub entry: CblEntry,
    pub found: CblFound,
    /// How its series was found. Missing when none was.
    pub how: Option<CblHow>,
    /// The rule that decided, when one did: `DELETE
    /// /reading-lists/import/rules/{id}` forgets it.
    pub rule_id: Option<i64>,
    /// The book the entry means, when one was found.
    pub book: Option<Book>,
    /// The books that fit, when there are several.
    pub candidates: Vec<Book>,
    /// The series that was looked in.
    pub series_id: Option<i64>,
    pub series_name: Option<String>,
}

/// What a reading list made from a CBL file would hold.
#[derive(Debug, Serialize, ToSchema)]
pub struct CblPlan {
    /// The name the file gives the list.
    pub name: Option<String>,
    /// The id of the reading list that already has that name.
    pub name_taken_by: Option<i64>,
    /// The file's entries, in its order.
    pub lines: Vec<CblLine>,
}

impl From<binge_core::CblPlan> for CblPlan {
    fn from(plan: binge_core::CblPlan) -> Self {
        let lines = plan.lines.into_iter().map(|line| {
            let (series_id, series_name) = line.series.unzip();
            CblLine {
                entry: CblEntry {
                    series: line.entry.series,
                    number: line.entry.number,
                    volume: line.entry.volume,
                    year: line.entry.year,
                },
                found: match line.found {
                    binge_core::CblFound::Book => CblFound::Book,
                    binge_core::CblFound::NoSeries => CblFound::NoSeries,
                    binge_core::CblFound::NoIssue => CblFound::NoIssue,
                    binge_core::CblFound::Several => CblFound::Several,
                },
                how: line.how.map(|how| match how {
                    binge_core::CblHow::Rule => CblHow::Rule,
                    binge_core::CblHow::Name => CblHow::Name,
                    binge_core::CblHow::NameAndVolume => CblHow::NameAndVolume,
                    binge_core::CblHow::WithoutArticle => CblHow::WithoutArticle,
                    binge_core::CblHow::WithoutWords => CblHow::WithoutWords,
                }),
                rule_id: line.rule_id,
                book: line.book.map(Book::from),
                candidates: line.candidates.into_iter().map(Book::from).collect(),
                series_id,
                series_name,
            }
        });
        Self {
            name: plan.name,
            name_taken_by: plan.name_taken_by,
            lines: lines.collect(),
        }
    }
}

/// Reads a CBL file, the reading list format of ComicRack, and says which
/// book of the server each of its entries means and how that was found. It
/// makes nothing: the list is made with `POST /reading-lists`, from the
/// books settled on. Only the owner can.
///
/// The request's body is the file, of 5 MB and 5,000 entries at most. An
/// entry's series is looked for by its name, by its name with the volume,
/// without its article, and without the words for a kind of publication,
/// in that order, until one finds a book with the entry's number. Among
/// several books with the number, the entry's volume and year decide. A
/// rule kept with `POST /reading-lists/import/rules` comes before all of
/// this.
#[utoipa::path(
    post,
    path = "/reading-lists/import",
    tag = "lists",
    operation_id = "planReadingListImport",
    request_body(content = Vec<u8>, content_type = "application/xml", description = "The CBL file"),
    responses(
        (status = OK, body = CblPlan),
        (status = BAD_REQUEST, description = "Not a CBL file the server can read, one that lists no books, or one too big", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn plan_import(
    State(state): State<AppState>,
    Owner(owner): Owner,
    file: Result<Bytes, BytesRejection>,
) -> Result<Json<CblPlan>, ApiError> {
    let file = whole(file, cbl::too_big, "file")?;
    let plan = state.catalog.plan_cbl(owner.id, &file).await?;
    Ok(Json(plan.into()))
}

/// What the owner says an entry of a CBL file means.
#[derive(Debug, Deserialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct NewCblRule {
    /// The entry, as the file writes it. A rule about a series goes by the
    /// entry's series and volume, and holds for every number.
    pub entry: CblEntry,
    /// The series that the entry's series is: its issues are looked for
    /// there. Give this or `book_id`, not both.
    pub series_id: Option<i64>,
    /// The book that this one entry is.
    pub book_id: Option<i64>,
}

/// A rule that was kept.
#[derive(Debug, Serialize, ToSchema)]
pub struct CblRule {
    pub id: i64,
}

/// Keeps what the owner says an entry of a CBL file means, for the imports
/// from now on: which series its series is, or which book the entry is. It
/// takes the place of the rule kept before for the same. Only the owner
/// can.
#[utoipa::path(
    post,
    path = "/reading-lists/import/rules",
    tag = "lists",
    operation_id = "keepImportRule",
    request_body = NewCblRule,
    responses(
        (status = CREATED, body = CblRule),
        (status = BAD_REQUEST, description = "Neither or both of a series and a book were given, or the one given is not on the server", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
    )
)]
async fn keep_rule(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(rule): Json<NewCblRule>,
) -> Result<(StatusCode, Json<CblRule>), ApiError> {
    let means = match (rule.series_id, rule.book_id) {
        (Some(series), None) => CblRuleFor::Series(series),
        (None, Some(book)) => CblRuleFor::Book(book),
        _ => {
            return Err(ApiError::new(
                ErrorCode::InvalidRequest,
                "Give the series or the book the entry means: one of the two.",
            ));
        }
    };
    let id = state
        .catalog
        .keep_cbl_rule(owner.id, &rule.entry.into(), means)
        .await?;
    Ok((StatusCode::CREATED, Json(CblRule { id })))
}

/// Forgets a rule kept for imports. Only the owner can.
#[utoipa::path(
    delete,
    path = "/reading-lists/import/rules/{id}",
    tag = "lists",
    operation_id = "forgetImportRule",
    params(("id" = i64, Path, description = "The rule")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn forget_rule(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.forget_cbl_rule(id).await?;
    Ok(StatusCode::NO_CONTENT)
}
