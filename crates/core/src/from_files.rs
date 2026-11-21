//! Reading lists and collections a library makes from what its files say.
//!
//! A file may say that its book is part of a story arc, with the book's
//! number in it, and that its series is part of a group. A library whose
//! owner asks for it makes a reading list of each arc and a collection of
//! each group. They are the owner's, shared with everyone, and from then on
//! they are a list and a collection like any other: the owner renames,
//! changes, shares and deletes them.
//!
//! The files only ever add. A file that is read says what its book is part
//! of, and that waits until a scan puts the book where the file says. A
//! scan takes nothing out: what the owner took out by hand stays out until
//! its file is read again. Nothing here writes to a file.

use std::cmp::Ordering;
use std::collections::{BTreeMap, HashMap, HashSet};

use sqlx::SqliteConnection;

use crate::accounts::UserId;
use crate::catalog::{Catalog, CatalogError, Library};
use crate::collections::{self, COLLECTIONS};
use crate::comicinfo::ComicInfo;
use crate::import;
use crate::lists::{self, LISTS};
use crate::natural::natural_cmp;
use crate::reading::list;
use crate::scan::ROWS_PER_STATEMENT;

/// What a library makes from what its files say.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FromFiles {
    /// A reading list of each story arc, in the order the files give.
    pub lists: bool,
    /// A collection of each group of series.
    pub collections: bool,
}

/// Writes down the arcs and the groups of series a book's file names, in
/// place of what it named before.
pub(crate) async fn note(
    tx: &mut SqliteConnection,
    book_id: i64,
    info: &ComicInfo,
) -> Result<(), sqlx::Error> {
    for table in ["book_arc", "book_series_group"] {
        sqlx::query(&format!("DELETE FROM {table} WHERE book_id = ?"))
            .bind(book_id)
            .execute(&mut *tx)
            .await?;
    }
    // A file may name one thing twice: the first time counts.
    for (name, number) in info.arcs() {
        sqlx::query(
            "INSERT OR IGNORE INTO book_arc (book_id, name_key, name, number) \
             VALUES (?, ?, ?, ?)",
        )
        .bind(book_id)
        .bind(import::name_key(name))
        .bind(name)
        .bind(number)
        .execute(&mut *tx)
        .await?;
    }
    for name in info.series_groups() {
        sqlx::query(
            "INSERT OR IGNORE INTO book_series_group (book_id, name_key, name) VALUES (?, ?, ?)",
        )
        .bind(book_id)
        .bind(import::name_key(name))
        .bind(name)
        .execute(&mut *tx)
        .await?;
    }
    Ok(())
}

/// Selects the books (`b`) whose files name an arc (`a`), in the libraries
/// that make lists of them. Whatever narrows it is added after.
const IN_ARCS: &str = "SELECT a.book_id, a.name_key, a.name, b.content_hash, a.number, \
     b.name AS book \
     FROM book_arc a JOIN book b ON b.id = a.book_id JOIN library l ON l.id = b.library_id \
     WHERE l.lists_from_files AND b.content_hash IS NOT NULL";

/// Selects the series (of the place `bp`) whose books' files (`b`) name a
/// group (`g`), in the libraries that make collections of them, each with
/// whether the group already had the series from another of its books.
/// Whatever narrows it is added after.
const IN_GROUPS: &str = "SELECT g.book_id, g.name_key, g.name, bp.series_id, \
     EXISTS (SELECT 1 FROM book_series_group said JOIN book other ON other.id = said.book_id \
             JOIN book_place there ON there.library_id = other.library_id \
             AND there.content_hash = other.content_hash \
             WHERE said.name_key = g.name_key AND NOT said.waiting \
             AND there.series_id = bp.series_id) AS had \
     FROM book_series_group g JOIN book b ON b.id = g.book_id \
     JOIN library l ON l.id = b.library_id JOIN book_place bp \
     ON bp.library_id = b.library_id AND bp.content_hash = b.content_hash \
     WHERE l.collections_from_files";

/// Which of what the files of a library say is put in lists and
/// collections.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Waiting {
    /// What waits to be: what was read since the last time. A scan does
    /// this, so what the owner took out since stays out.
    Only,
    /// All of it, as when the owner turns a choice on.
    OrNot,
}

impl Waiting {
    /// What narrows a select to the rows (`said`) of a library, `?1`,
    /// that are meant.
    fn of_library(self, said: &str) -> String {
        match self {
            Self::Only => format!(" AND b.library_id = ?1 AND {said}.waiting"),
            Self::OrNot => " AND b.library_id = ?1".to_owned(),
        }
    }
}

/// A book whose file names an arc.
#[derive(sqlx::FromRow)]
struct InArc {
    book_id: i64,
    name_key: String,
    /// The arc, as this file writes it.
    name: String,
    content_hash: String,
    number: Option<f64>,
    /// The file's name, for the order of books the files give no number.
    book: String,
}

/// A series one of whose books names a group.
#[derive(sqlx::FromRow)]
struct InGroup {
    book_id: i64,
    name_key: String,
    /// The group, as the file writes it.
    name: String,
    series_id: i64,
    /// Whether the group had the series already, from another of its
    /// books. A series is put in a collection once: each new issue names
    /// the group again, and would bring back a series its owner took out.
    had: bool,
}

/// The order the files give the books of an arc: by their numbers in it,
/// then the ones with no number, by name.
fn in_the_arc(a: &InArc, b: &InArc) -> Ordering {
    let by_number = match (a.number, b.number) {
        (Some(a), Some(b)) => a.total_cmp(&b),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    };
    by_number
        .then_with(|| natural_cmp(&a.book, &b.book))
        .then_with(|| a.content_hash.cmp(&b.content_hash))
}

/// Where a book with this number in an arc goes among the books of a list,
/// each with its own number in the arc: after the last one that comes
/// before it or with it, or else before the first that comes after it.
/// With no number, or among books that have none, it goes last. The books
/// with no number there, which someone put by hand, keep their neighbours.
fn place_for(order: &[(&str, Option<f64>)], number: Option<f64>) -> usize {
    let Some(number) = number else {
        return order.len();
    };
    let numbered = |at: &(&str, Option<f64>)| at.1;
    match order
        .iter()
        .rposition(|at| numbered(at).is_some_and(|other| other <= number))
    {
        Some(before) => before + 1,
        None => order
            .iter()
            .position(|at| numbered(at).is_some())
            .unwrap_or(order.len()),
    }
}

/// Marks what these books' files said of a name as done with: it waits
/// no more. `table` is where it is kept.
async fn done_with(
    tx: &mut SqliteConnection,
    table: &str,
    name_key: &str,
    books: impl Iterator<Item = i64>,
) -> Result<(), sqlx::Error> {
    let books: Vec<i64> = books.collect();
    for some in books.chunks(ROWS_PER_STATEMENT) {
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "UPDATE {table} SET waiting = 0 WHERE name_key = ? AND waiting AND book_id IN ({})",
            list(some)
        );
        sqlx::query(&query)
            .persistent(false)
            .bind(name_key)
            .execute(&mut *tx)
            .await?;
    }
    Ok(())
}

/// What was named, each name with all that names it. `key` gives the name
/// as names are compared.
fn by_name<T>(named: Vec<T>, key: impl Fn(&T) -> &String) -> BTreeMap<String, Vec<T>> {
    let mut by_name: BTreeMap<String, Vec<T>> = BTreeMap::new();
    for one in named {
        by_name.entry(key(&one).clone()).or_default().push(one);
    }
    by_name
}

/// Of the ways the files write a name, always the same one.
fn spelling<'a>(names: impl Iterator<Item = &'a str>) -> &'a str {
    names.min().unwrap_or_default()
}

/// Puts books in a list, each where its number in the arc says, and leaves
/// the ones the list already holds where they are. `books` come in the
/// order the files give.
async fn put_in_list(
    tx: &mut SqliteConnection,
    list_id: i64,
    arc: &str,
    books: &[InArc],
) -> Result<(), sqlx::Error> {
    // Every book of the list where it is now, with its number in the arc.
    let held: Vec<(String, i64, Option<f64>)> = sqlx::query_as(
        "SELECT lb.content_hash, lb.position, (SELECT MIN(a.number) FROM book b \
         JOIN book_arc a ON a.book_id = b.id \
         WHERE b.content_hash = lb.content_hash AND a.name_key = ?2) \
         FROM reading_list_book lb WHERE lb.list_id = ?1 ORDER BY lb.position",
    )
    .bind(list_id)
    .bind(arc)
    .fetch_all(&mut *tx)
    .await?;
    let before: HashMap<&str, i64> = held
        .iter()
        .map(|(identity, at, _)| (identity.as_str(), *at))
        .collect();
    let mut there: HashSet<&str> = before.keys().copied().collect();
    let mut order: Vec<(&str, Option<f64>)> = held
        .iter()
        .map(|(identity, _, number)| (identity.as_str(), *number))
        .collect();
    for book in books {
        // A book the library holds twice is one book.
        if there.insert(&book.content_hash) {
            let at = place_for(&order, book.number);
            order.insert(at, (&book.content_hash, book.number));
        }
    }
    if order.len() == held.len() {
        return Ok(());
    }
    let order = order.into_iter().map(|(identity, _)| identity);
    lists::set_order(tx, list_id, &before, order).await
}

impl Catalog {
    /// Makes and fills the owner's reading lists from the arcs the files of
    /// a library name. Each list is written by itself, so that reading
    /// goes on between two of them, and one that fails leaves its books
    /// waiting.
    async fn fill_lists(
        &self,
        owner: UserId,
        library_id: i64,
        which: Waiting,
    ) -> Result<(), CatalogError> {
        let named: Vec<InArc> = sqlx::query_as(&format!("{IN_ARCS}{}", which.of_library("a")))
            .bind(library_id)
            .fetch_all(self.db.pool())
            .await?;
        for (arc, mut books) in by_name(named, |book| &book.name_key) {
            let name = spelling(books.iter().map(|book| book.name.as_str())).to_owned();
            let mut tx = self.db.begin_write().await?;
            match LISTS.filled_by_files(&mut tx, owner, &arc, &name).await? {
                Some((list_id, made)) => {
                    if made {
                        // A list is made whole: with every book that names
                        // the arc, not only the ones that waited.
                        books = sqlx::query_as(&format!("{IN_ARCS} AND a.name_key = ?"))
                            .bind(&arc)
                            .fetch_all(&mut *tx)
                            .await?;
                    }
                    books.sort_by(in_the_arc);
                    put_in_list(&mut tx, list_id, &arc, &books).await?;
                }
                None => tracing::warn!(library_id, "an arc's name is too long for a list"),
            }
            let books = books.iter().map(|book| book.book_id);
            done_with(&mut tx, "book_arc", &arc, books).await?;
            tx.commit().await?;
        }
        Ok(())
    }

    /// Makes and fills the owner's collections from the groups of series
    /// the files of a library name, each by itself as the lists are.
    async fn fill_collections(
        &self,
        owner: UserId,
        library_id: i64,
        which: Waiting,
    ) -> Result<(), CatalogError> {
        let named: Vec<InGroup> = sqlx::query_as(&format!("{IN_GROUPS}{}", which.of_library("g")))
            .bind(library_id)
            .fetch_all(self.db.pool())
            .await?;
        for (group, mut series) in by_name(named, |series| &series.name_key) {
            let name = spelling(series.iter().map(|one| one.name.as_str())).to_owned();
            let mut tx = self.db.begin_write().await?;
            let filled = COLLECTIONS.filled_by_files(&mut tx, owner, &group, &name);
            match filled.await? {
                Some((collection_id, made)) => {
                    if made {
                        // A collection is made whole, as a list is.
                        series = sqlx::query_as(&format!("{IN_GROUPS} AND g.name_key = ?"))
                            .bind(&group)
                            .fetch_all(&mut *tx)
                            .await?;
                    }
                    // Only what waited is held back for a series the
                    // collection had: asked for all, it is all.
                    let put = series
                        .iter()
                        .filter(|one| made || which == Waiting::OrNot || !one.had);
                    let ids: Vec<i64> = put.map(|one| one.series_id).collect();
                    collections::put(&mut tx, collection_id, &ids).await?;
                }
                None => tracing::warn!(library_id, "a group's name is too long for a collection"),
            }
            let books = series.iter().map(|one| one.book_id);
            done_with(&mut tx, "book_series_group", &group, books).await?;
            tx.commit().await?;
        }
        Ok(())
    }

    /// Says what a library makes from its files from now on. What is turned
    /// on is made at once, from all its files were found to say. What is
    /// turned off makes nothing more: the lists and collections there are
    /// stay, as the owner's. When it cannot all be made, the library makes
    /// what it made before, so that asking again makes it.
    pub async fn make_from_files(
        &self,
        owner: UserId,
        library_id: i64,
        from: FromFiles,
    ) -> Result<Library, CatalogError> {
        let was = self.library(owner, library_id).await?;
        let was = FromFiles {
            lists: was.lists_from_files,
            collections: was.collections_from_files,
        };
        self.set_made_from_files(library_id, from).await?;
        let turned_on = FromFiles {
            lists: from.lists && !was.lists,
            collections: from.collections && !was.collections,
        };
        let made = self.fill_from_files(library_id, Waiting::OrNot, turned_on);
        if let Err(error) = made.await {
            self.set_made_from_files(library_id, was).await?;
            return Err(error);
        }
        self.library(owner, library_id).await
    }

    async fn set_made_from_files(
        &self,
        library_id: i64,
        from: FromFiles,
    ) -> Result<(), sqlx::Error> {
        sqlx::query(
            "UPDATE library SET lists_from_files = ?, collections_from_files = ? WHERE id = ?",
        )
        .bind(from.lists)
        .bind(from.collections)
        .bind(library_id)
        .execute(self.db.pool())
        .await?;
        Ok(())
    }

    /// What a library makes from its files.
    pub(crate) async fn made_from_files(&self, library_id: i64) -> Result<FromFiles, sqlx::Error> {
        let made: Option<(bool, bool)> = sqlx::query_as(
            "SELECT lists_from_files, collections_from_files FROM library WHERE id = ?",
        )
        .bind(library_id)
        .fetch_optional(self.db.pool())
        .await?;
        Ok(
            made.map_or_else(FromFiles::default, |(lists, collections)| FromFiles {
                lists,
                collections,
            }),
        )
    }

    /// Makes and fills the owner's lists and collections, `made` of them,
    /// from `which` of what the files of a library say.
    pub(crate) async fn fill_from_files(
        &self,
        library_id: i64,
        which: Waiting,
        made: FromFiles,
    ) -> Result<(), CatalogError> {
        if made == FromFiles::default() {
            return Ok(());
        }
        let owner: Option<UserId> = sqlx::query_scalar("SELECT id FROM user WHERE is_owner")
            .fetch_optional(self.db.pool())
            .await?;
        // A server has its owner before it has a library.
        let Some(owner) = owner else {
            return Ok(());
        };
        if made.lists {
            self.fill_lists(owner, library_id, which).await?;
        }
        if made.collections {
            self.fill_collections(owner, library_id, which).await?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures::write_tagged_book;
    use crate::scan::tests::{Shelf, shelf};

    /// A book of `series` whose file names arcs and groups of series.
    fn tagged(shelf: &Shelf, seed: u8, series: &str, name: &str, fields: &[(&str, &str)]) {
        let mut said = vec![("Series", series)];
        said.extend_from_slice(fields);
        let path = shelf.comics.join(series).join(format!("{name}.cbz"));
        write_tagged_book(&path, seed, &said);
    }

    /// A shelf of two series that cross in one arc, one of them in a group.
    async fn crossover() -> Shelf {
        let shelf = shelf().await;
        let arc =
            |number: &'static str| [("StoryArc", "The Long Night"), ("StoryArcNumber", number)];
        tagged(&shelf, 1, "Night Runner", "Night Runner 001", &arc("1"));
        tagged(&shelf, 2, "Night Runner", "Night Runner 002", &arc("3"));
        tagged(
            &shelf,
            3,
            "Day Watch",
            "Day Watch 001",
            &[
                ("StoryArc", "Dawn, the long night"),
                ("StoryArcNumber", "1, 2"),
                ("SeriesGroup", "Night City"),
            ],
        );
        tagged(&shelf, 4, "Day Watch", "Day Watch 002", &[]);
        shelf
    }

    async fn library(shelf: &Shelf, from: FromFiles) -> i64 {
        let added = shelf
            .catalog
            .add_library(shelf.owner, &shelf.comics, None, from)
            .await
            .unwrap();
        shelf.catalog.scan_finished(added.id).await;
        added.id
    }

    const BOTH: FromFiles = FromFiles {
        lists: true,
        collections: true,
    };

    /// The names of the books of the owner's list called `name`, in order.
    async fn in_list(shelf: &Shelf, name: &str) -> Vec<String> {
        let lists = shelf.catalog.reading_lists(shelf.owner).await.unwrap();
        let list = lists.into_iter().find(|list| list.name == name).unwrap();
        let books = shelf.catalog.books_in_list(shelf.owner, list.id);
        let names = books.await.unwrap().into_iter().map(|book| book.name);
        names.collect()
    }

    async fn list_names(shelf: &Shelf) -> Vec<String> {
        let lists = shelf.catalog.reading_lists(shelf.owner).await.unwrap();
        lists.into_iter().map(|list| list.name).collect()
    }

    #[tokio::test]
    async fn a_library_makes_lists_and_collections_from_what_its_files_say() {
        let shelf = crossover().await;
        library(&shelf, BOTH).await;

        // One list for each arc, however its name is written, in the order
        // the numbers give across series.
        assert_eq!(list_names(&shelf).await, ["Dawn", "The Long Night"]);
        assert_eq!(
            in_list(&shelf, "The Long Night").await,
            ["Night Runner 001", "Day Watch 001", "Night Runner 002"]
        );
        let lists = shelf.catalog.reading_lists(shelf.owner).await.unwrap();
        assert!(
            lists
                .iter()
                .all(|list| list.from_files && list.shared_with_all)
        );

        let collections = shelf.catalog.collections(shelf.owner).await.unwrap();
        let [collection] = &collections[..] else {
            panic!("one collection, of the one group");
        };
        assert_eq!(collection.name, "Night City");
        assert!(collection.from_files && collection.shared_with_all);
        let held = shelf
            .catalog
            .series_in_collection(shelf.owner, collection.id);
        let held: Vec<String> = held.await.unwrap().into_iter().map(|s| s.name).collect();
        assert_eq!(held, ["Day Watch"]);
    }

    #[tokio::test]
    async fn a_library_makes_nothing_until_it_is_asked_to() {
        let shelf = crossover().await;
        let id = library(&shelf, FromFiles::default()).await;
        assert!(list_names(&shelf).await.is_empty());
        assert!(
            shelf
                .catalog
                .collections(shelf.owner)
                .await
                .unwrap()
                .is_empty()
        );

        // Turned on, it makes them at once from what the scan kept.
        let only_lists = FromFiles {
            lists: true,
            collections: false,
        };
        let library = shelf.catalog.make_from_files(shelf.owner, id, only_lists);
        assert!(library.await.unwrap().lists_from_files);
        assert_eq!(list_names(&shelf).await, ["Dawn", "The Long Night"]);
        assert!(
            shelf
                .catalog
                .collections(shelf.owner)
                .await
                .unwrap()
                .is_empty()
        );

        // Turned off, what was made stays.
        let library = shelf
            .catalog
            .make_from_files(shelf.owner, id, FromFiles::default());
        assert!(!library.await.unwrap().lists_from_files);
        assert_eq!(list_names(&shelf).await.len(), 2);
    }

    #[tokio::test]
    async fn the_files_add_to_a_list_and_take_nothing_out() {
        let shelf = crossover().await;
        let id = library(&shelf, BOTH).await;
        let catalog = &shelf.catalog;
        let lists = catalog.reading_lists(shelf.owner).await.unwrap();
        let night = lists
            .into_iter()
            .find(|l| l.name == "The Long Night")
            .unwrap();

        // The owner renames it, takes a book out and puts one in by hand.
        catalog
            .change_list(shelf.owner, night.id, "Read this first", None)
            .await
            .unwrap();
        let books = catalog.books_in_list(shelf.owner, night.id).await.unwrap();
        catalog
            .remove_from_list(shelf.owner, night.id, books[0].id)
            .await
            .unwrap();
        let by_hand = shelf.books(id, "Day Watch").await[1].id;
        let which = crate::BooksToMark {
            books: vec![by_hand],
            ..crate::BooksToMark::default()
        };
        catalog
            .add_to_list(shelf.owner, night.id, &which)
            .await
            .unwrap();

        // A new file names the arc: it goes where its number says, in the
        // list the files made, under the name the owner gave it.
        let arc = [("StoryArc", "The Long Night"), ("StoryArcNumber", "2.5")];
        tagged(&shelf, 5, "Night Runner", "Night Runner 001.5", &arc);
        shelf.rescan(id).await;
        assert_eq!(list_names(&shelf).await, ["Dawn", "Read this first"]);
        assert_eq!(
            in_list(&shelf, "Read this first").await,
            [
                "Day Watch 001",
                "Night Runner 001.5",
                "Night Runner 002",
                "Day Watch 002"
            ]
        );

        // Deleted, it is made again, whole, by the next file that names it.
        catalog.delete_list(shelf.owner, night.id).await.unwrap();
        tagged(
            &shelf,
            6,
            "Night Runner",
            "Night Runner 003",
            &[("StoryArc", "The Long Night"), ("StoryArcNumber", "4")],
        );
        shelf.rescan(id).await;
        assert_eq!(
            in_list(&shelf, "The Long Night").await,
            [
                "Night Runner 001",
                "Day Watch 001",
                "Night Runner 001.5",
                "Night Runner 002",
                "Night Runner 003"
            ]
        );
    }

    #[tokio::test]
    async fn a_list_of_that_name_made_by_hand_is_filled() {
        let shelf = crossover().await;
        let id = library(&shelf, FromFiles::default()).await;
        let catalog = &shelf.catalog;
        let mine = catalog
            .create_list(
                shelf.owner,
                "the long night!",
                None,
                &crate::BooksToMark::default(),
            )
            .await
            .unwrap();
        catalog
            .make_from_files(shelf.owner, id, BOTH)
            .await
            .unwrap();

        assert_eq!(list_names(&shelf).await, ["Dawn", "the long night!"]);
        assert_eq!(in_list(&shelf, "the long night!").await.len(), 3);
        // It is still one made by hand, and still only its maker's.
        let mine = catalog.reading_list(shelf.owner, mine.id).await.unwrap();
        assert!(!mine.from_files && !mine.shared_with_all);
    }

    #[tokio::test]
    async fn a_series_taken_out_of_a_collection_is_not_brought_back_by_its_next_issue() {
        let shelf = crossover().await;
        let id = library(&shelf, BOTH).await;
        let catalog = &shelf.catalog;
        let collections = catalog.collections(shelf.owner).await.unwrap();
        let city = collections[0].id;
        let held = || async {
            let series = catalog.series_in_collection(shelf.owner, city);
            let names = series.await.unwrap().into_iter().map(|one| one.name);
            names.collect::<Vec<_>>()
        };
        let day_watch = catalog.series_in_collection(shelf.owner, city);
        let day_watch = day_watch.await.unwrap()[0].id;
        let out = catalog.remove_from_collection(shelf.owner, city, day_watch);
        out.await.unwrap();

        // The next issue names the group as every issue does, and another
        // series names it for the first time.
        let group = [("SeriesGroup", "Night City")];
        tagged(&shelf, 5, "Day Watch", "Day Watch 003", &group);
        tagged(&shelf, 6, "Night Runner", "Night Runner 003", &group);
        shelf.rescan(id).await;
        assert_eq!(held().await, ["Night Runner"]);

        // Asked for all of it again, the library puts back all the files
        // name.
        let off = FromFiles::default();
        catalog.make_from_files(shelf.owner, id, off).await.unwrap();
        catalog
            .make_from_files(shelf.owner, id, BOTH)
            .await
            .unwrap();
        assert_eq!(held().await, ["Day Watch", "Night Runner"]);
    }

    #[tokio::test]
    async fn what_a_scan_could_not_put_in_a_list_waits_for_the_next() {
        let shelf = crossover().await;
        let id = library(&shelf, FromFiles::default()).await;
        let waiting = || async {
            sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM book_arc WHERE waiting")
                .fetch_one(shelf.catalog.db.pool())
                .await
                .unwrap()
        };
        assert_eq!(waiting().await, 4);

        // As after a scan that read the files and stopped before the end:
        // the library makes lists, and none was made.
        sqlx::query("UPDATE library SET lists_from_files = 1")
            .execute(shelf.catalog.db.pool())
            .await
            .unwrap();
        assert!(list_names(&shelf).await.is_empty());

        // The next scan reads no file, and makes them.
        shelf.rescan(id).await;
        assert_eq!(list_names(&shelf).await, ["Dawn", "The Long Night"]);
        assert_eq!(waiting().await, 0);
    }

    #[test]
    fn a_book_goes_where_its_number_says_among_the_ones_there() {
        let order = |numbers: &[Option<f64>]| -> Vec<(&str, Option<f64>)> {
            numbers.iter().map(|n| ("", *n)).collect()
        };
        let there = order(&[Some(1.0), None, Some(3.0), None]);
        assert_eq!(place_for(&there, Some(0.5)), 0);
        assert_eq!(place_for(&there, Some(1.0)), 1);
        assert_eq!(place_for(&there, Some(2.0)), 1);
        assert_eq!(place_for(&there, Some(9.0)), 3);
        assert_eq!(place_for(&there, None), 4);
        assert_eq!(place_for(&order(&[None, None]), Some(1.0)), 2);
        assert_eq!(place_for(&[], Some(1.0)), 0);
    }
}
