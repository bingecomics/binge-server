//! What someone has read, looked back on: the days they read, the books
//! and pages of each month, and the writers they read most.
//!
//! It is told from the sittings (see `reading`): a sitting is reading that
//! happened, with when it started and ended and the pages it went from and
//! to. Days are the reader's own days, so whoever asks says how far their
//! clock is from UTC.

use std::collections::{HashMap, HashSet};

use crate::accounts::UserId;
use crate::catalog::{Book, Catalog, CatalogError, books, first_copy, may_read, now};
use crate::people::Role;

/// How many writers a year's most read are.
const MOST_READ: u32 = 10;

/// How long after their last page someone still counts as reading, in
/// seconds: the pause that ends a sitting.
const STILL_READING: i64 = crate::reading::PAUSE;

/// A day someone read.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct ReadingDay {
    /// The day, as `2026-11-18`.
    pub day: String,
    /// Books read from.
    pub books: i64,
    /// Pages gone through.
    pub pages: i64,
    /// Time spent reading, in seconds.
    pub seconds: i64,
}

/// A month someone read in.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct ReadingMonth {
    /// From 1, January.
    pub month: i64,
    pub books: i64,
    pub pages: i64,
    pub seconds: i64,
}

/// A writer someone read.
#[derive(Clone, Debug, PartialEq, Eq, sqlx::FromRow)]
pub struct ReadWriter {
    /// The person.
    pub id: i64,
    pub name: String,
    /// Books of theirs read from.
    pub books: i64,
    /// Pages of those books gone through.
    pub pages: i64,
}

/// What someone read in one year.
#[derive(Clone, Debug)]
pub struct ReadingYear {
    pub year: i64,
    /// The first year they read in. Missing when they never read.
    pub first_year: Option<i64>,
    /// The days they read, in order. A day without reading is left out.
    pub days: Vec<ReadingDay>,
    /// The months they read in, in order.
    pub months: Vec<ReadingMonth>,
    /// The writers they read most, the one with most books first.
    pub writers: Vec<ReadWriter>,
    /// Over the whole year.
    pub books: i64,
    pub pages: i64,
    pub seconds: i64,
}

/// Someone reading now.
#[derive(Clone, Debug)]
pub struct ReadingNow {
    pub user_id: UserId,
    pub user_name: String,
    pub book: Book,
    /// The page they were shown last, from 0.
    pub page: i64,
    /// When, in seconds since 1970.
    pub at: i64,
}

/// A sitting's start (`s`) on the reader's own clock, which is `?3` minutes
/// ahead of UTC.
const LOCAL: &str = "s.started_at + ?3 * 60";

/// The sittings (`s`) of the user `?1` in the year `?2` of their own clock.
const THEIRS: &str = "sitting s WHERE s.user_id = ?1 \
     AND s.started_at >= CAST(strftime('%s', printf('%04d-01-01', ?2)) AS INTEGER) - ?3 * 60 \
     AND s.started_at < CAST(strftime('%s', printf('%04d-01-01', ?2 + 1)) AS INTEGER) - ?3 * 60";

/// What is counted over sittings (`s`): books, pages and seconds.
const COUNTED: &str = "COUNT(DISTINCT s.content_hash) AS books, \
     COALESCE(SUM(ABS(s.last_page - s.first_page) + 1), 0) AS pages, \
     COALESCE(SUM(s.ended_at - s.started_at), 0) AS seconds";

/// How many of a reader's latest sittings say how fast they read.
const PACE_FROM: u32 = 100;

/// How many pages those sittings have to turn before a pace is believed.
const PACE_NEEDS: i64 = 20;

/// The seconds a page takes when it is read: a sitting that went faster
/// was leafing through the book, and one that went slower was left open.
const READING: std::ops::RangeInclusive<i64> = 3..=300;

impl Catalog {
    /// How long `user` takes over a page, in seconds, measured over their
    /// latest sittings. Missing until they have read enough to tell.
    pub async fn pace(&self, user: UserId) -> Result<Option<f64>, CatalogError> {
        // A sitting runs from its first page shown to its last, which is
        // the time the pages between them took.
        let (seconds, turned): (i64, i64) = sqlx::query_as(
            "SELECT COALESCE(SUM(seconds), 0), COALESCE(SUM(turned), 0) FROM \
             (SELECT ended_at - started_at AS seconds, ABS(last_page - first_page) AS turned \
              FROM sitting WHERE user_id = ?1 AND last_page <> first_page \
              AND ended_at - started_at BETWEEN ?2 * ABS(last_page - first_page) \
              AND ?3 * ABS(last_page - first_page) \
              ORDER BY started_at DESC LIMIT ?4)",
        )
        .bind(user)
        .bind(READING.start())
        .bind(READING.end())
        .bind(PACE_FROM)
        .fetch_one(self.db.pool())
        .await?;
        Ok((turned >= PACE_NEEDS).then(|| seconds as f64 / turned as f64))
    }

    /// What `user` read in a year. `minutes_ahead` is how far their clock
    /// is ahead of UTC, in minutes: it says where their days start.
    pub async fn reading_in(
        &self,
        user: UserId,
        year: i64,
        minutes_ahead: i64,
    ) -> Result<ReadingYear, CatalogError> {
        if !(1970..=9999).contains(&year) {
            return Err(CatalogError::Invalid("That is no year to look back on."));
        }
        if !(-14 * 60..=14 * 60).contains(&minutes_ahead) {
            return Err(CatalogError::Invalid(
                "A clock is at most 14 hours from UTC.",
            ));
        }
        let days = format!(
            "SELECT date({LOCAL}, 'unixepoch') AS day, {COUNTED} FROM {THEIRS} \
             GROUP BY day ORDER BY day"
        );
        let months = format!(
            "SELECT CAST(strftime('%m', {LOCAL}, 'unixepoch') AS INTEGER) AS month, {COUNTED} \
             FROM {THEIRS} GROUP BY month ORDER BY month"
        );
        let whole = format!("SELECT {COUNTED} FROM {THEIRS}");
        // A book in two libraries is one book read: its writers count once.
        let writers = format!(
            "SELECT p.id, p.name, COUNT(*) AS books, SUM(r.pages) AS pages \
             FROM (SELECT s.content_hash, SUM(ABS(s.last_page - s.first_page) + 1) AS pages \
                   FROM {THEIRS} GROUP BY s.content_hash) r \
             JOIN person p ON p.id IN \
              (SELECT n.person_id FROM book b JOIN book_credit c ON c.book_id = b.id \
               JOIN person_name n ON n.id = c.name_id \
               WHERE b.content_hash = r.content_hash AND c.role = '{}' AND {}) \
             GROUP BY p.id ORDER BY books DESC, pages DESC, p.name COLLATE natsort, p.id \
             LIMIT {MOST_READ}",
            Role::Writer.kept(),
            may_read("b.library_id")
        );
        let first_year = "SELECT CAST(strftime('%Y', MIN(s.started_at) + ?2 * 60, 'unixepoch') \
             AS INTEGER) FROM sitting s WHERE s.user_id = ?1";
        let pool = self.db.pool();
        let (days, months, (books, pages, seconds), writers, first_year) = tokio::try_join!(
            sqlx::query_as::<_, ReadingDay>(&days)
                .bind(user)
                .bind(year)
                .bind(minutes_ahead)
                .fetch_all(pool),
            sqlx::query_as::<_, ReadingMonth>(&months)
                .bind(user)
                .bind(year)
                .bind(minutes_ahead)
                .fetch_all(pool),
            sqlx::query_as::<_, (i64, i64, i64)>(&whole)
                .bind(user)
                .bind(year)
                .bind(minutes_ahead)
                .fetch_one(pool),
            sqlx::query_as::<_, ReadWriter>(&writers)
                .bind(user)
                .bind(year)
                .bind(minutes_ahead)
                .fetch_all(pool),
            sqlx::query_scalar::<_, Option<i64>>(first_year)
                .bind(user)
                .bind(minutes_ahead)
                .fetch_one(pool),
        )?;
        Ok(ReadingYear {
            year,
            first_year,
            days,
            months,
            writers,
            books,
            pages,
            seconds,
        })
    }

    /// Who is reading now, the one who turned a page last first: everyone
    /// whose last page was shown less than a pause ago. The books are as
    /// `owner`, who asks, sees them.
    pub async fn reading_now(&self, owner: UserId) -> Result<Vec<ReadingNow>, CatalogError> {
        let recent: Vec<(UserId, String, String, i64, i64)> = sqlx::query_as(
            "SELECT u.id, u.name, s.content_hash, s.last_page, s.ended_at \
             FROM sitting s JOIN user u ON u.id = s.user_id \
             WHERE s.ended_at >= ? ORDER BY s.ended_at DESC, s.id DESC",
        )
        .bind(now() - STILL_READING)
        .fetch_all(self.db.pool())
        .await?;
        // Of each reader, the book they turned a page of last.
        let mut seen = HashSet::new();
        let latest: Vec<_> = recent
            .into_iter()
            .filter(|(user_id, ..)| seen.insert(*user_id))
            .collect();
        if latest.is_empty() {
            return Ok(Vec::new());
        }

        // The user is the first parameter. Of a book held twice, the file
        // the server had first.
        let asked_for: Vec<String> = (0..latest.len())
            .map(|place| format!("?{}", place + 2))
            .collect();
        let sql = books(&format!(
            "AND b.content_hash IN ({}) AND b.id = {}",
            asked_for.join(", "),
            first_copy("b")
        ));
        let mut asked = sqlx::query_as::<_, Book>(&sql)
            .persistent(false)
            .bind(owner);
        for (_, _, identity, ..) in &latest {
            asked = asked.bind(identity);
        }
        let mut held: HashMap<String, Book> = HashMap::new();
        for book in asked.fetch_all(self.db.pool()).await? {
            if let Some(identity) = book.content_hash.clone() {
                held.insert(identity, book);
            }
        }
        Ok(latest
            .into_iter()
            // A book whose file is gone since is nothing to show.
            .filter_map(|(user_id, user_name, identity, page, at)| {
                Some(ReadingNow {
                    user_id,
                    user_name,
                    book: held.get(&identity)?.clone(),
                    page,
                    at,
                })
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    /// Noon of a day of 2026, UTC, in seconds since 1970.
    async fn noon(shelf: &Shelf, day: &str) -> i64 {
        sqlx::query_scalar("SELECT CAST(strftime('%s', ?1 || ' 12:00:00') AS INTEGER)")
            .bind(day)
            .fetch_one(shelf.catalog.db.pool())
            .await
            .unwrap()
    }

    /// Writes down that the owner sat with a book: when, for how long, and
    /// the pages gone from and to.
    async fn sat(shelf: &Shelf, book: &Book, started_at: i64, seconds: i64, pages: (i64, i64)) {
        sqlx::query(
            "INSERT INTO sitting \
             (user_id, content_hash, started_at, ended_at, first_page, last_page) \
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(shelf.owner)
        .bind(&book.content_hash)
        .bind(started_at)
        .bind(started_at + seconds)
        .bind(pages.0)
        .bind(pages.1)
        .execute(shelf.catalog.db.pool())
        .await
        .unwrap();
    }

    #[tokio::test]
    async fn the_pace_is_measured_from_reading_and_not_from_leafing() {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 1.cbz"), 3, 1);
        let id = shelf.add().await;
        let saga = shelf.books(id, "Saga").await.remove(0);
        let pace = || async { shelf.catalog.pace(shelf.owner).await.unwrap() };
        let noon = noon(&shelf, "2026-03-05").await;

        // Nothing read, and then too little to tell.
        assert_eq!(pace().await, None);
        sat(&shelf, &saga, noon, 300, (0, 10)).await;
        assert_eq!(pace().await, None);

        // Thirty pages in fifteen minutes, forwards and back.
        sat(&shelf, &saga, noon + 3600, 600, (20, 0)).await;
        assert_eq!(pace().await, Some(30.0));

        // A book leafed through, one left open, and a page looked at say
        // nothing about reading.
        sat(&shelf, &saga, noon + 7200, 10, (0, 200)).await;
        sat(&shelf, &saga, noon + 9000, 1700, (0, 2)).await;
        sat(&shelf, &saga, noon + 12000, 0, (5, 5)).await;
        assert_eq!(pace().await, Some(30.0));

        // Everyone has their own.
        let eve = crate::accounts::testing::member(&shelf.catalog.db, "Eve", &[id]).await;
        assert_eq!(shelf.catalog.pace(eve).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_year_is_told_by_its_days_its_months_and_its_writers() {
        let shelf = shelf().await;
        for (path, seed, writer) in [
            ("Saga/Saga 1.cbz", 1, "Brian K. Vaughan"),
            ("Saga/Saga 2.cbz", 2, "Brian K. Vaughan"),
            ("Fables/Fables 1.cbz", 3, "Bill Willingham"),
        ] {
            fixtures::write_tagged_book(&shelf.comics.join(path), seed, &[("Writer", writer)]);
        }
        let id = shelf.add().await;
        let saga = shelf.books(id, "Saga").await;
        let fables = shelf.books(id, "Fables").await.remove(0);

        let march = noon(&shelf, "2026-03-05").await;
        sat(&shelf, &saga[0], march, 600, (0, 9)).await;
        sat(&shelf, &saga[1], march + 3600, 300, (0, 4)).await;
        // Going back through a book is reading too.
        sat(&shelf, &saga[0], march + 7200, 60, (9, 8)).await;
        let july = noon(&shelf, "2026-07-01").await;
        sat(&shelf, &fables, july, 120, (0, 0)).await;
        // The last evening of the year before, and the first of the next.
        sat(
            &shelf,
            &fables,
            noon(&shelf, "2025-12-31").await + 11 * 3600,
            30,
            (0, 1),
        )
        .await;
        sat(
            &shelf,
            &fables,
            noon(&shelf, "2027-01-01").await,
            30,
            (0, 1),
        )
        .await;

        let year = shelf
            .catalog
            .reading_in(shelf.owner, 2026, 0)
            .await
            .unwrap();
        assert_eq!(year.first_year, Some(2025));
        assert_eq!((year.books, year.pages, year.seconds), (3, 18, 1080));
        let days: Vec<(&str, i64, i64, i64)> = year
            .days
            .iter()
            .map(|day| (day.day.as_str(), day.books, day.pages, day.seconds))
            .collect();
        assert_eq!(
            days,
            [("2026-03-05", 2, 17, 960), ("2026-07-01", 1, 1, 120)]
        );
        let months: Vec<(i64, i64, i64)> = year
            .months
            .iter()
            .map(|month| (month.month, month.books, month.pages))
            .collect();
        assert_eq!(months, [(3, 2, 17), (7, 1, 1)]);
        let writers: Vec<(&str, i64, i64)> = year
            .writers
            .iter()
            .map(|writer| (writer.name.as_str(), writer.books, writer.pages))
            .collect();
        assert_eq!(
            writers,
            [("Brian K. Vaughan", 2, 17), ("Bill Willingham", 1, 1)]
        );

        // An hour ahead of UTC, the last evening of 2025 is in 2026.
        let ahead = shelf
            .catalog
            .reading_in(shelf.owner, 2026, 60)
            .await
            .unwrap();
        assert_eq!(ahead.first_year, Some(2026));
        assert_eq!(ahead.days[0].day, "2026-01-01");
        assert_eq!(ahead.books, 3);
        assert_eq!(ahead.pages, 20);

        // Someone who read nothing has an empty year.
        let eve = crate::accounts::testing::member(&shelf.catalog.db, "Eve", &[id]).await;
        let nothing = shelf.catalog.reading_in(eve, 2026, 0).await.unwrap();
        assert_eq!(nothing.first_year, None);
        assert!(nothing.days.is_empty() && nothing.writers.is_empty());
        assert_eq!((nothing.books, nothing.pages, nothing.seconds), (0, 0, 0));

        let refused = shelf.catalog.reading_in(shelf.owner, 2026, 15 * 60).await;
        assert!(matches!(refused, Err(CatalogError::Invalid(_))));
    }

    #[tokio::test]
    async fn who_is_reading_now_is_whoever_turned_a_page_lately() {
        let shelf = shelf().await;
        fixtures::write_book(&shelf.comics.join("Saga/Saga 1.cbz"), 5, 1);
        fixtures::write_book(&shelf.comics.join("Saga/Saga 2.cbz"), 5, 2);
        let id = shelf.add().await;
        let saga = shelf.books(id, "Saga").await;
        let eve = crate::accounts::testing::member(&shelf.catalog.db, "Eve", &[id]).await;
        let now_reading = async || {
            let reading = shelf.catalog.reading_now(shelf.owner).await.unwrap();
            reading
                .into_iter()
                .map(|one| (one.user_id, one.book.name, one.page))
                .collect::<Vec<_>>()
        };
        assert!(now_reading().await.is_empty());

        let catalog = &shelf.catalog;
        catalog.save_progress(eve, saga[0].id, 2).await.unwrap();
        assert_eq!(now_reading().await, [(eve, "Saga 1".to_owned(), 2)]);

        // Their latest book only, and the owner as well when they read.
        sqlx::query("UPDATE sitting SET started_at = started_at - 60, ended_at = ended_at - 60")
            .execute(catalog.db.pool())
            .await
            .unwrap();
        catalog.save_progress(eve, saga[1].id, 1).await.unwrap();
        sqlx::query("UPDATE sitting SET started_at = started_at - 60, ended_at = ended_at - 60")
            .execute(catalog.db.pool())
            .await
            .unwrap();
        catalog
            .save_progress(shelf.owner, saga[0].id, 3)
            .await
            .unwrap();
        assert_eq!(
            now_reading().await,
            [
                (shelf.owner, "Saga 1".to_owned(), 3),
                (eve, "Saga 2".to_owned(), 1)
            ]
        );

        // After a pause nobody is reading.
        sqlx::query("UPDATE sitting SET started_at = started_at - ?1, ended_at = ended_at - ?1")
            .bind(STILL_READING + 60)
            .execute(catalog.db.pool())
            .await
            .unwrap();
        assert!(now_reading().await.is_empty());
    }
}
