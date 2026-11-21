//! What a person brings from the server they are leaving.
//!
//! Another server is read once, by whoever asks, with their key there:
//! [`Taken`] is what it said. It is kept as it was read, so that it can be
//! shown before anything is done with it ([`Brought`]): which of its
//! libraries are which here, how many of its files are books this server
//! holds, and what there is of reading, lists and collections to bring.
//!
//! Nothing here reads that server: this crate is given what was read. And
//! nothing here changes what the person has read or made on this server.
//! A file there is the same book as one here when it sits at the same place
//! under its library's folder, or else when it has the same name and size.

use std::collections::{HashMap, HashSet};

use crate::accounts::UserId;
use crate::catalog::{Catalog, CatalogError, may_read, now};
use crate::changes::file_name;
use crate::natural::natural_cmp;
use crate::reading::list;
use crate::search::fold;

/// What another server said, as it was read.
#[derive(Clone, Debug, Default)]
pub struct Taken {
    /// Which kind of server it is, and where it was asked.
    pub source: String,
    pub address: String,
    pub version: String,
    /// The name the person goes by there.
    pub their_name: String,
    /// Whether their key is of someone who manages that server.
    pub manages: bool,
    pub libraries: Vec<TakenLibrary>,
    pub series: Vec<TakenSeries>,
    pub books: Vec<TakenBook>,
    pub reading: Vec<TakenReading>,
    pub sittings: Vec<TakenSitting>,
    pub lists: Vec<TakenList>,
    pub collections: Vec<TakenCollection>,
    pub pages: Vec<TakenPage>,
    pub people: Vec<TakenPerson>,
}

#[derive(Clone, Debug, Default)]
pub struct TakenLibrary {
    pub id: i64,
    pub name: String,
    /// Its folders, as that server sees them.
    pub folders: Vec<String>,
}

#[derive(Clone, Debug, Default)]
pub struct TakenSeries {
    pub id: i64,
    pub library: i64,
    pub name: String,
    /// Whether the person wants to read it.
    pub wanted: bool,
}

/// A file there.
#[derive(Clone, Debug, Default)]
pub struct TakenBook {
    pub id: i64,
    /// What that server reads as one thing: one file nearly always.
    pub chapter: i64,
    pub series: i64,
    pub library: i64,
    /// The whole path, as that server sees it.
    pub path: String,
    pub file_size: i64,
    pub page_count: i64,
    /// Where the file's pages start among its chapter's.
    pub first_page: i64,
}

/// How far the person is in a chapter there.
#[derive(Clone, Debug, Default)]
pub struct TakenReading {
    pub chapter: i64,
    pub pages_read: i64,
    pub page_count: i64,
    /// When they last read it, in seconds since 1970.
    pub read_at: Option<i64>,
}

/// One stretch of reading there, of the ones that server keeps. What it
/// only knows as the day a chapter was last read is its [`TakenReading`].
#[derive(Clone, Debug, Default)]
pub struct TakenSitting {
    pub chapter: i64,
    pub started_at: i64,
    pub ended_at: i64,
    pub first_page: i64,
    pub last_page: i64,
    /// Whether that server timed it. When it did not, the times are the day
    /// it was read and say nothing about how long it took.
    pub measured: bool,
}

/// A reading list there: its chapters in its order.
#[derive(Clone, Debug, Default)]
pub struct TakenList {
    pub id: i64,
    pub name: String,
    pub summary: String,
    /// Whether the person made it.
    pub theirs: bool,
    /// Whether it was shown to everyone.
    pub shared: bool,
    pub chapters: Vec<i64>,
}

/// A collection there: its series.
#[derive(Clone, Debug, Default)]
pub struct TakenCollection {
    pub id: i64,
    pub name: String,
    pub summary: String,
    pub theirs: bool,
    pub shared: bool,
    pub series: Vec<i64>,
}

/// A page the person marked there, from 0 in its chapter.
#[derive(Clone, Debug, Default)]
pub struct TakenPage {
    pub chapter: i64,
    pub page: i64,
}

/// Someone who uses that server.
#[derive(Clone, Debug, Default)]
pub struct TakenPerson {
    pub id: i64,
    pub name: String,
    pub manages: bool,
    /// Whether they were kept from some of what is there by age.
    pub limited: bool,
    pub libraries: Vec<i64>,
}

/// What was read from another server, as the one who read it is shown it
/// before anything is done.
#[derive(Clone, Debug)]
pub struct Brought {
    pub source: String,
    pub address: String,
    pub version: String,
    pub their_name: String,
    pub manages: bool,
    pub taken_at: i64,
    pub libraries: Vec<BroughtLibrary>,
    pub reading: BroughtReading,
    /// The reading lists and the collections the person made or was shown.
    pub lists: i64,
    pub collections: i64,
    /// The series they want to read, and the pages they marked.
    pub wanted: i64,
    pub pages: i64,
    pub people: Vec<BroughtPerson>,
}

/// A library there, and how much of it this server holds.
#[derive(Clone, Debug)]
pub struct BroughtLibrary {
    pub their_id: i64,
    pub name: String,
    pub folders: Vec<String>,
    /// The library here that holds the same folder: the one it was taken
    /// for, or the one the person said.
    pub library_id: Option<i64>,
    /// Its files there.
    pub books: i64,
    /// The ones this server holds, at the same place under the folder.
    pub by_path: i64,
    /// The ones it holds somewhere else, told by name and size.
    pub by_file: i64,
    /// Some of the ones it does not hold: where each is under the folder.
    pub missing: Vec<String>,
}

/// The person's reading there.
#[derive(Clone, Debug, Default)]
pub struct BroughtReading {
    /// What they started and did not finish, and what they finished.
    pub started: i64,
    pub finished: i64,
    /// How many of those two are books this server holds.
    pub found: i64,
    /// The stretches of reading that server timed, and the seconds they
    /// took together.
    pub timed: i64,
    pub seconds: i64,
    /// The ones it only knows the day of, and the chapters it kept no
    /// stretch of and knows the day they were last read.
    pub dated: i64,
    /// When the first and the last of them started.
    pub first_at: Option<i64>,
    pub last_at: Option<i64>,
}

#[derive(Clone, Debug)]
pub struct BroughtPerson {
    pub name: String,
    pub manages: bool,
    pub limited: bool,
    /// The names of the libraries there that they read.
    pub libraries: Vec<String>,
}

/// How many of the files a library lacks are named in what is shown.
const MOST_MISSING: usize = 20;

/// How a file there was told to be a book here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum FoundBy {
    Path,
    File,
}

/// A path with `/` between its folders and none at its end.
fn slashed(path: &str) -> String {
    path.replace('\\', "/").trim_end_matches('/').to_owned()
}

/// Where `path` is under the longest of `folders` that holds it, or the
/// whole path when none does.
fn under(path: &str, folders: &[String]) -> String {
    let path = slashed(path);
    let held = folders
        .iter()
        .map(|folder| slashed(folder))
        .filter(|folder| {
            path.strip_prefix(folder.as_str())
                .is_some_and(|rest| rest.starts_with('/'))
        })
        .max_by_key(String::len);
    match held {
        Some(folder) => path[folder.len() + 1..].to_owned(),
        None => path,
    }
}

#[derive(sqlx::FromRow)]
struct TheirFile {
    their_id: i64,
    their_chapter: i64,
    their_library: i64,
    library_id: Option<i64>,
    path: String,
    file_size: i64,
    page_count: i64,
}

#[derive(sqlx::FromRow)]
struct OurFile {
    library_id: i64,
    path: String,
    file_size: i64,
    page_count: i64,
}

/// The name a file there and a file here share when they are the same:
/// the file's own, without regard to case.
fn named(path: &str) -> String {
    file_name(path).to_lowercase()
}

impl Catalog {
    /// Keeps what `user` read from another server, in place of what they
    /// read before. A library there is taken for the one here with the same
    /// folder, or else the same name, among the libraries `user` reads. When
    /// it is the same server as before, what they said of a library holds.
    pub async fn keep_brought(&self, user: UserId, taken: &Taken) -> Result<(), CatalogError> {
        let ours = self.libraries(user).await?;
        let folders: HashMap<i64, &[String]> = taken
            .libraries
            .iter()
            .map(|library| (library.id, library.folders.as_slice()))
            .collect();
        let places: Vec<String> = taken
            .books
            .iter()
            .map(|book| {
                let folders = folders.get(&book.library).copied().unwrap_or_default();
                under(&book.path, folders)
            })
            .collect();
        let mut tx = self.db.begin_write().await?;
        let said: HashMap<i64, Option<i64>> = sqlx::query_as(
            "SELECT bl.their_id, bl.library_id FROM brought_library bl \
             JOIN bringing b ON b.id = bl.bringing_id \
             WHERE b.user_id = ? AND b.source = ? AND b.address = ? AND bl.said",
        )
        .bind(user)
        .bind(&taken.source)
        .bind(&taken.address)
        .fetch_all(&mut *tx)
        .await?
        .into_iter()
        .collect();
        sqlx::query("DELETE FROM bringing WHERE user_id = ?")
            .bind(user)
            .execute(&mut *tx)
            .await?;
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO bringing \
             (user_id, source, address, version, their_name, manages, taken_at) \
             VALUES (?, ?, ?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user)
        .bind(&taken.source)
        .bind(&taken.address)
        .bind(&taken.version)
        .bind(&taken.their_name)
        .bind(taken.manages)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;

        for library in &taken.libraries {
            let same_folder = ours.iter().find(|ours| {
                let root = slashed(&ours.root_path);
                library.folders.iter().any(|folder| slashed(folder) == root)
            });
            let same_name = || {
                let name = fold(library.name.trim());
                ours.iter().find(|ours| fold(ours.name.trim()) == name)
            };
            let taken_for = || same_folder.or_else(same_name).map(|ours| ours.id);
            let was_said = said.get(&library.id).copied();
            sqlx::query(
                "INSERT INTO brought_library (bringing_id, their_id, name, library_id, said) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(library.id)
            .bind(&library.name)
            .bind(was_said.unwrap_or_else(taken_for))
            .bind(was_said.is_some())
            .execute(&mut *tx)
            .await?;
            for folder in &library.folders {
                sqlx::query(
                    "INSERT OR IGNORE INTO brought_folder (bringing_id, their_library, path) \
                     VALUES (?, ?, ?)",
                )
                .bind(id)
                .bind(library.id)
                .bind(folder)
                .execute(&mut *tx)
                .await?;
            }
        }
        for series in &taken.series {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_series \
                 (bringing_id, their_id, their_library, name, wanted) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(series.id)
            .bind(series.library)
            .bind(&series.name)
            .bind(series.wanted)
            .execute(&mut *tx)
            .await?;
        }
        for (book, place) in taken.books.iter().zip(&places) {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_book (bringing_id, their_id, their_chapter, \
                 their_series, their_library, path, file_size, page_count, first_page) \
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(book.id)
            .bind(book.chapter)
            .bind(book.series)
            .bind(book.library)
            .bind(place)
            .bind(book.file_size)
            .bind(book.page_count)
            .bind(book.first_page)
            .execute(&mut *tx)
            .await?;
        }
        for reading in &taken.reading {
            sqlx::query(
                "INSERT OR REPLACE INTO brought_reading \
                 (bringing_id, their_chapter, pages_read, page_count, read_at) \
                 VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(reading.chapter)
            .bind(reading.pages_read)
            .bind(reading.page_count)
            .bind(reading.read_at)
            .execute(&mut *tx)
            .await?;
        }
        for sitting in &taken.sittings {
            sqlx::query(
                "INSERT INTO brought_sitting (bringing_id, their_chapter, started_at, ended_at, \
                 first_page, last_page, measured) VALUES (?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(sitting.chapter)
            .bind(sitting.started_at)
            .bind(sitting.ended_at)
            .bind(sitting.first_page)
            .bind(sitting.last_page)
            .bind(sitting.measured)
            .execute(&mut *tx)
            .await?;
        }
        for made in &taken.lists {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_list \
                 (bringing_id, their_id, name, summary, theirs, shared) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(made.id)
            .bind(&made.name)
            .bind(&made.summary)
            .bind(made.theirs)
            .bind(made.shared)
            .execute(&mut *tx)
            .await?;
            for (position, chapter) in made.chapters.iter().enumerate() {
                sqlx::query(
                    "INSERT OR IGNORE INTO brought_list_book \
                     (bringing_id, their_list, position, their_chapter) VALUES (?, ?, ?, ?)",
                )
                .bind(id)
                .bind(made.id)
                .bind(position as i64)
                .bind(chapter)
                .execute(&mut *tx)
                .await?;
            }
        }
        for made in &taken.collections {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_collection \
                 (bringing_id, their_id, name, summary, theirs, shared) VALUES (?, ?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(made.id)
            .bind(&made.name)
            .bind(&made.summary)
            .bind(made.theirs)
            .bind(made.shared)
            .execute(&mut *tx)
            .await?;
            for series in &made.series {
                sqlx::query(
                    "INSERT OR IGNORE INTO brought_collection_series \
                     (bringing_id, their_collection, their_series) VALUES (?, ?, ?)",
                )
                .bind(id)
                .bind(made.id)
                .bind(series)
                .execute(&mut *tx)
                .await?;
            }
        }
        for page in &taken.pages {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_page (bringing_id, their_chapter, page) \
                 VALUES (?, ?, ?)",
            )
            .bind(id)
            .bind(page.chapter)
            .bind(page.page)
            .execute(&mut *tx)
            .await?;
        }
        for person in &taken.people {
            sqlx::query(
                "INSERT OR IGNORE INTO brought_person \
                 (bringing_id, their_id, name, manages, limited) VALUES (?, ?, ?, ?, ?)",
            )
            .bind(id)
            .bind(person.id)
            .bind(&person.name)
            .bind(person.manages)
            .bind(person.limited)
            .execute(&mut *tx)
            .await?;
            for library in &person.libraries {
                sqlx::query(
                    "INSERT OR IGNORE INTO brought_person_library \
                     (bringing_id, their_person, their_library) VALUES (?, ?, ?)",
                )
                .bind(id)
                .bind(person.id)
                .bind(library)
                .execute(&mut *tx)
                .await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// The files of a reading of another server, and how each that is a
    /// book here was told to be. Only the libraries that are known here are
    /// looked in, only where `user` reads, and only among the books whose
    /// pages were read: the rest have no identity for anything to hang on.
    async fn same_books(
        &self,
        user: UserId,
        bringing: i64,
    ) -> Result<(Vec<TheirFile>, HashMap<i64, FoundBy>), CatalogError> {
        let theirs: Vec<TheirFile> = sqlx::query_as(
            "SELECT bb.their_id, bb.their_chapter, bb.their_library, bl.library_id, bb.path, \
             bb.file_size, bb.page_count FROM brought_book bb \
             LEFT JOIN brought_library bl \
             ON bl.bringing_id = bb.bringing_id AND bl.their_id = bb.their_library \
             WHERE bb.bringing_id = ?",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        let mut libraries: Vec<i64> = theirs.iter().filter_map(|file| file.library_id).collect();
        libraries.sort_unstable();
        libraries.dedup();
        let query = format!(
            "SELECT b.library_id, b.path, b.file_size, b.page_count FROM book b \
             WHERE b.library_id IN ({}) AND b.content_hash IS NOT NULL AND {}",
            list(&libraries),
            may_read("b.library_id")
        );
        let ours: Vec<OurFile> = sqlx::query_as(&query)
            .persistent(false)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        let mut at: HashSet<(i64, String)> = HashSet::new();
        let mut so_named: HashMap<(i64, String, i64), Vec<&OurFile>> = HashMap::new();
        for file in &ours {
            let path = slashed(&file.path);
            let key = (file.library_id, named(&path), file.file_size);
            so_named.entry(key).or_default().push(file);
            at.insert((file.library_id, path));
        }
        let mut same = HashMap::new();
        for file in &theirs {
            let Some(library) = file.library_id else {
                continue;
            };
            if at.contains(&(library, file.path.clone())) {
                same.insert(file.their_id, FoundBy::Path);
                continue;
            }
            let key = (library, named(&file.path), file.file_size);
            // Of several with the name and the size, the one with as many
            // pages; none when that does not settle it.
            let found = match so_named.get(&key).map(Vec::as_slice) {
                Some([_]) => true,
                Some(several) => {
                    let fit = several
                        .iter()
                        .filter(|ours| ours.page_count == file.page_count);
                    fit.count() == 1
                }
                None => false,
            };
            if found {
                same.insert(file.their_id, FoundBy::File);
            }
        }
        Ok((theirs, same))
    }

    /// What `user` read from another server, and how much of it this server
    /// holds. Not found when they read none.
    pub async fn brought(&self, user: UserId) -> Result<Brought, CatalogError> {
        type Kept = (i64, String, String, String, String, bool, i64);
        let (bringing, source, address, version, their_name, manages, taken_at): Kept =
            sqlx::query_as(
                "SELECT id, source, address, version, their_name, manages, taken_at \
                 FROM bringing WHERE user_id = ?",
            )
            .bind(user)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)?;
        let (theirs, same) = self.same_books(user, bringing).await?;

        let rows: Vec<(i64, String, Option<i64>)> = sqlx::query_as(
            "SELECT their_id, name, library_id FROM brought_library \
             WHERE bringing_id = ? ORDER BY name COLLATE natsort, their_id",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        let folders: Vec<(i64, String)> = sqlx::query_as(
            "SELECT their_library, path FROM brought_folder WHERE bringing_id = ? ORDER BY path",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        let mut libraries: Vec<BroughtLibrary> = rows
            .into_iter()
            .map(|(their_id, name, library_id)| BroughtLibrary {
                their_id,
                name,
                library_id,
                folders: folders
                    .iter()
                    .filter(|(library, _)| *library == their_id)
                    .map(|(_, path)| path.clone())
                    .collect(),
                books: 0,
                by_path: 0,
                by_file: 0,
                missing: Vec::new(),
            })
            .collect();
        for file in &theirs {
            let Some(library) = libraries
                .iter_mut()
                .find(|library| library.their_id == file.their_library)
            else {
                continue;
            };
            library.books += 1;
            match same.get(&file.their_id) {
                Some(FoundBy::Path) => library.by_path += 1,
                Some(FoundBy::File) => library.by_file += 1,
                // Of a library that is none here, nothing is missing.
                None if library.library_id.is_some() => library.missing.push(file.path.clone()),
                None => {}
            }
        }
        for library in &mut libraries {
            library
                .missing
                .sort_by(|one, other| natural_cmp(one, other));
            library.missing.truncate(MOST_MISSING);
        }

        // A chapter is here when one of its files is.
        let here: HashSet<i64> = theirs
            .iter()
            .filter(|file| same.contains_key(&file.their_id))
            .map(|file| file.their_chapter)
            .collect();
        let read: Vec<(i64, i64, i64)> = sqlx::query_as(
            "SELECT their_chapter, pages_read, page_count FROM brought_reading \
             WHERE bringing_id = ? AND pages_read > 0",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        let mut reading = BroughtReading::default();
        for (chapter, pages_read, page_count) in read {
            if pages_read >= page_count {
                reading.finished += 1;
            } else {
                reading.started += 1;
            }
            if here.contains(&chapter) {
                reading.found += 1;
            }
        }
        (
            reading.timed,
            reading.seconds,
            reading.dated,
            reading.first_at,
            reading.last_at,
        ) = sqlx::query_as(
            // A chapter that server kept no stretch of, and knows the day
            // it was last read, was read that day.
            "SELECT COALESCE(SUM(measured), 0), \
             COALESCE(SUM(CASE WHEN measured THEN ended_at - started_at END), 0), \
             COALESCE(SUM(NOT measured), 0), MIN(started_at), MAX(started_at) FROM ( \
             SELECT measured, started_at, ended_at FROM brought_sitting WHERE bringing_id = ?1 \
             UNION ALL \
             SELECT 0, r.read_at, r.read_at FROM brought_reading r \
             WHERE r.bringing_id = ?1 AND r.pages_read > 0 AND r.read_at IS NOT NULL \
             AND NOT EXISTS (SELECT 1 FROM brought_sitting s \
             WHERE s.bringing_id = r.bringing_id AND s.their_chapter = r.their_chapter))",
        )
        .bind(bringing)
        .fetch_one(self.db.pool())
        .await?;

        let (lists, collections, wanted, pages): (i64, i64, i64, i64) = sqlx::query_as(
            "SELECT (SELECT COUNT(*) FROM brought_list WHERE bringing_id = ?1), \
             (SELECT COUNT(*) FROM brought_collection WHERE bringing_id = ?1), \
             (SELECT COUNT(*) FROM brought_series WHERE bringing_id = ?1 AND wanted), \
             (SELECT COUNT(*) FROM brought_page WHERE bringing_id = ?1)",
        )
        .bind(bringing)
        .fetch_one(self.db.pool())
        .await?;
        let people: Vec<(i64, String, bool, bool)> = sqlx::query_as(
            "SELECT their_id, name, manages, limited FROM brought_person \
             WHERE bringing_id = ? ORDER BY name COLLATE natsort, their_id",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        let reads: Vec<(i64, String)> = sqlx::query_as(
            "SELECT pl.their_person, bl.name FROM brought_person_library pl \
             JOIN brought_library bl \
             ON bl.bringing_id = pl.bringing_id AND bl.their_id = pl.their_library \
             WHERE pl.bringing_id = ? ORDER BY bl.name COLLATE natsort",
        )
        .bind(bringing)
        .fetch_all(self.db.pool())
        .await?;
        Ok(Brought {
            source,
            address,
            version,
            their_name,
            manages,
            taken_at,
            libraries,
            reading,
            lists,
            collections,
            wanted,
            pages,
            people: people
                .into_iter()
                .map(|(id, name, manages, limited)| BroughtPerson {
                    name,
                    manages,
                    limited,
                    libraries: reads
                        .iter()
                        .filter(|(person, _)| *person == id)
                        .map(|(_, library)| library.clone())
                        .collect(),
                })
                .collect(),
        })
    }

    /// Says which library here holds the folder of a library `user` read
    /// from another server, or with nothing, that none does. The library
    /// here is one they read.
    pub async fn say_brought_library(
        &self,
        user: UserId,
        their_id: i64,
        library_id: Option<i64>,
    ) -> Result<(), CatalogError> {
        if let Some(library_id) = library_id {
            match self.library(user, library_id).await {
                Err(CatalogError::NotFound) => return Err(CatalogError::Invalid(NO_SUCH_LIBRARY)),
                found => found?,
            };
        }
        let said = sqlx::query(
            "UPDATE brought_library SET library_id = ?, said = 1 WHERE their_id = ? \
             AND bringing_id = (SELECT id FROM bringing WHERE user_id = ?)",
        )
        .bind(library_id)
        .bind(their_id)
        .bind(user)
        .execute(self.db.pool())
        .await?;
        if said.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// Forgets what `user` read from another server. What was already made
    /// of it here stays.
    pub async fn forget_brought(&self, user: UserId) -> Result<(), CatalogError> {
        let forgotten = sqlx::query("DELETE FROM bringing WHERE user_id = ?")
            .bind(user)
            .execute(self.db.pool())
            .await?;
        if forgotten.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }
}

/// What is said of a library that is not one the person reads here.
const NO_SUCH_LIBRARY: &str = "That library is not on this server.";

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scan::tests::shelf;

    #[tokio::test]
    async fn a_chapter_with_only_its_day_was_read_that_day() {
        let shelf = shelf().await;
        let read = |chapter, read_at| TakenReading {
            chapter,
            pages_read: 1,
            page_count: 2,
            read_at,
        };
        let taken = Taken {
            source: "kavita".to_owned(),
            address: "http://there".to_owned(),
            // One the other server timed, one it knows the day of, and one
            // it knows nothing of but where it was left.
            reading: vec![read(1, Some(1_000)), read(2, Some(90_000)), read(3, None)],
            sittings: vec![TakenSitting {
                chapter: 1,
                started_at: 400,
                ended_at: 1_000,
                first_page: 0,
                last_page: 0,
                measured: true,
            }],
            ..Taken::default()
        };
        let catalog = &shelf.catalog;
        catalog.keep_brought(shelf.owner, &taken).await.unwrap();
        let reading = catalog.brought(shelf.owner).await.unwrap().reading;
        assert_eq!((reading.timed, reading.seconds, reading.dated), (1, 600, 1));
        assert_eq!(
            (reading.first_at, reading.last_at),
            (Some(400), Some(90_000))
        );
    }

    #[test]
    fn a_file_is_placed_under_its_librarys_folder() {
        let folders = ["/data/comics".to_owned(), "/data/comics/dc".to_owned()];
        // The longest folder that holds it is the one it is under.
        assert_eq!(
            under("/data/comics/dc/Night/N 001.cbz", &folders),
            "Night/N 001.cbz"
        );
        assert_eq!(
            under("/data/comics/Night/N 001.cbz", &folders),
            "Night/N 001.cbz"
        );
        // A folder that only starts the same holds nothing of it.
        assert_eq!(
            under("/data/comics2/N 001.cbz", &folders),
            "/data/comics2/N 001.cbz"
        );
        // Windows writes its paths the other way.
        let windows = ["C:\\Comics\\".to_owned()];
        assert_eq!(
            under("C:\\Comics\\Night\\N 001.cbz", &windows),
            "Night/N 001.cbz"
        );
        assert_eq!(named("Night/N 001.CBZ"), "n 001.cbz");
    }
}
