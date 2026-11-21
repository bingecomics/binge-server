//! Libraries: adding, removing and scanning them.

use std::path::PathBuf;

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum_extra::extract::Query;
use binge_core::{Catalog, Choice, Narrowing, SeriesOrder, Years};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use utoipa_axum::router::OpenApiRouter;
use utoipa_axum::routes;

use super::auth::{Caller, Owner};
use super::browse::{Book, PagedSeries, PartQuery};
use super::error::ApiError;
use super::people::NamedPerson;
use crate::app::AppState;

pub(super) fn routes() -> OpenApiRouter<AppState> {
    OpenApiRouter::new()
        .routes(routes!(list, add))
        .routes(routes!(get, remove))
        .routes(routes!(make_from_files))
        .routes(routes!(scan))
        .routes(routes!(series))
        .routes(routes!(letters))
        .routes(routes!(choices))
        .routes(routes!(unreadable))
        .routes(routes!(held_twice))
}

/// A folder of comics the server was pointed at.
#[derive(Debug, Serialize, ToSchema)]
pub struct Library {
    pub id: i64,
    pub name: String,
    /// The folder, as a full path on the server. Only the owner is told.
    pub path: Option<String>,
    pub series_count: i64,
    pub book_count: i64,
    /// When the last scan finished, in seconds since 1970.
    pub last_scanned_at: Option<i64>,
    /// How many of its files could not be read and are still to look at.
    /// `GET /libraries/{id}/unreadable` lists them. Only the owner is told.
    pub unreadable_count: Option<i64>,
    /// How many of the files that could not be read the owner set aside.
    /// Only the owner is told.
    pub set_aside_count: Option<i64>,
    /// What it makes from what its files say. Only the owner is told.
    pub from_files: Option<FromFiles>,
    pub scan: ScanStatus,
}

/// What a library makes from what its files say. What it makes is the
/// owner's, shared with everyone, and from then on a reading list or a
/// collection like any other. A scan adds what the files it read name, and
/// takes nothing out. No file is changed.
#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize, ToSchema)]
#[serde(deny_unknown_fields)]
pub struct FromFiles {
    /// A reading list of each story arc the files name (`StoryArc` with
    /// `StoryArcNumber`, and `AlternateSeries` with `AlternateNumber`), its
    /// books in the order those numbers give.
    pub lists: bool,
    /// A collection of each group of series the files name
    /// (`SeriesGroup`).
    pub collections: bool,
}

impl From<FromFiles> for binge_core::FromFiles {
    fn from(from: FromFiles) -> Self {
        Self {
            lists: from.lists,
            collections: from.collections,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    /// No scan is running.
    Idle,
    /// Walking the folder and listing books.
    Finding,
    /// Reading page sizes and making covers.
    Analyzing,
}

/// Where a library's scan stands. After a scan ends, its numbers stay until
/// the next one starts.
#[derive(Debug, Serialize, ToSchema)]
pub struct ScanStatus {
    pub phase: ScanPhase,
    /// Comic files seen in the folder so far.
    pub found: u32,
    /// Books that need their covers made in this scan.
    pub to_analyze: u32,
    /// Books whose covers are done.
    pub analyzed: u32,
    /// What went wrong in the last scan.
    pub problem: Option<String>,
}

impl From<binge_core::ScanStatus> for ScanStatus {
    fn from(status: binge_core::ScanStatus) -> Self {
        let phase = match status.phase {
            binge_core::ScanPhase::Idle => ScanPhase::Idle,
            binge_core::ScanPhase::Finding => ScanPhase::Finding,
            binge_core::ScanPhase::Analyzing => ScanPhase::Analyzing,
        };
        Self {
            phase,
            found: status.found,
            to_analyze: status.to_analyze,
            analyzed: status.analyzed,
            problem: status.problem,
        }
    }
}

/// A library as `user` is told about it.
fn describe(catalog: &Catalog, user: &binge_core::User, library: binge_core::Library) -> Library {
    Library {
        scan: catalog.scan_status(library.id).into(),
        id: library.id,
        name: library.name,
        // Where things are on the server's disk is the owner's business.
        path: user.is_owner.then_some(library.root_path),
        series_count: library.series_count,
        book_count: library.book_count,
        last_scanned_at: library.last_scanned_at,
        // What is still to look at is the owner's to look at.
        unreadable_count: user.is_owner.then_some(library.unreadable_count),
        set_aside_count: user.is_owner.then_some(library.set_aside_count),
        // What the server does with the files is the owner's to decide.
        from_files: user.is_owner.then_some(FromFiles {
            lists: library.lists_from_files,
            collections: library.collections_from_files,
        }),
    }
}

#[derive(Debug, Deserialize, ToSchema)]
pub struct NewLibrary {
    /// Full path of the folder on the server. A leading `~` means the home
    /// folder of the account the server runs as.
    pub path: String,
    /// Name to show. Defaults to the folder's name.
    pub name: Option<String>,
    /// What it makes from what its files say. Left out, it makes nothing
    /// until it is asked to.
    pub from_files: Option<FromFiles>,
}

/// Turns a leading `~` into the home folder, as a shell would.
fn expand_home(path: &str) -> PathBuf {
    let rest = match path.strip_prefix('~') {
        Some(rest) if rest.is_empty() || rest.starts_with('/') => rest,
        _ => return PathBuf::from(path),
    };
    match directories::BaseDirs::new() {
        Some(dirs) => dirs.home_dir().join(rest.trim_start_matches('/')),
        None => PathBuf::from(path),
    }
}

/// Lists the libraries the caller may read, in name order.
#[utoipa::path(
    get,
    path = "/libraries",
    tag = "libraries",
    operation_id = "listLibraries",
    responses((status = OK, body = [Library]))
)]
async fn list(
    State(state): State<AppState>,
    caller: Caller,
) -> Result<Json<Vec<Library>>, ApiError> {
    let libraries = state.catalog.libraries(caller.user.id).await?;
    Ok(Json(
        libraries
            .into_iter()
            .map(|library| describe(&state.catalog, &caller.user, library))
            .collect(),
    ))
}

/// Adds a folder as a library and starts scanning it. Only the owner can.
#[utoipa::path(
    post,
    path = "/libraries",
    tag = "libraries",
    operation_id = "addLibrary",
    request_body = NewLibrary,
    responses(
        (status = CREATED, body = Library),
        (status = BAD_REQUEST, description = "The path is not a folder the server can read", body = ApiError),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = CONFLICT, description = "The folder is already a library", body = ApiError),
    )
)]
async fn add(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Json(new): Json<NewLibrary>,
) -> Result<(StatusCode, Json<Library>), ApiError> {
    let path = expand_home(new.path.trim());
    let library = state
        .catalog
        .add_library(
            owner.id,
            &path,
            new.name.as_deref(),
            new.from_files.unwrap_or_default().into(),
        )
        .await?;
    let library = describe(&state.catalog, &owner, library);
    Ok((StatusCode::CREATED, Json(library)))
}

/// Returns one library with the state of its scan.
#[utoipa::path(
    get,
    path = "/libraries/{id}",
    tag = "libraries",
    operation_id = "getLibrary",
    params(("id" = i64, Path, description = "The library")),
    responses((status = OK, body = Library), (status = NOT_FOUND, body = ApiError))
)]
async fn get(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<Library>, ApiError> {
    let library = state.catalog.library(caller.user.id, id).await?;
    Ok(Json(describe(&state.catalog, &caller.user, library)))
}

/// Removes a library from the server. The folder and its files are not
/// touched. Only the owner can.
#[utoipa::path(
    delete,
    path = "/libraries/{id}",
    tag = "libraries",
    operation_id = "removeLibrary",
    params(("id" = i64, Path, description = "The library")),
    responses(
        (status = NO_CONTENT),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn remove(
    State(state): State<AppState>,
    _: Owner,
    Path(id): Path<i64>,
) -> Result<StatusCode, ApiError> {
    state.catalog.remove_library(id).await?;
    Ok(StatusCode::NO_CONTENT)
}

/// Says what a library makes from what its files say from now on. Only the
/// owner can.
///
/// What is turned on is made at once, from what the last scan found the
/// files to say. What is turned off makes nothing more: the reading lists
/// and collections there are stay.
#[utoipa::path(
    put,
    path = "/libraries/{id}/from-files",
    tag = "libraries",
    operation_id = "makeFromFiles",
    params(("id" = i64, Path, description = "The library")),
    request_body = FromFiles,
    responses(
        (status = OK, body = Library),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn make_from_files(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
    Json(from): Json<FromFiles>,
) -> Result<Json<Library>, ApiError> {
    let library = state
        .catalog
        .make_from_files(owner.id, id, from.into())
        .await?;
    Ok(Json(describe(&state.catalog, &owner, library)))
}

/// Starts scanning a library for added, changed and removed files. Does
/// nothing when a scan of it is already running. Only the owner can.
#[utoipa::path(
    post,
    path = "/libraries/{id}/scan",
    tag = "libraries",
    operation_id = "scanLibrary",
    params(("id" = i64, Path, description = "The library")),
    responses(
        (status = ACCEPTED, body = Library),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn scan(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<(StatusCode, Json<Library>), ApiError> {
    let library = state.catalog.library(owner.id, id).await?;
    state.catalog.start_scan(id);
    let library = describe(&state.catalog, &owner, library);
    Ok((StatusCode::ACCEPTED, Json(library)))
}

/// What a library's series are ordered by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SeriesSort {
    /// By name, from A.
    #[default]
    Name,
    /// By when its newest book was added, the newest first.
    Added,
    /// By the year its first book came out, the earliest first.
    Year,
    /// By when the caller last read from it, the latest first.
    LastRead,
    /// The ones with books left to read first.
    Unread,
    /// In no order. The same `seed` gives the same one again.
    Random,
}

impl From<SeriesSort> for binge_core::SeriesSort {
    fn from(sort: SeriesSort) -> Self {
        match sort {
            SeriesSort::Name => Self::Name,
            SeriesSort::Added => Self::Added,
            SeriesSort::Year => Self::Year,
            SeriesSort::LastRead => Self::LastRead,
            SeriesSort::Unread => Self::Unread,
            SeriesSort::Random => Self::Random,
        }
    }
}

impl From<binge_core::SeriesSort> for SeriesSort {
    fn from(sort: binge_core::SeriesSort) -> Self {
        match sort {
            binge_core::SeriesSort::Name => Self::Name,
            binge_core::SeriesSort::Added => Self::Added,
            binge_core::SeriesSort::Year => Self::Year,
            binge_core::SeriesSort::LastRead => Self::LastRead,
            binge_core::SeriesSort::Unread => Self::Unread,
            binge_core::SeriesSort::Random => Self::Random,
        }
    }
}

/// How far the caller is through a series.
#[derive(Clone, Copy, Debug, Deserialize, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReadState {
    /// They opened none of its books.
    Unread,
    /// They opened some, and have books left.
    InProgress,
    /// They read every book of it.
    Read,
}

impl From<ReadState> for binge_core::ReadState {
    fn from(state: ReadState) -> Self {
        match state {
            ReadState::Unread => Self::Unread,
            ReadState::InProgress => Self::InProgress,
            ReadState::Read => Self::Read,
        }
    }
}

impl From<binge_core::ReadState> for ReadState {
    fn from(state: binge_core::ReadState) -> Self {
        match state {
            binge_core::ReadState::Unread => Self::Unread,
            binge_core::ReadState::InProgress => Self::InProgress,
            binge_core::ReadState::Read => Self::Read,
        }
    }
}

/// Which series of a library, and in what order: what
/// `GET /libraries/{id}/series` takes, and what a view keeps. A series is
/// narrowed by what its books say: it is of a publisher, a year or a person
/// when one of its books is. Each thing to narrow by can be given several
/// times: a series stays when it has one of the values asked for, and none
/// of those ruled out with `not_`. It has to pass every kind of narrowing
/// given, and what is left out is not narrowed by.
#[derive(Debug, Default, Deserialize, Serialize, ToSchema, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct Asked {
    /// What to order the series by. By name when left out. Series the order
    /// says nothing about, such as one with no year, come last.
    #[serde(default)]
    #[param(inline)]
    pub sort: SeriesSort,
    /// Turns the order around.
    #[serde(default)]
    pub reverse: bool,
    /// Which of the orders `sort=random` gives. Keep it while reading the
    /// list a part at a time.
    #[serde(default)]
    pub seed: u32,
    /// How far the caller is through the series.
    #[serde(default)]
    #[param(inline)]
    pub read: Vec<ReadState>,
    #[serde(default)]
    #[param(inline)]
    pub not_read: Vec<ReadState>,
    /// A publisher, as the books name it.
    #[serde(default)]
    pub publisher: Vec<String>,
    #[serde(default)]
    pub not_publisher: Vec<String>,
    /// A year a book came out, as `1993`, or the ten from one, as `1990s`.
    #[serde(default)]
    pub year: Vec<String>,
    #[serde(default)]
    pub not_year: Vec<String>,
    /// The id of a person a book names as a writer.
    #[serde(default)]
    pub writer: Vec<i64>,
    #[serde(default)]
    pub not_writer: Vec<i64>,
    /// The id of a person a book names as a penciller.
    #[serde(default)]
    pub penciller: Vec<i64>,
    #[serde(default)]
    pub not_penciller: Vec<i64>,
}

impl Asked {
    pub(super) fn order(&self) -> SeriesOrder {
        SeriesOrder {
            sort: self.sort.into(),
            reverse: self.reverse,
            seed: self.seed,
        }
    }

    pub(super) fn narrowing(self) -> Result<Narrowing, ApiError> {
        let years = |written: Vec<String>| -> Result<Vec<Years>, ApiError> {
            let read = written.iter().map(|years| Years::parse(years));
            Ok(read.collect::<Result<_, _>>()?)
        };
        let read = |states: Vec<ReadState>| states.into_iter().map(Into::into).collect();
        Ok(Narrowing {
            read: Choice {
                asked: read(self.read),
                ruled_out: read(self.not_read),
            },
            publishers: Choice {
                asked: self.publisher,
                ruled_out: self.not_publisher,
            },
            years: Choice {
                asked: years(self.year)?,
                ruled_out: years(self.not_year)?,
            },
            writers: Choice {
                asked: self.writer,
                ruled_out: self.not_writer,
            },
            pencillers: Choice {
                asked: self.penciller,
                ruled_out: self.not_penciller,
            },
        })
    }

    /// What was asked for, written the way it is asked for.
    pub(super) fn of(order: SeriesOrder, narrowing: Narrowing) -> Self {
        let states =
            |states: Vec<binge_core::ReadState>| states.into_iter().map(Into::into).collect();
        let written = |years: Vec<Years>| years.into_iter().map(Years::written).collect();
        // Named one by one, so that a new thing to narrow by is not left out.
        let Narrowing {
            read,
            publishers,
            years,
            writers,
            pencillers,
        } = narrowing;
        Self {
            sort: order.sort.into(),
            reverse: order.reverse,
            seed: order.seed,
            read: states(read.asked),
            not_read: states(read.ruled_out),
            publisher: publishers.asked,
            not_publisher: publishers.ruled_out,
            year: written(years.asked),
            not_year: written(years.ruled_out),
            writer: writers.asked,
            not_writer: writers.ruled_out,
            penciller: pencillers.asked,
            not_penciller: pencillers.ruled_out,
        }
    }
}

/// Lists a part of the series of a library: those the narrowing leaves in,
/// in the order asked for, with how many there are in all.
#[utoipa::path(
    get,
    path = "/libraries/{id}/series",
    tag = "libraries",
    operation_id = "listSeries",
    params(("id" = i64, Path, description = "The library"), Asked, PartQuery),
    responses(
        (status = OK, body = PagedSeries),
        (status = BAD_REQUEST, description = "A year is not written as one", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn series(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Query(asked): Query<Asked>,
    Query(part): Query<PartQuery>,
) -> Result<Json<PagedSeries>, ApiError> {
    let order = asked.order();
    let narrowing = asked.narrowing()?;
    let part = state
        .catalog
        .series_part(caller.user.id, id, &narrowing, order, part.span())
        .await?;
    Ok(Json(part.into()))
}

/// A letter that the names of series start with.
#[derive(Debug, Serialize, ToSchema)]
pub struct Letter {
    /// A capital from A to Z, or `#` for every name that starts with
    /// anything else. A name is filed without its accent: `Élodie` under E.
    pub letter: String,
    /// How many series start with it.
    pub count: i64,
    /// Where the first of them is in the list by name: the `offset` to ask
    /// `GET /libraries/{id}/series` for it with.
    pub offset: i64,
}

/// Lists the letters the names of a library's series start with, for
/// jumping through the list by name: only the letters some series has, in
/// the order of the list. Takes the narrowing `GET /libraries/{id}/series`
/// takes and counts the series it leaves in; of the rest it reads `reverse`
/// only.
#[utoipa::path(
    get,
    path = "/libraries/{id}/series/letters",
    tag = "libraries",
    operation_id = "listSeriesLetters",
    params(("id" = i64, Path, description = "The library"), Asked),
    responses(
        (status = OK, body = [Letter]),
        (status = BAD_REQUEST, description = "A year is not written as one", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn letters(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Query(asked): Query<Asked>,
) -> Result<Json<Vec<Letter>>, ApiError> {
    let reverse = asked.reverse;
    let narrowing = asked.narrowing()?;
    let letters = state
        .catalog
        .series_letters(caller.user.id, id, &narrowing, reverse)
        .await?;
    Ok(Json(
        letters
            .into_iter()
            .map(|letter| Letter {
                letter: letter.letter.to_string(),
                count: letter.count,
                offset: letter.offset,
            })
            .collect(),
    ))
}

/// A publisher the series of a library can be narrowed by.
#[derive(Debug, Serialize, ToSchema)]
pub struct NamedPublisher {
    pub name: String,
    /// How many series of the library have a book of theirs.
    pub series_count: i64,
}

/// Ten years the series of a library can be narrowed by.
#[derive(Debug, Serialize, ToSchema)]
pub struct NamedDecade {
    /// The first of the ten years, such as `1990`. Ask for them as `1990s`.
    pub first_year: i64,
    /// How many series of the library have a book from them.
    pub series_count: i64,
}

/// What the series of a library can be narrowed by: every value its books
/// have.
#[derive(Debug, Serialize, ToSchema)]
pub struct LibraryChoices {
    /// The one most series have first.
    pub publishers: Vec<NamedPublisher>,
    /// The earliest first.
    pub decades: Vec<NamedDecade>,
    /// The one most series name first.
    pub writers: Vec<NamedPerson>,
    /// The one most series name first.
    pub pencillers: Vec<NamedPerson>,
}

/// Lists what the series of a library can be narrowed by.
#[utoipa::path(
    get,
    path = "/libraries/{id}/choices",
    tag = "libraries",
    operation_id = "listLibraryChoices",
    params(("id" = i64, Path, description = "The library")),
    responses((status = OK, body = LibraryChoices), (status = NOT_FOUND, body = ApiError))
)]
async fn choices(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
) -> Result<Json<LibraryChoices>, ApiError> {
    let choices = state.catalog.series_choices(caller.user.id, id).await?;
    let people =
        |people: Vec<binge_core::NamedPerson>| people.into_iter().map(NamedPerson::from).collect();
    Ok(Json(LibraryChoices {
        publishers: choices
            .publishers
            .into_iter()
            .map(|named| NamedPublisher {
                name: named.value,
                series_count: named.series_count,
            })
            .collect(),
        decades: choices
            .decades
            .into_iter()
            .map(|named| NamedDecade {
                first_year: named.value,
                series_count: named.series_count,
            })
            .collect(),
        writers: people(choices.writers),
        pencillers: people(choices.pencillers),
    }))
}

/// Which of the files that could not be read to list.
#[derive(Debug, Deserialize, IntoParams)]
#[into_params(parameter_in = Query)]
pub struct UnreadableQuery {
    /// Lists the ones the owner set aside, in place of the ones still to
    /// look at.
    #[serde(default)]
    pub set_aside: bool,
}

/// Lists the files of a library that could not be read, each with the reason
/// in `problem`, in the order of their paths. They are in no series. The
/// ones the owner set aside are left out, or are the ones listed when
/// `set_aside` is given, which only the owner can ask for.
#[utoipa::path(
    get,
    path = "/libraries/{id}/unreadable",
    tag = "libraries",
    operation_id = "listUnreadableFiles",
    params(("id" = i64, Path, description = "The library"), UnreadableQuery),
    responses(
        (status = OK, body = [Book]),
        (status = FORBIDDEN, description = "The caller is not the owner and asks for the ones set aside", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn unreadable(
    State(state): State<AppState>,
    caller: Caller,
    Path(id): Path<i64>,
    Query(query): Query<UnreadableQuery>,
) -> Result<Json<Vec<Book>>, ApiError> {
    if query.set_aside && !caller.user.is_owner {
        return Err(ApiError::forbidden(
            "Only the owner of this server sees the files set aside.",
        ));
    }
    let files = state
        .catalog
        .unreadable_in(caller.user.id, id, query.set_aside)
        .await?;
    Ok(Json(files.into_iter().map(Book::from).collect()))
}

/// A file of a book the library holds more than once.
#[derive(Debug, Serialize, ToSchema)]
pub struct HeldFile {
    pub book: Book,
    /// Where the file is, from the library's folder.
    pub path: String,
}

/// A book the library holds more than once.
#[derive(Debug, Serialize, ToSchema)]
pub struct HeldTwice {
    /// Its files, in the order of their paths: two at least.
    pub files: Vec<HeldFile>,
}

/// Lists the books a library holds more than once, each with its files, in
/// name order. Two files are one book when their pages are the same,
/// whatever their names and tags, so a copy that was retagged is found too.
/// The server deletes no file: which copy to keep is for the owner to do in
/// the folder. Only the owner can ask.
#[utoipa::path(
    get,
    path = "/libraries/{id}/duplicates",
    tag = "libraries",
    operation_id = "listDuplicates",
    params(("id" = i64, Path, description = "The library")),
    responses(
        (status = OK, body = [HeldTwice]),
        (status = FORBIDDEN, description = "The caller is not the owner", body = ApiError),
        (status = NOT_FOUND, body = ApiError),
    )
)]
async fn held_twice(
    State(state): State<AppState>,
    Owner(owner): Owner,
    Path(id): Path<i64>,
) -> Result<Json<Vec<HeldTwice>>, ApiError> {
    let held = state.catalog.held_twice_in(owner.id, id).await?;
    Ok(Json(
        held.into_iter()
            .map(|files| HeldTwice {
                files: files
                    .into_iter()
                    .map(|file| HeldFile {
                        book: file.book.into(),
                        path: file.path,
                    })
                    .collect(),
            })
            .collect(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_a_leading_tilde_means_home() {
        let home = directories::BaseDirs::new()
            .unwrap()
            .home_dir()
            .to_path_buf();
        assert_eq!(expand_home("~"), home);
        assert_eq!(expand_home("~/Comics"), home.join("Comics"));
        assert_eq!(expand_home("~other/Comics"), PathBuf::from("~other/Comics"));
        assert_eq!(
            expand_home("/data/~/Comics"),
            PathBuf::from("/data/~/Comics")
        );
    }
}
