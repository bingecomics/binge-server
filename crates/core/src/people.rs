//! People: who the books name as their writers and pencillers.
//!
//! Files spell one person several ways: with or without a middle initial,
//! or with a number after the name. So a person is the names they go by.
//! Every spelling a file uses is a name, a credit holds the name as the
//! file spells it, and making two people one moves the names of one to the
//! other. Two spellings that compare the same, as the names of series are
//! compared, are one name from the start. Everything past that is the
//! owner's to say: the server merges nobody by itself.

use std::collections::HashMap;

use sqlx::SqliteConnection;

use crate::accounts::UserId;
use crate::catalog::{
    Book, Catalog, CatalogError, PLACED, PartOf, Series, Span, books, may_read, series,
};
use crate::comicinfo::ComicInfo;
use crate::import;
use crate::layout::{MAX_NAME, a_name};
use crate::reading::list;
use crate::search;

/// The setting that says the books read before credits were kept have
/// theirs.
const CAUGHT_UP: &str = "people.credits_caught_up";

/// How many people one merge takes into another.
const MOST_MERGED: usize = 50;

/// How many books one question to the database asks about.
const BOOKS_PER_STATEMENT: usize = 500;

/// What a book's file can name a person for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, sqlx::Type)]
#[sqlx(rename_all = "snake_case")]
pub enum Role {
    Writer,
    Penciller,
}

impl Role {
    /// The role as a credit keeps it.
    pub(crate) fn kept(self) -> &'static str {
        match self {
            Self::Writer => "writer",
            Self::Penciller => "penciller",
        }
    }
}

/// A person, as one user sees them: counted over the books that user may
/// read. Someone none of those books names is not there for them.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct Person {
    pub id: i64,
    /// The name they are shown by: one of their names.
    pub name: String,
    /// Books that name them, for whatever job.
    pub book_count: i64,
    /// Books that name them as a writer.
    pub written_count: i64,
    /// Books that name them as a penciller.
    pub pencilled_count: i64,
    /// Series that hold those books.
    pub series_count: i64,
}

/// One way of writing a person's name.
#[derive(Clone, Debug, sqlx::FromRow)]
pub struct PersonName {
    pub id: i64,
    pub name: String,
}

/// Someone some books name for a job.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Credited {
    /// The person.
    pub id: i64,
    pub name: String,
}

/// Who some books name, the one most of them name first.
#[derive(Clone, Debug, Default)]
pub struct Credits {
    pub writers: Vec<Credited>,
    pub pencillers: Vec<Credited>,
}

/// Joins to books (`b`) the credits (`c`) that say who they name and for
/// what, each with the name (`n`) it spells them by.
pub(crate) const NAMED: &str = "JOIN book_credit c ON c.book_id = b.id \
     JOIN person_name n ON n.id = c.name_id";

/// SQL that holds for a book `b` that names the person `?2`. Their books
/// are found from their names, not asked of every book.
const NAMES_THEM: &str = "b.id IN (SELECT c.book_id FROM person_name n \
     JOIN book_credit c ON c.name_id = n.id WHERE n.person_id = ?2)";

/// Who a file names and for what, each name as the file spells it.
fn named(info: &ComicInfo) -> impl Iterator<Item = (Role, &str)> {
    (info.writers().map(|name| (Role::Writer, name)))
        .chain(info.pencillers().map(|name| (Role::Penciller, name)))
}

/// What ties a book (`b`) a user may read to the person (`p`) it names.
fn theirs() -> String {
    format!(
        "{NAMED} WHERE n.person_id = p.id AND {}",
        may_read("b.library_id")
    )
}

/// Selects the people (`p`) that books a user may read name. Takes the
/// user, then whatever `rest` asks for, from `?2`.
pub(crate) fn people(rest: &str) -> String {
    let theirs = theirs();
    let (writer, penciller) = (Role::Writer.kept(), Role::Penciller.kept());
    format!(
        "SELECT p.id, p.name, \
         (SELECT COUNT(DISTINCT b.id) FROM book b {theirs}) AS book_count, \
         (SELECT COUNT(DISTINCT b.id) FROM book b {theirs} AND c.role = '{writer}') \
          AS written_count, \
         (SELECT COUNT(DISTINCT b.id) FROM book b {theirs} AND c.role = '{penciller}') \
          AS pencilled_count, \
         (SELECT COUNT(DISTINCT bp.series_id) FROM {PLACED} {theirs}) AS series_count \
         FROM person p WHERE EXISTS (SELECT 1 FROM book b {theirs}) {rest}"
    )
}

/// Counts the people [`people`] selects with the same `rest`, which may
/// narrow and has no use for an order.
pub(crate) fn people_counted(rest: &str) -> String {
    format!(
        "SELECT COUNT(*) FROM person p WHERE EXISTS (SELECT 1 FROM book b {}) {rest}",
        theirs()
    )
}

/// A name as a file spells it, cut to what a name may be.
fn spelled(name: &str) -> &str {
    match name.char_indices().nth(MAX_NAME) {
        Some((end, _)) => name[..end].trim_end(),
        None => name,
    }
}

/// The name with this spelling, made with a person of its own when no file
/// spelled it before.
async fn name_id(tx: &mut SqliteConnection, name: &str) -> Result<i64, sqlx::Error> {
    let name = spelled(name);
    let key = import::name_key(name);
    let known: Option<i64> = sqlx::query_scalar("SELECT id FROM person_name WHERE name_key = ?")
        .bind(&key)
        .fetch_optional(&mut *tx)
        .await?;
    if let Some(id) = known {
        return Ok(id);
    }
    let person: i64 = sqlx::query_scalar("INSERT INTO person (name) VALUES (?) RETURNING id")
        .bind(name)
        .fetch_one(&mut *tx)
        .await?;
    add_name(tx, person, name, &key).await
}

async fn add_name(
    tx: &mut SqliteConnection,
    person: i64,
    name: &str,
    key: &str,
) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar(
        "INSERT INTO person_name (person_id, name, name_key, search_key) \
         VALUES (?, ?, ?, ?) RETURNING id",
    )
    .bind(person)
    .bind(name)
    .bind(key)
    .bind(search::key([name]))
    .fetch_one(&mut *tx)
    .await
}

/// Writes down who a book's file names, in place of what it named before.
pub(crate) async fn credit(
    tx: &mut SqliteConnection,
    book_id: i64,
    info: &ComicInfo,
) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM book_credit WHERE book_id = ?")
        .bind(book_id)
        .execute(&mut *tx)
        .await?;
    for (role, name) in named(info) {
        let name_id = name_id(tx, name).await?;
        // A file may name one person twice for a job.
        sqlx::query("INSERT OR IGNORE INTO book_credit (book_id, role, name_id) VALUES (?, ?, ?)")
            .bind(book_id)
            .bind(role)
            .bind(name_id)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// Whether there is such a person, whoever may see them.
async fn is_person(tx: &mut SqliteConnection, id: i64) -> Result<bool, sqlx::Error> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM person WHERE id = ?)")
        .bind(id)
        .fetch_one(tx)
        .await
}

impl Catalog {
    /// Credits the books whose files were read before credits were kept,
    /// from what was kept of them then. It is done once: a scan credits
    /// every book it reads from then on.
    pub async fn catch_up_credits(&self) -> Result<(), CatalogError> {
        let done: bool = sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting WHERE key = ?)")
            .bind(CAUGHT_UP)
            .fetch_one(self.db.pool())
            .await?;
        if done {
            return Ok(());
        }
        let mut tx = self.db.begin_write().await?;
        let behind: Vec<(i64, Option<String>, Option<String>)> = sqlx::query_as(
            "SELECT i.book_id, i.writer, i.penciller FROM book_info i \
             WHERE (i.writer IS NOT NULL OR i.penciller IS NOT NULL) \
             AND NOT EXISTS (SELECT 1 FROM book_credit c WHERE c.book_id = i.book_id)",
        )
        .fetch_all(&mut *tx)
        .await?;
        for (book_id, writer, penciller) in behind {
            let info = ComicInfo {
                writer,
                penciller,
                ..ComicInfo::default()
            };
            credit(&mut tx, book_id, &info).await?;
        }
        sqlx::query("INSERT OR IGNORE INTO setting (key, value) VALUES (?, '1')")
            .bind(CAUGHT_UP)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(())
    }

    pub async fn person(&self, user: UserId, id: i64) -> Result<Person, CatalogError> {
        let query = people("AND p.id = ?2");
        sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(self.db.pool())
            .await?
            .ok_or(CatalogError::NoSuchPerson)
    }

    /// A person with the names they go by, the one they are shown by first.
    pub async fn person_with_names(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<(Person, Vec<PersonName>), CatalogError> {
        let person = self.person(user, id).await?;
        let names = sqlx::query_as(
            "SELECT n.id, n.name FROM person_name n JOIN person p ON p.id = n.person_id \
             WHERE n.person_id = ? ORDER BY n.name <> p.name, n.name COLLATE natsort",
        )
        .bind(id)
        .fetch_all(self.db.pool())
        .await?;
        Ok((person, names))
    }

    /// The series that hold a book that names the person, in name order.
    pub async fn series_of_person(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<Vec<Series>, CatalogError> {
        let query = series(&format!(
            "AND s.id IN (SELECT bp.series_id FROM {PLACED} WHERE {NAMES_THEM}) \
             ORDER BY s.name COLLATE natsort, s.id"
        ));
        let found: Vec<Series> = sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_all(self.db.pool())
            .await?;
        if found.is_empty() {
            self.person(user, id).await?;
        }
        Ok(found)
    }

    /// A part of the books that name the person, from the first that came
    /// out. The ones that say no date come last, by series and number.
    pub async fn books_of_person(
        &self,
        user: UserId,
        id: i64,
        span: Span,
    ) -> Result<PartOf<Book>, CatalogError> {
        let query = books(&format!(
            "AND {NAMES_THEM} \
             ORDER BY COALESCE(bp.published, i.year * 10000 + 9999) NULLS LAST, \
             bs.name COLLATE natsort, bp.number COLLATE natsort NULLS LAST, \
             b.name COLLATE natsort, b.id LIMIT ?3 OFFSET ?4"
        ));
        let count = format!(
            "SELECT COUNT(*) FROM book b WHERE {} AND {NAMES_THEM}",
            may_read("b.library_id")
        );
        let (items, total): (Vec<Book>, i64) = tokio::try_join!(
            sqlx::query_as(&query)
                .bind(user)
                .bind(id)
                .bind(span.limit)
                .bind(span.offset)
                .fetch_all(self.db.pool()),
            sqlx::query_scalar(&count)
                .bind(user)
                .bind(id)
                .fetch_one(self.db.pool()),
        )?;
        if total == 0 {
            self.person(user, id).await?;
        }
        Ok(PartOf { items, total })
    }

    /// Who these books name. Two spellings of one person count as that
    /// person.
    pub(crate) async fn credited(&self, books: &[Book]) -> Result<Credits, CatalogError> {
        let ids: Vec<i64> = books.iter().map(|book| book.id).collect();
        // How many times the books name each person for each job.
        let mut times: HashMap<(Role, i64), (String, i64)> = HashMap::new();
        for some in ids.chunks(BOOKS_PER_STATEMENT) {
            // The ids are part of the statement, so it is not one to keep.
            let query = format!(
                "SELECT c.role, p.id, p.name, COUNT(*) FROM book_credit c \
                 JOIN person_name n ON n.id = c.name_id JOIN person p ON p.id = n.person_id \
                 WHERE c.book_id IN ({}) GROUP BY c.role, p.id",
                list(some)
            );
            let found: Vec<(Role, i64, String, i64)> = sqlx::query_as(&query)
                .persistent(false)
                .fetch_all(self.db.pool())
                .await?;
            for (role, id, name, count) in found {
                times.entry((role, id)).or_insert((name, 0)).1 += count;
            }
        }
        let mut named: Vec<(Role, Credited, i64)> = times
            .into_iter()
            .map(|((role, id), (name, times))| (role, Credited { id, name }, times))
            .collect();
        named.sort_by(|(_, a, a_times), (_, b, b_times)| {
            b_times.cmp(a_times).then_with(|| a.name.cmp(&b.name))
        });
        let of = |job: Role| {
            let people = named.iter().filter(|(role, ..)| *role == job);
            people.map(|(_, person, _)| person.clone()).collect()
        };
        Ok(Credits {
            writers: of(Role::Writer),
            pencillers: of(Role::Penciller),
        })
    }

    /// Who a book names.
    pub async fn credits_of(&self, user: UserId, book_id: i64) -> Result<Credits, CatalogError> {
        let book = self.book(user, book_id).await?;
        self.credited(std::slice::from_ref(&book)).await
    }

    /// Makes several people one: `others` are gone, and their names are
    /// names of `into`, who keeps the name they are shown by.
    pub async fn merge_people(&self, into: i64, others: &[i64]) -> Result<(), CatalogError> {
        let mut others = others.to_vec();
        others.sort_unstable();
        others.dedup();
        if others.is_empty() {
            return Err(CatalogError::Invalid("Say who to merge."));
        }
        if others.contains(&into) {
            return Err(CatalogError::Invalid(
                "A person cannot be merged with themself.",
            ));
        }
        if others.len() > MOST_MERGED {
            return Err(CatalogError::Invalid("That is too many people at once."));
        }
        let mut tx = self.db.begin_write().await?;
        // The ids are part of the statement, so it is not one to keep.
        let gone = list(&others);
        let there: i64 = sqlx::query_scalar(&format!(
            "SELECT COUNT(*) FROM person WHERE id = ? OR id IN ({gone})"
        ))
        .persistent(false)
        .bind(into)
        .fetch_one(&mut *tx)
        .await?;
        if there as usize != others.len() + 1 {
            return Err(CatalogError::NoSuchPerson);
        }
        sqlx::query(&format!(
            "UPDATE person_name SET person_id = ? WHERE person_id IN ({gone})"
        ))
        .persistent(false)
        .bind(into)
        .execute(&mut *tx)
        .await?;
        sqlx::query(&format!("DELETE FROM person WHERE id IN ({gone})"))
            .persistent(false)
            .execute(&mut *tx)
            .await?;
        crate::views::follow_merge(&mut tx, into, &others).await?;
        tx.commit().await?;
        Ok(())
    }

    /// Gives a person one more name, for the files that will spell them so.
    /// A name that is already theirs changes nothing.
    pub async fn name_person(&self, person: i64, name: &str) -> Result<(), CatalogError> {
        let name = a_name(name, "A person's name cannot be blank.")?;
        let key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        if !is_person(&mut tx, person).await? {
            return Err(CatalogError::NoSuchPerson);
        }
        let whose: Option<i64> =
            sqlx::query_scalar("SELECT person_id FROM person_name WHERE name_key = ?")
                .bind(&key)
                .fetch_optional(&mut *tx)
                .await?;
        match whose {
            Some(theirs) if theirs == person => {}
            Some(another) => return Err(CatalogError::NameOfAnother(another)),
            None => {
                add_name(&mut tx, person, name, &key).await?;
            }
        }
        tx.commit().await?;
        Ok(())
    }

    /// Shows a person by another of their names.
    pub async fn show_person_by(&self, person: i64, name_id: i64) -> Result<(), CatalogError> {
        let shown = sqlx::query(
            "UPDATE person SET name = n.name FROM person_name n \
             WHERE person.id = ?1 AND n.id = ?2 AND n.person_id = ?1",
        )
        .bind(person)
        .bind(name_id)
        .execute(self.db.pool())
        .await?;
        if shown.rows_affected() == 0 {
            return Err(CatalogError::NoSuchPerson);
        }
        Ok(())
    }

    /// Makes one of a person's names a person of its own again, and gives
    /// back that person. The books that spell the name so follow it. The
    /// name they are shown by cannot leave them.
    pub async fn split_person(&self, person: i64, name_id: i64) -> Result<Credited, CatalogError> {
        let mut tx = self.db.begin_write().await?;
        let found: Option<(String, bool)> = sqlx::query_as(
            "SELECT n.name, n.name = p.name FROM person_name n \
             JOIN person p ON p.id = n.person_id WHERE n.id = ? AND n.person_id = ?",
        )
        .bind(name_id)
        .bind(person)
        .fetch_optional(&mut *tx)
        .await?;
        let Some((name, shown_by)) = found else {
            return Err(CatalogError::NoSuchPerson);
        };
        if shown_by {
            return Err(CatalogError::Invalid(
                "That is the name they are shown by. Show them by another name first.",
            ));
        }
        let id: i64 = sqlx::query_scalar("INSERT INTO person (name) VALUES (?) RETURNING id")
            .bind(&name)
            .fetch_one(&mut *tx)
            .await?;
        sqlx::query("UPDATE person_name SET person_id = ? WHERE id = ?")
            .bind(id)
            .bind(name_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        Ok(Credited { id, name })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;
    use crate::scan::tests::{Shelf, shelf};

    /// Three books by one writer spelled three ways, one of them drawn by
    /// someone else.
    async fn written_three_ways() -> (Shelf, i64) {
        let shelf = shelf().await;
        for (path, seed, writer, penciller) in [
            ("Saga/Saga 001.cbz", 1, "Brian K. Vaughan", "Fiona Staples"),
            ("Saga/Saga 002.cbz", 2, "brian k vaughan", "Fiona Staples"),
            (
                "Paper Girls/Paper Girls 001.cbz",
                3,
                "Brian Vaughan (2)",
                "Cliff Chiang",
            ),
        ] {
            fixtures::write_tagged_book(
                &shelf.comics.join(path),
                seed,
                &[("Writer", writer), ("Penciller", penciller)],
            );
        }
        let library = shelf.add().await;
        (shelf, library)
    }

    async fn found(shelf: &Shelf, words: &str) -> Vec<Person> {
        let found = shelf
            .catalog
            .search(shelf.owner, words, Some(10))
            .await
            .unwrap();
        found.people
    }

    #[tokio::test]
    async fn spellings_that_compare_alike_are_one_person() {
        let (shelf, _) = written_three_ways().await;
        let brians = found(&shelf, "vaughan").await;
        let names: Vec<(&str, i64, i64)> = brians
            .iter()
            .map(|person| (person.name.as_str(), person.book_count, person.series_count))
            .collect();
        // The one most books name first, under the spelling seen first.
        assert_eq!(
            names,
            [("Brian K. Vaughan", 2, 1), ("Brian Vaughan (2)", 1, 1)]
        );
        assert_eq!(brians[0].written_count, 2);
        assert_eq!(brians[0].pencilled_count, 0);
        assert!(found(&shelf, "nobody").await.is_empty());
    }

    #[tokio::test]
    async fn people_are_listed_a_part_at_a_time() {
        let (shelf, _) = written_three_ways().await;
        let named = |words: &'static str, offset, limit| {
            let shelf = &shelf;
            async move {
                let span = Span { offset, limit };
                let part = shelf.catalog.people(shelf.owner, words, span).await;
                let part = part.unwrap();
                let names: Vec<String> = part.items.into_iter().map(|one| one.name).collect();
                (names, part.total)
            }
        };
        // The ones most books name first, then by name.
        let everyone = [
            "Brian K. Vaughan",
            "Fiona Staples",
            "Brian Vaughan (2)",
            "Cliff Chiang",
        ];
        assert_eq!(
            named("", 0, 10).await,
            (everyone.map(String::from).to_vec(), 4)
        );
        assert_eq!(
            named("", 1, 2).await,
            (
                everyone[1..3].iter().map(|name| name.to_string()).collect(),
                4
            )
        );
        assert_eq!(
            named("VAUGHAN brian", 0, 10).await,
            (
                vec![
                    "Brian K. Vaughan".to_owned(),
                    "Brian Vaughan (2)".to_owned()
                ],
                2
            )
        );
        assert_eq!(named("nobody", 0, 10).await, (Vec::new(), 0));
        // Someone who may read no library is shown nobody.
        let ben = crate::accounts::testing::member(&shelf.catalog.db, "Ben", &[]).await;
        let none = Span {
            offset: 0,
            limit: 10,
        };
        assert_eq!(shelf.catalog.people(ben, "", none).await.unwrap().total, 0);
    }

    #[tokio::test]
    async fn two_people_are_made_one_and_parted_again() {
        let (shelf, _) = written_three_ways().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let brians = found(&shelf, "vaughan").await;
        let (brian, other) = (brians[0].id, brians[1].id);

        catalog.merge_people(brian, &[other]).await.unwrap();
        let merged = catalog.person(owner, brian).await.unwrap();
        assert_eq!((merged.book_count, merged.series_count), (3, 2));
        assert!(matches!(
            catalog.person(owner, other).await,
            Err(CatalogError::NoSuchPerson)
        ));
        let names = catalog.person_with_names(owner, brian).await.unwrap().1;
        let spelled: Vec<&str> = names.iter().map(|name| name.name.as_str()).collect();
        assert_eq!(spelled, ["Brian K. Vaughan", "Brian Vaughan (2)"]);
        // Found by the other name too, and named once by a series' books.
        assert_eq!(found(&shelf, "vaughan 2").await[0].id, brian);
        let series = catalog.series_of_person(owner, brian).await.unwrap();
        assert_eq!(series.len(), 2);
        let paper = series.iter().find(|s| s.name == "Paper Girls").unwrap();
        let overview = catalog.series_overview(owner, paper.id).await.unwrap();
        assert_eq!(
            overview.about.writers,
            [Credited {
                id: brian,
                name: "Brian K. Vaughan".into()
            }]
        );
        let all = Span {
            offset: 0,
            limit: 10,
        };
        let books = catalog.books_of_person(owner, brian, all).await.unwrap();
        assert_eq!((books.items.len(), books.total), (3, 3));

        // A file read again keeps who its spelling was said to be.
        let library = paper.library_id;
        fixtures::write_tagged_book(
            &shelf.comics.join("Paper Girls/Paper Girls 001.cbz"),
            3,
            &[("Writer", "Brian Vaughan (2)"), ("Title", "Retagged")],
        );
        shelf.rescan(library).await;
        assert_eq!(catalog.person(owner, brian).await.unwrap().book_count, 3);

        // The name they are shown by stays theirs; another can leave.
        let (shown, second) = (names[0].id, names[1].id);
        assert!(matches!(
            catalog.split_person(brian, shown).await,
            Err(CatalogError::Invalid(_))
        ));
        catalog.show_person_by(brian, second).await.unwrap();
        assert_eq!(
            catalog.person(owner, brian).await.unwrap().name,
            "Brian Vaughan (2)"
        );
        let apart = catalog.split_person(brian, shown).await.unwrap();
        assert_eq!(apart.name, "Brian K. Vaughan");
        assert_eq!(catalog.person(owner, apart.id).await.unwrap().book_count, 2);
        assert_eq!(catalog.person(owner, brian).await.unwrap().book_count, 1);
    }

    #[tokio::test]
    async fn a_person_takes_a_name_nobody_else_goes_by() {
        let (shelf, _) = written_three_ways().await;
        let (catalog, owner) = (&shelf.catalog, shelf.owner);
        let brians = found(&shelf, "vaughan").await;
        let (brian, other) = (brians[0].id, brians[1].id);

        catalog.name_person(brian, " B. K. Vaughan ").await.unwrap();
        // Theirs already, however it is spelled.
        catalog.name_person(brian, "BK Vaughan").await.unwrap();
        assert_eq!(
            catalog
                .person_with_names(owner, brian)
                .await
                .unwrap()
                .1
                .len(),
            2
        );
        assert_eq!(found(&shelf, "b. k.").await[0].id, brian);
        assert!(matches!(
            catalog.name_person(brian, "brian vaughan 2").await,
            Err(CatalogError::NameOfAnother(id)) if id == other
        ));
        assert!(matches!(
            catalog.name_person(brian, "  ").await,
            Err(CatalogError::Invalid(_))
        ));
        assert!(matches!(
            catalog.name_person(999, "Someone").await,
            Err(CatalogError::NoSuchPerson)
        ));
        for refused in [
            catalog.merge_people(brian, &[brian]).await,
            catalog.merge_people(brian, &[]).await,
        ] {
            assert!(matches!(refused, Err(CatalogError::Invalid(_))));
        }
        assert!(matches!(
            catalog.merge_people(brian, &[999]).await,
            Err(CatalogError::NoSuchPerson)
        ));
    }

    #[tokio::test]
    async fn books_read_before_credits_were_kept_are_credited() {
        let (shelf, _) = written_three_ways().await;
        let catalog = &shelf.catalog;
        sqlx::query("DELETE FROM person")
            .execute(catalog.db.pool())
            .await
            .unwrap();
        assert!(found(&shelf, "vaughan").await.is_empty());

        catalog.catch_up_credits().await.unwrap();
        catalog.catch_up_credits().await.unwrap();
        assert_eq!(found(&shelf, "vaughan").await.len(), 2);
        assert_eq!(found(&shelf, "staples").await[0].pencilled_count, 2);
    }
}
