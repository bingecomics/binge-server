//! Changing the organization: the series of a library, their groups, and
//! where each book sits.
//!
//! Every change here does exactly what was asked, or is refused with the
//! reason. Nothing is decided on the side: a name that is taken is an error
//! and not a merge, a place names its series and group by id and never makes
//! one, and a series or group that was emptied stays until it is deleted.
//! Import is the only thing that decides by itself, once, for a book that has
//! no place yet.

use sqlx::SqliteConnection;

use crate::accounts::UserId;
use crate::catalog::{Book, Catalog, CatalogError, Group, GroupKind, PLACED, Series, now};
use crate::comicinfo::MOST_VOLUMES;
use crate::{import, search};

/// Where a book is to sit. It is the whole place: what it leaves out, the
/// book does not have.
#[derive(Clone, Debug)]
pub struct Place {
    pub series_id: i64,
    /// A group of that series. Without one the book sits directly in it.
    pub group_id: Option<i64>,
    /// The issue's number, as text: `12`, `0.5`.
    pub number: Option<String>,
    /// The date published, written `2016`, `2016-07` or `2016-07-21`.
    pub published: Option<String>,
}

/// Longest name a series or group takes, in characters.
pub(crate) const MAX_NAME: usize = 200;

/// Longest issue number a place takes, in characters.
const MAX_NUMBER: usize = 32;

/// A short text someone typed, as it is kept: without the spaces around it.
/// `when_blank` and `when_long` are what to say when nothing is left of it,
/// and when it is more than `most` characters.
fn typed<'a>(
    text: &'a str,
    most: usize,
    when_blank: &'static str,
    when_long: &'static str,
) -> Result<&'a str, CatalogError> {
    let text = text.trim();
    if text.is_empty() {
        return Err(CatalogError::Invalid(when_blank));
    }
    if text.chars().count() > most {
        return Err(CatalogError::Invalid(when_long));
    }
    if text.chars().any(char::is_control) {
        return Err(CatalogError::Invalid(
            "It cannot hold line breaks or other control characters.",
        ));
    }
    Ok(text)
}

/// A name as it is kept. A blank one is refused with `when_blank`: nothing
/// is read as "no name" but leaving it out.
pub(crate) fn a_name<'a>(name: &'a str, when_blank: &'static str) -> Result<&'a str, CatalogError> {
    typed(
        name,
        MAX_NAME,
        when_blank,
        "A name is at most 200 characters.",
    )
}

pub(crate) fn series_name(name: &str) -> Result<&str, CatalogError> {
    a_name(name, "A series needs a name.")
}

/// An issue's number as a place keeps it. A blank one is refused: nothing is
/// read as "no number" but leaving it out.
pub(crate) fn issue_number(number: &str) -> Result<&str, CatalogError> {
    typed(
        number,
        MAX_NUMBER,
        "Leave the issue number out when the book has none.",
        "An issue number is at most 32 characters.",
    )
}

/// The number and name a group may have, from the ones given. Every kind of
/// group takes the same: a number from 1 to 65535, as a volume's is at
/// import, a name, both or neither.
fn group_parts(
    number: Option<i64>,
    name: Option<&str>,
) -> Result<(Option<i64>, Option<&str>), CatalogError> {
    if number.is_some_and(|number| !(1..=MOST_VOLUMES).contains(&number)) {
        return Err(CatalogError::Invalid(
            "A group's number is from 1 to 65535. Leave it out when the group has none.",
        ));
    }
    let blank = "Leave the name out when the group has none.";
    Ok((number, name.map(|name| a_name(name, blank)).transpose()?))
}

/// A date written `2016`, `2016-07` or `2016-07-21`, as the number a place
/// keeps: see [`import::date_number`].
fn date(written: &str) -> Result<i64, CatalogError> {
    let parts: Option<Vec<i64>> = written
        .trim()
        .split('-')
        .map(|part| {
            let digits = !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit());
            digits.then(|| part.parse().ok()).flatten()
        })
        .collect();
    let read = match parts.as_deref() {
        Some([year @ 1..=9999]) => Some((year, None, None)),
        Some([year @ 1..=9999, month @ 1..=12]) => Some((year, Some(*month), None)),
        Some([year @ 1..=9999, month @ 1..=12, day])
            if (1..=import::days_in(*year, *month)).contains(day) =>
        {
            Some((year, Some(*month), Some(*day)))
        }
        _ => None,
    };
    let (year, month, day) = read.ok_or(CatalogError::Invalid(
        "Write the date published as 2016, 2016-07 or 2016-07-21.",
    ))?;
    Ok(import::date_number(*year, month, day))
}

/// The date a place keeps, written `2016`, `2016-07` or `2016-07-21`: as much
/// of it as is known.
pub fn date_written(published: i64) -> String {
    let (year, month, day) = (published / 10_000, published / 100 % 100, published % 100);
    // Only what `date` would take back: a place is given whole, so what is
    // read from one must be fit to write to one.
    match month {
        1..=12 if (1..=import::days_in(year, month)).contains(&day) => {
            format!("{year:04}-{month:02}-{day:02}")
        }
        1..=12 => format!("{year:04}-{month:02}"),
        _ => format!("{year:04}"),
    }
}

/// The series a library knows by a name with that key.
pub(crate) async fn series_with_key(
    tx: &mut SqliteConnection,
    library_id: i64,
    name_key: &str,
) -> Result<Option<i64>, sqlx::Error> {
    sqlx::query_scalar("SELECT id FROM series WHERE library_id = ? AND name_key = ?")
        .bind(library_id)
        .bind(name_key)
        .fetch_optional(tx)
        .await
}

pub(crate) async fn add_series(
    tx: &mut SqliteConnection,
    library_id: i64,
    name: &str,
    name_key: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO series (library_id, name, name_key, search_key, created_at) \
         VALUES (?, ?, ?, ?, ?) RETURNING id",
    )
    .bind(library_id)
    .bind(name)
    .bind(name_key)
    .bind(search::series_key(name))
    .bind(now())
    .fetch_one(tx)
    .await
}

pub(crate) async fn add_group(
    tx: &mut SqliteConnection,
    series_id: i64,
    kind: GroupKind,
    number: Option<i64>,
    name: Option<&str>,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO series_group (series_id, kind, number, name) VALUES (?, ?, ?, ?) \
         RETURNING id",
    )
    .bind(series_id)
    .bind(kind)
    .bind(number)
    .bind(name)
    .fetch_one(tx)
    .await
}

/// The group of a series that has that kind, number and name, other than
/// `except`. Names are compared as series' names are.
async fn group_like(
    tx: &mut SqliteConnection,
    series_id: i64,
    kind: GroupKind,
    number: Option<i64>,
    name: Option<&str>,
    except: Option<i64>,
) -> Result<Option<i64>, sqlx::Error> {
    let alike: Vec<(i64, Option<String>)> = sqlx::query_as(
        "SELECT id, name FROM series_group WHERE series_id = ? AND kind = ? AND number IS ?",
    )
    .bind(series_id)
    .bind(kind)
    .bind(number)
    .fetch_all(tx)
    .await?;
    let key = name.map(import::name_key);
    let same = alike
        .into_iter()
        .find(|(id, other)| Some(*id) != except && other.as_deref().map(import::name_key) == key);
    Ok(same.map(|(id, _)| id))
}

/// Refuses when the series already has such a group: see [`group_like`].
async fn refuse_same_group(
    tx: &mut SqliteConnection,
    series_id: i64,
    kind: GroupKind,
    number: Option<i64>,
    name: Option<&str>,
    except: Option<i64>,
) -> Result<(), CatalogError> {
    match group_like(tx, series_id, kind, number, name, except).await? {
        Some(id) => Err(CatalogError::GroupExists(id)),
        None => Ok(()),
    }
}

/// What books sit in: its table, and the column of a place that names it.
struct Holder {
    table: &'static str,
    place: &'static str,
}

const SERIES: Holder = Holder {
    table: "series",
    place: "series_id",
};
const GROUP: Holder = Holder {
    table: "series_group",
    place: "group_id",
};

/// Refuses when the series or group is not there, or holds books of the
/// library now.
async fn must_be_empty(
    tx: &mut SqliteConnection,
    holder: &Holder,
    id: i64,
) -> Result<(), CatalogError> {
    let Holder { table, place } = holder;
    let query = format!(
        "SELECT (SELECT COUNT(*) FROM {PLACED} WHERE bp.{place} = t.id) \
         FROM {table} t WHERE t.id = ?"
    );
    let books: Option<i64> = sqlx::query_scalar(&query)
        .bind(id)
        .fetch_optional(tx)
        .await?;
    match books {
        None => Err(CatalogError::NotFound),
        Some(0) => Ok(()),
        Some(_) => Err(CatalogError::NotEmpty),
    }
}

/// These are for the owner, who is `owner` wherever what was changed is
/// given back as they see it.
impl Catalog {
    /// Adds a series with no books to a library.
    pub async fn create_series(
        &self,
        owner: UserId,
        library_id: i64,
        name: &str,
    ) -> Result<Series, CatalogError> {
        let name = series_name(name)?;
        let name_key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        let there: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM library WHERE id = ?)")
            .bind(library_id)
            .fetch_one(&mut *tx)
            .await?;
        if !there {
            return Err(CatalogError::NotFound);
        }
        if let Some(taken) = series_with_key(&mut tx, library_id, &name_key).await? {
            return Err(CatalogError::NameTaken(taken));
        }
        let id = add_series(&mut tx, library_id, name, &name_key).await?;
        tx.commit().await?;
        self.series(owner, id).await
    }

    /// Gives a series another name. Its books stay in it.
    pub async fn rename_series(
        &self,
        owner: UserId,
        id: i64,
        name: &str,
    ) -> Result<Series, CatalogError> {
        let name = series_name(name)?;
        let name_key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        let library_id: i64 = sqlx::query_scalar("SELECT library_id FROM series WHERE id = ?")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(CatalogError::NotFound)?;
        // Its own name written another way is not taken.
        let taken = series_with_key(&mut tx, library_id, &name_key).await?;
        if let Some(other) = taken.filter(|other| *other != id) {
            return Err(CatalogError::NameTaken(other));
        }
        sqlx::query("UPDATE series SET name = ?, name_key = ?, search_key = ? WHERE id = ?")
            .bind(name)
            .bind(&name_key)
            .bind(search::series_key(name))
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.series(owner, id).await
    }

    /// Deletes a series that holds no books, with its groups. The places it
    /// kept for books whose files are gone go with it: such a book is placed
    /// afresh if it is seen again.
    pub async fn delete_series(&self, id: i64) -> Result<(), CatalogError> {
        let mut tx = self.db.begin_write().await?;
        must_be_empty(&mut tx, &SERIES, id).await?;
        sqlx::query("DELETE FROM series WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        Ok(tx.commit().await?)
    }

    /// Adds a group with no books to a series.
    pub async fn create_group(
        &self,
        owner: UserId,
        series_id: i64,
        kind: GroupKind,
        number: Option<i64>,
        name: Option<&str>,
    ) -> Result<Group, CatalogError> {
        let (number, name) = group_parts(number, name)?;
        let mut tx = self.db.begin_write().await?;
        let there: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM series WHERE id = ?)")
            .bind(series_id)
            .fetch_one(&mut *tx)
            .await?;
        if !there {
            return Err(CatalogError::NotFound);
        }
        refuse_same_group(&mut tx, series_id, kind, number, name, None).await?;
        let id = add_group(&mut tx, series_id, kind, number, name).await?;
        tx.commit().await?;
        self.group(owner, id).await
    }

    /// Gives a group another number or name: what it has after this is what
    /// is given here. Its kind and its books stay.
    pub async fn rename_group(
        &self,
        owner: UserId,
        id: i64,
        number: Option<i64>,
        name: Option<&str>,
    ) -> Result<Group, CatalogError> {
        let (number, name) = group_parts(number, name)?;
        let mut tx = self.db.begin_write().await?;
        let (series_id, kind): (i64, GroupKind) =
            sqlx::query_as("SELECT series_id, kind FROM series_group WHERE id = ?")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CatalogError::NotFound)?;
        refuse_same_group(&mut tx, series_id, kind, number, name, Some(id)).await?;
        sqlx::query("UPDATE series_group SET number = ?, name = ? WHERE id = ?")
            .bind(number)
            .bind(name)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.group(owner, id).await
    }

    /// Deletes a group that holds no books. A book whose file is gone and
    /// whose place was in the group keeps its series, without a group.
    pub async fn delete_group(&self, id: i64) -> Result<(), CatalogError> {
        let mut tx = self.db.begin_write().await?;
        must_be_empty(&mut tx, &GROUP, id).await?;
        sqlx::query("DELETE FROM series_group WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        Ok(tx.commit().await?)
    }

    /// Puts a book where `place` says. The place is kept by the book's
    /// identity, so every file of the library that holds the same pages
    /// moves with it.
    pub async fn place_book(
        &self,
        owner: UserId,
        book_id: i64,
        place: &Place,
    ) -> Result<Book, CatalogError> {
        let refused = |reason| Err(CatalogError::Invalid(reason));
        let number = place.number.as_deref().map(issue_number).transpose()?;
        let published = place.published.as_deref().map(date).transpose()?;

        let mut tx = self.db.begin_write().await?;
        let (library_id, identity): (i64, Option<String>) =
            sqlx::query_as("SELECT library_id, content_hash FROM book WHERE id = ?")
                .bind(book_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(CatalogError::NotFound)?;
        let Some(identity) = identity else {
            return refused("This file could not be read, so it has no place to change.");
        };
        let series_in: Option<i64> =
            sqlx::query_scalar("SELECT library_id FROM series WHERE id = ?")
                .bind(place.series_id)
                .fetch_optional(&mut *tx)
                .await?;
        match series_in {
            None => return refused("There is no series with that id."),
            Some(other) if other != library_id => {
                return refused(
                    "That series is in another library. A book stays in the library its \
                     file is in.",
                );
            }
            Some(_) => {}
        }
        if let Some(group_id) = place.group_id {
            let group_in: Option<i64> =
                sqlx::query_scalar("SELECT series_id FROM series_group WHERE id = ?")
                    .bind(group_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            match group_in {
                None => return refused("There is no group with that id."),
                Some(other) if other != place.series_id => {
                    return refused("That group is in another series.");
                }
                Some(_) => {}
            }
        }
        sqlx::query(
            "INSERT INTO book_place (library_id, content_hash, series_id, group_id, number, \
             published) VALUES (?, ?, ?, ?, ?, ?) \
             ON CONFLICT (library_id, content_hash) DO UPDATE SET \
             series_id = excluded.series_id, group_id = excluded.group_id, \
             number = excluded.number, published = excluded.published",
        )
        .bind(library_id)
        .bind(identity)
        .bind(place.series_id)
        .bind(place.group_id)
        .bind(number)
        .bind(published)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        self.book(owner, book_id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    impl Shelf {
        async fn series_called(&self, library_id: i64, name: &str) -> Series {
            let series = self.catalog.series_in(self.owner, library_id).await;
            let mut series = series.unwrap().into_iter();
            series.find(|series| series.name == name).unwrap()
        }

        /// The names of a series' books, in reading order.
        async fn names_in(&self, series_id: i64) -> Vec<String> {
            let books = self.catalog.books_in(self.owner, series_id).await.unwrap();
            books.into_iter().map(|book| book.name).collect()
        }

        async fn book_called(&self, series_id: i64, name: &str) -> Book {
            let books = self.catalog.books_in(self.owner, series_id).await.unwrap();
            books.into_iter().find(|book| book.name == name).unwrap()
        }
    }

    fn directly_in(series_id: i64) -> Place {
        Place {
            series_id,
            group_id: None,
            number: None,
            published: None,
        }
    }

    #[test]
    fn a_date_is_written_with_as_much_as_is_known() {
        for (written, number) in [
            ("2016", 20169999),
            ("2016-07", 20160799),
            (" 2016-07-21 ", 20160721),
            ("0987-1-2", 9870102),
            ("2016-02-29", 20160229),
        ] {
            assert_eq!(date(written).unwrap(), number, "{written}");
        }
        assert_eq!(date_written(20169999), "2016");
        assert_eq!(date_written(20160799), "2016-07");
        assert_eq!(date_written(20160721), "2016-07-21");
        // What a file's tags may have left in a place is written as far as
        // it can be given back.
        assert_eq!(date_written(20160015), "2016");
        assert_eq!(date_written(20160231), "2016-02");
        assert_eq!(date(&date_written(20160231)).unwrap(), 20160299);

        for bad in [
            "",
            "July 2016",
            "2016-13",
            "2016-07-32",
            "2016-04-31",
            "2015-02-29",
            "2016-",
            "-2016",
            "2016-+7",
            "0",
            "2016-07-21-1",
            "20160721",
        ] {
            assert!(matches!(date(bad), Err(CatalogError::Invalid(_))), "{bad}");
        }
    }

    #[test]
    fn a_group_takes_a_number_a_name_both_or_neither() {
        assert_eq!(group_parts(None, None).unwrap(), (None, None));
        assert_eq!(group_parts(Some(2), None).unwrap(), (Some(2), None));
        assert_eq!(
            group_parts(None, Some(" Year One ")).unwrap(),
            (None, Some("Year One"))
        );
        assert_eq!(
            group_parts(Some(65535), Some("Rebirth")).unwrap(),
            (Some(65535), Some("Rebirth"))
        );

        for (number, name) in [
            (Some(0), None),
            (Some(65536), None),
            (Some(-1), Some("Rebirth")),
            (None, Some("  ")),
            (None, Some("Year\nOne")),
        ] {
            let refused = group_parts(number, name);
            assert!(
                matches!(refused, Err(CatalogError::Invalid(_))),
                "{number:?} {name:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_series_is_made_renamed_and_deleted_only_as_asked() {
        let shelf = shelf().await;
        fixtures::write_book(
            &shelf.comics.join("Night Runner/Night Runner 001.cbz"),
            2,
            1,
        );
        let library = shelf.add().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let runner = shelf.series_called(library, "Night Runner").await;

        let made = catalog.create_series(owner, library, "  Odd Squad ").await;
        let made = made.unwrap();
        assert_eq!((made.name.as_str(), made.book_count), ("Odd Squad", 0));
        assert!(made.cover.is_none());
        // It is listed, and counted, with no books in it.
        let listed = catalog.series_in(owner, library).await.unwrap();
        assert_eq!(listed.len(), 2);
        assert_eq!(
            catalog.library(owner, library).await.unwrap().series_count,
            2
        );
        assert!(catalog.books_in(owner, made.id).await.unwrap().is_empty());

        // A name that is taken, however it is written, is refused, and says
        // by which series.
        for taken in ["Night Runner", "night-runner", "NIGHTRUNNER"] {
            let again = catalog.create_series(owner, library, taken).await;
            assert!(
                matches!(again, Err(CatalogError::NameTaken(id)) if id == runner.id),
                "{taken}"
            );
            let renamed = catalog.rename_series(owner, made.id, taken).await;
            assert!(
                matches!(renamed, Err(CatalogError::NameTaken(id)) if id == runner.id),
                "{taken}"
            );
        }
        assert_eq!(shelf.series_called(library, "Odd Squad").await.id, made.id);
        for bad in ["", "   ", "Odd\tSquad", &"x".repeat(201)] {
            let refused = catalog.create_series(owner, library, bad).await;
            assert!(matches!(refused, Err(CatalogError::Invalid(_))));
        }

        // A series takes its own name written another way, and a new one.
        let same = catalog
            .rename_series(owner, runner.id, "Night-Runner")
            .await;
        assert_eq!(same.unwrap().name, "Night-Runner");
        let renamed = catalog.rename_series(owner, runner.id, "The Runner").await;
        let renamed = renamed.unwrap();
        assert_eq!((renamed.id, renamed.book_count), (runner.id, 1));
        assert_eq!(shelf.names_in(runner.id).await, ["Night Runner 001"]);
        // The old name is free again.
        let reused = catalog.create_series(owner, library, "Night Runner").await;
        let reused = reused.unwrap();

        // Only an empty series is deleted.
        let kept = catalog.delete_series(runner.id).await;
        assert!(matches!(kept, Err(CatalogError::NotEmpty)));
        assert_eq!(shelf.names_in(runner.id).await, ["Night Runner 001"]);
        catalog.delete_series(reused.id).await.unwrap();
        catalog.delete_series(made.id).await.unwrap();
        let left = catalog.series_in(owner, library).await.unwrap();
        assert_eq!(left.len(), 1);

        for missing in [made.id, 999] {
            assert!(matches!(
                catalog.rename_series(owner, missing, "Gone").await,
                Err(CatalogError::NotFound)
            ));
            assert!(matches!(
                catalog.delete_series(missing).await,
                Err(CatalogError::NotFound)
            ));
        }
        assert!(matches!(
            catalog.create_series(owner, 999, "Nowhere").await,
            Err(CatalogError::NotFound)
        ));
    }

    #[tokio::test]
    async fn a_group_is_made_renamed_and_deleted_only_as_asked() {
        let shelf = shelf().await;
        fixtures::write_tagged_book(
            &shelf.comics.join("Night Runner/Night Runner v2 001.cbz"),
            1,
            &[("Series", "Night Runner"), ("Volume", "2"), ("Number", "1")],
        );
        let library = shelf.add().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let runner = shelf.series_called(library, "Night Runner").await;
        let groups = catalog.groups_in(owner, runner.id).await.unwrap();
        let [second] = groups.as_slice() else {
            panic!("import made {groups:?}");
        };
        assert_eq!((second.kind, second.number), (GroupKind::Volume, Some(2)));
        assert_eq!(second.book_count, 1);

        let first = catalog.create_group(owner, runner.id, GroupKind::Volume, Some(1), None);
        let first = first.await.unwrap();
        assert_eq!((first.series_id, first.book_count), (runner.id, 0));
        let tale = catalog
            .create_group(owner, runner.id, GroupKind::Limited, None, Some("Year One"))
            .await
            .unwrap();
        assert_eq!(tale.name.as_deref(), Some("Year One"));
        // They are listed with no books in them, in reading order.
        let listed = catalog.groups_in(owner, runner.id).await.unwrap();
        let ids: Vec<i64> = listed.iter().map(|group| group.id).collect();
        assert_eq!(ids, [first.id, second.id, tale.id]);

        // The same group twice is refused, and says which one it is.
        let again = catalog.create_group(owner, runner.id, GroupKind::Volume, Some(2), None);
        assert!(matches!(again.await, Err(CatalogError::GroupExists(id)) if id == second.id));
        let again = catalog
            .create_group(owner, runner.id, GroupKind::Limited, None, Some("year-one"))
            .await;
        assert!(matches!(again, Err(CatalogError::GroupExists(id)) if id == tale.id));
        let clash = catalog.rename_group(owner, first.id, Some(2), None).await;
        assert!(matches!(clash, Err(CatalogError::GroupExists(id)) if id == second.id));
        // Another name makes another group.
        let other = catalog
            .create_group(owner, runner.id, GroupKind::Limited, None, None)
            .await
            .unwrap();

        // A rename keeps the group and its books, and may leave it as it is.
        let third = catalog.rename_group(owner, second.id, Some(3), None).await;
        let third = third.unwrap();
        assert_eq!(
            (third.id, third.number, third.book_count),
            (second.id, Some(3), 1)
        );
        let same = catalog.rename_group(owner, tale.id, None, Some("Year One"));
        assert_eq!(same.await.unwrap().name.as_deref(), Some("Year One"));
        // What no group may have is refused, whatever the kind.
        let unfit = catalog.rename_group(owner, tale.id, Some(0), None).await;
        assert!(matches!(unfit, Err(CatalogError::Invalid(_))));

        // Only an empty group is deleted.
        let kept = catalog.delete_group(second.id).await;
        assert!(matches!(kept, Err(CatalogError::NotEmpty)));
        catalog.delete_group(other.id).await.unwrap();
        catalog.delete_group(first.id).await.unwrap();
        let left = catalog.groups_in(owner, runner.id).await.unwrap();
        let ids: Vec<i64> = left.iter().map(|group| group.id).collect();
        assert_eq!(ids, [second.id, tale.id]);

        for missing in [first.id, 999] {
            assert!(matches!(
                catalog.rename_group(owner, missing, Some(9), None).await,
                Err(CatalogError::NotFound)
            ));
            assert!(matches!(
                catalog.delete_group(missing).await,
                Err(CatalogError::NotFound)
            ));
        }
        let nowhere = catalog.create_group(owner, 999, GroupKind::OneShot, None, None);
        assert!(matches!(nowhere.await, Err(CatalogError::NotFound)));
    }

    #[tokio::test]
    async fn a_book_goes_exactly_where_it_is_put() {
        let shelf = shelf().await;
        for number in 1..=3 {
            let file = format!("Night Runner/Night Runner {number:03}.cbz");
            fixtures::write_book(&shelf.comics.join(file), 2, number);
        }
        std::fs::write(shelf.comics.join("Night Runner/Broken.cbz"), b"no").unwrap();
        let library = shelf.add().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let runner = shelf.series_called(library, "Night Runner").await;
        let squad = catalog.create_series(owner, library, "Odd Squad").await;
        let squad = squad.unwrap();
        let one_shots = catalog
            .create_group(owner, squad.id, GroupKind::OneShot, None, None)
            .await
            .unwrap();
        let book = shelf.book_called(runner.id, "Night Runner 002").await;

        let place = Place {
            series_id: squad.id,
            group_id: Some(one_shots.id),
            number: Some(" 7 ".to_owned()),
            published: Some("1991-05".to_owned()),
        };
        let moved = catalog.place_book(owner, book.id, &place).await.unwrap();
        assert_eq!(moved.id, book.id);
        assert_eq!(moved.series_id, Some(squad.id));
        assert_eq!(moved.group_id, Some(one_shots.id));
        assert_eq!(moved.number.as_deref(), Some("7"));
        assert_eq!(moved.published, Some(19910599));
        assert_eq!(
            shelf.names_in(runner.id).await,
            ["Night Runner 001", "Night Runner 003"]
        );
        assert_eq!(shelf.names_in(squad.id).await, ["Night Runner 002"]);

        // The place is the whole place: what it leaves out is gone.
        let in_squad = directly_in(squad.id);
        let plain = catalog.place_book(owner, book.id, &in_squad).await;
        let plain = plain.unwrap();
        assert_eq!(plain.series_id, Some(squad.id));
        assert_eq!(
            (plain.group_id, plain.number, plain.published),
            (None, None, None)
        );

        // A scan leaves a place that was changed as it is.
        shelf.rescan(library).await;
        assert_eq!(shelf.names_in(squad.id).await, ["Night Runner 002"]);

        // What cannot be done is refused, and nothing is made on the side.
        let elsewhere = tempfile::tempdir().unwrap();
        let other_library = catalog
            .add_library(
                owner,
                elsewhere.path(),
                Some("Other"),
                crate::FromFiles::default(),
            )
            .await
            .unwrap();
        let far = catalog.create_series(owner, other_library.id, "Far Away");
        let far = far.await.unwrap();
        let odd_group = Place {
            group_id: Some(one_shots.id),
            ..directly_in(runner.id)
        };
        let bad_date = Place {
            published: Some("May 1991".to_owned()),
            ..directly_in(runner.id)
        };
        let numbered = |number: &str| Place {
            number: Some(number.to_owned()),
            ..directly_in(runner.id)
        };
        let no_group = Place {
            group_id: Some(999),
            ..directly_in(runner.id)
        };
        for refused in [
            directly_in(999),
            directly_in(far.id),
            odd_group,
            no_group,
            bad_date,
            numbered("  "),
            numbered(&"1".repeat(33)),
        ] {
            let answer = catalog.place_book(owner, book.id, &refused).await;
            assert!(
                matches!(answer, Err(CatalogError::Invalid(_))),
                "{refused:?}"
            );
        }
        assert_eq!(shelf.names_in(squad.id).await, ["Night Runner 002"]);
        assert_eq!(catalog.series_in(owner, library).await.unwrap().len(), 2);

        // A file that could not be read has no place to change.
        let broken = catalog.unreadable_in(owner, library, false).await.unwrap();
        let unread = catalog.place_book(owner, broken[0].id, &in_squad).await;
        assert!(matches!(unread, Err(CatalogError::Invalid(_))));
        let missing = catalog.place_book(owner, 999, &in_squad).await;
        assert!(matches!(missing, Err(CatalogError::NotFound)));
    }

    #[tokio::test]
    async fn every_kind_of_group_is_filled_and_read_the_same_way() {
        let shelf = shelf().await;
        let folder = shelf.comics.join("Night Runner");
        fixtures::write_book(&folder.join("Night Runner v2 001.cbz"), 2, 1);
        fixtures::write_book(&folder.join("Night Runner SP01.cbz"), 2, 2);
        let library = shelf.add().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let runner = shelf.series_called(library, "Night Runner").await;
        let groups = catalog.groups_in(owner, runner.id).await.unwrap();
        let [volume, one_shots] = groups.as_slice() else {
            panic!("import made {groups:?}");
        };

        // The owner names both. Import still knows the volume by its number,
        // and leaves a named group with no number to the owner.
        let named = catalog.rename_group(owner, volume.id, Some(2), Some("Rebirth"));
        named.await.unwrap();
        let named = catalog.rename_group(owner, one_shots.id, None, Some("Sketchbooks"));
        named.await.unwrap();
        fixtures::write_book(&folder.join("Night Runner v2 002.cbz"), 2, 3);
        fixtures::write_book(&folder.join("Night Runner SP02.cbz"), 2, 4);
        shelf.rescan(library).await;
        let groups = catalog.groups_in(owner, runner.id).await.unwrap();
        let listed: Vec<_> = groups
            .iter()
            .map(|group| (group.kind, group.name.as_deref(), group.book_count))
            .collect();
        assert_eq!(
            listed,
            [
                (GroupKind::Volume, Some("Rebirth"), 2),
                (GroupKind::OneShot, Some("Sketchbooks"), 1),
                (GroupKind::OneShot, None, 1),
            ]
        );

        // One-shots with numbers are read by number, as a volume's books are.
        for (name, number) in [("Night Runner SP01", "2"), ("Night Runner SP02", "1")] {
            let book = shelf.book_called(runner.id, name).await;
            let place = Place {
                group_id: Some(one_shots.id),
                number: Some(number.to_owned()),
                ..directly_in(runner.id)
            };
            catalog.place_book(owner, book.id, &place).await.unwrap();
        }
        let read = catalog.books_in_group(owner, one_shots.id).await.unwrap();
        let names: Vec<_> = read.iter().map(|book| book.name.as_str()).collect();
        assert_eq!(names, ["Night Runner SP02", "Night Runner SP01"]);
    }

    #[tokio::test]
    async fn a_series_whose_files_are_gone_stays_until_it_is_deleted() {
        let shelf = shelf().await;
        let file = shelf.comics.join("Night Runner/Night Runner 001.cbz");
        fixtures::write_book(&file, 2, 1);
        let library = shelf.add().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let runner = shelf.series_called(library, "Night Runner").await;

        std::fs::remove_file(&file).unwrap();
        shelf.rescan(library).await;
        // Nothing removed it: it shows, with no books.
        let emptied = shelf.series_called(library, "Night Runner").await;
        assert_eq!((emptied.id, emptied.book_count), (runner.id, 0));

        // Deleting it forgets where the book was, so the book is placed
        // afresh when its file is back.
        catalog.delete_series(runner.id).await.unwrap();
        assert!(catalog.series_in(owner, library).await.unwrap().is_empty());
        fixtures::write_book(&file, 2, 1);
        shelf.rescan(library).await;
        let back = shelf.series_called(library, "Night Runner").await;
        assert_ne!(back.id, runner.id);
        assert_eq!(shelf.names_in(back.id).await, ["Night Runner 001"]);
    }
}
