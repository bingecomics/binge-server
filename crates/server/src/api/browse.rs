//! Browsing: series, their books, covers, and search.

use axum::Json;
use axum::extract::{Path, Query, State};
use axum::http::header;
use axum::response::IntoResponse;
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::Caller;
use super::error::ApiError;
use super::lists::ReadingList;
use super::people::{Credit, Person};
use super::reading::Progress;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(series))
        .routes(routes!(overview))
        .routes(routes!(books))
        .routes(routes!(group_overview))
        .routes(routes!(group_books))
        .routes(routes!(cover))
        .routes(routes!(search))
}

/// A series of a library: the books that go by one name. The server keeps
/// it as its own record. It is not a folder, and a book stays in its series
/// when its file is moved, renamed or retagged. It may hold no books: a
/// series stays until it is deleted.
#[derive(Debug, Serialize, ToSchema)]
pub struct Series {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub book_count: i64,
    /// How many of its books the caller has not read to the end.
    pub unread_count: i64,
    /// Name of the series' cover, for `/covers/{cover}`: the one chosen for
    /// it, or else the cover of its first book in reading order. Missing
    /// until a book has one.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the series.
    pub cover_chosen: bool,
}

/// What a group of a series holds.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum GroupKind {
    /// One run of numbered issues.
    Volume,
    /// A short series told under the series' name.
    Limited,
    /// The yearly issues, of one volume when the group has its number.
    Annual,
    /// The books that are no part of a run: each is read by itself.
    OneShot,
    /// Issues of other titles that the series' people turn up in.
    Appearance,
    /// Issues gathered into books.
    Collected,
}

impl From<binge_core::GroupKind> for GroupKind {
    fn from(kind: binge_core::GroupKind) -> Self {
        match kind {
            binge_core::GroupKind::Volume => Self::Volume,
            binge_core::GroupKind::Limited => Self::Limited,
            binge_core::GroupKind::Annual => Self::Annual,
            binge_core::GroupKind::OneShot => Self::OneShot,
            binge_core::GroupKind::Appearance => Self::Appearance,
            binge_core::GroupKind::Collected => Self::Collected,
        }
    }
}

impl From<GroupKind> for binge_core::GroupKind {
    fn from(kind: GroupKind) -> Self {
        match kind {
            GroupKind::Volume => Self::Volume,
            GroupKind::Limited => Self::Limited,
            GroupKind::Annual => Self::Annual,
            GroupKind::OneShot => Self::OneShot,
            GroupKind::Appearance => Self::Appearance,
            GroupKind::Collected => Self::Collected,
        }
    }
}

/// A part of a series: one of its volumes, a limited series told under its
/// name, its annuals, its one-shots, its appearances in other titles, its
/// collected editions. It may hold no books.
#[derive(Debug, Serialize, ToSchema)]
pub struct Group {
    pub id: i64,
    pub series_id: i64,
    pub kind: GroupKind,
    /// The number it is known by, when it has one: a volume's, or for
    /// annuals that of the volume they are of.
    pub number: Option<i64>,
    /// The group's own name.
    pub name: Option<String>,
    pub book_count: i64,
    /// How many of its books the caller has not read to the end.
    pub unread_count: i64,
    /// Names the cover image of the book that stands for the group: the
    /// first of its books, in reading order, that has one. Fetch it from
    /// `/covers/{cover}`.
    pub cover: Option<String>,
}

impl From<binge_core::Group> for Group {
    fn from(group: binge_core::Group) -> Self {
        Self {
            id: group.id,
            series_id: group.series_id,
            kind: group.kind.into(),
            number: group.number,
            name: group.name,
            book_count: group.book_count,
            unread_count: group.unread_count,
            cover: group.cover,
        }
    }
}

impl From<binge_core::Series> for Series {
    fn from(series: binge_core::Series) -> Self {
        Self {
            id: series.id,
            library_id: series.library_id,
            name: series.name,
            book_count: series.book_count,
            unread_count: series.unread_count,
            cover: series.cover,
            cover_chosen: series.cover_chosen,
        }
    }
}

/// One comic file.
#[derive(Debug, Serialize, ToSchema)]
pub struct Book {
    pub id: i64,
    pub library_id: i64,
    /// The series the book is in. Missing for a file that could not be read:
    /// it is in no series, and `GET /libraries/{id}/unreadable` lists it.
    pub series_id: Option<i64>,
    /// The name of that series. Away from the series' own page a book is
    /// called by it and its number: "The Flash #12".
    pub series_name: Option<String>,
    /// The group of the series the book is in. Missing for a book that sits
    /// directly in its series.
    pub group_id: Option<i64>,
    /// What that group holds. A book of the annuals is called by it:
    /// "Batman Annual #1".
    pub group_kind: Option<GroupKind>,
    /// The issue's number where the book sits, such as `12` or `0.5`. It is
    /// the server's own record, not the file's tag, which is `info.number`.
    pub number: Option<String>,
    /// The date published where the book sits, written `2016`, `2016-07` or
    /// `2016-07-21`: as much as is known. Books with no number are read in
    /// its order.
    pub published: Option<String>,
    pub name: String,
    pub page_count: i64,
    /// Size of the file in bytes.
    pub file_size: i64,
    /// When the server first saw the file, in seconds since 1970.
    pub added_at: i64,
    /// What the book is, computed from its pages. It stays the same when the
    /// file is moved, renamed or retagged. Missing when the file could not be
    /// read. Page images are at `/pages/{identity}/{number}`.
    pub identity: Option<String>,
    /// Name of the book's cover, for `/covers/{cover}`: the one chosen for
    /// it, or else its first page. Missing until the scan has made that.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the book.
    pub cover_chosen: bool,
    /// Why the file could not be read as a comic.
    pub problem: Option<String>,
    /// How far the caller has read the book. Missing when they never opened
    /// it past its first page.
    pub progress: Option<Progress>,
    /// What the book's file says about itself. Missing when it says nothing.
    pub info: Option<BookInfo>,
}

/// What a book's file says about itself, from the `ComicInfo.xml` a tagging
/// tool put inside it. Each part is missing when the file does not give it.
#[derive(Debug, Serialize, ToSchema)]
pub struct BookInfo {
    pub title: Option<String>,
    /// The series the file says it belongs to. The server reads it once, to
    /// place a new book; after that it is only what the file claims, and
    /// `series_id` says where the book is.
    pub series: Option<String>,
    /// The issue's number as written, such as `12`, `0.5` or `Annual 1`.
    pub number: Option<String>,
    pub volume: Option<i64>,
    pub year: Option<i64>,
    /// From 1 to 12.
    pub month: Option<i64>,
    /// From 1 to 31.
    pub day: Option<i64>,
    /// One name, or several separated by commas.
    pub writer: Option<String>,
    /// One name, or several separated by commas.
    pub penciller: Option<String>,
    pub publisher: Option<String>,
    pub summary: Option<String>,
    /// A language code, such as `en` or `es`.
    pub language: Option<String>,
    /// The kind of publication, such as `Annual` or `TPB`.
    pub format: Option<String>,
}

impl BookInfo {
    fn from(info: binge_core::ComicInfo) -> Option<Self> {
        (!info.is_empty()).then_some(Self {
            title: info.title,
            series: info.series,
            number: info.number,
            volume: info.volume,
            year: info.year,
            month: info.month,
            day: info.day,
            writer: info.writer,
            penciller: info.penciller,
            publisher: info.publisher,
            summary: info.summary,
            language: info.language,
            format: info.format,
        })
    }
}

impl From<binge_core::Book> for Book {
    fn from(book: binge_core::Book) -> Self {
        Self {
            cover: book.cover,
            cover_chosen: book.cover_chosen,
            id: book.id,
            library_id: book.library_id,
            series_id: book.series_id,
            series_name: book.series_name,
            group_id: book.group_id,
            group_kind: book.group_kind.map(GroupKind::from),
            number: book.number,
            published: book.published.map(binge_core::layout::date_written),
            name: book.name,
            page_count: book.page_count,
            file_size: book.file_size,
            added_at: book.added_at,
            identity: book.content_hash,
            problem: book.problem,
            progress: book.progress.map(Progress::from),
            info: BookInfo::from(book.info),
        }
    }
}

/// How far the caller is through some books: a series, or a group of one.
#[derive(Debug, Serialize, ToSchema)]
pub struct ReadSoFar {
    /// How many of the books can be read. The rest are files the server
    /// could not understand.
    pub readable_count: i64,
    /// Pages in all the books.
    pub page_count: i64,
    /// Books the caller read to the end, out of `readable_count`.
    pub books_read: i64,
    /// Pages the caller has been through, over all the books.
    pub pages_read: i64,
    /// How long the pages not yet read will take the caller, in seconds, at
    /// their own pace: the time a page took them over their latest
    /// sittings. Missing until they have read enough here to tell.
    pub seconds_left: Option<i64>,
    /// The book to open to go on with them. It follows what the caller did
    /// last among these books: a book they left unfinished, or after one
    /// they finished, the first unfinished book behind it, or at the end the
    /// first unfinished one anywhere. Someone who read nothing, or
    /// everything, starts at the first book in reading order: when
    /// `books_read` is `readable_count`, this is where to read them again.
    /// Missing when none of the books can be read.
    pub next: Option<Book>,
}

impl From<binge_core::ReadSoFar> for ReadSoFar {
    fn from(read: binge_core::ReadSoFar) -> Self {
        Self {
            readable_count: read.readable_count,
            page_count: read.page_count,
            books_read: read.books_read,
            pages_read: read.pages_read,
            seconds_left: read.seconds_left,
            next: read.next.map(Book::from),
        }
    }
}

/// Some books seen whole, for the caller: the books of a series, or of a
/// group of one.
#[derive(Debug, Serialize, ToSchema)]
pub struct BooksOverview {
    /// How far the caller is through them, and the book to open next.
    pub read: ReadSoFar,
    /// The publisher most of the books name.
    pub publisher: Option<String>,
    /// The earliest year the books name.
    pub first_year: Option<i64>,
    /// The latest year the books name.
    pub last_year: Option<i64>,
    /// Who the books name as writers, the one most books name first. Two
    /// spellings of one person are that person, once.
    pub writers: Vec<Credit>,
    /// Who the books name as pencillers, the one most books name first.
    pub pencillers: Vec<Credit>,
    pub summary: Option<Summary>,
}

/// What some books are about. No file describes a series or a group, so
/// these are the words of the first of the books, in reading order, that
/// describes itself.
#[derive(Debug, Serialize, ToSchema)]
pub struct Summary {
    pub text: String,
    /// The book the words are from. Missing for a reading list, whose
    /// summary is its own.
    pub book: Option<Book>,
}

impl From<binge_core::BooksOverview> for BooksOverview {
    fn from(about: binge_core::BooksOverview) -> Self {
        Self {
            read: about.read.into(),
            publisher: about.publisher,
            first_year: about.first_year,
            last_year: about.last_year,
            writers: about.writers.into_iter().map(Credit::from).collect(),
            pencillers: about.pencillers.into_iter().map(Credit::from).collect(),
            summary: about.summary.map(|summary| Summary {
                text: summary.text,
                book: summary.book.map(Book::from),
            }),
        }
    }
}

/// Which part of a long list to answer with.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct PartQuery {
    /// How many to pass over, from the start of the list.
    #[serde(default)]
    pub offset: u32,
    /// How many to answer with: 100 when left out, 500 at most.
    pub limit: Option<u32>,
}

impl PartQuery {
    pub(super) fn span(&self) -> binge_core::Span {
        binge_core::Span::asked(self.offset, self.limit)
    }
}

/// A part of a list of series.
#[derive(Debug, Serialize, ToSchema)]
pub struct PagedSeries {
    pub series: Vec<Series>,
    /// How many series the whole list holds.
    pub total: i64,
}

impl From<binge_core::PartOf<binge_core::Series>> for PagedSeries {
    fn from(part: binge_core::PartOf<binge_core::Series>) -> Self {
        Self {
            series: part.items.into_iter().map(Series::from).collect(),
            total: part.total,
        }
    }
}

/// A part of a list of books.
#[derive(Debug, Serialize, ToSchema)]
pub struct PagedBooks {
    pub books: Vec<Book>,
    /// How many books the whole list holds.
    pub total: i64,
}

impl From<binge_core::PartOf<binge_core::Book>> for PagedBooks {
    fn from(part: binge_core::PartOf<binge_core::Book>) -> Self {
        Self {
            books: part.items.into_iter().map(Book::from).collect(),
            total: part.total,
        }
    }
}

/// A series seen whole, for the page that presents it: what its books say
/// about it together, and how far the caller is through it.
#[derive(Debug, Serialize, ToSchema)]
pub struct SeriesOverview {
    pub series: Series,
    /// Its groups, in the order they are read, the ones that hold no books
    /// included. Empty when it has none.
    pub groups: Vec<Group>,
    #[serde(flatten)]
    pub about: BooksOverview,
}

impl From<binge_core::SeriesOverview> for SeriesOverview {
    fn from(overview: binge_core::SeriesOverview) -> Self {
        Self {
            series: overview.series.into(),
            groups: overview.groups.into_iter().map(Group::from).collect(),
            about: overview.about.into(),
        }
    }
}

/// A group of a series seen whole, for the page that presents it: what its
/// books say about it together, and how far the caller is through it. What
/// to open next follows what they did last in this group, whatever they
/// read elsewhere.
#[derive(Debug, Serialize, ToSchema)]
pub struct GroupOverview {
    pub group: Group,
    /// The series it is a part of.
    pub series: Series,
    #[serde(flatten)]
    pub about: BooksOverview,
}

impl From<binge_core::GroupOverview> for GroupOverview {
    fn from(overview: binge_core::GroupOverview) -> Self {
        Self {
            group: overview.group.into(),
            series: overview.series.into(),
            about: overview.about.into(),
        }
    }
}

/// Returns one series.
#[utoipa::path(
    get,
    path = "/series/{id}",
    tag = "browse",
    operation_id = "getSeries",
    params(("id" = i64, Path, description = "The series")),
    responses((status = OK, body = Series), (status = NOT_FOUND, body = ApiError))
)]
async fn series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Series>, ApiError> {
    Ok(Json(state.catalog.series(caller.user.id, id).await?.into()))
}

/// Returns one series seen whole, for the page that presents it: what its
/// books say about it together, how far the caller is through it, and the
/// book to open next.
#[utoipa::path(
    get,
    path = "/series/{id}/overview",
    tag = "browse",
    operation_id = "getSeriesOverview",
    params(("id" = i64, Path, description = "The series")),
    responses((status = OK, body = SeriesOverview), (status = NOT_FOUND, body = ApiError))
)]
async fn overview(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<SeriesOverview>, ApiError> {
    let overview = state.catalog.series_overview(caller.user.id, id).await?;
    Ok(Json(overview.into()))
}

/// Lists the books of a series in the order they are read: group by group,
/// as `groups` of the series' overview lists them, with the books in no
/// group after the limited series and before the annuals. Inside a group
/// they go by issue number, and the books without one by the date they were
/// published.
#[utoipa::path(
    get,
    path = "/series/{id}/books",
    tag = "browse",
    operation_id = "listBooks",
    params(("id" = i64, Path, description = "The series")),
    responses((status = OK, body = [Book]), (status = NOT_FOUND, body = ApiError))
)]
async fn books(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state.catalog.books_in(caller.user.id, id).await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Returns one group of a series seen whole, for the page that presents it:
/// the group, its series, what its books say about it together, how far the
/// caller is through it, and the book to open next.
#[utoipa::path(
    get,
    path = "/groups/{id}/overview",
    tag = "browse",
    operation_id = "getGroupOverview",
    params(("id" = i64, Path, description = "The group")),
    responses((status = OK, body = GroupOverview), (status = NOT_FOUND, body = ApiError))
)]
async fn group_overview(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<GroupOverview>, ApiError> {
    let overview = state.catalog.group_overview(caller.user.id, id).await?;
    Ok(Json(overview.into()))
}

/// Lists the books of a group in the order they are read: by issue number,
/// and the books without one by the date they were published.
#[utoipa::path(
    get,
    path = "/groups/{id}/books",
    tag = "browse",
    operation_id = "listGroupBooks",
    params(("id" = i64, Path, description = "The group")),
    responses((status = OK, body = [Book]), (status = NOT_FOUND, body = ApiError))
)]
async fn group_books(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Vec<Book>>, ApiError> {
    let books = state.catalog.books_in_group(caller.user.id, id).await?;
    Ok(Json(books.into_iter().map(Book::from).collect()))
}

/// Returns a cover image. A cover never changes under its name, so it can be
/// cached for good.
#[utoipa::path(
    get,
    path = "/covers/{cover}",
    tag = "browse",
    operation_id = "getCover",
    params(("cover" = String, Path, description = "The `cover` of a series or book")),
    responses(
        (status = OK, description = "A JPEG image", content_type = "image/jpeg", body = Vec<u8>),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn cover(
    State(state): State<AppState>,
    caller: Caller,
    Path(cover): Path<String>,
) -> Result<impl IntoResponse, ApiError> {
    let missing = || ApiError::not_found("There is no such cover.");
    let path = state
        .catalog
        .cover_path(caller.user.id, &cover)
        .await?
        .ok_or_else(missing)?;
    let image = tokio::fs::read(path).await.map_err(|_| missing())?;
    let headers = [
        (header::CONTENT_TYPE, "image/jpeg"),
        (header::CACHE_CONTROL, super::KEPT_FOR_GOOD),
    ];
    Ok((headers, image))
}

#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct SearchQuery {
    /// The words to look for. Case and accents do not matter.
    #[serde(default)]
    pub q: String,
    /// How many of each kind to answer with: 50 when left out, and at most.
    pub limit: Option<u32>,
}

/// What a search found.
#[derive(Debug, Serialize, ToSchema)]
pub struct SearchResults {
    /// Series found by their name, in name order.
    pub series: Vec<Series>,
    /// Whether more series were found than are listed.
    pub more_series: bool,
    /// Books found by their name, title, series, writer, penciller or
    /// publisher, in name order.
    pub books: Vec<Book>,
    /// Whether more books were found than are listed.
    pub more_books: bool,
    /// Reading lists found by their name, in name order.
    pub lists: Vec<ReadingList>,
    /// Whether more reading lists were found than are listed.
    pub more_lists: bool,
    /// People found by any name they go by, the ones most books name first.
    pub people: Vec<Person>,
    /// Whether more people were found than are listed.
    pub more_people: bool,
}

/// Searches what the caller may read. Finds the series, books, reading
/// lists and people that hold every word of `q`, anywhere in a word, and
/// answers with at most `limit` of each, saying when there are more. A
/// search with no words finds nothing.
#[utoipa::path(
    get,
    path = "/search",
    tag = "browse",
    operation_id = "search",
    params(SearchQuery),
    responses((status = OK, body = SearchResults))
)]
async fn search(
    State(state): State<AppState>,
    caller: Caller,
    Query(query): Query<SearchQuery>,
) -> Result<Json<SearchResults>, ApiError> {
    let found = state
        .catalog
        .search(caller.user.id, &query.q, query.limit)
        .await?;
    Ok(Json(SearchResults {
        series: found.series.into_iter().map(Series::from).collect(),
        more_series: found.more_series,
        books: found.books.into_iter().map(Book::from).collect(),
        more_books: found.more_books,
        lists: found.lists.into_iter().map(ReadingList::from).collect(),
        more_lists: found.more_lists,
        people: found.people.into_iter().map(Person::from).collect(),
        more_people: found.more_people,
    }))
}
