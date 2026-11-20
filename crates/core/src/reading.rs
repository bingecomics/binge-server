//! Reading: one book, its pages, how far it has been read, and the lists
//! that start from there.
//!
//! Progress and page images hang on the book's identity, not on its row or
//! its path, so a moved or renamed file keeps both. Progress is each
//! reader's own, and everything here is answered for one user: a book in a
//! library they may not read is not there for them.

use std::io;
use std::path::Path;

use sqlx::SqliteConnection;

use crate::accounts::UserId;
use crate::archive::{self, Archive, ArchiveError};
use crate::catalog::{
    Book, Catalog, CatalogError, PLACED, Series, books, first_copy, may_read, now, series,
};
use crate::overview::up_next;
use crate::thumbnail;

/// One page of a book, as the scan recorded it.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct Page {
    /// Place in reading order, from 0.
    pub number: i64,
    /// Size of the stored image in bytes.
    pub byte_size: i64,
    /// Missing until the background pass of a scan has read it, and for an
    /// image the server cannot understand.
    pub width: Option<i64>,
    pub height: Option<i64>,
}

/// A page image exactly as the archive stores it.
#[derive(Clone, Debug)]
pub struct PageImage {
    pub bytes: Vec<u8>,
    pub media_type: &'static str,
}

/// How far a reader has read a book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Progress {
    /// The last page shown, from 0.
    pub page: i64,
    /// Whether that was the book's last page.
    pub completed: bool,
    /// Seconds since 1970.
    pub updated_at: i64,
}

/// Some books, named by what holds them: these books, and every book of
/// these groups, these series and these reading lists. What a mark is put
/// on, and what is added to a reading list.
#[derive(Clone, Debug, Default)]
pub struct BooksToMark {
    pub books: Vec<i64>,
    pub groups: Vec<i64>,
    pub series: Vec<i64>,
    pub lists: Vec<i64>,
}

impl BooksToMark {
    /// How many books, groups, series and lists are named.
    pub(crate) fn named(&self) -> usize {
        self.books.len() + self.groups.len() + self.series.len() + self.lists.len()
    }
}

/// Where a copy of a book is on disk.
#[derive(sqlx::FromRow)]
struct Copy {
    root_path: String,
    path: String,
}

impl Catalog {
    pub async fn book(&self, user: UserId, id: i64) -> Result<Book, CatalogError> {
        let query = books("AND b.id = ?2");
        sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NotFound)
    }

    /// The pages of a book in reading order. Empty for a file that could not
    /// be read.
    pub async fn pages(&self, user: UserId, book_id: i64) -> Result<Vec<Page>, CatalogError> {
        let query = format!(
            "SELECT pg.number, pg.byte_size, pg.width, pg.height \
             FROM page pg JOIN book b ON b.id = pg.book_id \
             WHERE pg.book_id = ?2 AND {} ORDER BY pg.number",
            may_read("b.library_id")
        );
        let pages: Vec<Page> = sqlx::query_as(&query)
            .bind(user)
            .bind(book_id)
            .fetch_all(self.db.pool())
            .await?;
        if pages.is_empty() {
            self.book(user, book_id).await?;
        }
        Ok(pages)
    }

    /// Reads a page of the book with this identity out of its archive, from
    /// a library `user` may read.
    ///
    /// The archive is checked against the identity before anything is read
    /// from it, so what comes back under an identity and a number is always
    /// the same image, whatever happened to the file since the last scan.
    pub async fn page_image(
        &self,
        user: UserId,
        content_hash: &str,
        number: i64,
    ) -> Result<PageImage, CatalogError> {
        let number = usize::try_from(number).map_err(|_| CatalogError::NoSuchPage)?;
        // Usually one file. More when the same pages are in the catalog
        // twice; then any copy that can be read will do.
        let query = format!(
            "SELECT l.root_path, b.path FROM book b JOIN library l ON l.id = b.library_id \
             WHERE b.content_hash = ?2 AND {} ORDER BY b.id",
            may_read("b.library_id")
        );
        let copies: Vec<Copy> = sqlx::query_as(&query)
            .bind(user)
            .bind(content_hash)
            .fetch_all(self.db.pool())
            .await?;

        let identity = content_hash.to_owned();
        tokio::task::spawn_blocking(move || {
            let mut failure = None;
            for copy in &copies {
                let file = Path::new(&copy.root_path).join(&copy.path);
                match read_page(&file, &identity, number) {
                    Ok(image) => return Ok(image),
                    // The file is fine and has no such page; neither has any
                    // other copy, and nothing is wrong on the server.
                    Err(CatalogError::NoSuchPage) => return Err(CatalogError::NoSuchPage),
                    Err(error) => {
                        tracing::warn!(file = %file.display(), %error, "page not read");
                        // What is wrong with the first copy is what to say.
                        failure.get_or_insert(error);
                    }
                }
            }
            Err(failure.unwrap_or(CatalogError::NoSuchPage))
        })
        .await?
    }

    /// A page of the book with this identity as a small picture, for a
    /// strip of pages and for a card of one. Read as [`Catalog::page_image`]
    /// reads it, then scaled down.
    pub async fn small_page(
        &self,
        user: UserId,
        content_hash: &str,
        number: i64,
    ) -> Result<PageImage, CatalogError> {
        let page = self.page_image(user, content_hash, number).await?;
        tokio::task::spawn_blocking(move || {
            let bytes = thumbnail::small_page(&page.bytes).map_err(|error| {
                tracing::warn!(%error, "page not made small");
                CatalogError::Invalid("The server cannot make a small picture of this page.")
            })?;
            Ok(PageImage {
                bytes,
                media_type: "image/jpeg",
            })
        })
        .await?
    }

    /// Whether a library `user` may read holds a book with this identity
    /// that has a page of this number. Says nothing about the file being
    /// there.
    pub async fn has_page(
        &self,
        user: UserId,
        content_hash: &str,
        number: i64,
    ) -> Result<bool, CatalogError> {
        let query = format!(
            "SELECT 1 FROM book b JOIN page p ON p.book_id = b.id AND p.number = ?2 \
             WHERE b.content_hash = ?3 AND {} LIMIT 1",
            may_read("b.library_id")
        );
        let found: Option<i64> = sqlx::query_scalar(&query)
            .bind(user)
            .bind(number)
            .bind(content_hash)
            .fetch_optional(self.db.pool())
            .await?;
        Ok(found.is_some())
    }

    /// Records that `user` read the book up to `page`. Reaching the last
    /// page completes the book; reading it again from an earlier page takes
    /// that back.
    pub async fn save_progress(
        &self,
        user: UserId,
        book_id: i64,
        page: i64,
    ) -> Result<Progress, CatalogError> {
        let book = self.book(user, book_id).await?;
        // A file that could not be read has no pages and no identity.
        let Some(identity) = book.content_hash else {
            return Err(CatalogError::InvalidPage);
        };
        if !(0..book.page_count).contains(&page) {
            return Err(CatalogError::InvalidPage);
        }
        let progress = Progress {
            page,
            completed: page == book.page_count - 1,
            updated_at: now(),
        };
        // The progress and the sitting it goes on are written together.
        let mut saving = self.db.pool().begin().await?;
        sqlx::query(
            "INSERT INTO progress (user_id, content_hash, page, completed, updated_at) \
             VALUES (?, ?, ?, ?, ?) \
             ON CONFLICT (user_id, content_hash) DO UPDATE SET page = excluded.page, \
             completed = excluded.completed, updated_at = excluded.updated_at",
        )
        .bind(user)
        .bind(&identity)
        .bind(progress.page)
        .bind(progress.completed)
        .bind(progress.updated_at)
        .execute(&mut *saving)
        .await?;
        record_sitting(&mut saving, user, &identity, &progress).await?;
        saving.commit().await?;
        Ok(progress)
    }

    /// Takes a series off the list of [`Catalog::up_next`] for `user`, until
    /// they read from it again.
    pub async fn take_off_up_next(&self, user: UserId, series_id: i64) -> Result<(), CatalogError> {
        let query = format!(
            "INSERT INTO off_up_next (user_id, series_id, taken_off_at) \
             SELECT ?1, s.id, ?3 FROM series s WHERE s.id = ?2 AND {} \
             ON CONFLICT (user_id, series_id) DO UPDATE SET taken_off_at = excluded.taken_off_at",
            may_read("s.library_id")
        );
        let taken_off = sqlx::query(&query)
            .bind(user)
            .bind(series_id)
            .bind(now())
            .execute(self.db.pool())
            .await?;
        if taken_off.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// Marks books as read to the end by `user`, or as never opened, and
    /// returns how many that changed. A book already read stays as it was,
    /// with the time it was read.
    ///
    /// What is not there, or is in a library `user` may not read, is passed
    /// over, and so are files that could not be read. The mark hangs on a
    /// book's identity, like all progress: every copy of the book has it.
    ///
    /// More than 10,000 books, groups, series and lists named at once are
    /// refused.
    pub async fn mark_read(
        &self,
        user: UserId,
        which: &BooksToMark,
        read: bool,
    ) -> Result<u64, CatalogError> {
        if which.named() > MOST_NAMED {
            return Err(CatalogError::Invalid(TOO_MANY_NAMED));
        }
        // A book of a list is there by its identity, not by where it sits.
        let in_lists = "reading_list_book lb JOIN book b ON b.content_hash = lb.content_hash";
        let chosen = [
            (PLACED, "b.id", &which.books),
            (PLACED, "bp.group_id", &which.groups),
            (PLACED, "bp.series_id", &which.series),
            (in_lists, "lb.list_id", &which.lists),
        ]
        .into_iter()
        .filter(|(_, _, ids)| !ids.is_empty())
        .map(|(from, column, ids)| {
            format!(
                "SELECT b.content_hash, b.page_count FROM {from} WHERE {column} IN ({}) AND {}",
                list(ids),
                may_read("b.library_id")
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
        if chosen.is_empty() {
            return Ok(0);
        }
        // The ids are part of the statement, so it is not one to keep for
        // the next time.
        let marked = if read {
            // `WHERE true` tells SQLite that `ON` is not a join's.
            let query = format!(
                "INSERT INTO progress (user_id, content_hash, page, completed, updated_at) \
                 SELECT ?1, content_hash, MAX(page_count) - 1, 1, ?2 FROM ({chosen}) \
                 WHERE true GROUP BY content_hash \
                 ON CONFLICT (user_id, content_hash) DO UPDATE SET page = excluded.page, \
                 completed = 1, updated_at = excluded.updated_at WHERE NOT progress.completed"
            );
            sqlx::query(&query)
                .persistent(false)
                .bind(user)
                .bind(now())
                .execute(self.db.pool())
                .await?
        } else {
            let query = format!(
                "DELETE FROM progress WHERE user_id = ?1 AND content_hash IN \
                 (SELECT content_hash FROM ({chosen}))"
            );
            sqlx::query(&query)
                .persistent(false)
                .bind(user)
                .execute(self.db.pool())
                .await?
        };
        Ok(marked.rows_affected())
    }

    /// Books `user` started and did not finish, the one read last first.
    /// The same pages in the catalog twice are listed once.
    pub async fn continue_reading(
        &self,
        user: UserId,
        limit: u32,
    ) -> Result<Vec<Book>, CatalogError> {
        let query = books(&format!(
            "AND p.completed = 0 \
             AND b.id = {} \
             ORDER BY p.updated_at DESC, b.id DESC LIMIT ?2",
            first_copy("b")
        ));
        Ok(sqlx::query_as(&query)
            .bind(user)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?)
    }

    /// The readable books the server saw last in the libraries `user` may
    /// read, newest first.
    pub async fn recently_added(
        &self,
        user: UserId,
        limit: u32,
    ) -> Result<Vec<Book>, CatalogError> {
        let query =
            books("AND b.content_hash IS NOT NULL ORDER BY b.added_at DESC, b.id DESC LIMIT ?2");
        Ok(sqlx::query_as(&query)
            .bind(user)
            .bind(limit)
            .fetch_all(self.db.pool())
            .await?)
    }

    /// For each series `user` finished a book of and opened none since, the
    /// book that follows: see [`up_next`]. The series read last comes first.
    ///
    /// A series is on the list while it is fresh: read from in the last
    /// [`READ_LATELY`] seconds, or given a book in the last [`ADDED_LATELY`],
    /// which brings back a series whose reader was waiting for more. One
    /// taken off with [`Catalog::take_off_up_next`] stays off until its
    /// reader reads from it again.
    pub async fn up_next(&self, user: UserId, limit: u32) -> Result<Vec<Book>, CatalogError> {
        let now = now();
        // The series the user read from, the one read last first.
        let query = format!(
            "SELECT s.id FROM {PLACED} JOIN series s ON s.id = bp.series_id \
             JOIN progress p ON p.content_hash = bp.content_hash AND p.user_id = ?1 \
             LEFT JOIN off_up_next off ON off.user_id = ?1 AND off.series_id = s.id \
             WHERE {} \
             GROUP BY s.id \
             HAVING (MAX(p.updated_at) >= ?2 \
             OR (SELECT MAX(b.added_at) FROM {PLACED} WHERE bp.series_id = s.id) >= ?3) \
             AND (off.taken_off_at IS NULL OR off.taken_off_at < MAX(p.updated_at)) \
             ORDER BY MAX(p.updated_at) DESC, s.id",
            may_read("s.library_id")
        );
        let read_from: Vec<i64> = sqlx::query_scalar(&query)
            .bind(user)
            .bind(now - READ_LATELY)
            .bind(now - ADDED_LATELY)
            .fetch_all(self.db.pool())
            .await?;

        let mut offered: Vec<Book> = Vec::new();
        for series_id in read_from {
            if offered.len() == limit as usize {
                break;
            }
            let books = self.books_in(user, series_id).await?;
            // Two libraries that hold the same pages would each offer them.
            if let Some(book) = up_next(&books)
                && !offered
                    .iter()
                    .any(|other| other.content_hash == book.content_hash)
            {
                offered.push(book.clone());
            }
        }
        Ok(offered)
    }

    /// The series of the libraries `user` may read that hold books, the one
    /// given a book last first.
    pub async fn recently_updated(
        &self,
        user: UserId,
        limit: u32,
    ) -> Result<Vec<Series>, CatalogError> {
        let limit = limit as usize;
        // The newest books lead to their series. They are read a stretch at
        // a time, leaving out the series already found, so that a library
        // of any size costs a few stretches and not all of its books.
        let mut newest: Vec<i64> = Vec::new();
        while newest.len() < limit {
            let query = format!(
                "SELECT bp.series_id FROM {PLACED} \
                 WHERE {} AND bp.series_id NOT IN ({}) \
                 ORDER BY b.added_at DESC, b.id DESC LIMIT {STRETCH}",
                may_read("bp.library_id"),
                list(&newest)
            );
            let stretch: Vec<i64> = sqlx::query_scalar(&query)
                .bind(user)
                .fetch_all(self.db.pool())
                .await?;
            for id in &stretch {
                if newest.len() < limit && !newest.contains(id) {
                    newest.push(*id);
                }
            }
            if stretch.len() < STRETCH {
                break;
            }
        }

        let query = series(&format!("AND s.id IN ({})", list(&newest)));
        let mut found: Vec<Series> = sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        found.sort_by_key(|series| newest.iter().position(|id| *id == series.id));
        Ok(found)
    }
}

/// How many of the newest books [`Catalog::recently_updated`] reads at a
/// time. The tests read two, so that a few books make several stretches.
const STRETCH: usize = if cfg!(test) { 2 } else { 200 };

/// Ids as the inside of an SQL list.
pub(crate) fn list(ids: &[i64]) -> String {
    let ids: Vec<String> = ids.iter().map(i64::to_string).collect();
    ids.join(", ")
}

/// Keeps the record of when `user` read: the page just saved goes on the
/// sitting with the book that is still going on, or starts one when the
/// page before it was saved more than [`PAUSE`] seconds ago.
async fn record_sitting(
    saving: &mut SqliteConnection,
    user: UserId,
    identity: &str,
    saved: &Progress,
) -> Result<(), CatalogError> {
    let going_on = sqlx::query(
        "UPDATE sitting SET ended_at = ?1, last_page = ?2 WHERE id = \
         (SELECT id FROM sitting WHERE user_id = ?3 AND content_hash = ?4 AND ended_at >= ?5 \
          ORDER BY ended_at DESC LIMIT 1)",
    )
    .bind(saved.updated_at)
    .bind(saved.page)
    .bind(user)
    .bind(identity)
    .bind(saved.updated_at - PAUSE)
    .execute(&mut *saving)
    .await?;
    if going_on.rows_affected() == 0 {
        sqlx::query(
            "INSERT INTO sitting \
             (user_id, content_hash, started_at, ended_at, first_page, last_page) \
             VALUES (?1, ?2, ?3, ?3, ?4, ?4)",
        )
        .bind(user)
        .bind(identity)
        .bind(saved.updated_at)
        .bind(saved.page)
        .execute(&mut *saving)
        .await?;
    }
    Ok(())
}

/// How long after a page was saved the next page still belongs to the same
/// sitting, in seconds: half an hour.
pub(crate) const PAUSE: i64 = 30 * 60;

/// How many books, groups, series and lists one request may name: the ids
/// go into a statement, which has to stay a size the database takes in its
/// stride.
pub(crate) const MOST_NAMED: usize = 10_000;

/// What a request that names more is answered with.
pub(crate) const TOO_MANY_NAMED: &str =
    "That is too many at once: name 10,000 books, groups, series and reading lists at most.";

/// How long after its reader last read from it a series stays on the list
/// of [`Catalog::up_next`], in seconds: 30 days.
const READ_LATELY: i64 = 30 * 24 * 60 * 60;

/// How long a book added to a series keeps it on the list of
/// [`Catalog::up_next`], in seconds: 7 days.
const ADDED_LATELY: i64 = 7 * 24 * 60 * 60;

/// Reads page `number` from the archive at `file`, if the archive still
/// holds the pages that `content_hash` names.
fn read_page(file: &Path, content_hash: &str, number: usize) -> Result<PageImage, CatalogError> {
    let shown = || file.display().to_string();
    let mut archive = Archive::open(file).map_err(|error| match error {
        ArchiveError::Io(error) if error.kind() == io::ErrorKind::NotFound => {
            CatalogError::FileMissing(shown())
        }
        other => other.into(),
    })?;
    // From the archive's index, which opening it has already read.
    let contents = archive.contents()?;
    if contents.content_hash != content_hash {
        return Err(CatalogError::FileChanged(shown()));
    }
    let page = contents.pages.get(number).ok_or(CatalogError::NoSuchPage)?;
    Ok(PageImage {
        bytes: archive.read(&page.name)?,
        // A page is only ever a file whose name gives a media type.
        media_type: archive::media_type(&page.name).unwrap_or("application/octet-stream"),
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::accounts;
    use crate::db::Database;
    use crate::fixtures;

    const NOBODY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    struct Shelf {
        _dir: tempfile::TempDir,
        comics: PathBuf,
        catalog: Catalog,
        library_id: i64,
        /// Who reads in these tests, unless a test says otherwise.
        owner: UserId,
    }

    /// A catalog over a `Comics` folder holding the given books, each as
    /// (path, pages, seed), scanned.
    async fn shelf(books: &[(&str, u8, u8)]) -> Shelf {
        let dir = tempfile::tempdir().unwrap();
        let data = dir.path().join("data");
        let comics = dir.path().join("Comics");
        std::fs::create_dir_all(&data).unwrap();
        std::fs::create_dir_all(&comics).unwrap();
        for (path, pages, seed) in books {
            fixtures::write_book(&comics.join(path), *pages, *seed);
        }
        let db = Database::open(&data.join("binge.db")).await.unwrap();
        let owner = accounts::testing::owner(&db).await;
        let catalog = Catalog::new(db, &data);
        let library = catalog.add_library(owner, &comics, None).await.unwrap();
        catalog.scan_finished(library.id).await;
        Shelf {
            comics,
            catalog,
            library_id: library.id,
            owner,
            _dir: dir,
        }
    }

    impl Shelf {
        async fn rescan(&self) {
            self.catalog.start_scan(self.library_id);
            self.catalog.scan_finished(self.library_id).await;
        }

        /// Records how far the owner read a book.
        async fn save(&self, book_id: i64, page: i64) -> Result<Progress, CatalogError> {
            self.catalog.save_progress(self.owner, book_id, page).await
        }

        /// A page image, asked for by the owner.
        async fn image(&self, identity: &str, number: i64) -> Result<PageImage, CatalogError> {
            self.catalog.page_image(self.owner, identity, number).await
        }

        /// The book of that name, wherever it is in the library.
        async fn book(&self, name: &str) -> Book {
            let query = books("AND b.name = ?2");
            sqlx::query_as(&query)
                .bind(self.owner)
                .bind(name)
                .fetch_one(self.catalog.db.pool())
                .await
                .unwrap()
        }

        /// Makes every book look as if it was added `seconds` earlier.
        async fn age_books(&self, seconds: i64) {
            sqlx::query("UPDATE book SET added_at = added_at - ?")
                .bind(seconds)
                .execute(self.catalog.db.pool())
                .await
                .unwrap();
        }

        /// Makes the book's progress look `seconds` older than it is.
        async fn age_progress(&self, book: &Book, seconds: i64) {
            sqlx::query("UPDATE progress SET updated_at = updated_at - ? WHERE content_hash = ?")
                .bind(seconds)
                .bind(&book.content_hash)
                .execute(self.catalog.db.pool())
                .await
                .unwrap();
        }
    }

    fn names(books: &[Book]) -> Vec<&str> {
        books.iter().map(|book| book.name.as_str()).collect()
    }

    #[tokio::test]
    async fn a_book_lists_its_pages_with_their_sizes() {
        let shelf = shelf(&[("Saga/Saga 1.cbz", 3, 1)]).await;
        let book = shelf.book("Saga 1").await;
        assert_eq!(
            shelf.catalog.book(shelf.owner, book.id).await.unwrap().name,
            "Saga 1"
        );
        assert_eq!(book.library_id, shelf.library_id);
        assert!(book.progress.is_none());

        let pages = shelf.catalog.pages(shelf.owner, book.id).await.unwrap();
        assert_eq!(pages.len(), 3);
        assert_eq!(
            pages.iter().map(|page| page.number).collect::<Vec<_>>(),
            [0, 1, 2]
        );
        assert_eq!((pages[0].width, pages[0].height), (Some(40), Some(60)));
        assert!(pages[0].byte_size > 0);

        assert!(matches!(
            shelf.catalog.book(shelf.owner, book.id + 1).await,
            Err(CatalogError::NotFound)
        ));
        assert!(matches!(
            shelf.catalog.pages(shelf.owner, book.id + 1).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn a_page_comes_back_as_the_archive_stores_it() {
        let shelf = shelf(&[]).await;
        let (first, second) = (fixtures::page(40, 60, 1), fixtures::page(40, 60, 2));
        fixtures::write_cbz(
            &shelf.comics.join("Book.cbz"),
            &[
                ("2.png", second.clone()),
                ("ComicInfo.xml", b"<ComicInfo/>".to_vec()),
                ("1.png", first.clone()),
            ],
        );
        shelf.rescan().await;
        let catalog = &shelf.catalog;
        let book = shelf.book("Book").await;
        let identity = book.content_hash.as_deref().unwrap();

        let image = catalog.page_image(shelf.owner, identity, 0).await.unwrap();
        assert_eq!(image.bytes, first);
        assert_eq!(image.media_type, "image/png");
        assert_eq!(
            catalog
                .page_image(shelf.owner, identity, 1)
                .await
                .unwrap()
                .bytes,
            second
        );

        for missing in [2, -1] {
            assert!(matches!(
                catalog.page_image(shelf.owner, identity, missing).await,
                Err(CatalogError::NoSuchPage)
            ));
        }
        assert!(matches!(
            catalog.page_image(shelf.owner, NOBODY, 0).await,
            Err(CatalogError::NoSuchPage)
        ));

        assert!(catalog.has_page(shelf.owner, identity, 1).await.unwrap());
        assert!(!catalog.has_page(shelf.owner, identity, 2).await.unwrap());
        assert!(!catalog.has_page(shelf.owner, NOBODY, 0).await.unwrap());
    }

    #[tokio::test]
    async fn a_page_is_only_read_from_the_file_that_was_scanned() {
        let shelf = shelf(&[("Book.cbz", 2, 1)]).await;
        let book = shelf.book("Book").await;
        let identity = book.content_hash.as_deref().unwrap();
        let file = shelf.comics.join("Book.cbz");

        // Other pages under the same name: not this book any more.
        fixtures::write_book(&file, 2, 9);
        assert!(matches!(
            shelf.image(identity, 0).await,
            Err(CatalogError::FileChanged(_))
        ));

        std::fs::remove_file(&file).unwrap();
        assert!(matches!(
            shelf.image(identity, 0).await,
            Err(CatalogError::FileMissing(_))
        ));
    }

    #[tokio::test]
    async fn a_retagged_file_gives_its_pages_without_a_rescan() {
        let shelf = shelf(&[("Book.cbz", 2, 1)]).await;
        let book = shelf.book("Book").await;
        let identity = book.content_hash.as_deref().unwrap();
        let first = shelf.image(identity, 0).await.unwrap().bytes;
        let second = shelf.image(identity, 1).await.unwrap().bytes;

        // The same pages under other names, with metadata added.
        fixtures::write_cbz(
            &shelf.comics.join("Book.cbz"),
            &[
                ("ComicInfo.xml", b"<ComicInfo/>".to_vec()),
                ("a.png", first),
                ("b.png", second.clone()),
            ],
        );
        let image = shelf.image(identity, 1).await.unwrap();
        assert_eq!(image.bytes, second);
    }

    #[tokio::test]
    async fn a_page_is_read_from_any_copy_of_the_book() {
        let shelf = shelf(&[("Book.cbz", 2, 1), ("Copy/Book copy.cbz", 2, 1)]).await;
        let book = shelf.book("Book").await;
        let identity = book.content_hash.as_deref().unwrap();
        let expected = shelf.image(identity, 1).await.unwrap().bytes;

        std::fs::remove_file(shelf.comics.join("Book.cbz")).unwrap();
        let image = shelf.image(identity, 1).await.unwrap();
        assert_eq!(image.bytes, expected);
    }

    #[tokio::test]
    async fn progress_is_saved_and_completes_on_the_last_page() {
        // The second file has no pages, so it is listed but cannot be read.
        let shelf = shelf(&[("Book.cbz", 3, 1), ("Broken.cbz", 0, 0)]).await;
        let book = shelf.book("Book").await;

        let saved = shelf.save(book.id, 1).await.unwrap();
        assert_eq!((saved.page, saved.completed), (1, false));
        assert_eq!(shelf.book("Book").await.progress, Some(saved));

        let done = shelf.save(book.id, 2).await.unwrap();
        assert!(done.completed);
        // Reading it again from the start takes the completion back.
        let again = shelf.save(book.id, 0).await.unwrap();
        assert!(!again.completed);

        for outside in [-1, 3] {
            assert!(matches!(
                shelf.save(book.id, outside).await,
                Err(CatalogError::InvalidPage)
            ));
        }
        let broken = shelf.book("Broken").await;
        assert!(matches!(
            shelf.save(broken.id, 0).await,
            Err(CatalogError::InvalidPage)
        ));
        assert!(matches!(
            shelf.save(book.id + 100, 0).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn progress_follows_the_book_when_its_file_moves() {
        let shelf = shelf(&[("Saga 1.cbz", 4, 1)]).await;
        let before = shelf.book("Saga 1").await;
        shelf.save(before.id, 2).await.unwrap();

        std::fs::create_dir_all(shelf.comics.join("Saga")).unwrap();
        std::fs::rename(
            shelf.comics.join("Saga 1.cbz"),
            shelf.comics.join("Saga/Saga 001.cbz"),
        )
        .unwrap();
        shelf.rescan().await;

        let after = shelf.book("Saga 001").await;
        assert_ne!(after.id, before.id);
        assert_eq!(after.progress.map(|progress| progress.page), Some(2));
    }

    #[tokio::test]
    async fn progress_outlives_the_library() {
        let shelf = shelf(&[("Book.cbz", 4, 1)]).await;
        let book = shelf.book("Book").await;
        shelf.save(book.id, 2).await.unwrap();

        shelf
            .catalog
            .remove_library(shelf.library_id)
            .await
            .unwrap();
        let library = shelf
            .catalog
            .add_library(shelf.owner, &shelf.comics, None)
            .await
            .unwrap();
        shelf.catalog.scan_finished(library.id).await;

        let back = shelf.book("Book").await;
        assert_eq!(back.progress.map(|progress| progress.page), Some(2));
    }

    #[tokio::test]
    async fn progress_is_each_readers_own() {
        let shelf = shelf(&[("Book.cbz", 4, 1)]).await;
        let catalog = &shelf.catalog;
        let ben = accounts::testing::member(&catalog.db, "Ben", &[shelf.library_id]).await;
        let book = shelf.book("Book").await;

        catalog
            .save_progress(shelf.owner, book.id, 1)
            .await
            .unwrap();
        // Ben sees the book, and has not opened it.
        let bens = catalog.book(ben, book.id).await.unwrap();
        assert!(bens.progress.is_none());
        assert!(catalog.continue_reading(ben, 10).await.unwrap().is_empty());

        catalog.save_progress(ben, book.id, 3).await.unwrap();
        let bens = catalog.book(ben, book.id).await.unwrap();
        assert_eq!(
            bens.progress.map(|p| (p.page, p.completed)),
            Some((3, true))
        );
        let owners = catalog.book(shelf.owner, book.id).await.unwrap();
        assert_eq!(
            owners.progress.map(|p| (p.page, p.completed)),
            Some((1, false))
        );
        assert_eq!(
            names(&catalog.continue_reading(shelf.owner, 10).await.unwrap()),
            ["Book"]
        );
        assert!(catalog.continue_reading(ben, 10).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_library_someone_may_not_read_is_not_there_for_them() {
        let shelf = shelf(&[("Saga/Saga 1.cbz", 2, 1), ("Loose.cbz", 2, 2)]).await;
        let catalog = &shelf.catalog;
        let ben = accounts::testing::member(&catalog.db, "Ben", &[]).await;
        let book = shelf.book("Saga 1").await;
        let identity = book.content_hash.as_deref().unwrap();
        let series = book.series_id.unwrap();
        catalog
            .save_progress(shelf.owner, book.id, 0)
            .await
            .unwrap();

        assert!(catalog.libraries(ben).await.unwrap().is_empty());
        assert!(catalog.recently_added(ben, 10).await.unwrap().is_empty());
        assert!(catalog.continue_reading(ben, 10).await.unwrap().is_empty());
        assert!(catalog.recently_updated(ben, 10).await.unwrap().is_empty());
        let not_found = [
            catalog.library(ben, shelf.library_id).await.err(),
            catalog.series_in(ben, shelf.library_id).await.err(),
            catalog.series(ben, series).await.err(),
            catalog.books_in(ben, series).await.err(),
            catalog.book(ben, book.id).await.err(),
            catalog.pages(ben, book.id).await.err(),
            catalog.save_progress(ben, book.id, 1).await.err(),
        ];
        for (number, error) in not_found.iter().enumerate() {
            assert!(
                matches!(error, Some(CatalogError::NotFound)),
                "{number}: {error:?}"
            );
        }
        assert!(matches!(
            catalog.page_image(ben, identity, 0).await,
            Err(CatalogError::NoSuchPage)
        ));
        assert!(!catalog.has_page(ben, identity, 0).await.unwrap());
        assert!(catalog.cover_path(ben, identity).await.unwrap().is_none());
        assert!(
            catalog
                .cover_path(shelf.owner, identity)
                .await
                .unwrap()
                .is_some()
        );

        // Someone who does not exist reads nothing either.
        let nobody = UserId(99);
        assert!(catalog.libraries(nobody).await.unwrap().is_empty());
        assert!(matches!(
            catalog.book(nobody, book.id).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn the_same_book_is_read_from_a_library_the_reader_may_read() {
        let shelf = shelf(&[("Book.cbz", 2, 1)]).await;
        let catalog = &shelf.catalog;
        let owner = shelf.owner;
        // The same pages in a second library, the only one Ben may read.
        let lent = shelf.comics.parent().unwrap().join("Lent");
        fixtures::write_book(&lent.join("Lent book.cbz"), 2, 1);
        let lent = catalog.add_library(owner, &lent, None).await.unwrap();
        catalog.scan_finished(lent.id).await;
        let ben = accounts::testing::member(&catalog.db, "Ben", &[lent.id]).await;
        let (first, copy) = (shelf.book("Book").await, shelf.book("Lent book").await);
        assert_eq!(first.content_hash, copy.content_hash);
        let identity = copy.content_hash.as_deref().unwrap();

        let his: Vec<_> = catalog.libraries(ben).await.unwrap();
        assert_eq!(his.iter().map(|l| l.id).collect::<Vec<_>>(), [lent.id]);
        assert_eq!(
            names(&catalog.recently_added(ben, 10).await.unwrap()),
            ["Lent book"]
        );
        catalog.save_progress(ben, copy.id, 0).await.unwrap();
        // The copy Ben may read is the one listed for him.
        assert_eq!(
            names(&catalog.continue_reading(ben, 10).await.unwrap()),
            ["Lent book"]
        );
        assert!(catalog.cover_path(ben, identity).await.unwrap().is_some());

        // With his copy gone from disk, the other copy is not read for him.
        let expected = catalog.page_image(ben, identity, 1).await.unwrap().bytes;
        std::fs::remove_file(Path::new(&lent.root_path).join("Lent book.cbz")).unwrap();
        assert!(matches!(
            catalog.page_image(ben, identity, 1).await,
            Err(CatalogError::FileMissing(_))
        ));
        assert_eq!(
            catalog.page_image(owner, identity, 1).await.unwrap().bytes,
            expected
        );
    }

    #[tokio::test]
    async fn continue_reading_lists_unfinished_books_latest_first() {
        let shelf = shelf(&[
            ("A.cbz", 3, 1),
            ("B.cbz", 3, 2),
            ("C.cbz", 3, 3),
            ("Never opened.cbz", 3, 4),
            // The same pages as A, in a second place.
            ("Copies/A again.cbz", 3, 1),
        ])
        .await;
        let (a, b, c) = (
            shelf.book("A").await,
            shelf.book("B").await,
            shelf.book("C").await,
        );
        shelf.save(a.id, 1).await.unwrap();
        shelf.save(b.id, 1).await.unwrap();
        shelf.save(c.id, 2).await.unwrap();
        shelf.age_progress(&a, 60).await;
        shelf.age_progress(&c, 30).await;

        let reading = shelf
            .catalog
            .continue_reading(shelf.owner, 10)
            .await
            .unwrap();
        // C is finished, and A is listed once although two files hold it;
        // which of the two is not promised.
        assert_eq!(reading.len(), 2);
        assert_eq!(reading[0].name, "B");
        assert_eq!(reading[1].content_hash, a.content_hash);
        assert_eq!(reading[1].progress.as_ref().map(|p| p.page), Some(1));
        // Both copies carry the progress all the same.
        assert!(shelf.book("A again").await.progress.is_some());

        let one = shelf
            .catalog
            .continue_reading(shelf.owner, 1)
            .await
            .unwrap();
        assert_eq!(names(&one), ["B"]);
    }

    #[tokio::test]
    async fn recently_added_lists_readable_books_newest_first() {
        let shelf = shelf(&[("Old.cbz", 1, 1)]).await;
        shelf.age_books(60).await;
        fixtures::write_book(&shelf.comics.join("New.cbz"), 1, 2);
        std::fs::write(shelf.comics.join("Broken.cbz"), b"not a zip").unwrap();
        shelf.rescan().await;

        let recent = shelf.catalog.recently_added(shelf.owner, 10).await.unwrap();
        assert_eq!(names(&recent), ["New", "Old"]);
        let one = shelf.catalog.recently_added(shelf.owner, 1).await.unwrap();
        assert_eq!(names(&one), ["New"]);
    }

    #[tokio::test]
    async fn up_next_offers_the_book_after_a_finished_one() {
        let shelf = shelf(&[
            ("Saga/Saga 1.cbz", 2, 1),
            ("Saga/Saga 2.cbz", 2, 2),
            ("Saga/Saga 3.cbz", 2, 3),
            ("Fables/Fables 1.cbz", 2, 4),
            ("Fables/Fables 2.cbz", 2, 5),
            ("Alone/Alone 1.cbz", 2, 6),
        ])
        .await;
        let next = async || shelf.catalog.up_next(shelf.owner, 10).await.unwrap();
        assert!(next().await.is_empty());

        // The last page finishes a book.
        shelf.save(shelf.book("Saga 1").await.id, 1).await.unwrap();
        assert_eq!(names(&next().await), ["Saga 2"]);

        // An opened book is being read: it is not offered any more.
        let second = shelf.book("Saga 2").await;
        shelf.save(second.id, 0).await.unwrap();
        assert!(next().await.is_empty());
        shelf.save(second.id, 1).await.unwrap();
        assert_eq!(names(&next().await), ["Saga 3"]);

        // The series read last comes first, and one with nothing left to
        // read offers nothing.
        shelf.age_progress(&shelf.book("Saga 1").await, 60).await;
        shelf.age_progress(&second, 60).await;
        shelf
            .save(shelf.book("Fables 1").await.id, 1)
            .await
            .unwrap();
        shelf.save(shelf.book("Alone 1").await.id, 1).await.unwrap();
        assert_eq!(names(&next().await), ["Fables 2", "Saga 3"]);
        let one = shelf.catalog.up_next(shelf.owner, 1).await.unwrap();
        assert_eq!(names(&one), ["Fables 2"]);

        // Someone else has read nothing.
        let ben = accounts::testing::member(&shelf.catalog.db, "Ben", &[shelf.library_id]).await;
        assert!(shelf.catalog.up_next(ben, 10).await.unwrap().is_empty());

        // The same pages in a second library are offered once.
        let copies = shelf.comics.parent().unwrap().join("Copies");
        fixtures::write_book(&copies.join("Fables/Fables 1 again.cbz"), 2, 4);
        fixtures::write_book(&copies.join("Fables/Fables 2 again.cbz"), 2, 5);
        let copies = shelf.catalog.add_library(shelf.owner, &copies, None);
        shelf.catalog.scan_finished(copies.await.unwrap().id).await;
        let offered = next().await;
        assert_eq!(offered.len(), 2);
        assert_eq!(
            offered[0].content_hash,
            shelf.book("Fables 2").await.content_hash
        );
        assert_eq!(offered[1].name, "Saga 3");
    }

    #[tokio::test]
    async fn books_are_marked_read_and_unread_by_book_group_and_series() {
        let shelf = shelf(&[
            ("Saga/Saga 1.cbz", 2, 1),
            ("Saga/Saga 2.cbz", 3, 2),
            ("Saga/Saga 3.cbz", 2, 3),
            ("Fables/Fables 1.cbz", 2, 4),
        ])
        .await;
        std::fs::write(shelf.comics.join("Fables/Broken.cbz"), b"not a zip").unwrap();
        shelf.rescan().await;
        let mark = async |which: BooksToMark, read| {
            let marked = shelf.catalog.mark_read(shelf.owner, &which, read);
            marked.await.unwrap()
        };
        let series = async |name: &str| {
            let all = shelf.catalog.series_in(shelf.owner, shelf.library_id);
            let found = all.await.unwrap().into_iter().find(|s| s.name == name);
            found.unwrap()
        };
        let saga = series("Saga").await;
        assert_eq!((saga.book_count, saga.unread_count), (3, 3));

        // One book, read before: it keeps the time it was read.
        let first = shelf.book("Saga 1").await;
        shelf.save(first.id, 1).await.unwrap();
        shelf.age_progress(&first, 60).await;
        let read_at = shelf.book("Saga 1").await.progress.unwrap().updated_at;

        // A series: every book of it that was not read yet, to its last page.
        let whole = || BooksToMark {
            series: vec![saga.id],
            ..BooksToMark::default()
        };
        assert_eq!(mark(whole(), true).await, 2);
        let second = shelf.book("Saga 2").await.progress.unwrap();
        assert_eq!((second.page, second.completed), (2, true));
        assert_eq!(
            shelf.book("Saga 1").await.progress.unwrap().updated_at,
            read_at
        );
        assert_eq!(series("Saga").await.unread_count, 0);
        assert_eq!(mark(whole(), true).await, 0);

        let many = BooksToMark {
            books: (0..=MOST_NAMED as i64).collect(),
            ..BooksToMark::default()
        };
        let refused = shelf.catalog.mark_read(shelf.owner, &many, true).await;
        assert!(matches!(refused, Err(CatalogError::Invalid(_))));
        // Reading goes on after the last of them: nothing is left here.
        assert!(
            shelf
                .catalog
                .up_next(shelf.owner, 10)
                .await
                .unwrap()
                .is_empty()
        );

        // Books by themselves, one of them unfinished, one that is not there.
        let third = shelf.book("Saga 3").await;
        shelf.save(first.id, 0).await.unwrap();
        let two = BooksToMark {
            books: vec![first.id, third.id, 9999],
            ..BooksToMark::default()
        };
        assert_eq!(mark(two, false).await, 2);
        assert!(shelf.book("Saga 1").await.progress.is_none());
        assert!(shelf.book("Saga 2").await.progress.is_some());
        assert_eq!(series("Saga").await.unread_count, 2);

        // A file that could not be read is passed over.
        let fables = series("Fables").await;
        let broken = shelf.book("Broken").await;
        assert!(broken.content_hash.is_none());
        let with_broken = BooksToMark {
            books: vec![broken.id],
            series: vec![fables.id],
            ..BooksToMark::default()
        };
        assert_eq!(mark(with_broken, true).await, 1);
        assert!(shelf.book("Broken").await.progress.is_none());
        assert_eq!(mark(BooksToMark::default(), true).await, 0);

        // Someone else reads for themselves, and only where they may.
        let db = &shelf.catalog.db;
        let ben = accounts::testing::member(db, "Ben", &[shelf.library_id]).await;
        let eve = accounts::testing::member(db, "Eve", &[]).await;
        let all = shelf
            .catalog
            .series_in(ben, shelf.library_id)
            .await
            .unwrap();
        assert!(all.iter().all(|s| s.unread_count == s.book_count));
        assert_eq!(
            shelf.catalog.mark_read(eve, &whole(), true).await.unwrap(),
            0
        );
        assert_eq!(
            shelf.catalog.mark_read(ben, &whole(), true).await.unwrap(),
            3
        );
        assert_eq!(series("Saga").await.unread_count, 2);
    }

    #[tokio::test]
    async fn up_next_forgets_a_series_until_it_gets_a_book() {
        let shelf = shelf(&[("Saga/Saga 1.cbz", 2, 1), ("Saga/Saga 2.cbz", 2, 2)]).await;
        let next = async || shelf.catalog.up_next(shelf.owner, 10).await.unwrap();
        let first = shelf.book("Saga 1").await;
        shelf.save(first.id, 1).await.unwrap();
        shelf.age_books(ADDED_LATELY + 60).await;
        assert_eq!(names(&next().await), ["Saga 2"]);

        shelf.age_progress(&first, READ_LATELY + 60).await;
        assert!(next().await.is_empty());

        // A new book brings the series back, with the book that was next.
        fixtures::write_book(&shelf.comics.join("Saga/Saga 3.cbz"), 2, 3);
        shelf.rescan().await;
        assert_eq!(names(&next().await), ["Saga 2"]);
    }

    #[tokio::test]
    async fn a_series_taken_off_up_next_stays_off_until_it_is_read_from() {
        let shelf = shelf(&[
            ("Saga/Saga 1.cbz", 2, 1),
            ("Saga/Saga 2.cbz", 2, 2),
            ("Saga/Saga 3.cbz", 2, 3),
        ])
        .await;
        let next = async || shelf.catalog.up_next(shelf.owner, 10).await.unwrap();
        let first = shelf.book("Saga 1").await;
        let series_id = first.series_id.unwrap();
        shelf.save(first.id, 1).await.unwrap();
        // Read a while before it is taken off.
        shelf.age_progress(&first, 60).await;
        assert_eq!(names(&next().await), ["Saga 2"]);

        let off = shelf.catalog.take_off_up_next(shelf.owner, series_id);
        off.await.unwrap();
        assert!(next().await.is_empty());
        // A book added does not bring it back: only reading does.
        fixtures::write_book(&shelf.comics.join("Saga/Saga 4.cbz"), 2, 4);
        shelf.rescan().await;
        assert!(next().await.is_empty());
        sqlx::query("UPDATE off_up_next SET taken_off_at = taken_off_at - 30")
            .execute(shelf.catalog.db.pool())
            .await
            .unwrap();
        shelf.save(shelf.book("Saga 2").await.id, 1).await.unwrap();
        assert_eq!(names(&next().await), ["Saga 3"]);

        // It is each reader's own row, and only of a series they may read.
        let db = &shelf.catalog.db;
        let eve = accounts::testing::member(db, "Eve", &[]).await;
        let refused = shelf.catalog.take_off_up_next(eve, series_id).await;
        assert!(matches!(refused, Err(CatalogError::NotFound)));
    }

    #[tokio::test]
    async fn reading_is_recorded_one_sitting_at_a_time() {
        let shelf = shelf(&[("Saga/Saga 1.cbz", 5, 1), ("Saga/Saga 2.cbz", 2, 2)]).await;
        let first = shelf.book("Saga 1").await;
        let sittings = async || {
            let all: Vec<(String, i64, i64, i64)> = sqlx::query_as(
                "SELECT content_hash, first_page, last_page, ended_at - started_at \
                 FROM sitting ORDER BY id",
            )
            .fetch_all(shelf.catalog.db.pool())
            .await
            .unwrap();
            all
        };
        let hash = first.content_hash.clone().unwrap();

        // Pages shown one after the other are one sitting.
        shelf.save(first.id, 1).await.unwrap();
        shelf.save(first.id, 2).await.unwrap();
        assert_eq!(sittings().await, [(hash.clone(), 1, 2, 0)]);

        // After a long pause, the next page starts another one.
        sqlx::query("UPDATE sitting SET started_at = started_at - ?1, ended_at = ended_at - ?1")
            .bind(PAUSE + 60)
            .execute(shelf.catalog.db.pool())
            .await
            .unwrap();
        shelf.save(first.id, 3).await.unwrap();
        shelf.save(first.id, 4).await.unwrap();
        assert_eq!(
            sittings().await,
            [(hash.clone(), 1, 2, 0), (hash.clone(), 3, 4, 0)]
        );

        // A mark is not reading, and takes no reading back.
        let second = shelf.book("Saga 2").await;
        let both = BooksToMark {
            books: vec![first.id, second.id],
            ..BooksToMark::default()
        };
        shelf
            .catalog
            .mark_read(shelf.owner, &both, true)
            .await
            .unwrap();
        shelf
            .catalog
            .mark_read(shelf.owner, &both, false)
            .await
            .unwrap();
        assert_eq!(sittings().await.len(), 2);
    }

    #[tokio::test]
    async fn recently_updated_lists_series_by_their_newest_book() {
        let shelf = shelf(&[("Saga/Saga 1.cbz", 1, 1), ("Fables/Fables 1.cbz", 1, 2)]).await;
        shelf.age_books(60).await;
        // More new books in one series than are read at a time.
        for number in 2..=4 {
            let file = format!("Fables/Fables {number}.cbz");
            fixtures::write_book(&shelf.comics.join(file), 1, number + 1);
        }
        shelf.rescan().await;
        let updated = async |limit| {
            let series = shelf.catalog.recently_updated(shelf.owner, limit).await;
            let names: Vec<String> = series.unwrap().into_iter().map(|s| s.name).collect();
            names
        };

        assert_eq!(updated(10).await, ["Fables", "Saga"]);
        assert_eq!(updated(1).await, ["Fables"]);
    }
}
