//! Favorite pages: pages a reader marked while reading, to find them again.
//!
//! A favorite hangs on the book's identity and the page's number, like
//! progress, so it stays with the page when the book's file is moved or
//! renamed. Everyone has their own, and a page of a book in a library they
//! may not read is not there for them.

use crate::accounts::UserId;
use crate::catalog::{
    Book, Catalog, CatalogError, PLACE_OF, PartOf, Span, first_copy, now, reading_order,
};

/// How many favorite pages one person has at most, so that the table
/// cannot be used to store something else.
const MOST_FAVORITES: i64 = 5_000;

/// A favorite page of someone's, with the book it is a page of.
#[derive(Clone, Debug)]
pub struct FavoritePage {
    pub book: Book,
    /// The page, from 0.
    pub page: i64,
    /// When it became a favorite, in seconds since 1970.
    pub added_at: i64,
}

impl Catalog {
    /// Makes a page of the book with this identity a favorite of `user`'s.
    /// A page that already is one stays as it was, with the time it became
    /// one.
    pub async fn add_favorite_page(
        &self,
        user: UserId,
        content_hash: &str,
        number: i64,
    ) -> Result<(), CatalogError> {
        if !self.has_page(user, content_hash, number).await? {
            return Err(CatalogError::NoSuchPage);
        }
        let mut adding = self.db.begin_write().await?;
        let others: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM favorite_page \
             WHERE user_id = ?1 AND NOT (content_hash = ?2 AND page = ?3)",
        )
        .bind(user)
        .bind(content_hash)
        .bind(number)
        .fetch_one(&mut *adding)
        .await?;
        if others >= MOST_FAVORITES {
            return Err(CatalogError::Invalid(
                "That is too many favorite pages: one person has 5,000 at most.",
            ));
        }
        sqlx::query(
            "INSERT INTO favorite_page (user_id, content_hash, page, added_at) VALUES (?, ?, ?, ?) \
             ON CONFLICT (user_id, content_hash, page) DO NOTHING",
        )
        .bind(user)
        .bind(content_hash)
        .bind(number)
        .bind(now())
        .execute(&mut *adding)
        .await?;
        adding.commit().await?;
        Ok(())
    }

    /// Makes a page no longer a favorite of `user`'s. A page that was not
    /// one is no error.
    pub async fn remove_favorite_page(
        &self,
        user: UserId,
        content_hash: &str,
        number: i64,
    ) -> Result<(), CatalogError> {
        sqlx::query(
            "DELETE FROM favorite_page WHERE user_id = ? AND content_hash = ? AND page = ?",
        )
        .bind(user)
        .bind(content_hash)
        .bind(number)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// The pages of a book that are favorites of `user`'s, in reading order.
    pub async fn favorite_pages_in(
        &self,
        user: UserId,
        book_id: i64,
    ) -> Result<Vec<i64>, CatalogError> {
        let book = self.book(user, book_id).await?;
        // A file that could not be read has no identity and no pages.
        let Some(identity) = book.content_hash else {
            return Ok(Vec::new());
        };
        Ok(sqlx::query_scalar(
            "SELECT page FROM favorite_page WHERE user_id = ? AND content_hash = ? ORDER BY page",
        )
        .bind(user)
        .bind(identity)
        .fetch_all(self.db.pool())
        .await?)
    }

    /// A part of the favorite pages of `user`, series by series in the order of
    /// their names, and inside a series book by book as they are read. The
    /// pages of books in no series come last. A page of a book that is not
    /// on the server now is left out, and is there again when its book is.
    pub async fn favorite_pages(
        &self,
        user: UserId,
        span: Span,
    ) -> Result<PartOf<FavoritePage>, CatalogError> {
        // The file that stands for a favorite's book, as `user` sees it.
        let copy = first_copy("f");
        let count = format!(
            "SELECT COUNT(*) FROM favorite_page f WHERE f.user_id = ?1 AND {copy} IS NOT NULL"
        );
        let part = format!(
            "SELECT b.id, f.page, f.added_at FROM favorite_page f JOIN book b ON b.id = {copy} \
             {PLACE_OF} WHERE f.user_id = ?1 \
             ORDER BY bs.name COLLATE natsort NULLS LAST, bs.id, {}, b.id, f.page \
             LIMIT ?2 OFFSET ?3",
            reading_order()
        );
        let (total, favorites): (i64, Vec<(i64, i64, i64)>) = tokio::try_join!(
            sqlx::query_scalar(&count)
                .bind(user)
                .fetch_one(self.db.pool()),
            sqlx::query_as(&part)
                .bind(user)
                .bind(span.limit)
                .bind(span.offset)
                .fetch_all(self.db.pool()),
        )?;

        let ids: Vec<i64> = favorites.iter().map(|(book_id, ..)| *book_id).collect();
        let books = self.books_by_id(user, &ids).await?;
        let items = favorites
            .into_iter()
            .filter_map(|(book_id, page, added_at)| {
                Some(FavoritePage {
                    book: books.get(&book_id)?.clone(),
                    page,
                    added_at,
                })
            })
            .collect();
        Ok(PartOf { items, total })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::accounts;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    const NOBODY: &str = "0000000000000000000000000000000000000000000000000000000000000000";

    /// A library holding the given books, each as (path, pages, seed).
    async fn library(held: &[(&str, u8, u8)]) -> (Shelf, i64) {
        let shelf = shelf().await;
        for (path, pages, seed) in held {
            fixtures::write_book(&shelf.comics.join(path), *pages, *seed);
        }
        let id = shelf.add().await;
        (shelf, id)
    }

    /// The first book of a series, with its identity.
    async fn first_of(shelf: &Shelf, library_id: i64, series: &str) -> (i64, String) {
        let book = shelf.books(library_id, series).await.remove(0);
        (book.id, book.content_hash.unwrap())
    }

    /// The owner's favorites, as (book's name, page), and how many in all.
    async fn favorites(shelf: &Shelf, span: Span) -> (Vec<(String, i64)>, i64) {
        let part = shelf
            .catalog
            .favorite_pages(shelf.owner, span)
            .await
            .unwrap();
        let pages = part
            .items
            .into_iter()
            .map(|favorite| (favorite.book.name, favorite.page))
            .collect();
        (pages, part.total)
    }

    const ALL: Span = Span {
        offset: 0,
        limit: 100,
    };

    #[tokio::test]
    async fn a_page_is_a_favorite_once_and_no_longer() {
        let (shelf, library_id) = library(&[("Night Runner/Night Runner 001.cbz", 4, 1)]).await;
        let (id, identity) = first_of(&shelf, library_id, "Night Runner").await;
        let catalog = &shelf.catalog;

        catalog
            .add_favorite_page(shelf.owner, &identity, 2)
            .await
            .unwrap();
        catalog
            .add_favorite_page(shelf.owner, &identity, 0)
            .await
            .unwrap();
        // Added again, it is still one favorite.
        catalog
            .add_favorite_page(shelf.owner, &identity, 2)
            .await
            .unwrap();
        assert_eq!(
            catalog.favorite_pages_in(shelf.owner, id).await.unwrap(),
            [0, 2]
        );

        catalog
            .remove_favorite_page(shelf.owner, &identity, 0)
            .await
            .unwrap();
        catalog
            .remove_favorite_page(shelf.owner, &identity, 0)
            .await
            .unwrap();
        assert_eq!(
            catalog.favorite_pages_in(shelf.owner, id).await.unwrap(),
            [2]
        );
    }

    #[tokio::test]
    async fn only_a_page_that_is_there_can_be_a_favorite() {
        let (shelf, library_id) = library(&[("Night Runner/Night Runner 001.cbz", 4, 1)]).await;
        let (_, identity) = first_of(&shelf, library_id, "Night Runner").await;
        let catalog = &shelf.catalog;

        for (identity, page) in [(identity.as_str(), 4), (identity.as_str(), -1), (NOBODY, 0)] {
            assert!(matches!(
                catalog.add_favorite_page(shelf.owner, identity, page).await,
                Err(CatalogError::NoSuchPage)
            ));
        }
        assert!(matches!(
            catalog.favorite_pages_in(shelf.owner, 999).await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn favorites_are_listed_by_series_and_in_parts() {
        let (shelf, library_id) = library(&[
            ("Zot/Zot 001.cbz", 3, 1),
            ("Night Runner/Night Runner 002.cbz", 3, 2),
            ("Night Runner/Night Runner 001.cbz", 3, 3),
        ])
        .await;
        let catalog = &shelf.catalog;
        let runner = shelf.books(library_id, "Night Runner").await;
        let zot = shelf.books(library_id, "Zot").await;
        for (book, page) in [
            (&zot[0], 1),
            (&runner[1], 0),
            (&runner[0], 2),
            (&runner[0], 1),
        ] {
            let identity = book.content_hash.as_deref().unwrap();
            catalog
                .add_favorite_page(shelf.owner, identity, page)
                .await
                .unwrap();
        }

        let (pages, total) = favorites(&shelf, ALL).await;
        assert_eq!(total, 4);
        assert_eq!(
            pages,
            [
                ("Night Runner 001".to_owned(), 1),
                ("Night Runner 001".to_owned(), 2),
                ("Night Runner 002".to_owned(), 0),
                ("Zot 001".to_owned(), 1),
            ]
        );

        let last = Span {
            offset: 3,
            limit: 2,
        };
        assert_eq!(
            favorites(&shelf, last).await,
            (vec![("Zot 001".to_owned(), 1)], 4)
        );
    }

    #[tokio::test]
    async fn favorites_are_each_readers_own() {
        let (shelf, library_id) = library(&[("Night Runner/Night Runner 001.cbz", 4, 1)]).await;
        let (id, identity) = first_of(&shelf, library_id, "Night Runner").await;
        let catalog = &shelf.catalog;
        let member = accounts::testing::member(&catalog.db, "Ana", &[library_id]).await;
        let stranger = accounts::testing::member(&catalog.db, "Ben", &[]).await;

        catalog
            .add_favorite_page(shelf.owner, &identity, 1)
            .await
            .unwrap();
        catalog
            .add_favorite_page(member, &identity, 3)
            .await
            .unwrap();
        assert_eq!(
            catalog.favorite_pages_in(shelf.owner, id).await.unwrap(),
            [1]
        );
        assert_eq!(catalog.favorite_pages_in(member, id).await.unwrap(), [3]);

        // Someone who may not read the library has no such page to mark.
        assert!(matches!(
            catalog.add_favorite_page(stranger, &identity, 1).await,
            Err(CatalogError::NoSuchPage)
        ));
        let none = catalog.favorite_pages(stranger, ALL).await.unwrap();
        assert_eq!((none.items.len(), none.total), (0, 0));
    }
}
