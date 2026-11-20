//! Libraries, series and books: what the server knows about the comics.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use tokio::sync::Semaphore;

use crate::accounts::UserId;
use crate::archive::ArchiveError;
use crate::comicinfo::ComicInfo;
use crate::db::Database;
use crate::reading::Progress;
use crate::scan::{Scan, analysis_workers};
use crate::thumbnail::Thumbnails;

/// What went wrong. `Display` is for the log; [`CatalogError::for_people`]
/// is what to tell the person using the server.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    #[error("there is no such library, series, group, book or reading list")]
    NotFound,
    #[error("there is no such page")]
    NoSuchPage,
    #[error("there is no such rule for imports")]
    NoSuchRule,
    #[error("there is no such person")]
    NoSuchPerson,
    #[error("the page is not in the book")]
    InvalidPage,
    /// Holds the file's path.
    #[error("the book's file is not there: {0}")]
    FileMissing(String),
    /// Holds the file's path.
    #[error("the book's file is not the one that was scanned: {0}")]
    FileChanged(String),
    /// Holds the reason, written for the person who typed the path.
    #[error("{0}")]
    InvalidPath(String),
    #[error("that folder is already a library")]
    AlreadyAdded,
    /// A change to the organization that cannot be made as asked. Holds the
    /// reason, written for whoever asked.
    #[error("{0}")]
    Invalid(&'static str),
    /// Holds the id of the series that has the name.
    #[error("series {0} of the library already has that name")]
    NameTaken(i64),
    /// Holds the id of the group that is the same.
    #[error("the series already has that group: {0}")]
    GroupExists(i64),
    /// Holds the id of the reading list that has the name.
    #[error("reading list {0} already has that name")]
    ListNameTaken(i64),
    /// Holds the id of the person who goes by the name.
    #[error("person {0} already goes by that name")]
    NameOfAnother(i64),
    #[error("it still holds books")]
    NotEmpty,
    #[error("the cover could not be kept: {0}")]
    CoverNotKept(#[source] std::io::Error),
    /// Holds the folder's path.
    #[error("the library folder is not there: {0}")]
    Unavailable(String),
    #[error(transparent)]
    Archive(#[from] ArchiveError),
    #[error("the database failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl CatalogError {
    /// What to tell the person using the server. The cause goes to the log.
    pub fn for_people(&self) -> &str {
        match self {
            Self::NotFound => "There is no such library, series, group, book or reading list.",
            Self::NoSuchPage => "There is no such page.",
            Self::NoSuchRule => "There is no such rule.",
            Self::NoSuchPerson => "There is no such person.",
            Self::InvalidPage => "That page is not in this book.",
            Self::FileMissing(_) => {
                "This book's file is not where it was. Check that its drive is connected, \
                 or scan the library again."
            }
            Self::FileChanged(_) => {
                "This book's file has changed. Scan the library again to read it."
            }
            Self::InvalidPath(reason) => reason,
            Self::AlreadyAdded => "That folder is already a library.",
            Self::Invalid(reason) => reason,
            Self::NameTaken(_) => {
                "Another series in this library already has that name. Names that differ \
                 only in capitals, accents, spaces or punctuation count as the same."
            }
            Self::GroupExists(_) => "This series already has that group.",
            Self::ListNameTaken(_) => {
                "Another reading list already has that name. Names that differ only in \
                 capitals, accents, spaces or punctuation count as the same."
            }
            Self::NameOfAnother(_) => {
                "Someone else already goes by that name. Merge the two instead."
            }
            Self::NotEmpty => "It still holds books. Move them somewhere else first.",
            Self::Unavailable(_) => {
                "The library folder could not be read. Check that its drive is connected."
            }
            Self::Archive(error) => error.for_people(),
            Self::CoverNotKept(_) | Self::Database(_) | Self::Task(_) => crate::FAILED_FOR_PEOPLE,
        }
    }

    /// The id of the series, group, reading list or person that is in the
    /// way of what was asked.
    pub fn in_the_way(&self) -> Option<i64> {
        match self {
            Self::NameTaken(id)
            | Self::GroupExists(id)
            | Self::ListNameTaken(id)
            | Self::NameOfAnother(id) => Some(*id),
            _ => None,
        }
    }
}

/// Which part of a long list is asked for.
#[derive(Clone, Copy, Debug)]
pub struct Span {
    /// How many things to pass over, from the start of the list.
    pub offset: u32,
    /// How many things to give at most.
    pub limit: u32,
}

impl Span {
    /// How many things of a list are given when it is not said how many.
    pub const AT_ONCE: u32 = 100;

    /// The most things of a list given at once.
    pub const MOST_AT_ONCE: u32 = 500;

    /// The part someone asks for, who may not say how many things and may
    /// not have more than [`Self::MOST_AT_ONCE`].
    pub fn asked(offset: u32, limit: Option<u32>) -> Self {
        Self {
            offset,
            limit: limit.unwrap_or(Self::AT_ONCE).min(Self::MOST_AT_ONCE),
        }
    }
}

/// A part of a long list.
#[derive(Clone, Debug)]
pub struct PartOf<T> {
    pub items: Vec<T>,
    /// How many things the whole list holds.
    pub total: i64,
}

/// A file of a book the library holds more than once.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct HeldFile {
    #[sqlx(flatten)]
    pub book: Book,
    /// Where the file is, from the library's folder.
    pub path: String,
}

/// A folder of comics the server was pointed at.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Library {
    pub id: i64,
    pub name: String,
    pub root_path: String,
    /// Seconds since 1970, when the last scan finished.
    pub last_scanned_at: Option<i64>,
    pub series_count: i64,
    pub book_count: i64,
    /// How many of its files could not be read, not counting the ones set
    /// aside.
    pub unreadable_count: i64,
    /// How many of them were set aside.
    pub set_aside_count: i64,
}

/// A series of a library: the books that go by one name. It is Binge's own
/// record, not a folder; import decides which series a new book joins. It may
/// hold no books: nothing removes a series but being asked to.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Series {
    pub id: i64,
    pub library_id: i64,
    pub name: String,
    pub book_count: i64,
    /// How many of its books the reader it is listed for has not read to
    /// the end.
    pub unread_count: i64,
    /// Name of the series' cover: the one chosen for it, or else that of
    /// the first of its books, in [`reading_order`], that has one.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the series.
    pub cover_chosen: bool,
}

/// What a group of a series holds.
#[derive(Clone, Copy, Debug, PartialEq, Eq, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
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

/// A part of a series with its own page.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Group {
    pub id: i64,
    pub series_id: i64,
    pub kind: GroupKind,
    /// The number it is known by, when it has one: a volume's, or for
    /// annuals that of the volume they are of.
    pub number: Option<i64>,
    /// A name of its own.
    pub name: Option<String>,
    pub book_count: i64,
    /// How many of its books the reader it is listed for has not read to
    /// the end.
    pub unread_count: i64,
    /// Name of the cover of the book that stands for the group: the first
    /// of its books, in [`reading_order`], that has one.
    pub cover: Option<String>,
}

/// One comic file.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Book {
    pub id: i64,
    pub library_id: i64,
    /// The series the book is in. Missing for a file that could not be
    /// read: a place is kept by a book's identity, and it has none.
    pub series_id: Option<i64>,
    /// The name of that series, which a book is called by away from the
    /// series' own page.
    pub series_name: Option<String>,
    /// The group of the series the book is in. Missing for a book directly
    /// in its series.
    pub group_id: Option<i64>,
    /// What that group holds, which says how the book is called: an annual
    /// is "Annual #1", not "#1".
    pub group_kind: Option<GroupKind>,
    /// The issue's number, as its place has it: `12`, `0.5`.
    #[sqlx(rename = "place_number")]
    pub number: Option<String>,
    /// The date published, as its place has it, in the number
    /// [`crate::layout::date_written`] reads.
    pub published: Option<i64>,
    pub name: String,
    pub page_count: i64,
    pub file_size: i64,
    /// Seconds since 1970.
    pub added_at: i64,
    /// The book's identity. Missing when the file could not be read.
    pub content_hash: Option<String>,
    /// Why the file could not be read as a comic.
    pub problem: Option<String>,
    /// Name of the book's cover: the one chosen for it, or else its
    /// identity, which the cover made from its first page is kept under.
    /// Missing until a scan has made that one.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the book.
    pub cover_chosen: bool,
    /// How far the book has been read. Missing when it was never opened past
    /// its first page.
    #[sqlx(flatten, try_from = "ProgressColumns")]
    pub progress: Option<Progress>,
    /// What the book's file says about itself.
    #[sqlx(flatten)]
    pub info: ComicInfo,
}

impl Book {
    /// The year the book came out: the year of the date where it sits, or
    /// else the one its file gives. [`YEAR`] is the same for a statement.
    pub fn year(&self) -> Option<i64> {
        self.published.map(|date| date / 10000).or(self.info.year)
    }
}

/// The columns [`books`] takes from a book's row of `progress`: all there, or
/// all missing.
#[derive(sqlx::FromRow)]
struct ProgressColumns {
    progress_page: Option<i64>,
    progress_completed: Option<bool>,
    progress_updated_at: Option<i64>,
}

impl From<ProgressColumns> for Option<Progress> {
    fn from(row: ProgressColumns) -> Self {
        Some(Progress {
            page: row.progress_page?,
            completed: row.progress_completed?,
            updated_at: row.progress_updated_at?,
        })
    }
}

/// SQL that holds when the user may read the library whose id is the
/// expression `library`: the owner reads every library, anyone else the ones
/// they were given.
///
/// Every query that answers a person goes through this, so a library someone
/// may not read looks to them exactly like one that is not there. In all of
/// them the user is the first parameter, `?1`, and whatever else the query
/// takes is numbered from `?2`.
pub(crate) fn may_read(library: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM user u WHERE u.id = ?1 AND (u.is_owner OR EXISTS \
         (SELECT 1 FROM library_access a \
          WHERE a.user_id = u.id AND a.library_id = {library})))"
    )
}

/// SQL for the file that stands for a book as the user `?1` sees it: of the
/// files that hold the pages of the row `of` (it has a `content_hash`), the
/// first in a library they may read. Nothing when they may read none.
pub(crate) fn first_copy(of: &str) -> String {
    format!(
        "(SELECT MIN(same.id) FROM book same \
         WHERE same.content_hash = {of}.content_hash AND {})",
        may_read("same.library_id")
    )
}

/// SQL that holds for a file (`b`) that could not be read: it has no
/// identity.
const UNREADABLE: &str = "b.content_hash IS NULL";

/// Selects the libraries a user may read. Takes the user, then whatever
/// `rest` asks for, from `?2`.
fn libraries(rest: &str) -> String {
    format!(
        "SELECT l.id, l.name, l.root_path, l.last_scanned_at, \
         (SELECT COUNT(*) FROM series s WHERE s.library_id = l.id) AS series_count, \
         (SELECT COUNT(*) FROM book b WHERE b.library_id = l.id) AS book_count, \
         (SELECT COUNT(*) FROM book b WHERE b.library_id = l.id AND {UNREADABLE} \
          AND b.set_aside_at IS NULL) AS unreadable_count, \
         (SELECT COUNT(*) FROM book b WHERE b.library_id = l.id AND {UNREADABLE} \
          AND b.set_aside_at IS NOT NULL) AS set_aside_count \
         FROM library l WHERE {} {rest}",
        may_read("l.id")
    )
}

/// The places (`bp`) of a library's books, each with the files (`b`) that
/// hold that book now. A place is kept by the book's identity, so it has no
/// file when the book is gone, and two when the library holds it twice.
pub(crate) const PLACED: &str = "book_place bp JOIN book b \
     ON b.library_id = bp.library_id AND b.content_hash = bp.content_hash";

/// Joins a file (`b`) with its place as `bp`, the series of that place as
/// `bs` and its group as `g`, which is what [`reading_order`] orders by. A
/// book has one place and one group at most, so the joins add no rows.
pub(crate) const PLACE_OF: &str = "LEFT JOIN book_place bp \
     ON bp.library_id = b.library_id AND bp.content_hash = b.content_hash \
     LEFT JOIN series bs ON bs.id = bp.series_id \
     LEFT JOIN series_group g ON g.id = bp.group_id";

/// The year a book (`bp`, with what its file says as `i`) came out: the
/// year of the date where it sits, or else the one its file gives. See
/// [`Book::year`].
pub(crate) const YEAR: &str = "COALESCE(bp.published / 10000, i.year)";

/// SQL that holds for a book (`b`) the user has not read to the end.
pub(crate) const UNREAD: &str = "NOT EXISTS (SELECT 1 FROM progress p \
     WHERE p.user_id = ?1 AND p.content_hash = b.content_hash AND p.completed)";

/// Orders groups (`g`, missing for books directly in their series): volumes
/// by number, then limited series, then the books in no group, then the
/// annuals, the collected editions, the one-shots, and the appearances in
/// other titles last.
const GROUP_ORDER: &str = "CASE COALESCE(g.kind, '') WHEN 'volume' THEN 0 WHEN 'limited' THEN 1 \
     WHEN '' THEN 2 WHEN 'annual' THEN 3 WHEN 'collected' THEN 4 WHEN 'one_shot' THEN 5 \
     ELSE 6 END, \
     COALESCE(g.number, 9999999), COALESCE(g.id, 0)";

/// Orders a series' books (`b`, each with its place as `bp` and its group as
/// `g`) the way they are read: group by group, and inside a group as
/// [`ORDER_IN_GROUP`] says.
///
/// The first book in this order stands for its series: it is the first one
/// published, as far as the places say.
pub(crate) fn reading_order() -> String {
    format!("{GROUP_ORDER}, {ORDER_IN_GROUP}")
}

/// Orders the books (`b`, each with its place as `bp`) of one group, the
/// same way whatever the group's kind: by issue number, then the books
/// without one by the date they were published, the undated ones last. File
/// name order settles the rest.
const ORDER_IN_GROUP: &str =
    "bp.number COLLATE natsort NULLS LAST, bp.published NULLS LAST, b.name COLLATE natsort";

/// Joins the cover chosen for a book `b`, as `bc`.
pub(crate) const CHOSEN: &str = "LEFT JOIN book_cover bc ON bc.content_hash = b.content_hash";

/// The name of the cover of a book `b`: the one chosen for it, or else the
/// one made from its first page. Goes with [`CHOSEN`].
pub(crate) const COVER: &str = "COALESCE(bc.cover, CASE WHEN b.has_cover THEN b.content_hash END)";

/// Selects the series a user may read, the ones that hold no books
/// included. Takes the user, then whatever `rest` asks for, from `?2`.
pub(crate) fn series(rest: &str) -> String {
    format!(
        "SELECT s.id, s.library_id, s.name, \
         (SELECT COUNT(*) FROM {PLACED} WHERE bp.series_id = s.id) AS book_count, \
         (SELECT COUNT(*) FROM {PLACED} WHERE bp.series_id = s.id AND {UNREAD}) \
          AS unread_count, \
         COALESCE(sc.cover, \
          (SELECT {COVER} FROM {PLACED} {CHOSEN} LEFT JOIN series_group g ON g.id = bp.group_id \
           WHERE bp.series_id = s.id AND {COVER} IS NOT NULL \
           ORDER BY {} LIMIT 1)) AS cover, \
         sc.cover IS NOT NULL AS cover_chosen \
         FROM series s LEFT JOIN series_cover sc ON sc.series_id = s.id WHERE {} {rest}",
        reading_order(),
        may_read("s.library_id")
    )
}

/// Selects the groups of the series a user may read, the ones that hold no
/// books included. Takes the user, then whatever `rest` asks for, from `?2`.
fn groups(rest: &str) -> String {
    format!(
        "SELECT g.id, g.series_id, g.kind, g.number, g.name, \
         (SELECT COUNT(*) FROM {PLACED} WHERE bp.group_id = g.id) AS book_count, \
         (SELECT COUNT(*) FROM {PLACED} WHERE bp.group_id = g.id AND {UNREAD}) \
          AS unread_count, \
         (SELECT {COVER} FROM {PLACED} {CHOSEN} \
          WHERE bp.group_id = g.id AND {COVER} IS NOT NULL \
          ORDER BY {ORDER_IN_GROUP} LIMIT 1) AS cover \
         FROM series_group g JOIN series s ON s.id = g.series_id \
         WHERE {} {rest}",
        may_read("s.library_id")
    )
}

/// Selects the books a user may read, each with what its file says about
/// itself and with that user's progress, which hangs on the book's identity.
/// Takes the user, then whatever `rest` asks for, from `?2`.
///
/// Each book comes with its place as `bp`, the series of that place as `bs`
/// and its group as `g`, for `rest` to ask for a series' books and to order
/// them by [`reading_order`]. A book has one place and one group at most, so
/// the joins add no rows. A file that could not be read has none of them.
pub(crate) fn books(rest: &str) -> String {
    format!(
        "SELECT b.id, b.library_id, bp.series_id, bs.name AS series_name, bp.group_id, \
         g.kind AS group_kind, bp.number AS place_number, bp.published, b.name, b.page_count, \
         b.file_size, b.added_at, b.content_hash, b.problem, \
         {COVER} AS cover, bc.cover IS NOT NULL AS cover_chosen, \
         p.page AS progress_page, p.completed AS progress_completed, \
         p.updated_at AS progress_updated_at, \
         i.title, i.series, i.number, i.volume, i.year, i.month, i.day, \
         i.writer, i.penciller, i.publisher, i.summary, i.language, i.format \
         FROM book b LEFT JOIN progress p \
         ON p.content_hash = b.content_hash AND p.user_id = ?1 \
         LEFT JOIN book_info i ON i.book_id = b.id {CHOSEN} {PLACE_OF} \
         WHERE {} {rest}",
        may_read("b.library_id")
    )
}

/// The catalog of libraries, with the scanner that fills it.
///
/// Cheap to clone; clones share the database and the state of running scans.
#[derive(Clone)]
pub struct Catalog {
    pub(crate) db: Database,
    pub(crate) thumbnails: Thumbnails,
    /// The covers people chose: see [`Catalog::choose_cover`].
    pub(crate) chosen_covers: Thumbnails,
    /// Held while a chosen cover changes, so that one change at a time
    /// decides which files are still shown.
    pub(crate) changing_covers: Arc<tokio::sync::Mutex<()>>,
    pub(crate) scans: Arc<Mutex<HashMap<i64, Scan>>>,
    /// Limits how many books are opened for covers at once, over all scans.
    pub(crate) analysis: Arc<Semaphore>,
}

impl Catalog {
    /// `data_dir` is where covers are kept.
    pub fn new(db: Database, data_dir: &Path) -> Self {
        Self {
            db,
            thumbnails: Thumbnails::new(data_dir),
            chosen_covers: Thumbnails::chosen(data_dir),
            changing_covers: Arc::default(),
            scans: Arc::default(),
            analysis: Arc::new(Semaphore::new(analysis_workers())),
        }
    }

    /// The libraries `user` may read, in name order.
    pub async fn libraries(&self, user: UserId) -> Result<Vec<Library>, CatalogError> {
        let query = libraries("ORDER BY l.name COLLATE natsort");
        Ok(sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?)
    }

    pub async fn library(&self, user: UserId, id: i64) -> Result<Library, CatalogError> {
        let query = libraries("AND l.id = ?2");
        sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)
    }

    /// Adds the folder at `path` as a library and starts scanning it. Without
    /// a name, the library is called after the folder. `owner` is who adds
    /// it, and gets it back as they see it.
    pub async fn add_library(
        &self,
        owner: UserId,
        path: &Path,
        name: Option<&str>,
    ) -> Result<Library, CatalogError> {
        let invalid = |message: &str| CatalogError::InvalidPath(message.to_owned());
        if !path.is_absolute() {
            return Err(invalid(
                "Give the full path of the folder, starting from the root.",
            ));
        }
        let root = tokio::fs::canonicalize(path)
            .await
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::NotFound => invalid("There is no folder at that path."),
                std::io::ErrorKind::PermissionDenied => {
                    invalid("The server is not allowed to read that folder.")
                }
                _ => CatalogError::InvalidPath(format!("That folder cannot be read: {error}.")),
            })?;
        let is_folder = tokio::fs::metadata(&root)
            .await
            .is_ok_and(|found| found.is_dir());
        if !is_folder {
            return Err(invalid("That path is a file, not a folder."));
        }
        let Some(root_path) = root.to_str() else {
            return Err(invalid("That path has characters the server cannot store."));
        };
        let name = name
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .or_else(|| root.file_name().and_then(|folder| folder.to_str()))
            .unwrap_or(root_path);

        let added = sqlx::query_scalar::<_, i64>(
            "INSERT INTO library (name, root_path, created_at) VALUES (?, ?, ?) RETURNING id",
        )
        .bind(name)
        .bind(root_path)
        .bind(now())
        .fetch_one(self.db.pool())
        .await;
        let id = match added {
            Ok(id) => id,
            Err(sqlx::Error::Database(error)) if error.is_unique_violation() => {
                return Err(CatalogError::AlreadyAdded);
            }
            Err(error) => return Err(error.into()),
        };

        self.start_scan(id);
        self.library(owner, id).await
    }

    /// Forgets a library: its series, its books and their places. The folder
    /// and its files are not touched. What hangs on a book's identity stays,
    /// for when the book is seen again: its cover and everyone's progress.
    pub async fn remove_library(&self, id: i64) -> Result<(), CatalogError> {
        self.cancel_scan(id);
        let removed = sqlx::query("DELETE FROM library WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        if removed.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// The series of a library, in name order.
    pub async fn series_in(
        &self,
        user: UserId,
        library_id: i64,
    ) -> Result<Vec<Series>, CatalogError> {
        let query = series("AND s.library_id = ?2 ORDER BY s.name COLLATE natsort");
        let series: Vec<Series> = sqlx::query_as(&query)
            .bind(user)
            .bind(library_id)
            .fetch_all(self.db.pool())
            .await?;
        // Only an empty answer leaves open whether the library is there.
        if series.is_empty() {
            self.library(user, library_id).await?;
        }
        Ok(series)
    }

    /// The files of a library that could not be read, in the order of their
    /// paths: the ones still to look at, or with `set_aside` the ones that
    /// were set aside. They are in no series: a book's place is kept by its
    /// identity, and these have none.
    pub async fn unreadable_in(
        &self,
        user: UserId,
        library_id: i64,
        set_aside: bool,
    ) -> Result<Vec<Book>, CatalogError> {
        let query = books(&format!(
            "AND b.library_id = ?2 AND {UNREADABLE} AND (b.set_aside_at IS NOT NULL) = ?3 \
             ORDER BY b.path COLLATE natsort"
        ));
        let found: Vec<Book> = sqlx::query_as(&query)
            .bind(user)
            .bind(library_id)
            .bind(set_aside)
            .fetch_all(self.db.pool())
            .await?;
        if found.is_empty() {
            self.library(user, library_id).await?;
        }
        Ok(found)
    }

    /// Sets a file that could not be read aside, as seen and not to be
    /// listed with the others, or takes it back from there. A scan that
    /// reads the file again and still cannot lists it again.
    pub async fn set_aside(&self, book_id: i64, aside: bool) -> Result<(), CatalogError> {
        let set = sqlx::query(&format!(
            "UPDATE book AS b SET set_aside_at = CASE WHEN ?2 THEN COALESCE(set_aside_at, ?3) END \
             WHERE b.id = ?1 AND {UNREADABLE}"
        ))
        .bind(book_id)
        .bind(aside)
        .bind(now())
        .execute(self.db.pool())
        .await?;
        if set.rows_affected() == 0 {
            // Only a file that could not be read is set aside.
            let there: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM book WHERE id = ?)")
                .bind(book_id)
                .fetch_one(self.db.pool())
                .await?;
            return Err(if there {
                CatalogError::Invalid("This book can be read, so there is nothing to set aside.")
            } else {
                CatalogError::NotFound
            });
        }
        Ok(())
    }

    /// The books a library holds more than once: the files of each, one
    /// book after another by name. Two files are one book when their pages
    /// are the same, whatever their names and tags.
    pub async fn held_twice_in(
        &self,
        user: UserId,
        library_id: i64,
    ) -> Result<Vec<Vec<HeldFile>>, CatalogError> {
        self.library(user, library_id).await?;
        let twice = books(
            "AND b.library_id = ?2 AND b.content_hash IN \
             (SELECT content_hash FROM book WHERE library_id = ?2 \
              GROUP BY content_hash HAVING COUNT(*) > 1)",
        );
        let query = format!(
            "SELECT held.*, f.path FROM ({twice}) held JOIN book f ON f.id = held.id \
             ORDER BY held.content_hash, f.path COLLATE natsort"
        );
        let copies: Vec<HeldFile> = sqlx::query_as(&query)
            .bind(user)
            .bind(library_id)
            .fetch_all(self.db.pool())
            .await?;
        let mut held: Vec<Vec<HeldFile>> = Vec::new();
        for copy in copies {
            match held.last_mut() {
                Some(files) if files[0].book.content_hash == copy.book.content_hash => {
                    files.push(copy);
                }
                _ => held.push(vec![copy]),
            }
        }
        held.sort_by(|a, b| crate::natural::natural_cmp(&a[0].book.name, &b[0].book.name));
        Ok(held)
    }

    pub async fn series(&self, user: UserId, id: i64) -> Result<Series, CatalogError> {
        let query = series("AND s.id = ?2");
        sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)
    }

    /// The groups of a series, in the order they are read.
    pub async fn groups_in(
        &self,
        user: UserId,
        series_id: i64,
    ) -> Result<Vec<Group>, CatalogError> {
        let query = groups(&format!("AND g.series_id = ?2 ORDER BY {GROUP_ORDER}"));
        Ok(sqlx::query_as(&query)
            .bind(user)
            .bind(series_id)
            .fetch_all(self.db.pool())
            .await?)
    }

    pub async fn group(&self, user: UserId, id: i64) -> Result<Group, CatalogError> {
        let query = groups("AND g.id = ?2");
        sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)
    }

    /// The books of a series, in [`reading_order`].
    pub async fn books_in(&self, user: UserId, series_id: i64) -> Result<Vec<Book>, CatalogError> {
        let query = books(&format!(
            "AND bp.series_id = ?2 ORDER BY {}",
            reading_order()
        ));
        let found: Vec<Book> = sqlx::query_as(&query)
            .bind(user)
            .bind(series_id)
            .fetch_all(self.db.pool())
            .await?;
        if found.is_empty() {
            self.series(user, series_id).await?;
        }
        Ok(found)
    }

    /// The books of a group, in the order they are read.
    pub async fn books_in_group(
        &self,
        user: UserId,
        group_id: i64,
    ) -> Result<Vec<Book>, CatalogError> {
        let query = books(&format!("AND bp.group_id = ?2 ORDER BY {ORDER_IN_GROUP}"));
        let found: Vec<Book> = sqlx::query_as(&query)
            .bind(user)
            .bind(group_id)
            .fetch_all(self.db.pool())
            .await?;
        if found.is_empty() {
            self.group(user, group_id).await?;
        }
        Ok(found)
    }

    /// The file holding the cover of this name, if the name is well formed
    /// and something `user` may see shows it: a book whose identity it is,
    /// or a book, a series or a reading list it was chosen for. The file
    /// may not exist yet.
    pub async fn cover_path(
        &self,
        user: UserId,
        cover: &str,
    ) -> Result<Option<PathBuf>, CatalogError> {
        let (Some(made), Some(chosen)) =
            (self.thumbnails.path(cover), self.chosen_covers.path(cover))
        else {
            return Ok(None);
        };
        // Says whether the cover is a chosen one, for whoever may see it.
        let book = may_read("b.library_id");
        let query = format!(
            "SELECT 0 FROM book b WHERE b.content_hash = ?2 AND {book} \
             UNION ALL \
             SELECT 1 FROM book_cover c JOIN book b ON b.content_hash = c.content_hash \
             WHERE c.cover = ?2 AND {book} \
             UNION ALL \
             SELECT 1 FROM series_cover c JOIN series s ON s.id = c.series_id \
             WHERE c.cover = ?2 AND {} \
             UNION ALL \
             SELECT 1 FROM reading_list_cover c WHERE c.cover = ?2 AND {} LIMIT 1",
            may_read("s.library_id"),
            crate::lists::may_see("c.list_id")
        );
        let shown: Option<bool> = sqlx::query_scalar(&query)
            .bind(user)
            .bind(cover)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(shown.map(|was_chosen| if was_chosen { chosen } else { made }))
    }
}

/// Seconds since 1970.
pub(crate) fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts;
    use crate::fixtures;

    /// A catalog with nothing in it, and its owner.
    async fn catalog(dir: &Path) -> (Catalog, UserId) {
        let data = dir.join("data");
        std::fs::create_dir_all(&data).unwrap();
        let db = Database::open(&data.join("binge.db")).await.unwrap();
        let owner = accounts::testing::owner(&db).await;
        (Catalog::new(db, &data), owner)
    }

    #[tokio::test]
    async fn a_library_needs_a_real_folder_given_in_full() {
        let dir = tempfile::tempdir().unwrap();
        let (catalog, owner) = catalog(dir.path()).await;
        let comics = dir.path().join("Comics");
        fixtures::write_book(&comics.join("One.cbz"), 1, 1);

        for bad in [
            Path::new("Comics"),
            &dir.path().join("missing"),
            &comics.join("One.cbz"),
        ] {
            let refused = catalog.add_library(owner, bad, None).await;
            assert!(
                matches!(refused, Err(CatalogError::InvalidPath(_))),
                "{bad:?}"
            );
        }
        assert!(catalog.libraries(owner).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_library_is_named_after_its_folder_unless_told_otherwise() {
        let dir = tempfile::tempdir().unwrap();
        let (catalog, owner) = catalog(dir.path()).await;
        let (comics, manga) = (dir.path().join("Comics"), dir.path().join("Manga"));
        std::fs::create_dir_all(&comics).unwrap();
        std::fs::create_dir_all(&manga).unwrap();

        let first = catalog
            .add_library(owner, &comics, Some("  "))
            .await
            .unwrap();
        assert_eq!(first.name, "Comics");
        let second = catalog
            .add_library(owner, &manga, Some(" Tankobon "))
            .await
            .unwrap();
        assert_eq!(second.name, "Tankobon");

        let again = catalog.add_library(owner, &comics, None).await;
        assert!(matches!(again, Err(CatalogError::AlreadyAdded)));
        let names: Vec<_> = catalog
            .libraries(owner)
            .await
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(names, ["Comics", "Tankobon"]);
    }

    #[tokio::test]
    async fn unknown_ids_are_not_found() {
        let dir = tempfile::tempdir().unwrap();
        let (catalog, owner) = catalog(dir.path()).await;
        assert!(matches!(
            catalog.library(owner, 7).await,
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            catalog.series_in(owner, 7).await,
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            catalog.series(owner, 7).await,
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            catalog.books_in(owner, 7).await,
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            catalog.remove_library(7).await,
            Err(CatalogError::NotFound)
        ));
    }
}
