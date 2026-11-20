//! Covers people chose, in place of the ones the server shows by itself.
//!
//! By itself a book shows its first page, and a series or a reading list
//! the cover of its first book. Each can be given another cover: any page
//! of a book, or a picture. The cover made from it is a file in the data directory; nothing
//! is written to the library.

use std::io;

use crate::accounts::UserId;
use crate::catalog::{Catalog, CatalogError};
use crate::thumbnail::ThumbnailError;

/// What a cover is chosen for, by its id.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CoverOf {
    Book(i64),
    Series(i64),
    List(i64),
}

/// What a chosen cover is made from.
#[derive(Clone, Debug)]
pub enum CoverFrom {
    /// A page of a book, from 0.
    Page { book_id: i64, page: i64 },
    /// A picture: the bytes of a JPEG, PNG, WebP or GIF file.
    Picture(Vec<u8>),
}

impl CoverFrom {
    /// The biggest picture taken for a cover, in bytes: 20 MB.
    pub const BIGGEST_PICTURE: usize = 20 * 1024 * 1024;

    /// What a bigger picture is answered with. Also for the one who takes
    /// pictures in and stops reading at that size, never having one to give.
    pub fn too_big() -> CatalogError {
        CatalogError::Invalid("That picture is too big. The server takes one of 20 MB at most.")
    }
}

/// What to say of a picture that is none the server can read.
const NOT_A_PICTURE: &str =
    "That is not a picture the server can read. It reads JPEG, PNG, WebP and GIF files.";

impl From<ThumbnailError> for CatalogError {
    fn from(error: ThumbnailError) -> Self {
        match error {
            ThumbnailError::Image(_) => Self::Invalid(NOT_A_PICTURE),
            ThumbnailError::Io(error) => Self::CoverNotKept(error),
            ThumbnailError::BadIdentity => Self::CoverNotKept(io::Error::other(error)),
        }
    }
}

/// Where a choice of cover is recorded, with the cover chosen before.
struct Record {
    /// A book's identity, or the id of a series or of a reading list: a
    /// book's cover hangs on its identity, like progress.
    of: Whose,
    before: Option<String>,
}

enum Whose {
    Book(String),
    Series(i64),
    List(i64),
}

impl Catalog {
    /// Gives a book, a series or a reading list the cover `user` chose for
    /// it, in place of the one it had. The same picture chosen twice is one file.
    ///
    /// A book's cover goes with the book's identity: every copy of the book
    /// shows it. A file that could not be read has none to choose.
    pub async fn choose_cover(
        &self,
        user: UserId,
        of: CoverOf,
        from: CoverFrom,
    ) -> Result<(), CatalogError> {
        let _changing = self.changing_covers.lock().await;
        // Looked up first, so that nothing is kept for what is not there.
        let record = self.cover_record(user, of).await?;
        let picture = match from {
            CoverFrom::Page { book_id, page } => {
                let book = self.book(user, book_id).await?;
                let identity = book.content_hash.ok_or(CatalogError::NoSuchPage)?;
                self.page_image(user, &identity, page).await?.bytes
            }
            CoverFrom::Picture(bytes) if bytes.len() > CoverFrom::BIGGEST_PICTURE => {
                return Err(CoverFrom::too_big());
            }
            CoverFrom::Picture(bytes) => bytes,
        };
        let covers = self.chosen_covers.clone();
        let cover = tokio::task::spawn_blocking(move || covers.keep(&picture)).await??;
        let recorded = self.record_cover(record, Some(cover.clone())).await;
        if recorded.is_err() {
            // Not written down, so nothing may show it: its file goes again.
            let _ = self.drop_unused_cover(&cover).await;
        }
        recorded
    }

    /// Takes back the cover chosen for a book, a series or a reading list,
    /// which then shows the one the server gives it. Nothing happens when none was chosen.
    pub async fn forget_cover(&self, user: UserId, of: CoverOf) -> Result<(), CatalogError> {
        let _changing = self.changing_covers.lock().await;
        let record = self.cover_record(user, of).await?;
        self.record_cover(record, None).await
    }

    async fn cover_record(&self, user: UserId, of: CoverOf) -> Result<Record, CatalogError> {
        let (of, cover, chosen) = match of {
            CoverOf::Book(id) => {
                let book = self.book(user, id).await?;
                let identity = book.content_hash.ok_or(CatalogError::Invalid(
                    "This file could not be read, so it has no cover to choose.",
                ))?;
                (Whose::Book(identity), book.cover, book.cover_chosen)
            }
            CoverOf::Series(id) => {
                let series = self.series(user, id).await?;
                (Whose::Series(id), series.cover, series.cover_chosen)
            }
            CoverOf::List(id) => {
                let list = self.reading_list(user, id).await?;
                (Whose::List(id), list.cover, list.cover_chosen)
            }
        };
        let before = if chosen { cover } else { None };
        Ok(Record { of, before })
    }

    /// Writes the choice down, or with no `cover` takes it back, and removes
    /// the file of the cover chosen before once nothing shows it.
    async fn record_cover(
        &self,
        record: Record,
        cover: Option<String>,
    ) -> Result<(), CatalogError> {
        let change = match (&record.of, &cover) {
            (Whose::Book(identity), Some(cover)) => sqlx::query(
                "INSERT INTO book_cover (content_hash, cover) VALUES (?, ?) \
                 ON CONFLICT (content_hash) DO UPDATE SET cover = excluded.cover",
            )
            .bind(identity)
            .bind(cover),
            (Whose::Series(id), Some(cover)) => sqlx::query(
                "INSERT INTO series_cover (series_id, cover) VALUES (?, ?) \
                 ON CONFLICT (series_id) DO UPDATE SET cover = excluded.cover",
            )
            .bind(id)
            .bind(cover),
            (Whose::List(id), Some(cover)) => sqlx::query(
                "INSERT INTO reading_list_cover (list_id, cover) VALUES (?, ?) \
                 ON CONFLICT (list_id) DO UPDATE SET cover = excluded.cover",
            )
            .bind(id)
            .bind(cover),
            (Whose::Book(identity), None) => {
                sqlx::query("DELETE FROM book_cover WHERE content_hash = ?").bind(identity)
            }
            (Whose::List(id), None) => {
                sqlx::query("DELETE FROM reading_list_cover WHERE list_id = ?").bind(id)
            }
            (Whose::Series(id), None) => {
                sqlx::query("DELETE FROM series_cover WHERE series_id = ?").bind(id)
            }
        };
        change.execute(self.db.pool()).await?;

        if let Some(before) = record
            .before
            .filter(|before| Some(before) != cover.as_ref())
        {
            self.drop_unused_cover(&before).await?;
        }
        Ok(())
    }

    /// Removes the file of a chosen cover that no book, no series and no
    /// reading list shows any more.
    pub(crate) async fn drop_unused_cover(&self, cover: &str) -> Result<(), CatalogError> {
        let shown: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM book_cover WHERE cover = ?1) \
             OR EXISTS (SELECT 1 FROM series_cover WHERE cover = ?1) \
             OR EXISTS (SELECT 1 FROM reading_list_cover WHERE cover = ?1)",
        )
        .bind(cover)
        .fetch_one(self.db.pool())
        .await?;
        if !shown
            && let Some(path) = self.chosen_covers.path(cover)
            && let Err(error) = tokio::fs::remove_file(&path).await
        {
            // The choice is made all the same; the file is only left over.
            tracing::warn!(file = %path.display(), %error, "an unused cover was not removed");
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::accounts;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    /// A library of one series of two books, scanned, and where the covers
    /// chosen in it are kept.
    async fn saga() -> (Shelf, PathBuf) {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 001.cbz"), 3, 1);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 002.cbz"), 3, 2);
        shelf.add().await;
        let covers = shelf.comics.with_file_name("data").join("covers");
        (shelf, covers)
    }

    #[tokio::test]
    async fn a_chosen_cover_takes_the_place_of_the_first_page() {
        let (shelf, covers) = saga().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let series = catalog.series_in(owner, 1).await.unwrap().remove(0);
        let books = catalog.books_in(owner, series.id).await.unwrap();
        let (first, second) = (&books[0], &books[1]);
        assert_eq!(series.cover, first.content_hash);
        assert!(!series.cover_chosen && !first.cover_chosen);

        // A page of the second book, for the series.
        let of = CoverOf::Series(series.id);
        let from = CoverFrom::Page {
            book_id: second.id,
            page: 2,
        };
        catalog.choose_cover(owner, of, from).await.unwrap();
        let chosen = catalog.series(owner, series.id).await.unwrap();
        assert!(chosen.cover_chosen);
        let cover = chosen.cover.unwrap();
        assert_ne!(Some(&cover), first.content_hash.as_ref());
        let file = catalog.cover_path(owner, &cover).await.unwrap().unwrap();
        assert!(file.canonicalize().unwrap().starts_with(&covers));
        // It is shown to those who may read the series, and to nobody else.
        let ben = accounts::testing::member(&catalog.db, "ben", &[]).await;
        assert!(catalog.cover_path(ben, &cover).await.unwrap().is_none());

        // A picture, for the first book: the series it stands for keeps its own.
        let picture = CoverFrom::Picture(fixtures::page(600, 900, 7));
        let of_book = CoverOf::Book(first.id);
        catalog.choose_cover(owner, of_book, picture).await.unwrap();
        let book = catalog.book(owner, first.id).await.unwrap();
        assert!(book.cover_chosen && book.cover != book.content_hash);
        assert_eq!(
            catalog.series(owner, series.id).await.unwrap().cover,
            Some(cover.clone())
        );

        // Taken back, the series shows its first book's cover: the chosen one.
        catalog.forget_cover(owner, of).await.unwrap();
        let series = catalog.series(owner, series.id).await.unwrap();
        assert!(!series.cover_chosen);
        assert_eq!(series.cover, book.cover);
        // Nothing shows the series' old cover, so its file is gone.
        assert!(!file.exists());
        assert!(catalog.cover_path(owner, &cover).await.unwrap().is_none());

        catalog.forget_cover(owner, of_book).await.unwrap();
        catalog.forget_cover(owner, of_book).await.unwrap();
        let book = catalog.book(owner, first.id).await.unwrap();
        assert!(!book.cover_chosen && book.cover == book.content_hash);
    }

    #[tokio::test]
    async fn a_cover_that_cannot_be_chosen_says_why() {
        let (shelf, covers) = saga().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let series = catalog.series_in(owner, 1).await.unwrap().remove(0);
        let book = catalog.books_in(owner, series.id).await.unwrap().remove(0);
        let of = CoverOf::Series(series.id);

        let no_picture = CoverFrom::Picture(b"not a picture".to_vec());
        let refused = catalog.choose_cover(owner, of, no_picture).await;
        assert!(matches!(refused, Err(CatalogError::Invalid(_))));

        let too_big = CoverFrom::Picture(vec![0; CoverFrom::BIGGEST_PICTURE + 1]);
        let refused = catalog.choose_cover(owner, of, too_big).await;
        assert!(matches!(refused, Err(CatalogError::Invalid(_))));

        let no_page = CoverFrom::Page {
            book_id: book.id,
            page: 3,
        };
        let refused = catalog.choose_cover(owner, of, no_page).await;
        assert!(matches!(refused, Err(CatalogError::NoSuchPage)));

        let picture = || CoverFrom::Picture(fixtures::page(60, 90, 7));
        let nowhere = catalog
            .choose_cover(owner, CoverOf::Series(99), picture())
            .await;
        assert!(matches!(nowhere, Err(CatalogError::NotFound)));
        let nowhere = catalog
            .choose_cover(owner, CoverOf::Book(99), picture())
            .await;
        assert!(matches!(nowhere, Err(CatalogError::NotFound)));
        assert!(!catalog.series(owner, series.id).await.unwrap().cover_chosen);
        assert!(!covers.exists());
    }
}
