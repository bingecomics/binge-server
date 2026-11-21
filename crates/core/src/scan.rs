//! Scanning: bringing the catalog in line with what is in a library folder.
//!
//! A scan has two passes. The first walks the folder and reads each new or
//! changed archive's index, which is quick, so series and books appear at
//! once. A book the library has not seen before is given its place then, by
//! [`crate::import`]; one it has seen goes back to the place it had. The
//! second pass opens the books to read page sizes and make covers, and runs
//! a few books at a time in the background.
//!
//! A scan only ever reads the library folder.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, MutexGuard, PoisonError};
use std::time::UNIX_EPOCH;

use sqlx::{QueryBuilder, Sqlite, SqliteConnection};
use tokio::task::JoinSet;
use walkdir::WalkDir;

use crate::archive::{self, Archive, ArchiveError, Contents};
use crate::catalog::{Catalog, CatalogError, GroupKind, now};
use crate::changes::{Arrival, Left};
use crate::comicinfo::ComicInfo;
use crate::from_files::{self, Waiting};
use crate::import;
use crate::layout;
use crate::natural::natural_cmp;
use crate::people;
use crate::search;
use crate::thumbnail::Thumbnails;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScanPhase {
    /// No scan is running.
    #[default]
    Idle,
    /// Walking the folder and listing books.
    Finding,
    /// Reading page sizes and making covers.
    Analyzing,
}

/// Where a library's scan stands. After a scan ends, the numbers of that scan
/// stay until the next one starts.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ScanStatus {
    pub phase: ScanPhase,
    /// Comic files seen in the folder so far.
    pub found: u32,
    /// Books the background pass has to open in this scan.
    pub to_analyze: u32,
    /// Books the background pass is done with.
    pub analyzed: u32,
    /// What went wrong in the last scan, in words for the owner.
    pub problem: Option<String>,
}

/// A library's scan, running or finished.
pub(crate) struct Scan {
    status: ScanStatus,
    /// Set when the library is removed, to stop the scan early.
    cancelled: Arc<AtomicBool>,
}

/// A comic file found on disk.
struct Found {
    /// Path relative to the library root, with `/` between parts.
    path: String,
    /// The part of `path` before the file name; empty for the root.
    folder: String,
    /// File name without its extension.
    name: String,
    file_size: i64,
    /// Milliseconds since 1970.
    modified_at: i64,
}

struct Walk {
    files: Vec<Found>,
    /// False when part of the folder could not be read. Books are only
    /// removed from the catalog after a complete walk.
    complete: bool,
}

/// Archive indexes read at once in the first pass. Reading one is mostly
/// waiting for the disk or the network.
const INDEX_READERS: usize = 4;

/// Rows sent to the database in one statement, well inside its limit on
/// values per statement.
pub(crate) const ROWS_PER_STATEMENT: usize = 500;

/// What the catalog already holds about a file.
#[derive(sqlx::FromRow)]
struct Known {
    id: i64,
    path: String,
    file_size: i64,
    modified_at: i64,
    content_hash: Option<String>,
}

/// Gives the book in `file` its place in the library, unless it has one.
/// Says whether it had one.
///
/// A place is kept by the book's identity. A book the library has seen
/// before sits where it did: moving, renaming or retagging its file changes
/// nothing. A new one is placed by [`import::place`], which makes its series
/// and its group when they are not there yet.
async fn keep_place(
    tx: &mut SqliteConnection,
    library_id: i64,
    file: &Found,
    info: &ComicInfo,
    identity: &str,
) -> Result<bool, sqlx::Error> {
    let kept: bool = sqlx::query_scalar(
        "SELECT EXISTS \
         (SELECT 1 FROM book_place WHERE library_id = ? AND content_hash = ?)",
    )
    .bind(library_id)
    .bind(identity)
    .fetch_one(&mut *tx)
    .await?;
    if kept {
        return Ok(true);
    }

    let placed = import::place(&file.name, &file.folder, info);
    let series_id = series_named(tx, library_id, &placed.series).await?;
    let group_id = match placed.group {
        Some((kind, number)) => Some(group_of(tx, series_id, kind, number).await?),
        None => None,
    };
    sqlx::query(
        "INSERT INTO book_place (library_id, content_hash, series_id, group_id, number, \
         published) VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(library_id)
    .bind(identity)
    .bind(series_id)
    .bind(group_id)
    .bind(&placed.number)
    .bind(placed.published)
    .execute(&mut *tx)
    .await?;
    Ok(false)
}

/// The series a library knows by `name`, made when it is not there yet.
async fn series_named(
    tx: &mut SqliteConnection,
    library_id: i64,
    name: &str,
) -> Result<i64, sqlx::Error> {
    let name_key = import::name_key(name);
    // Looked up before it is added: an insert that finds the series already
    // there would still use up an id.
    match layout::series_with_key(tx, library_id, &name_key).await? {
        Some(id) => Ok(id),
        None => layout::add_series(tx, library_id, name, &name_key).await,
    }
}

/// The group of a series that import means by that kind and number, made
/// when it is not there yet. A numbered group is the one with that number
/// whatever name the owner has given it since, the unnamed one first. With
/// no number it is the group with no name: one the owner named is a part of
/// its kind that import does not fill.
async fn group_of(
    tx: &mut SqliteConnection,
    series_id: i64,
    kind: GroupKind,
    number: Option<i64>,
) -> Result<i64, sqlx::Error> {
    let found: Option<i64> = sqlx::query_scalar(
        "SELECT id FROM series_group WHERE series_id = ?1 AND kind = ?2 AND number IS ?3 \
         AND (name IS NULL OR ?3 IS NOT NULL) ORDER BY name IS NOT NULL, id LIMIT 1",
    )
    .bind(series_id)
    .bind(kind)
    .bind(number)
    .fetch_optional(&mut *tx)
    .await?;
    match found {
        Some(id) => Ok(id),
        None => layout::add_group(tx, series_id, kind, number, None).await,
    }
}

impl Catalog {
    /// Where a library's scan stands.
    pub fn scan_status(&self, library_id: i64) -> ScanStatus {
        self.scans()
            .get(&library_id)
            .map(|scan| scan.status.clone())
            .unwrap_or_default()
    }

    /// Starts scanning a library in the background. Does nothing when a scan
    /// of it is already running.
    pub fn start_scan(&self, library_id: i64) {
        if let Some(cancelled) = self.begin_scan(library_id) {
            let catalog = self.clone();
            tokio::spawn(async move { catalog.scan_to_end(library_id, &cancelled).await });
        }
    }

    /// Starts scanning every library, as the server does when it starts.
    pub async fn start_scanning_all(&self) -> Result<(), CatalogError> {
        let libraries: Vec<i64> = sqlx::query_scalar("SELECT id FROM library")
            .fetch_all(self.db.pool())
            .await?;
        for library_id in libraries {
            self.start_scan(library_id);
        }
        Ok(())
    }

    fn scans(&self) -> MutexGuard<'_, HashMap<i64, Scan>> {
        self.scans.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Does nothing once the library has been removed.
    fn update_scan(&self, library_id: i64, change: impl FnOnce(&mut ScanStatus)) {
        if let Some(scan) = self.scans().get_mut(&library_id) {
            change(&mut scan.status);
        }
    }

    /// Marks a library as being scanned. `None` when it already is.
    fn begin_scan(&self, library_id: i64) -> Option<Arc<AtomicBool>> {
        let mut scans = self.scans();
        let running = scans
            .get(&library_id)
            .is_some_and(|scan| scan.status.phase != ScanPhase::Idle);
        if running {
            return None;
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        let status = ScanStatus {
            phase: ScanPhase::Finding,
            ..ScanStatus::default()
        };
        scans.insert(
            library_id,
            Scan {
                status,
                cancelled: cancelled.clone(),
            },
        );
        Some(cancelled)
    }

    /// Forgets a library's scan and tells it to stop if it is running,
    /// because the library is going away.
    pub(crate) fn cancel_scan(&self, library_id: i64) {
        if let Some(scan) = self.scans().remove(&library_id) {
            scan.cancelled.store(true, Ordering::Relaxed);
        }
    }

    /// Runs a scan and records how it ended in its status.
    async fn scan_to_end(&self, library_id: i64, cancelled: &AtomicBool) {
        let problem = match self.run_scan(library_id, cancelled).await {
            Ok(note) => note,
            Err(error) => {
                tracing::warn!(library_id, %error, "scan failed");
                Some(error.for_people().to_owned())
            }
        };
        self.update_scan(library_id, |status| {
            status.phase = ScanPhase::Idle;
            status.problem = problem;
        });
    }

    /// Runs both passes. Returns a note for the owner when the scan worked
    /// but could not see everything.
    async fn run_scan(
        &self,
        library_id: i64,
        cancelled: &AtomicBool,
    ) -> Result<Option<String>, CatalogError> {
        let root_path: String = sqlx::query_scalar("SELECT root_path FROM library WHERE id = ?")
            .bind(library_id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)?;
        let root = PathBuf::from(&root_path);
        let started_at = now();

        let walk = {
            let catalog = self.clone();
            let root = root.clone();
            tokio::task::spawn_blocking(move || {
                walk(&root, |found| {
                    catalog.update_scan(library_id, |status| status.found = found)
                })
            })
            .await?
            .ok_or_else(|| CatalogError::Unavailable(root_path.clone()))?
        };

        let known: HashMap<String, Known> = sqlx::query_as::<_, Known>(
            "SELECT id, path, file_size, modified_at, content_hash FROM book WHERE library_id = ?",
        )
        .bind(library_id)
        .fetch_all(self.db.pool())
        .await?
        .into_iter()
        .map(|book| (book.path.clone(), book))
        .collect();

        // New files, and files whose size or time is not what it was.
        let to_read: Vec<(&Found, Option<&Known>)> = walk
            .files
            .iter()
            .map(|file| (file, known.get(&file.path)))
            .filter(|(file, existing)| {
                existing.is_none_or(|book| {
                    book.file_size != file.file_size || book.modified_at != file.modified_at
                })
            })
            .collect();
        // The files at paths the library did not know, for the record of
        // what this scan changed.
        let mut arrivals = Vec::new();
        // Indexes are read a few at a time; the results are stored in order.
        for batch in to_read.chunks(INDEX_READERS) {
            if cancelled.load(Ordering::Relaxed) {
                return Ok(None);
            }
            let reads: Vec<_> = batch
                .iter()
                .map(|(file, _)| {
                    let path = root.join(&file.path);
                    tokio::task::spawn_blocking(move || archive::read_book(&path))
                })
                .collect();
            for (&(file, existing), read) in batch.iter().zip(reads) {
                let contents = read.await?;
                if let Err(error) = &contents {
                    tracing::warn!(file = %file.path, %error, "not readable as a comic");
                }
                let (identity, held_before) = self
                    .store_book(library_id, file, existing, contents)
                    .await?;
                if existing.is_none() {
                    arrivals.push(Arrival {
                        path: &file.path,
                        identity,
                        held_before,
                    });
                }
            }
        }

        // By their paths whatever order the folder gave them in, so that of
        // two files with the same pages the same one counts as moved.
        arrivals.sort_by(|a, b| natural_cmp(a.path, b.path));
        // Books are only taken for gone when every folder could be read.
        let on_disk: HashSet<&str> = walk.files.iter().map(|file| file.path.as_str()).collect();
        let (here, mut gone): (Vec<&Known>, Vec<&Known>) = known
            .values()
            .partition(|book| !walk.complete || on_disk.contains(book.path.as_str()));
        gone.sort_by(|a, b| natural_cmp(&a.path, &b.path));
        for chunk in gone.chunks(ROWS_PER_STATEMENT) {
            let mut delete = QueryBuilder::<Sqlite>::new("DELETE FROM book WHERE id IN (");
            let mut ids = delete.separated(", ");
            for book in chunk {
                ids.push_bind(book.id);
            }
            delete.push(")").build().execute(self.db.pool()).await?;
        }
        let left: Vec<Left> = gone
            .iter()
            .map(|book| Left {
                path: &book.path,
                identity: book.content_hash.as_deref(),
            })
            .collect();
        let here = here
            .iter()
            .filter_map(|book| book.content_hash.as_deref())
            .collect();
        // The record is for looking back on: a scan goes on without it.
        if let Err(error) = self
            .record_scan(library_id, started_at, &arrivals, &left, here)
            .await
        {
            tracing::warn!(library_id, %error, "could not record what the scan changed");
        }
        // Lists and collections are there to read by: a scan goes on
        // without them. What it could not put in them waits for the next.
        let made = match self.made_from_files(library_id).await {
            Ok(made) => self.fill_from_files(library_id, Waiting::Only, made).await,
            Err(error) => Err(error.into()),
        };
        if let Err(error) = made {
            tracing::warn!(library_id, %error, "could not make lists and collections from the files");
        }
        self.analyze(library_id, &root, cancelled).await?;

        sqlx::query("UPDATE library SET last_scanned_at = ? WHERE id = ?")
            .bind(now())
            .bind(library_id)
            .execute(self.db.pool())
            .await?;

        Ok((!walk.complete).then(|| {
            "Some folders could not be read, so books missing from them were kept.".to_owned()
        }))
    }

    /// Writes what the first pass learned about one file: the book, its
    /// pages, what the file says about itself, and its place when it is new
    /// to the library. A file that could not be read has no identity to keep
    /// a place by, so it gets none.
    ///
    /// Gives back the book's identity and whether the library had a place
    /// for it already.
    async fn store_book(
        &self,
        library_id: i64,
        file: &Found,
        existing: Option<&Known>,
        contents: Result<(Contents, ComicInfo), ArchiveError>,
    ) -> Result<(Option<String>, bool), CatalogError> {
        let (pages, content_hash, info, problem) = match contents {
            Ok((contents, info)) => (contents.pages, Some(contents.content_hash), info, None),
            Err(error) => (
                Vec::new(),
                None,
                ComicInfo::default(),
                Some(error.for_people()),
            ),
        };

        // A file that changed around the same pages, as when it is retagged,
        // keeps what the background pass already made of them.
        let same_pages = content_hash.is_some()
            && existing.is_some_and(|book| book.content_hash == content_hash);

        let mut tx = self.db.begin_write().await?;
        let held_before = match &content_hash {
            Some(identity) => keep_place(&mut tx, library_id, file, &info, identity).await?,
            None => false,
        };

        let search_key = search::book_key(&file.name, &info);

        let book_id: i64 = sqlx::query_scalar(
            "INSERT INTO book (library_id, name, path, file_size, modified_at, \
             page_count, content_hash, problem, added_at, search_key) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?11) \
             ON CONFLICT (library_id, path) DO UPDATE SET \
             name = excluded.name, \
             file_size = excluded.file_size, modified_at = excluded.modified_at, \
             page_count = excluded.page_count, content_hash = excluded.content_hash, \
             problem = excluded.problem, search_key = excluded.search_key, \
             set_aside_at = NULL, \
             has_cover = CASE WHEN ?10 THEN has_cover ELSE 0 END, \
             analyzed_at = CASE WHEN ?10 THEN analyzed_at END \
             RETURNING id",
        )
        .bind(library_id)
        .bind(&file.name)
        .bind(&file.path)
        .bind(file.file_size)
        .bind(file.modified_at)
        .bind(pages.len() as i64)
        .bind(&content_hash)
        .bind(problem)
        .bind(now())
        .bind(same_pages)
        .bind(&search_key)
        .fetch_one(&mut *tx)
        .await?;

        sqlx::query("DELETE FROM book_info WHERE book_id = ?")
            .bind(book_id)
            .execute(&mut *tx)
            .await?;
        if !info.is_empty() {
            sqlx::query(
                "INSERT INTO book_info (book_id, title, series, number, volume, year, month, \
                 day, writer, penciller, publisher, summary, language, format) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(book_id)
            .bind(&info.title)
            .bind(&info.series)
            .bind(&info.number)
            .bind(info.volume)
            .bind(info.year)
            .bind(info.month)
            .bind(info.day)
            .bind(&info.writer)
            .bind(&info.penciller)
            .bind(&info.publisher)
            .bind(&info.summary)
            .bind(&info.language)
            .bind(&info.format)
            .execute(&mut *tx)
            .await?;
        }
        people::credit(&mut tx, book_id, &info).await?;
        from_files::note(&mut tx, book_id, &info).await?;

        if !same_pages {
            sqlx::query("DELETE FROM page WHERE book_id = ?")
                .bind(book_id)
                .execute(&mut *tx)
                .await?;
        }
        let numbered: Vec<_> = pages.iter().enumerate().collect();
        for chunk in numbered.chunks(ROWS_PER_STATEMENT) {
            let mut insert = QueryBuilder::<Sqlite>::new(
                "INSERT INTO page (book_id, number, entry, byte_size) ",
            );
            insert.push_values(chunk, |mut row, (number, page)| {
                row.push_bind(book_id)
                    .push_bind(*number as i64)
                    .push_bind(&page.name)
                    .push_bind(page.byte_size as i64);
            });
            // Same pages may still have new names inside the archive.
            insert.push(
                " ON CONFLICT (book_id, number) DO UPDATE SET \
                 entry = excluded.entry, byte_size = excluded.byte_size",
            );
            insert.build().execute(&mut *tx).await?;
        }
        tx.commit().await?;
        Ok((content_hash, held_before))
    }

    /// The background pass: page sizes and covers for every book that does
    /// not have them yet, a few books at a time.
    async fn analyze(
        &self,
        library_id: i64,
        root: &Path,
        cancelled: &AtomicBool,
    ) -> Result<(), CatalogError> {
        let pending: Vec<(i64, String, String)> = sqlx::query_as(
            "SELECT id, path, content_hash FROM book \
             WHERE library_id = ? AND analyzed_at IS NULL AND content_hash IS NOT NULL",
        )
        .bind(library_id)
        .fetch_all(self.db.pool())
        .await?;
        self.update_scan(library_id, |status| {
            status.phase = ScanPhase::Analyzing;
            status.to_analyze = pending.len() as u32;
        });

        let mut tasks = JoinSet::new();
        for (book_id, path, content_hash) in pending {
            if cancelled.load(Ordering::Relaxed) {
                break;
            }
            // Let go of the books that are done, not all of them at the end.
            while tasks.try_join_next().is_some() {}
            let Ok(permit) = self.analysis.clone().acquire_owned().await else {
                break;
            };
            let catalog = self.clone();
            let file = root.join(&path);
            tasks.spawn(async move {
                let _permit = permit;
                // A book that fails here stays unanalyzed and is tried again
                // by the next scan.
                if let Err(error) = catalog.analyze_book(book_id, file, content_hash).await {
                    tracing::warn!(file = %path, %error, "could not analyze");
                }
                catalog.update_scan(library_id, |status| status.analyzed += 1);
            });
        }
        while tasks.join_next().await.is_some() {}
        Ok(())
    }

    async fn analyze_book(
        &self,
        book_id: i64,
        file: PathBuf,
        content_hash: String,
    ) -> Result<(), CatalogError> {
        let entries: Vec<String> =
            sqlx::query_scalar("SELECT entry FROM page WHERE book_id = ? ORDER BY number")
                .bind(book_id)
                .fetch_all(self.db.pool())
                .await?;
        let thumbnails = self.thumbnails.clone();
        let analysis = tokio::task::spawn_blocking(move || {
            analyze_file(&file, &entries, &content_hash, &thumbnails)
        })
        .await??;

        let sizes: Vec<(i64, u32, u32)> = (0..)
            .zip(&analysis.page_sizes)
            .filter_map(|(number, size)| size.map(|(width, height)| (number, width, height)))
            .collect();

        let mut tx = self.db.begin_write().await?;
        for chunk in sizes.chunks(ROWS_PER_STATEMENT) {
            let mut update = QueryBuilder::<Sqlite>::new("WITH size (number, width, height) AS (");
            update.push_values(chunk, |mut row, (number, width, height)| {
                row.push_bind(*number).push_bind(*width).push_bind(*height);
            });
            update
                .push(
                    ") UPDATE page SET width = size.width, height = size.height FROM size \
                     WHERE page.book_id = ",
                )
                .push_bind(book_id)
                .push(" AND page.number = size.number");
            update.build().execute(&mut *tx).await?;
        }
        sqlx::query("UPDATE book SET has_cover = ?, analyzed_at = ? WHERE id = ?")
            .bind(analysis.has_cover)
            .bind(now())
            .bind(book_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }
}

/// How many books the background pass opens at once, over all libraries. It
/// decodes images, so it is kept to part of the machine.
pub(crate) fn analysis_workers() -> usize {
    let cores = std::thread::available_parallelism().map_or(1, usize::from);
    (cores / 2).clamp(1, 4)
}

struct Analysis {
    /// Width and height of each page, where they could be read.
    page_sizes: Vec<Option<(u32, u32)>>,
    has_cover: bool,
}

/// Opens one book, reads its page sizes and makes its cover from the first
/// page unless a book with the same identity already has one.
fn analyze_file(
    file: &Path,
    entries: &[String],
    content_hash: &str,
    thumbnails: &Thumbnails,
) -> Result<Analysis, ArchiveError> {
    let mut archive = Archive::open(file)?;
    let page_sizes = entries
        .iter()
        .map(|entry| archive.dimensions(entry))
        .collect();

    let mut has_cover = thumbnails.exists(content_hash);
    if !has_cover && let Some(first) = entries.first() {
        let made = archive
            .read(first)
            .map_err(|error| error.to_string())
            .and_then(|image| {
                thumbnails
                    .make(content_hash, &image)
                    .map_err(|error| error.to_string())
            });
        match made {
            Ok(()) => has_cover = true,
            Err(error) => tracing::warn!(file = %file.display(), %error, "no cover"),
        }
    }
    Ok(Analysis {
        page_sizes,
        has_cover,
    })
}

/// Lists the comic files under `root`, reporting the running count through
/// `on_found`. `None` when the root is not there, as when its drive is not
/// connected.
fn walk(root: &Path, on_found: impl Fn(u32)) -> Option<Walk> {
    if !root.is_dir() {
        return None;
    }
    let mut walk = Walk {
        files: Vec::new(),
        complete: true,
    };
    let entries = WalkDir::new(root)
        .follow_links(true)
        // In the order of their names, not in the order the file system
        // lists them, which differs from one to the next: the first file
        // of a series gives the series its spelling.
        .sort_by_file_name()
        // Hidden folders hold other tools' data, never the owner's comics.
        .into_iter()
        .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()));
    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            // A link that points back up its own path: nothing was missed.
            Err(error) if error.loop_ancestor().is_some() => continue,
            Err(error) => {
                tracing::warn!(%error, "part of the library could not be read");
                walk.complete = false;
                continue;
            }
        };
        if !entry.file_type().is_file() || !archive::is_comic(entry.path()) {
            continue;
        }
        let Ok(metadata) = entry.metadata() else {
            walk.complete = false;
            continue;
        };
        let relative = entry.path().strip_prefix(root).unwrap_or(entry.path());
        let parts: Option<Vec<&str>> = relative.iter().map(|part| part.to_str()).collect();
        let Some(parts) = parts else {
            tracing::warn!(file = %entry.path().display(), "skipped: its name is not valid text");
            continue;
        };
        let Some((file_name, folders)) = parts.split_last() else {
            continue;
        };
        let name = Path::new(file_name)
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or(file_name);
        let modified_at = metadata
            .modified()
            .ok()
            .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |elapsed| elapsed.as_millis() as i64);
        walk.files.push(Found {
            path: parts.join("/"),
            folder: folders.join("/"),
            name: name.to_owned(),
            file_size: metadata.len() as i64,
            modified_at,
        });
        on_found(walk.files.len() as u32);
    }
    Some(walk)
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_str().is_some_and(|name| name.starts_with('.'))
}

#[cfg(any(test, feature = "fixtures"))]
impl Catalog {
    /// Waits until no scan of the library is running. For tests.
    pub async fn scan_finished(&self, library_id: i64) {
        while self.scan_status(library_id).phase != ScanPhase::Idle {
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::accounts::{self, UserId};
    use crate::db::Database;
    use crate::fixtures;
    use crate::from_files::FromFiles;

    pub(crate) struct Shelf {
        _dir: tempfile::TempDir,
        pub(crate) comics: PathBuf,
        pub(crate) catalog: Catalog,
        /// Who looks at the catalog in these tests.
        pub(crate) owner: UserId,
    }

    /// A catalog with an empty `Comics` folder next to its data directory.
    pub(crate) async fn shelf() -> Shelf {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let comics = dir.path().join("Comics");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&comics).unwrap();
        let db = Database::open(&data.join("binge.db")).await.unwrap();
        let owner = accounts::testing::owner(&db).await;
        let catalog = Catalog::new(db, &data);
        Shelf {
            comics: comics.canonicalize().unwrap(),
            catalog,
            owner,
            _dir: dir,
        }
    }

    impl Shelf {
        /// Adds the folder as a library and waits for its first scan.
        pub(crate) async fn add(&self) -> i64 {
            let library = self
                .catalog
                .add_library(self.owner, &self.comics, None, FromFiles::default())
                .await
                .unwrap();
            self.catalog.scan_finished(library.id).await;
            library.id
        }

        /// Scans the library again and waits for the scan to end.
        pub(crate) async fn rescan(&self, library_id: i64) {
            self.catalog.start_scan(library_id);
            self.catalog.scan_finished(library_id).await;
        }

        /// Where the book's cover is kept.
        fn cover(&self, book: &crate::Book) -> PathBuf {
            let identity = book.content_hash.as_deref().unwrap();
            self.catalog.thumbnails.path(identity).unwrap()
        }

        async fn series_names(&self, library_id: i64) -> Vec<String> {
            let series = self
                .catalog
                .series_in(self.owner, library_id)
                .await
                .unwrap();
            series.into_iter().map(|series| series.name).collect()
        }

        async fn unreadable(&self, library_id: i64) -> Vec<crate::Book> {
            self.catalog
                .unreadable_in(self.owner, library_id, false)
                .await
                .unwrap()
        }

        pub(crate) async fn books(&self, library_id: i64, series_name: &str) -> Vec<crate::Book> {
            let series = self
                .catalog
                .series_in(self.owner, library_id)
                .await
                .unwrap();
            let series = series.into_iter().find(|s| s.name == series_name).unwrap();
            self.catalog.books_in(self.owner, series.id).await.unwrap()
        }
    }

    #[tokio::test]
    async fn a_scan_turns_files_into_books_and_puts_them_in_series() {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 010.cbz"), 2, 10);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 002.cbz"), 3, 2);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 1.cbz"), 2, 1);
        fixtures::write_book(&shelf.comics.join("Publisher/Paper Girls/Vol 1.cbz"), 1, 20);
        fixtures::write_book(&shelf.comics.join("Loose.cbz"), 1, 30);
        fixtures::write_book(&shelf.comics.join(".trash/Gone.cbz"), 1, 40);
        std::fs::write(shelf.comics.join("Saga/notes.txt"), "not a comic").unwrap();

        let id = shelf.add().await;

        // A file whose name gives no series takes its folder's. One directly
        // in the library folder is a series of its own.
        assert_eq!(
            shelf.series_names(id).await,
            ["Loose", "Paper Girls", "Saga"]
        );
        let library = shelf.catalog.library(shelf.owner, id).await.unwrap();
        assert_eq!((library.series_count, library.book_count), (3, 5));
        let loose = shelf.books(id, "Loose").await;
        assert_eq!(loose.len(), 1);
        assert!(loose[0].cover.is_some());
        assert!(library.last_scanned_at.is_some());

        let saga = shelf.books(id, "Saga").await;
        let names: Vec<_> = saga.iter().map(|book| book.name.as_str()).collect();
        assert_eq!(names, ["Saga 1", "Saga 002", "Saga 010"]);
        assert_eq!(saga[1].page_count, 3);
        assert!(
            saga.iter()
                .all(|book| book.cover.is_some() && book.problem.is_none())
        );

        let status = shelf.catalog.scan_status(id);
        assert_eq!(status.phase, ScanPhase::Idle);
        assert_eq!(
            (status.found, status.to_analyze, status.analyzed),
            (5, 5, 5)
        );
        assert_eq!(status.problem, None);
    }

    #[tokio::test]
    async fn the_background_pass_records_page_sizes_and_makes_covers() {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 1.cbz"), 2, 1);
        let id = shelf.add().await;

        let book = shelf.books(id, "Saga").await.remove(0);
        assert!(shelf.cover(&book).is_file());
        assert_eq!(book.cover, book.content_hash);

        let sizes: Vec<(i64, i64)> =
            sqlx::query_as("SELECT width, height FROM page WHERE book_id = ? ORDER BY number")
                .bind(book.id)
                .fetch_all(shelf.catalog.db.pool())
                .await
                .unwrap();
        assert_eq!(sizes, [(40, 60), (40, 60)]);

        // The series shows the cover of its first book.
        let series = shelf
            .catalog
            .series_in(shelf.owner, id)
            .await
            .unwrap()
            .remove(0);
        assert_eq!(series.cover, book.content_hash);
    }

    #[tokio::test]
    async fn an_unreadable_file_is_listed_with_its_problem() {
        let shelf = shelf().await;
        std::fs::create_dir_all(shelf.comics.join("Odd")).unwrap();
        std::fs::write(shelf.comics.join("Odd/broken.cbz"), "not a zip").unwrap();
        let id = shelf.add().await;

        let book = shelf.unreadable(id).await.remove(0);
        assert_eq!(book.name, "broken");
        assert_eq!(book.page_count, 0);
        assert!(book.problem.is_some());
        assert!(book.content_hash.is_none() && book.cover.is_none());
        // It is in no series, and makes none.
        assert_eq!(book.series_id, None);
        assert!(shelf.series_names(id).await.is_empty());
    }

    #[tokio::test]
    async fn an_unreadable_file_set_aside_is_not_listed_until_it_is_read_again() {
        let shelf = shelf().await;
        let broken = shelf.comics.join("broken.cbz");
        std::fs::write(&broken, "not a zip").unwrap();
        fixtures::write_book(&shelf.comics.join("Saga 1.cbz"), 1, 1);
        let id = shelf.add().await;
        let count = async || {
            let library = shelf.catalog.library(shelf.owner, id).await.unwrap();
            library.unreadable_count
        };
        let aside = async || {
            let files = shelf.catalog.unreadable_in(shelf.owner, id, true).await;
            files.unwrap().len()
        };
        let file = shelf.unreadable(id).await.remove(0);
        assert_eq!((count().await, aside().await), (1, 0));

        shelf.catalog.set_aside(file.id, true).await.unwrap();
        assert!(shelf.unreadable(id).await.is_empty());
        assert_eq!((count().await, aside().await), (0, 1));
        // A scan that finds the file as it was leaves it aside.
        shelf.rescan(id).await;
        assert_eq!((count().await, aside().await), (0, 1));

        // It can be taken back.
        shelf.catalog.set_aside(file.id, false).await.unwrap();
        assert_eq!((count().await, aside().await), (1, 0));
        shelf.catalog.set_aside(file.id, true).await.unwrap();

        // Read again and still not a comic, it is listed again.
        std::fs::write(&broken, "still not a zip, and longer").unwrap();
        shelf.rescan(id).await;
        assert_eq!((count().await, aside().await), (1, 0));

        // A book that can be read is not set aside.
        let book = shelf.books(id, "Saga").await.remove(0);
        let refused = shelf.catalog.set_aside(book.id, true).await;
        assert!(matches!(refused, Err(CatalogError::Invalid(_))));
        let refused = shelf.catalog.set_aside(0, true).await;
        assert!(matches!(refused, Err(CatalogError::NotFound)));
    }

    #[tokio::test]
    async fn a_rescan_picks_up_added_changed_and_removed_files() {
        let shelf = shelf().await;
        let (kept, changed, removed) = (
            shelf.comics.join("Saga/Saga 1.cbz"),
            shelf.comics.join("Saga/Saga 2.cbz"),
            shelf.comics.join("Lonely/Only.cbz"),
        );
        fixtures::write_book(&kept, 2, 1);
        fixtures::write_book(&changed, 2, 2);
        fixtures::write_book(&removed, 2, 3);
        let id = shelf.add().await;

        let before = shelf.books(id, "Saga").await;
        let old_cover = shelf.cover(&before[1]);
        let removed_cover = shelf.cover(&shelf.books(id, "Lonely").await[0]);
        assert!(old_cover.is_file() && removed_cover.is_file());

        fixtures::write_book(&changed, 5, 22);
        std::fs::remove_file(&removed).unwrap();
        fixtures::write_book(&shelf.comics.join("Saga/Saga 3.cbz"), 1, 4);
        shelf.rescan(id).await;

        // The emptied series stays, with no books: nothing removes a series
        // but being asked to.
        assert_eq!(shelf.series_names(id).await, ["Lonely", "Saga"]);
        assert!(shelf.books(id, "Lonely").await.is_empty());
        let after = shelf.books(id, "Saga").await;
        assert_eq!(after.len(), 3);
        // The unchanged book was left alone, the changed one kept its place.
        assert_eq!(after[0].id, before[0].id);
        assert_eq!(after[1].id, before[1].id);
        assert_eq!(after[1].page_count, 5);
        assert_ne!(after[1].content_hash, before[1].content_hash);
        assert!(after.iter().all(|book| book.cover.is_some()));
        // A cover is made once and kept, for when its book comes back.
        assert!(old_cover.is_file() && removed_cover.is_file());

        // Only the changed and the new book needed the background pass.
        assert_eq!(shelf.catalog.scan_status(id).to_analyze, 2);
    }

    #[tokio::test]
    async fn a_retagged_file_keeps_what_the_background_pass_made() {
        let shelf = shelf().await;
        let file = shelf.comics.join("Saga/Saga 1.cbz");
        let pages = [fixtures::page(40, 60, 1), fixtures::page(40, 60, 2)];
        fixtures::write_cbz(
            &file,
            &[("a.png", pages[0].clone()), ("b.png", pages[1].clone())],
        );
        let id = shelf.add().await;
        let before = shelf.books(id, "Saga").await.remove(0);

        // The same pages under new names, with a metadata file added.
        fixtures::write_cbz(
            &file,
            &[
                ("ComicInfo.xml", b"<ComicInfo/>".to_vec()),
                ("01.png", pages[0].clone()),
                ("02.png", pages[1].clone()),
            ],
        );
        shelf.rescan(id).await;

        let after = shelf.books(id, "Saga").await.remove(0);
        assert_eq!(after.id, before.id);
        assert_eq!(after.content_hash, before.content_hash);
        assert_ne!(after.file_size, before.file_size);
        assert_eq!(shelf.catalog.scan_status(id).to_analyze, 0);
        assert!(after.cover.is_some() && shelf.cover(&after).is_file());

        let stored: Vec<(String, Option<i64>)> =
            sqlx::query_as("SELECT entry, width FROM page WHERE book_id = ? ORDER BY number")
                .bind(after.id)
                .fetch_all(shelf.catalog.db.pool())
                .await
                .unwrap();
        assert_eq!(
            stored,
            [
                ("01.png".to_owned(), Some(40)),
                ("02.png".to_owned(), Some(40))
            ]
        );
    }

    #[tokio::test]
    async fn cbr_and_pdf_files_become_books_like_any_other() {
        let shelf = shelf().await;
        let scans = [
            fixtures::jpeg_page(30, 45, 1),
            fixtures::jpeg_page(30, 45, 2),
        ];
        fixtures::write_cbr(
            &shelf.comics.join("Saga/Saga 1.cbr"),
            &[
                ("a.png", fixtures::page(40, 60, 1)),
                ("b.png", fixtures::page(40, 60, 2)),
            ],
        );
        fixtures::write_pdf(
            &shelf.comics.join("Saga/Saga 2.pdf"),
            &[
                fixtures::PdfPage::Scan(scans[0].clone()),
                fixtures::PdfPage::Scan(scans[1].clone()),
            ],
        );
        std::fs::write(shelf.comics.join("Saga/Saga 3.pdf"), b"%PDF-1.4 cut off").unwrap();
        let id = shelf.add().await;

        let books = shelf.books(id, "Saga").await;
        let names: Vec<_> = books.iter().map(|book| book.name.as_str()).collect();
        assert_eq!(names, ["Saga 1", "Saga 2"]);
        for book in &books {
            assert_eq!(book.page_count, 2, "{}", book.name);
            assert!(
                book.cover.is_some() && shelf.cover(book).is_file(),
                "{}",
                book.name
            );
        }
        let cut_off = shelf.unreadable(id).await.remove(0);
        assert_eq!(
            (cut_off.name.as_str(), cut_off.problem.as_deref()),
            ("Saga 3", Some("This file is not a readable PDF."))
        );

        let widths: Vec<(i64, Option<i64>)> =
            sqlx::query_as("SELECT book_id, width FROM page WHERE number = 1 ORDER BY book_id")
                .fetch_all(shelf.catalog.db.pool())
                .await
                .unwrap();
        assert_eq!(widths, [(books[0].id, Some(40)), (books[1].id, Some(30))]);

        // A page of the PDF comes back as the picture the PDF holds.
        let identity = books[1].content_hash.as_deref().unwrap();
        let page = shelf
            .catalog
            .page_image(shelf.owner, identity, 1)
            .await
            .unwrap();
        assert_eq!(page.bytes, scans[1]);
        assert_eq!(page.media_type, "image/jpeg");
    }

    #[tokio::test]
    async fn what_a_file_says_about_itself_is_kept_and_follows_a_retag() {
        let shelf = shelf().await;
        let file = shelf.comics.join("Saga/Saga 1.cbz");
        let tagged = |fields: &[(&str, &str)]| fixtures::write_tagged_book(&file, 1, fields);
        tagged(&[
            ("Title", "The Long Night"),
            ("Number", "1"),
            ("Year", "2019"),
        ]);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 2.cbz"), 1, 2);
        let id = shelf.add().await;

        let books = shelf.books(id, "Saga").await;
        assert_eq!(books[0].info.title.as_deref(), Some("The Long Night"));
        assert_eq!(books[0].info.number.as_deref(), Some("1"));
        assert_eq!(books[0].info.year, Some(2019));
        assert!(books[1].info.is_empty());

        tagged(&[("Title", "The Short Day"), ("Writer", "Ana Pérez")]);
        shelf.rescan(id).await;
        let book = shelf.books(id, "Saga").await.remove(0);
        assert_eq!(book.id, books[0].id);
        assert_eq!(book.content_hash, books[0].content_hash);
        assert_eq!(book.info.title.as_deref(), Some("The Short Day"));
        assert_eq!(book.info.writer.as_deref(), Some("Ana Pérez"));
        assert_eq!(book.info.year, None);

        // Without the metadata file, the book says nothing again.
        fixtures::write_cbz(
            &file,
            &[
                ("01.png", fixtures::page(40, 60, 1)),
                ("readme.txt", b"hello there".to_vec()),
            ],
        );
        shelf.rescan(id).await;
        assert!(shelf.books(id, "Saga").await[0].info.is_empty());
    }

    #[tokio::test]
    async fn search_finds_series_and_books_by_their_words() {
        let shelf = shelf().await;
        let book = |path: &str, seed: u8, fields: &[(&str, &str)]| {
            fixtures::write_tagged_book(&shelf.comics.join(path), seed, fields);
        };
        book(
            "Saga/Saga 001.cbz",
            1,
            &[("Title", "The Long Night"), ("Writer", "Élodie Durand")],
        );
        book("Saga/Saga 002.cbz", 2, &[("Title", "100% Night")]);
        book(
            "Batman/Batman 404.cbz",
            3,
            &[("Publisher", "Small Press"), ("Series", "Año Uno")],
        );
        fixtures::write_book(&shelf.comics.join("Loose Saga.cbz"), 1, 4);
        let id = shelf.add().await;

        let search = async |user: UserId, text: &str| {
            let found = shelf.catalog.search(user, text, Some(10)).await.unwrap();
            let series: Vec<String> = found.series.into_iter().map(|one| one.name).collect();
            let books: Vec<String> = found.books.into_iter().map(|one| one.name).collect();
            (series, books)
        };
        let names = |names: &[&str]| -> Vec<String> {
            names.iter().map(|name| (*name).to_owned()).collect()
        };

        // A series by its name, books by their file's name.
        assert_eq!(
            search(shelf.owner, "saga").await,
            (
                names(&["Loose Saga", "Saga"]),
                names(&["Loose Saga", "Saga 001", "Saga 002"])
            )
        );
        // Case and accents do not matter, in the search or in what is found.
        assert_eq!(search(shelf.owner, "ANO").await.0, names(&["Año Uno"]));
        assert_eq!(search(shelf.owner, "elodie").await.1, names(&["Saga 001"]));
        assert_eq!(
            search(shelf.owner, "DURAND élo").await.1,
            names(&["Saga 001"])
        );
        // Every word has to be there, anywhere in a word.
        assert_eq!(
            search(shelf.owner, "night").await.1,
            names(&["Saga 001", "Saga 002"])
        );
        assert_eq!(
            search(shelf.owner, "long night").await.1,
            names(&["Saga 001"])
        );
        assert_eq!(
            search(shelf.owner, "press 404").await.1,
            names(&["Batman 404"])
        );
        assert_eq!(search(shelf.owner, "night batman").await, (vec![], vec![]));
        // What looks like a pattern is searched for as written.
        assert_eq!(search(shelf.owner, "100%").await.1, names(&["Saga 002"]));
        assert_eq!(search(shelf.owner, "%").await.1, names(&["Saga 002"]));
        assert_eq!(search(shelf.owner, "_").await, (vec![], vec![]));
        assert_eq!(search(shelf.owner, "   ").await, (vec![], vec![]));

        // No more than asked for, and it says when there are more.
        let few = shelf
            .catalog
            .search(shelf.owner, "saga", Some(2))
            .await
            .unwrap();
        assert_eq!(few.books.len(), 2);
        assert!(few.more_books && !few.more_series);
        let all = shelf
            .catalog
            .search(shelf.owner, "saga", Some(3))
            .await
            .unwrap();
        assert_eq!(all.books.len(), 3);
        assert!(!all.more_books);

        // Only in the libraries the user may read.
        let db = &shelf.catalog.db;
        let reader = accounts::testing::member(db, "reader", &[id]).await;
        let stranger = accounts::testing::member(db, "stranger", &[]).await;
        assert_eq!(
            search(reader, "saga").await.0,
            names(&["Loose Saga", "Saga"])
        );
        assert_eq!(search(stranger, "saga").await, (vec![], vec![]));
    }

    #[tokio::test]
    async fn a_series_is_the_books_that_go_by_one_name_wherever_they_are() {
        let shelf = shelf().await;
        let comics = &shelf.comics;
        // One series in two folders, and two series in one.
        fixtures::write_book(&comics.join("Scans/Night Runner 001.cbz"), 1, 1);
        fixtures::write_book(&comics.join("More/night-runner 002.cbz"), 1, 2);
        fixtures::write_book(&comics.join("More/Odd Squad 001.cbz"), 1, 3);
        // The tag is believed before the name, and the name before the folder.
        fixtures::write_tagged_book(
            &comics.join("More/Unsorted 7.cbz"),
            4,
            &[("Series", "Night Runner"), ("Number", "3")],
        );
        let id = shelf.add().await;

        assert_eq!(shelf.series_names(id).await, ["Night Runner", "Odd Squad"]);
        let runner = shelf.books(id, "Night Runner").await;
        let read: Vec<_> = runner
            .iter()
            .map(|book| (book.name.as_str(), book.number.as_deref()))
            .collect();
        assert_eq!(
            read,
            [
                ("Night Runner 001", Some("1")),
                ("night-runner 002", Some("2")),
                ("Unsorted 7", Some("3")),
            ]
        );
        // None of them names a volume or is a special: no groups.
        assert!(runner.iter().all(|book| book.group_id.is_none()));
        let series_id = runner[0].series_id.unwrap();
        let groups = shelf.catalog.groups_in(shelf.owner, series_id).await;
        assert!(groups.unwrap().is_empty());
    }

    #[tokio::test]
    async fn volumes_annuals_and_one_shots_are_groups_read_in_order() {
        let shelf = shelf().await;
        let comics = &shelf.comics;
        fixtures::write_book(&comics.join("Odd Squad/Odd Squad v2 001.cbz"), 1, 1);
        fixtures::write_book(&comics.join("Odd Squad/Odd Squad v1 010.cbz"), 1, 2);
        fixtures::write_book(&comics.join("Odd Squad/Odd Squad v1 9.cbz"), 1, 3);
        fixtures::write_book(&comics.join("Odd Squad/Odd Squad 100.cbz"), 1, 4);
        fixtures::write_book(&comics.join("Odd Squad/Odd Squad SP02 - Later.cbz"), 1, 5);
        fixtures::write_book(&comics.join("Odd Squad/The Lost Year.cbz"), 1, 6);
        fixtures::write_tagged_book(
            &comics.join("Odd Squad/Odd Squad SP01 - First.cbz"),
            7,
            &[("Year", "1963"), ("Month", "6")],
        );
        fixtures::write_tagged_book(
            &comics.join("Odd Squad/Odd Squad Annual 1.cbz"),
            8,
            &[
                ("Series", "Odd Squad"),
                ("Year", "1962"),
                ("Format", "Annual"),
                ("Volume", "1"),
            ],
        );
        let id = shelf.add().await;

        let books = shelf.books(id, "Odd Squad").await;
        let series_id = books[0].series_id.unwrap();
        let groups = shelf
            .catalog
            .groups_in(shelf.owner, series_id)
            .await
            .unwrap();
        let listed: Vec<_> = groups
            .iter()
            .map(|group| (group.kind, group.number, group.book_count))
            .collect();
        assert_eq!(
            listed,
            [
                (GroupKind::Volume, Some(1), 2),
                (GroupKind::Volume, Some(2), 1),
                (GroupKind::Annual, Some(1), 1),
                (GroupKind::OneShot, None, 3),
            ]
        );

        // Volume by volume, then the book in no volume, then the annuals,
        // then the one-shots: the dated ones by date, the others by name.
        let read: Vec<_> = books
            .iter()
            .map(|book| {
                let group = groups
                    .iter()
                    .position(|group| Some(group.id) == book.group_id);
                (book.name.as_str(), group)
            })
            .collect();
        assert_eq!(
            read,
            [
                ("Odd Squad v1 9", Some(0)),
                ("Odd Squad v1 010", Some(0)),
                ("Odd Squad v2 001", Some(1)),
                ("Odd Squad 100", None),
                ("Odd Squad Annual 1", Some(2)),
                ("Odd Squad SP01 - First", Some(3)),
                ("Odd Squad SP02 - Later", Some(3)),
                ("The Lost Year", Some(3)),
            ]
        );
        // The first book read stands for the series.
        let series = shelf.catalog.series(shelf.owner, series_id).await.unwrap();
        assert_eq!(series.cover, books[0].content_hash);
    }

    #[tokio::test]
    async fn a_book_keeps_its_place_whatever_happens_to_its_file() {
        let shelf = shelf().await;
        let comics = &shelf.comics;
        let pages = [("01.png", fixtures::page(40, 60, 1))];
        let first = comics.join("Scans/Night Runner v1 001.cbz");
        fixtures::write_cbz(&first, &pages);
        fixtures::write_book(&comics.join("Scans/Night Runner v1 002.cbz"), 1, 2);
        let id = shelf.add().await;
        let before = shelf.books(id, "Night Runner").await.remove(0);
        assert_eq!(before.number.as_deref(), Some("1"));
        let place = |book: &crate::Book| (book.series_id, book.group_id, book.number.clone());

        // Retagged, to say it is something else.
        let mut retagged = pages.to_vec();
        retagged.push((
            "ComicInfo.xml",
            fixtures::comic_info(&[("Series", "Odd Squad"), ("Number", "44"), ("Volume", "9")]),
        ));
        fixtures::write_cbz(&first, &retagged);
        shelf.rescan(id).await;
        // Then renamed and moved to another folder.
        let moved = comics.join("Elsewhere/Odd Squad 044.cbz");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(&first, &moved).unwrap();
        shelf.rescan(id).await;

        assert_eq!(shelf.series_names(id).await, ["Night Runner"]);
        let after = shelf.books(id, "Night Runner").await.remove(0);
        // The file's name and tags are shown as they are now. The place is
        // the one it was given.
        assert_eq!(after.name, "Odd Squad 044");
        assert_eq!(after.info.series.as_deref(), Some("Odd Squad"));
        assert_eq!(place(&after), place(&before));

        // Gone for a while, with the rest of its series, and back again.
        let away = shelf.comics.parent().unwrap().join("away.cbz");
        std::fs::rename(&moved, &away).unwrap();
        std::fs::remove_file(comics.join("Scans/Night Runner v1 002.cbz")).unwrap();
        shelf.rescan(id).await;
        // The series stays, with no books in it.
        assert_eq!(shelf.series_names(id).await, ["Night Runner"]);
        let library = shelf.catalog.library(shelf.owner, id).await.unwrap();
        assert_eq!((library.series_count, library.book_count), (1, 0));
        let emptied = shelf.catalog.series(shelf.owner, before.series_id.unwrap());
        assert_eq!(emptied.await.unwrap().book_count, 0);

        std::fs::rename(&away, &moved).unwrap();
        shelf.rescan(id).await;
        let back = shelf.books(id, "Night Runner").await.remove(0);
        assert_eq!(place(&back), place(&before));
    }

    #[tokio::test]
    async fn a_file_that_cannot_be_read_has_no_place_until_it_can() {
        let shelf = shelf().await;
        let broken = shelf.comics.join("Scans/Night Runner 002.cbz");
        fixtures::write_book(&shelf.comics.join("Scans/Night Runner 001.cbz"), 1, 1);
        std::fs::write(&broken, "not a zip").unwrap();
        let id = shelf.add().await;

        // A place is kept by a book's identity, and this file has none.
        assert_eq!(shelf.books(id, "Night Runner").await.len(), 1);
        let unreadable = shelf.unreadable(id).await;
        assert_eq!(unreadable.len(), 1);
        assert_eq!(
            (unreadable[0].series_id, &unreadable[0].number),
            (None, &None)
        );

        // Once it can be read, it is imported like any other book.
        fixtures::write_book(&broken, 3, 2);
        shelf.rescan(id).await;
        assert!(shelf.unreadable(id).await.is_empty());
        let books = shelf.books(id, "Night Runner").await;
        assert_eq!(books.len(), 2);
        assert_eq!(books[1].number.as_deref(), Some("2"));
    }

    #[tokio::test]
    async fn a_moved_file_keeps_its_identity_and_cover() {
        let shelf = shelf().await;
        let original = shelf.comics.join("Saga/Saga 1.cbz");
        fixtures::write_book(&original, 2, 1);
        let id = shelf.add().await;
        let before = shelf.books(id, "Saga").await.remove(0);

        let moved = shelf.comics.join("Image/Saga/Saga 001.cbz");
        std::fs::create_dir_all(moved.parent().unwrap()).unwrap();
        std::fs::rename(&original, &moved).unwrap();
        shelf.rescan(id).await;

        let after = shelf.books(id, "Saga").await.remove(0);
        assert_eq!(after.name, "Saga 001");
        assert_eq!(after.content_hash, before.content_hash);
        assert!(after.cover.is_some() && shelf.cover(&after).is_file());
    }

    #[tokio::test]
    async fn a_missing_library_folder_removes_nothing() {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 1.cbz"), 2, 1);
        let id = shelf.add().await;

        // As when an external drive is not connected.
        let away = shelf.comics.with_file_name("Comics (away)");
        std::fs::rename(&shelf.comics, &away).unwrap();
        shelf.rescan(id).await;

        assert!(shelf.catalog.scan_status(id).problem.is_some());
        assert_eq!(shelf.books(id, "Saga").await.len(), 1);

        std::fs::rename(&away, &shelf.comics).unwrap();
        shelf.rescan(id).await;
        assert_eq!(shelf.catalog.scan_status(id).problem, None);
    }

    #[tokio::test]
    async fn removing_a_library_forgets_it_and_leaves_the_files() {
        let shelf = shelf().await;
        let file = shelf.comics.join("Saga/Saga 1.cbz");
        fixtures::write_book(&file, 2, 1);
        let id = shelf.add().await;
        let book = shelf.books(id, "Saga").await.remove(0);
        let cover = shelf.cover(&book);
        let made = cover.metadata().unwrap().modified().unwrap();

        shelf.catalog.remove_library(id).await.unwrap();

        assert!(
            shelf
                .catalog
                .libraries(shelf.owner)
                .await
                .unwrap()
                .is_empty()
        );
        assert!(file.is_file());
        // The cover stays: it hangs on the book's identity, not the library.
        assert!(cover.is_file());
        let pages: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM page")
            .fetch_one(shelf.catalog.db.pool())
            .await
            .unwrap();
        assert_eq!(pages, 0);

        // Added again, the book finds the cover it had: it is not made twice.
        let id = shelf.add().await;
        let book = shelf.books(id, "Saga").await.remove(0);
        assert_eq!(book.cover, book.content_hash);
        assert_eq!(cover.metadata().unwrap().modified().unwrap(), made);
    }
}
