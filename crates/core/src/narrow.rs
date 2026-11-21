//! Finding one's way in a library: its series narrowed to some, in an order
//! that was asked for, a part at a time.
//!
//! A series is narrowed by what its books say. It is of a publisher, a year
//! or a person when one of its books is. How far it was read is the
//! reader's own: nothing of it opened, some of it, or all of it.

use sqlx::Arguments;
use sqlx::sqlite::SqliteArguments;

use crate::accounts::UserId;
use crate::catalog::{
    Catalog, CatalogError, PLACED, PartOf, Series, Span, UNREAD, YEAR, may_read, series,
};
use crate::people::{NAMED, Role};
use crate::search::fold;

/// The books (`bp`, `b`) of a library with what their files say (`i`).
fn told() -> String {
    format!("{PLACED} LEFT JOIN book_info i ON i.book_id = b.id")
}

/// SQL that holds for a series `s` with a book that `rest` holds for. The
/// series with such a book are found once, not asked of each series: asked
/// of each, a publisher with half the library's books is walked through
/// once per series.
fn has_a_book(from: &str, rest: &str) -> String {
    format!("s.id IN (SELECT bp.series_id FROM {from} WHERE bp.series_id IS NOT NULL{rest})")
}

/// What a library's series are ordered by.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SeriesSort {
    /// By name, from A.
    #[default]
    Name,
    /// By when its newest book was added, the newest first.
    Added,
    /// By the year its first book came out, the earliest first.
    Year,
    /// By when the reader last read from it, the latest first.
    LastRead,
    /// The ones with books left to read first.
    Unread,
    /// In no order: one that changes with the seed.
    Random,
}

/// The order a library's series are asked for in.
#[derive(Clone, Copy, Debug, Default)]
pub struct SeriesOrder {
    pub sort: SeriesSort,
    /// Turns the order around.
    pub reverse: bool,
    /// Which of the orders that are no order: the same seed gives the same
    /// one, so that a list in it can be read a part at a time.
    pub seed: u32,
}

/// How far a reader is through a series.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReadState {
    /// They opened none of its books.
    Unread,
    /// They opened some, and have books left.
    InProgress,
    /// They read every book of it.
    Read,
}

/// The years a series is asked to have a book from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Years {
    /// One year.
    Year(i64),
    /// The ten years from this one.
    Decade(i64),
}

impl Years {
    /// Reads `1993`, or `1990s` for the ten years from 1990.
    pub fn parse(written: &str) -> Result<Self, CatalogError> {
        let invalid =
            CatalogError::Invalid("A year is written like 1993, and ten of them like 1990s.");
        let (digits, decade) = match written.strip_suffix('s') {
            Some(digits) => (digits, true),
            None => (written, false),
        };
        // Four digits and nothing else: `+199` is no year.
        if digits.len() != 4 || !digits.bytes().all(|digit| digit.is_ascii_digit()) {
            return Err(invalid);
        }
        let Ok(year) = digits.parse::<i64>() else {
            return Err(invalid);
        };
        match decade {
            true if year % 10 == 0 => Ok(Self::Decade(year)),
            true => Err(invalid),
            false => Ok(Self::Year(year)),
        }
    }

    fn first_and_last(self) -> (i64, i64) {
        match self {
            Self::Year(year) => (year, year),
            Self::Decade(first) => (first, first + 9),
        }
    }
}

/// The values of one thing series are narrowed by. A series stays when it
/// has one of those asked for, if any is, and none of those ruled out.
#[derive(Clone, Debug)]
pub struct Choice<T> {
    pub asked: Vec<T>,
    pub ruled_out: Vec<T>,
}

impl<T> Default for Choice<T> {
    fn default() -> Self {
        Self {
            asked: Vec::new(),
            ruled_out: Vec::new(),
        }
    }
}

/// What a library's series are narrowed by. A series stays when every part
/// leaves it in.
#[derive(Clone, Debug, Default)]
pub struct Narrowing {
    pub read: Choice<ReadState>,
    pub publishers: Choice<String>,
    pub years: Choice<Years>,
    /// People, by their ids.
    pub writers: Choice<i64>,
    pub pencillers: Choice<i64>,
}

/// A letter that series' names start with, in a list ordered by name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Letter {
    /// A capital from A to Z, or `#` for every name that starts with
    /// anything else.
    pub letter: char,
    /// How many series start with it.
    pub count: i64,
    /// Where the first of them is in the list, from 0.
    pub offset: i64,
}

/// One value a library's series can be narrowed by.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Named<T> {
    pub value: T,
    /// How many series of the library have it.
    pub series_count: i64,
}

/// Someone a library's series can be narrowed by.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct NamedPerson {
    pub id: i64,
    pub name: String,
    /// How many series of the library have a book that names them.
    pub series_count: i64,
}

/// What a library's series can be narrowed by: every value its books have.
#[derive(Clone, Debug)]
pub struct Choices {
    /// The one most series have first.
    pub publishers: Vec<Named<String>>,
    /// The first year of each ten that has books, the earliest first.
    pub decades: Vec<Named<i64>>,
    /// The one most series name first.
    pub writers: Vec<NamedPerson>,
    pub pencillers: Vec<NamedPerson>,
}

/// A value a statement takes.
enum Value {
    Number(i64),
    Text(String),
}

/// A statement being put together: the values it takes after the user,
/// who is `?1`.
#[derive(Default)]
struct Asked {
    values: Vec<Value>,
}

impl Asked {
    /// Takes one more value and gives the place it has in the statement.
    fn value(&mut self, value: Value) -> String {
        self.values.push(value);
        format!("?{}", self.values.len() + 1)
    }

    fn number(&mut self, number: i64) -> String {
        self.value(Value::Number(number))
    }

    /// The places of several values, as the inside of an SQL list.
    fn every(&mut self, values: impl IntoIterator<Item = Value>) -> String {
        let places: Vec<String> = values.into_iter().map(|value| self.value(value)).collect();
        places.join(", ")
    }

    /// What to bind to the statement, for `user`.
    fn arguments(&self, user: UserId) -> Result<SqliteArguments<'static>, sqlx::Error> {
        let mut arguments = SqliteArguments::default();
        arguments.add(user).map_err(sqlx::Error::Encode)?;
        for value in &self.values {
            match value {
                Value::Number(number) => arguments.add(*number),
                Value::Text(text) => arguments.add(text.clone()),
            }
            .map_err(sqlx::Error::Encode)?;
        }
        Ok(arguments)
    }
}

/// SQL that holds for a series `s` the reader, `?1`, opened a book of.
fn started() -> String {
    has_a_book(
        &format!("{PLACED} JOIN progress p ON p.content_hash = b.content_hash AND p.user_id = ?1"),
        "",
    )
}

/// SQL that holds for a series `s` with a book the reader has not read to
/// the end.
fn has_books_left() -> String {
    has_a_book(PLACED, &format!(" AND {UNREAD}"))
}

/// SQL that holds for a series `s` this far read.
fn read_this_far(state: ReadState) -> String {
    match state {
        ReadState::Unread => format!("NOT {}", started()),
        ReadState::InProgress => format!("({} AND {})", started(), has_books_left()),
        ReadState::Read => format!("({} AND NOT {})", has_a_book(PLACED, ""), has_books_left()),
    }
}

/// Adds to `sql` what one kind of choice leaves in: series that `holds`
/// says hold for the values asked for, and not for the ones ruled out.
fn both<T>(sql: &mut String, choice: &Choice<T>, mut holds: impl FnMut(&[T]) -> String) {
    for (values, said) in [(&choice.asked, " AND "), (&choice.ruled_out, " AND NOT ")] {
        if !values.is_empty() {
            sql.push_str(said);
            sql.push_str(&holds(values));
        }
    }
}

/// SQL that holds for a series `s` the narrowing leaves in, each part of it
/// as ` AND …`.
fn narrowed(narrowing: &Narrowing, asked: &mut Asked) -> String {
    let mut sql = String::new();
    both(&mut sql, &narrowing.read, |states| {
        let any: Vec<String> = states.iter().copied().map(read_this_far).collect();
        format!("({})", any.join(" OR "))
    });
    both(&mut sql, &narrowing.publishers, |names| {
        let names = asked.every(names.iter().cloned().map(Value::Text));
        has_a_book(&told(), &format!(" AND i.publisher IN ({names})"))
    });
    both(&mut sql, &narrowing.years, |years| {
        let any: Vec<String> = years
            .iter()
            .map(|years| {
                let (first, last) = years.first_and_last();
                format!(
                    "{YEAR} BETWEEN {} AND {}",
                    asked.number(first),
                    asked.number(last)
                )
            })
            .collect();
        has_a_book(&told(), &format!(" AND ({})", any.join(" OR ")))
    });
    for (role, people) in [
        (Role::Writer, &narrowing.writers),
        (Role::Penciller, &narrowing.pencillers),
    ] {
        both(&mut sql, people, |people| {
            let people = asked.every(people.iter().copied().map(Value::Number));
            has_a_book(
                &format!("{PLACED} {NAMED}"),
                &format!(
                    " AND c.role = '{}' AND n.person_id IN ({people})",
                    role.kept()
                ),
            )
        });
    }
    sql
}

/// The `ORDER BY` that puts series `s` in the order asked for. Series the
/// order says nothing about come last, and name order settles the rest.
fn ordered(order: SeriesOrder, asked: &mut Asked) -> String {
    let in_series = "bp.series_id = s.id";
    // What to order by, and whether its own order is from the highest.
    let (by, from_highest) = match order.sort {
        SeriesSort::Name => ("s.name COLLATE natsort".to_owned(), false),
        SeriesSort::Added => (
            format!("(SELECT MAX(b.added_at) FROM {PLACED} WHERE {in_series})"),
            true,
        ),
        SeriesSort::Year => (
            format!("(SELECT MIN({YEAR}) FROM {} WHERE {in_series})", told()),
            false,
        ),
        SeriesSort::LastRead => (
            format!(
                "(SELECT MAX(p.updated_at) FROM {PLACED} JOIN progress p \
                 ON p.content_hash = b.content_hash AND p.user_id = ?1 WHERE {in_series})"
            ),
            true,
        ),
        SeriesSort::Unread => (format!("NOT {}", has_books_left()), false),
        SeriesSort::Random => (shuffled(order.seed, asked), false),
    };
    let direction = if from_highest != order.reverse {
        "DESC"
    } else {
        "ASC"
    };
    format!("ORDER BY {by} {direction} NULLS LAST, s.name COLLATE natsort, s.id")
}

/// SQL for the number a series `s` has in the shuffle of a seed. Each id
/// lands on a number of its own, and another seed is another order, not
/// the same one begun elsewhere: the seed chooses what the ids are
/// multiplied by, and the low half of the product is folded into the high
/// one before a second multiplication. Every step stays under 63 bits,
/// where SQLite still counts in whole numbers.
fn shuffled(seed: u32, asked: &mut Asked) -> String {
    // An odd number of 31 bits, so that no two ids land on one.
    let by = ((u64::from(seed) + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 33) | 1;
    let by = asked.number(i64::try_from(by).expect("31 bits fit"));
    let once = format!("(((s.id + 1) * {by}) % 2147483648)");
    // SQLite has no exclusive or: it is what either has less what both have.
    let folded = format!("(({once} | ({once} >> 15)) - ({once} & ({once} >> 15)))");
    format!("(({folded} * 739982445) % 2147483648)")
}

/// The letter a name is filed under.
fn letter_of(name: &str) -> char {
    match fold(name).trim_start().chars().next() {
        Some(first) if first.is_ascii_alphabetic() => first.to_ascii_uppercase(),
        _ => '#',
    }
}

impl Catalog {
    /// A part of a library's series: those the narrowing leaves in, in the
    /// order asked for.
    pub async fn series_part(
        &self,
        user: UserId,
        library_id: i64,
        narrowing: &Narrowing,
        order: SeriesOrder,
        span: Span,
    ) -> Result<PartOf<Series>, CatalogError> {
        let mut asked = Asked::default();
        let library = asked.number(library_id);
        let left_in = format!(
            "AND s.library_id = {library}{}",
            narrowed(narrowing, &mut asked)
        );
        let count = format!(
            "SELECT COUNT(*) FROM series s WHERE {} {left_in}",
            may_read("s.library_id")
        );
        let counted = asked.arguments(user)?;
        let order = ordered(order, &mut asked);
        let (limit, offset) = (
            asked.number(span.limit.into()),
            asked.number(span.offset.into()),
        );
        // The part is chosen first, so that only its series are filled in.
        let query = series(&format!(
            "AND s.id IN (SELECT s.id FROM series s WHERE {} {left_in} {order} \
             LIMIT {limit} OFFSET {offset}) {order}",
            may_read("s.library_id")
        ));
        let (items, total): (Vec<Series>, i64) = tokio::try_join!(
            sqlx::query_as_with(&query, asked.arguments(user)?).fetch_all(self.db.pool()),
            sqlx::query_scalar_with(&count, counted).fetch_one(self.db.pool()),
        )?;
        // Only an empty answer leaves open whether the library is there.
        if total == 0 {
            self.library(user, library_id).await?;
        }
        Ok(PartOf { items, total })
    }

    /// The letters the names of a library's series start with, among those
    /// the narrowing leaves in, with where each starts in the list by name.
    /// `reverse` is for the list from Z.
    pub async fn series_letters(
        &self,
        user: UserId,
        library_id: i64,
        narrowing: &Narrowing,
        reverse: bool,
    ) -> Result<Vec<Letter>, CatalogError> {
        let mut asked = Asked::default();
        let library = asked.number(library_id);
        let left_in = narrowed(narrowing, &mut asked);
        let by_name = SeriesOrder {
            reverse,
            ..SeriesOrder::default()
        };
        let query = format!(
            "SELECT s.name FROM series s WHERE {} AND s.library_id = {library}{left_in} {}",
            may_read("s.library_id"),
            ordered(by_name, &mut asked)
        );
        let names: Vec<String> = sqlx::query_scalar_with(&query, asked.arguments(user)?)
            .fetch_all(self.db.pool())
            .await?;
        if names.is_empty() {
            self.library(user, library_id).await?;
        }
        let mut letters: Vec<Letter> = Vec::new();
        for (offset, name) in names.iter().enumerate() {
            let letter = letter_of(name);
            // Names that start with no letter are not all together: digits
            // come before the letters and some signs after them, so `#`
            // may come up again.
            match letters.iter_mut().find(|known| known.letter == letter) {
                Some(known) => known.count += 1,
                None => letters.push(Letter {
                    letter,
                    count: 1,
                    offset: offset as i64,
                }),
            }
        }
        Ok(letters)
    }

    /// What the series of a library can be narrowed by.
    pub async fn series_choices(
        &self,
        user: UserId,
        library_id: i64,
    ) -> Result<Choices, CatalogError> {
        // Whoever may read the library may know all that its books say.
        self.library(user, library_id).await?;
        let told = told();
        let publishers = format!(
            "SELECT i.publisher AS value, COUNT(DISTINCT bp.series_id) AS series_count \
             FROM {told} WHERE bp.library_id = ? AND i.publisher IS NOT NULL \
             GROUP BY i.publisher ORDER BY series_count DESC, i.publisher COLLATE natsort"
        );
        let decades = format!(
            "SELECT {YEAR} / 10 * 10 AS value, COUNT(DISTINCT bp.series_id) AS series_count \
             FROM {told} WHERE bp.library_id = ? AND {YEAR} IS NOT NULL \
             GROUP BY value ORDER BY value"
        );
        let people = format!(
            "SELECT p.id, p.name, COUNT(DISTINCT bp.series_id) AS series_count \
             FROM {PLACED} {NAMED} JOIN person p ON p.id = n.person_id \
             WHERE bp.library_id = ? AND c.role = ? \
             GROUP BY p.id ORDER BY series_count DESC, p.name COLLATE natsort, p.id"
        );
        let pool = self.db.pool();
        let (publishers, decades, writers, pencillers) = tokio::try_join!(
            sqlx::query_as(&publishers).bind(library_id).fetch_all(pool),
            sqlx::query_as(&decades).bind(library_id).fetch_all(pool),
            sqlx::query_as(&people)
                .bind(library_id)
                .bind(Role::Writer)
                .fetch_all(pool),
            sqlx::query_as(&people)
                .bind(library_id)
                .bind(Role::Penciller)
                .fetch_all(pool),
        )?;
        Ok(Choices {
            publishers,
            decades,
            writers,
            pencillers,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    /// A library of five series: who published each, when, and who wrote
    /// it. The owner read all of Zot, the first of Saga's two books, and
    /// nothing else.
    async fn library() -> (Shelf, i64) {
        let shelf = shelf().await;
        for (seed, path, publisher, year, writer) in [
            (1, "Saga/Saga 001.cbz", "Image", "2012", "Brian K. Vaughan"),
            (2, "Saga/Saga 002.cbz", "Image", "2012", "Brian K. Vaughan"),
            (
                3,
                "The Invisibles/The Invisibles 001.cbz",
                "Vertigo",
                "1994",
                "Grant Morrison",
            ),
            (
                4,
                "Preacher/Preacher 001.cbz",
                "Vertigo",
                "1995",
                "Garth Ennis",
            ),
            (5, "Y/Y 001.cbz", "Vertigo", "2002", "Brian K. Vaughan"),
            (6, "Zot/Zot 001.cbz", "Eclipse", "1984", "Scott McCloud"),
        ] {
            fixtures::write_tagged_book(
                &shelf.comics.join(path),
                seed,
                &[("Publisher", publisher), ("Year", year), ("Writer", writer)],
            );
        }
        fixtures::write_book(&shelf.comics.join("100 Bullets/100 Bullets 001.cbz"), 1, 7);
        let library = shelf.add().await;
        for (series, book) in [("Zot", 0), ("Saga", 0)] {
            let book = shelf.books(library, series).await.remove(book);
            let read = shelf.catalog.save_progress(shelf.owner, book.id, 0).await;
            assert!(read.unwrap().completed);
        }
        (shelf, library)
    }

    const ALL: Span = Span {
        offset: 0,
        limit: 100,
    };

    async fn names(shelf: &Shelf, library: i64, narrowing: &Narrowing) -> Vec<String> {
        ordered_names(shelf, library, narrowing, SeriesOrder::default()).await
    }

    async fn ordered_names(
        shelf: &Shelf,
        library: i64,
        narrowing: &Narrowing,
        order: SeriesOrder,
    ) -> Vec<String> {
        let part = shelf
            .catalog
            .series_part(shelf.owner, library, narrowing, order, ALL)
            .await
            .unwrap();
        assert_eq!(part.total as usize, part.items.len());
        part.items.into_iter().map(|series| series.name).collect()
    }

    fn asked<T>(values: impl IntoIterator<Item = T>) -> Choice<T> {
        Choice {
            asked: values.into_iter().collect(),
            ruled_out: Vec::new(),
        }
    }

    fn ruled_out<T>(values: impl IntoIterator<Item = T>) -> Choice<T> {
        Choice {
            asked: Vec::new(),
            ruled_out: values.into_iter().collect(),
        }
    }

    #[tokio::test]
    async fn series_are_narrowed_by_what_their_books_say() {
        let (shelf, library) = library().await;
        let all = names(&shelf, library, &Narrowing::default()).await;
        assert_eq!(
            all,
            [
                "100 Bullets",
                "Preacher",
                "Saga",
                "The Invisibles",
                "Y",
                "Zot"
            ]
        );

        // Unread Vertigo from the nineties.
        let narrowing = Narrowing {
            read: asked([ReadState::Unread]),
            publishers: asked(["Vertigo".to_owned()]),
            years: asked([Years::Decade(1990)]),
            ..Narrowing::default()
        };
        assert_eq!(
            names(&shelf, library, &narrowing).await,
            ["Preacher", "The Invisibles"]
        );

        let by = |narrowing| {
            let shelf = &shelf;
            async move { names(shelf, library, &narrowing).await }
        };
        let read = |choice| Narrowing {
            read: choice,
            ..Narrowing::default()
        };
        assert_eq!(by(read(asked([ReadState::InProgress]))).await, ["Saga"]);
        assert_eq!(by(read(asked([ReadState::Read]))).await, ["Zot"]);
        assert_eq!(
            by(read(asked([ReadState::Read, ReadState::InProgress]))).await,
            ["Saga", "Zot"]
        );
        assert_eq!(
            by(read(ruled_out([ReadState::Unread]))).await,
            ["Saga", "Zot"]
        );
        assert_eq!(
            by(Narrowing {
                publishers: ruled_out(["Vertigo".to_owned(), "Image".to_owned()]),
                ..Narrowing::default()
            })
            .await,
            ["100 Bullets", "Zot"]
        );
        assert_eq!(
            by(Narrowing {
                years: asked([Years::Year(1995), Years::Decade(1980)]),
                ..Narrowing::default()
            })
            .await,
            ["Preacher", "Zot"]
        );
        // One that says no year is from none.
        assert_eq!(
            by(Narrowing {
                years: ruled_out([Years::Decade(1990), Years::Decade(2000)]),
                ..Narrowing::default()
            })
            .await,
            ["100 Bullets", "Saga", "Zot"]
        );

        let choices = shelf.catalog.series_choices(shelf.owner, library).await;
        let choices = choices.unwrap();
        let brian = &choices.writers[0];
        assert_eq!(
            (brian.name.as_str(), brian.series_count),
            ("Brian K. Vaughan", 2)
        );
        assert_eq!(
            by(Narrowing {
                writers: asked([brian.id]),
                publishers: ruled_out(["Image".to_owned()]),
                ..Narrowing::default()
            })
            .await,
            ["Y"]
        );
        // They drew none of them.
        assert!(
            by(Narrowing {
                pencillers: asked([brian.id]),
                ..Narrowing::default()
            })
            .await
            .is_empty()
        );
        let publishers: Vec<(&str, i64)> = choices
            .publishers
            .iter()
            .map(|named| (named.value.as_str(), named.series_count))
            .collect();
        assert_eq!(publishers, [("Vertigo", 3), ("Eclipse", 1), ("Image", 1)]);
        let decades: Vec<(i64, i64)> = choices
            .decades
            .iter()
            .map(|named| (named.value, named.series_count))
            .collect();
        assert_eq!(decades, [(1980, 1), (1990, 2), (2000, 1), (2010, 1)]);
        assert!(choices.pencillers.is_empty());
    }

    #[tokio::test]
    async fn series_come_in_the_order_asked_for_a_part_at_a_time() {
        let (shelf, library) = library().await;
        let everything = Narrowing::default();
        let by = |sort, reverse| {
            let (shelf, everything) = (&shelf, &everything);
            let order = SeriesOrder {
                sort,
                reverse,
                seed: 7,
            };
            async move { ordered_names(shelf, library, everything, order).await }
        };
        assert_eq!(
            by(SeriesSort::Name, true).await,
            [
                "Zot",
                "Y",
                "The Invisibles",
                "Saga",
                "Preacher",
                "100 Bullets"
            ]
        );
        // The ones that say no year last, whichever way round.
        assert_eq!(
            by(SeriesSort::Year, false).await,
            [
                "Zot",
                "The Invisibles",
                "Preacher",
                "Y",
                "Saga",
                "100 Bullets"
            ]
        );
        assert_eq!(
            by(SeriesSort::Year, true).await,
            [
                "Saga",
                "Y",
                "Preacher",
                "The Invisibles",
                "Zot",
                "100 Bullets"
            ]
        );
        // Zot was read before Saga; the rest never, in name order.
        assert_eq!(
            by(SeriesSort::LastRead, false).await[..3],
            ["Saga", "Zot", "100 Bullets"]
        );
        assert_eq!(by(SeriesSort::Unread, false).await.last().unwrap(), "Zot");
        let shuffled = by(SeriesSort::Random, false).await;
        assert_eq!(shuffled, by(SeriesSort::Random, false).await);
        let mut sorted = shuffled.clone();
        sorted.sort();
        assert_eq!(sorted.len(), 6);
        assert_ne!(shuffled, sorted);
        // Another seed is another order, not the same one begun elsewhere:
        // turned until Saga is first, the orders still differ.
        let mut orders = std::collections::BTreeSet::new();
        for seed in 1..=8 {
            let order = SeriesOrder {
                sort: SeriesSort::Random,
                reverse: false,
                seed,
            };
            let mut names = ordered_names(&shelf, library, &everything, order).await;
            let saga = names.iter().position(|name| name == "Saga").unwrap();
            names.rotate_left(saga);
            orders.insert(names);
        }
        assert!(orders.len() >= 4, "{orders:?}");

        let second = Span {
            offset: 2,
            limit: 2,
        };
        let part = shelf
            .catalog
            .series_part(
                shelf.owner,
                library,
                &everything,
                SeriesOrder::default(),
                second,
            )
            .await
            .unwrap();
        let names: Vec<&str> = part
            .items
            .iter()
            .map(|series| series.name.as_str())
            .collect();
        assert_eq!((names, part.total), (vec!["Saga", "The Invisibles"], 6));

        let letters = shelf
            .catalog
            .series_letters(shelf.owner, library, &everything, false)
            .await
            .unwrap();
        let letters: Vec<(char, i64, i64)> = letters
            .into_iter()
            .map(|letter| (letter.letter, letter.count, letter.offset))
            .collect();
        assert_eq!(
            letters,
            [
                ('#', 1, 0),
                ('P', 1, 1),
                ('S', 1, 2),
                ('T', 1, 3),
                ('Y', 1, 4),
                ('Z', 1, 5)
            ]
        );

        assert!(matches!(
            shelf
                .catalog
                .series_part(shelf.owner, 99, &everything, SeriesOrder::default(), ALL)
                .await,
            Err(CatalogError::NotFound)
        ));
    }

    #[test]
    fn years_are_read_one_or_ten_at_a_time() {
        assert_eq!(Years::parse("1993").unwrap(), Years::Year(1993));
        assert_eq!(Years::parse("1990s").unwrap(), Years::Decade(1990));
        for bad in ["", "93", "1993s", "19900s", "199x", "s", "-199", "+199"] {
            assert!(Years::parse(bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_name_is_filed_under_its_first_letter_without_its_accent() {
        assert_eq!(letter_of("saga"), 'S');
        assert_eq!(letter_of("Élodie"), 'E');
        assert_eq!(letter_of("  Zot!"), 'Z');
        assert_eq!(letter_of("100 Bullets"), '#');
        assert_eq!(letter_of("¡Hola!"), '#');
        assert_eq!(letter_of(""), '#');
    }
}
