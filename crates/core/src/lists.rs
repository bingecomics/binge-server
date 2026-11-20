//! Reading lists: books in an order someone chose, across series.
//!
//! A list belongs to the person who made it. Only they change it, and they
//! share it with the people they choose or with everyone on the server. Its
//! books are held by their identity, so a list keeps a book whose file was
//! moved, renamed, or is gone for a while. Everything here is answered for
//! one user: a list shows them the books they may read, and one that was
//! shared with them and would show them none is not there for them.

use std::collections::{HashMap, HashSet};

use sqlx::{SqliteConnection, SqliteExecutor};

use crate::accounts::{UserId, UserName};
use crate::catalog::{
    Book, CHOSEN, COVER, Catalog, CatalogError, UNREAD, YEAR, books, first_copy, may_read, now,
};
use crate::import;
use crate::layout::a_name;
use crate::overview::{BooksOverview, Summary};
use crate::reading::{BooksToMark, MOST_NAMED, TOO_MANY_NAMED, list};

/// How many covers a list shows together, as one.
const COVERS_SHOWN: usize = 4;

/// Longest summary a list takes, in characters.
const MAX_SUMMARY: usize = 4000;

/// What a request that names a book the list cannot take is answered with.
const NOT_ON_SERVER: &str =
    "One of the books, groups, series or reading lists named is not on the server.";

/// What sharing with someone who is not there is refused with.
const NOT_A_USER: &str = "One of the people named is not on the server.";

/// What a file that could not be read is refused with: it has no identity
/// for a list to hold it by.
pub(crate) const UNREADABLE: &str = "A file that could not be read cannot be in a reading list.";

/// A reading list, as one user sees it.
#[derive(Clone, Debug)]
pub struct ReadingList {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    /// How many of its books the user may read and are on the server now.
    pub book_count: i64,
    /// How many of those the user has not read to the end.
    pub unread_count: i64,
    /// Name of the list's cover: the one chosen for it, or else the cover of
    /// its first book that has one.
    pub cover: Option<String>,
    /// Whether the cover is one that was chosen for the list.
    pub cover_chosen: bool,
    /// The covers to show together as one: those of its first books, four
    /// different ones at most. None when a cover was chosen for the list.
    pub covers: Vec<String>,
    /// The earliest and the latest year its books came out: the year of a
    /// book's date where it sits, or else the one its file gives.
    pub first_year: Option<i64>,
    pub last_year: Option<i64>,
    /// Whose list it is.
    pub made_by: UserName,
    /// Whether it is the list of the user it is shown to.
    pub mine: bool,
    /// Whether everyone on the server sees it.
    pub shared_with_all: bool,
    /// The people it is shared with one by one, by name. Only the one whose
    /// list it is is told: for anyone else there are none here.
    pub shared_with: Vec<UserName>,
}

/// Who a list is shared with from now on. It is all of it: the people not
/// named no longer see the list.
#[derive(Clone, Debug, Default)]
pub struct Sharing {
    /// Everyone on the server, those who join later too.
    pub everyone: bool,
    /// The people chosen one by one.
    pub people: Vec<UserId>,
}

/// A reading list seen whole, for one reader.
#[derive(Clone, Debug)]
pub struct ReadingListOverview {
    pub list: ReadingList,
    /// What its books say together, and how far the reader is through them.
    /// What to open next follows what they did last among these books.
    pub about: BooksOverview,
}

/// SQL that holds when the user, `?1`, may see the list whose id is the
/// expression `list`: their own lists, and of the ones shared with them or
/// with everyone, those that hold a book they may read.
pub(crate) fn may_see(list: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM reading_list seen WHERE seen.id = {list} AND (seen.user_id = ?1 \
         OR ((seen.shared_with_all OR EXISTS (SELECT 1 FROM reading_list_share shared \
              WHERE shared.list_id = seen.id AND shared.user_id = ?1)) \
             AND EXISTS (SELECT 1 FROM reading_list_book lb \
              JOIN book b ON b.content_hash = lb.content_hash \
              WHERE lb.list_id = seen.id AND {}))))",
        may_read("b.library_id")
    )
}

/// Selects the lists a user may see, without what is worked out from their
/// books. Takes the user, then whatever `rest` asks for, from `?2`.
pub(crate) fn lists(rest: &str) -> String {
    format!(
        "SELECT rl.id, rl.name, rl.summary, lc.cover AS chosen, rl.user_id, \
         maker.name AS made_by, rl.user_id = ?1 AS mine, rl.shared_with_all \
         FROM reading_list rl JOIN user maker ON maker.id = rl.user_id \
         LEFT JOIN reading_list_cover lc ON lc.list_id = rl.id WHERE {} {rest}",
        may_see("rl.id")
    )
}

/// A list as its own row has it.
#[derive(sqlx::FromRow)]
pub(crate) struct Row {
    id: i64,
    name: String,
    summary: Option<String>,
    chosen: Option<String>,
    user_id: UserId,
    made_by: String,
    mine: bool,
    shared_with_all: bool,
}

/// What a list's summary is worked out from: one row for each file that
/// holds one of its books, in the list's order.
#[derive(sqlx::FromRow)]
struct Held {
    list_id: i64,
    content_hash: String,
    cover: Option<String>,
    year: Option<i64>,
    unread: bool,
}

impl ReadingList {
    /// `held` are the rows of this list, in its order. A book the server
    /// holds twice has two, and counts once. `shared_with` are the people
    /// it is shared with one by one.
    fn of(row: Row, held: &[Held], shared_with: Vec<UserName>) -> Self {
        let mut seen = HashSet::new();
        let books: Vec<&Held> = held
            .iter()
            .filter(|book| seen.insert(book.content_hash.as_str()))
            .collect();
        // Two books may show the same chosen cover.
        let mut shown = HashSet::new();
        let covers: Vec<String> = books
            .iter()
            .filter_map(|book| book.cover.as_deref())
            .filter(|cover| shown.insert(*cover))
            .take(COVERS_SHOWN)
            .map(str::to_owned)
            .collect();
        let years = || books.iter().filter_map(|book| book.year);
        let cover_chosen = row.chosen.is_some();
        Self {
            id: row.id,
            name: row.name,
            summary: row.summary,
            book_count: books.len() as i64,
            unread_count: books.iter().filter(|book| book.unread).count() as i64,
            cover_chosen,
            cover: row.chosen.or_else(|| covers.first().cloned()),
            covers: if cover_chosen { Vec::new() } else { covers },
            first_year: years().min(),
            last_year: years().max(),
            made_by: UserName {
                id: row.user_id,
                name: row.made_by,
            },
            mine: row.mine,
            shared_with_all: row.shared_with_all,
            shared_with,
        }
    }
}

/// A list's name as it is kept.
fn list_name(name: &str) -> Result<&str, CatalogError> {
    a_name(name, "A reading list needs a name.")
}

/// A list's summary as it is kept: without the spaces around it, and none
/// at all when nothing is left of it.
fn list_summary(summary: Option<&str>) -> Result<Option<&str>, CatalogError> {
    let summary = summary.map(str::trim).filter(|text| !text.is_empty());
    if summary.is_some_and(|text| text.chars().count() > MAX_SUMMARY) {
        return Err(CatalogError::Invalid(
            "A summary is at most 4,000 characters.",
        ));
    }
    Ok(summary)
}

/// The list of `user` that has a name, if any. A name is taken among one
/// person's lists: two people may each have a list called the same.
pub(crate) async fn list_with_key(
    db: impl SqliteExecutor<'_>,
    user: UserId,
    name_key: &str,
) -> Result<Option<i64>, CatalogError> {
    Ok(
        sqlx::query_scalar("SELECT id FROM reading_list WHERE user_id = ? AND name_key = ?")
            .bind(user)
            .bind(name_key)
            .fetch_optional(db)
            .await?,
    )
}

/// Whether a list `user` sees is their own. A list they may not see is not
/// there for them.
async fn is_theirs(
    db: impl SqliteExecutor<'_>,
    user: UserId,
    id: i64,
) -> Result<bool, CatalogError> {
    let query = format!(
        "SELECT rl.user_id = ?1 FROM reading_list rl WHERE rl.id = ?2 AND {}",
        may_see("rl.id")
    );
    let mine: Option<bool> = sqlx::query_scalar(&query)
        .bind(user)
        .bind(id)
        .fetch_optional(db)
        .await?;
    mine.ok_or(CatalogError::NotFound)
}

/// Refuses unless the list is `user`'s own: only the one who made a list
/// changes it. A list they may not see is not there for them.
async fn theirs(db: impl SqliteExecutor<'_>, user: UserId, id: i64) -> Result<(), CatalogError> {
    if is_theirs(db, user, id).await? {
        Ok(())
    } else {
        Err(CatalogError::NotYours)
    }
}

/// Puts books at the end of a list, in the order given, and returns how
/// many that added. A book the list already holds stays where it is.
async fn append(
    tx: &mut SqliteConnection,
    list_id: i64,
    identities: &[String],
) -> Result<u64, CatalogError> {
    let mut next: i64 = sqlx::query_scalar(
        "SELECT COALESCE(MAX(position), -1) + 1 FROM reading_list_book WHERE list_id = ?",
    )
    .bind(list_id)
    .fetch_one(&mut *tx)
    .await?;
    let mut added = 0;
    for identity in identities {
        let put = sqlx::query(
            "INSERT OR IGNORE INTO reading_list_book (list_id, content_hash, position) \
             VALUES (?, ?, ?)",
        )
        .bind(list_id)
        .bind(identity)
        .bind(next)
        .execute(&mut *tx)
        .await?;
        if put.rows_affected() > 0 {
            added += 1;
            next += 1;
        }
    }
    Ok(added)
}

impl Catalog {
    /// The reading lists `user` may see, in name order.
    pub async fn reading_lists(&self, user: UserId) -> Result<Vec<ReadingList>, CatalogError> {
        let query = lists("ORDER BY rl.name COLLATE natsort, rl.id");
        let rows = sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        self.with_books(user, rows).await
    }

    pub async fn reading_list(&self, user: UserId, id: i64) -> Result<ReadingList, CatalogError> {
        let query = lists("AND rl.id = ?2");
        let rows = sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_all(self.db.pool())
            .await?;
        let found = self.with_books(user, rows).await?;
        found.into_iter().next().ok_or(CatalogError::NotFound)
    }

    /// The reading lists that hold a book, in name order. None for a file
    /// that could not be read.
    pub async fn lists_of_book(
        &self,
        user: UserId,
        book_id: i64,
    ) -> Result<Vec<ReadingList>, CatalogError> {
        let Some(identity) = self.book(user, book_id).await?.content_hash else {
            return Ok(Vec::new());
        };
        let query = lists(
            "AND EXISTS (SELECT 1 FROM reading_list_book of_it \
             WHERE of_it.list_id = rl.id AND of_it.content_hash = ?2) \
             ORDER BY rl.name COLLATE natsort, rl.id",
        );
        let rows = sqlx::query_as(&query)
            .bind(user)
            .bind(identity)
            .fetch_all(self.db.pool())
            .await?;
        self.with_books(user, rows).await
    }

    /// Makes lists of their rows: adds what is worked out from the books
    /// `user` may read in each.
    pub(crate) async fn with_books(
        &self,
        user: UserId,
        rows: Vec<Row>,
    ) -> Result<Vec<ReadingList>, CatalogError> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "SELECT lb.list_id, lb.content_hash, {COVER} AS cover, \
             {YEAR} AS year, {UNREAD} AS unread \
             FROM reading_list_book lb JOIN book b ON b.content_hash = lb.content_hash {CHOSEN} \
             LEFT JOIN book_info i ON i.book_id = b.id \
             LEFT JOIN book_place bp \
             ON bp.library_id = b.library_id AND bp.content_hash = b.content_hash \
             WHERE lb.list_id IN ({}) AND {} ORDER BY lb.list_id, lb.position, b.id",
            list(&ids),
            may_read("b.library_id")
        );
        let held: Vec<Held> = sqlx::query_as(&query)
            .persistent(false)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        let mut by_list: HashMap<i64, Vec<Held>> = HashMap::new();
        for book in held {
            by_list.entry(book.list_id).or_default().push(book);
        }
        let mut shared_with = self.shared_with(&rows).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let held = by_list.remove(&row.id).unwrap_or_default();
                let shared_with = shared_with.remove(&row.id).unwrap_or_default();
                ReadingList::of(row, &held, shared_with)
            })
            .collect())
    }

    /// The people each of these lists is shared with one by one, in name
    /// order, for the lists that are the asker's own. Who else a list was
    /// shared with is not said to someone it was shared with.
    async fn shared_with(&self, rows: &[Row]) -> Result<HashMap<i64, Vec<UserName>>, CatalogError> {
        let mine: Vec<i64> = rows
            .iter()
            .filter(|row| row.mine)
            .map(|row| row.id)
            .collect();
        if mine.is_empty() {
            return Ok(HashMap::new());
        }
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "SELECT shared.list_id, u.id, u.name FROM reading_list_share shared \
             JOIN user u ON u.id = shared.user_id WHERE shared.list_id IN ({}) \
             ORDER BY u.name COLLATE natsort, u.id",
            list(&mine)
        );
        let shares: Vec<(i64, UserId, String)> = sqlx::query_as(&query)
            .persistent(false)
            .fetch_all(self.db.pool())
            .await?;
        let mut by_list: HashMap<i64, Vec<UserName>> = HashMap::new();
        for (list_id, id, name) in shares {
            by_list
                .entry(list_id)
                .or_default()
                .push(UserName { id, name });
        }
        Ok(by_list)
    }

    /// The books of a list `user` may read, in the list's order. A book the
    /// server holds twice is listed once.
    pub async fn books_in_list(
        &self,
        user: UserId,
        list_id: i64,
    ) -> Result<Vec<Book>, CatalogError> {
        // A list holds books its reader may read whether or not it is
        // shown to them, so that is asked first.
        is_theirs(self.db.pool(), user, list_id).await?;
        self.books_of_seen_list(user, list_id).await
    }

    /// The books of a list already known to be there for `user`.
    async fn books_of_seen_list(
        &self,
        user: UserId,
        list_id: i64,
    ) -> Result<Vec<Book>, CatalogError> {
        let query = books(&format!(
            "AND b.content_hash IN \
             (SELECT lb.content_hash FROM reading_list_book lb WHERE lb.list_id = ?2) \
             AND b.id = {} \
             ORDER BY (SELECT lb.position FROM reading_list_book lb \
                       WHERE lb.list_id = ?2 AND lb.content_hash = b.content_hash)",
            first_copy("b")
        ));
        Ok(sqlx::query_as(&query)
            .bind(user)
            .bind(list_id)
            .fetch_all(self.db.pool())
            .await?)
    }

    /// A reading list seen whole, as `user` reads it. What it is about is
    /// what the list says of itself: its books are of several series, so
    /// what one of them says is not about the list.
    pub async fn reading_list_overview(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<ReadingListOverview, CatalogError> {
        let list = self.reading_list(user, id).await?;
        let books = self.books_of_seen_list(user, id).await?;
        let mut about = self.books_overview(user, &books).await?;
        about.summary = list
            .summary
            .clone()
            .map(|text| Summary { text, book: None });
        Ok(ReadingListOverview { list, about })
    }

    /// The books with these ids that `user` may read, by id.
    pub(crate) async fn books_by_id(
        &self,
        user: UserId,
        ids: &[i64],
    ) -> Result<HashMap<i64, Book>, CatalogError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        // The ids are part of the statement, so it is not one to keep.
        let query = books(&format!("AND b.id IN ({})", list(ids)));
        let found: Vec<Book> = sqlx::query_as(&query)
            .persistent(false)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        Ok(found.into_iter().map(|book| (book.id, book)).collect())
    }

    /// The identities of the books named, in the order they are named:
    /// the books, then those of the groups, the series and the lists, each
    /// in the order it is read. A book named twice is there once.
    ///
    /// Refused when something named is not there for `user`, or is a file
    /// that could not be read: a list is given exactly the books asked for.
    async fn identities(
        &self,
        user: UserId,
        which: &BooksToMark,
    ) -> Result<Vec<String>, CatalogError> {
        if which.named() > MOST_NAMED {
            return Err(CatalogError::Invalid(TOO_MANY_NAMED));
        }
        let missing = |error| match error {
            CatalogError::NotFound => CatalogError::Invalid(NOT_ON_SERVER),
            other => other,
        };
        let mut held: Vec<Book> = Vec::new();
        let mut by_id = self.books_by_id(user, &which.books).await?;
        let mut taken = HashSet::new();
        for id in &which.books {
            // Named twice, it was taken the first time.
            if let Some(book) = by_id.remove(id) {
                taken.insert(*id);
                held.push(book);
            } else if !taken.contains(id) {
                return Err(CatalogError::Invalid(NOT_ON_SERVER));
            }
        }
        for id in &which.groups {
            held.extend(self.books_in_group(user, *id).await.map_err(missing)?);
        }
        for id in &which.series {
            held.extend(self.books_in(user, *id).await.map_err(missing)?);
        }
        for id in &which.lists {
            held.extend(self.books_in_list(user, *id).await.map_err(missing)?);
        }

        let mut seen = HashSet::new();
        let mut identities = Vec::with_capacity(held.len());
        for book in held {
            let identity = book.content_hash.ok_or(CatalogError::Invalid(UNREADABLE))?;
            if seen.insert(identity.clone()) {
                identities.push(identity);
            }
        }
        Ok(identities)
    }

    /// Makes a reading list of `user`'s own that holds the books named, in
    /// the order they are named: see [`Catalog::add_to_list`]. Nobody else
    /// sees it until it is shared. A name another of their lists has is
    /// refused.
    pub async fn create_list(
        &self,
        user: UserId,
        name: &str,
        summary: Option<&str>,
        which: &BooksToMark,
    ) -> Result<ReadingList, CatalogError> {
        let name = list_name(name)?;
        let summary = list_summary(summary)?;
        let name_key = import::name_key(name);
        let identities = self.identities(user, which).await?;
        let mut tx = self.db.begin_write().await?;
        if let Some(taken) = list_with_key(&mut *tx, user, &name_key).await? {
            return Err(CatalogError::ListNameTaken(taken));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO reading_list (user_id, name, name_key, summary, created_at) \
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user)
        .bind(name)
        .bind(&name_key)
        .bind(summary)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;
        append(&mut tx, id, &identities).await?;
        tx.commit().await?;
        self.reading_list(user, id).await
    }

    /// Gives a list of `user`'s another name and summary. Its books stay.
    /// It is all of both: without a summary, the list has none.
    pub async fn change_list(
        &self,
        user: UserId,
        id: i64,
        name: &str,
        summary: Option<&str>,
    ) -> Result<ReadingList, CatalogError> {
        let name = list_name(name)?;
        let summary = list_summary(summary)?;
        let name_key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        theirs(&mut *tx, user, id).await?;
        // Its own name written another way is not taken.
        let taken = list_with_key(&mut *tx, user, &name_key).await?;
        if let Some(other) = taken.filter(|other| *other != id) {
            return Err(CatalogError::ListNameTaken(other));
        }
        sqlx::query("UPDATE reading_list SET name = ?, name_key = ?, summary = ? WHERE id = ?")
            .bind(name)
            .bind(&name_key)
            .bind(summary)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.reading_list(user, id).await
    }

    /// Says who a list of `user`'s is shared with from now on: see
    /// [`Sharing`]. Someone named who is not on the server refuses it, and
    /// nothing changes. The one whose list it is needs no sharing, and
    /// naming them changes nothing.
    pub async fn share_list(
        &self,
        user: UserId,
        id: i64,
        sharing: &Sharing,
    ) -> Result<ReadingList, CatalogError> {
        let people: HashSet<UserId> = sharing
            .people
            .iter()
            .copied()
            .filter(|person| *person != user)
            .collect();
        let mut tx = self.db.begin_write().await?;
        theirs(&mut *tx, user, id).await?;
        sqlx::query("UPDATE reading_list SET shared_with_all = ? WHERE id = ?")
            .bind(sharing.everyone)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("DELETE FROM reading_list_share WHERE list_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        for person in people {
            // Nothing is kept of a request that names someone who is not
            // there: leaving here takes back what was written so far.
            let put = sqlx::query(
                "INSERT INTO reading_list_share (list_id, user_id) \
                 SELECT ?, u.id FROM user u WHERE u.id = ?",
            )
            .bind(id)
            .bind(person)
            .execute(&mut *tx)
            .await?;
            if put.rows_affected() == 0 {
                return Err(CatalogError::Invalid(NOT_A_USER));
            }
        }
        tx.commit().await?;
        self.reading_list(user, id).await
    }

    /// Deletes a list of `user`'s. Its books are not touched, nor is
    /// anyone's progress in them.
    pub async fn delete_list(&self, user: UserId, id: i64) -> Result<(), CatalogError> {
        // The cover chosen for it may be shown by nothing else afterwards.
        let _changing = self.changing_covers.lock().await;
        theirs(self.db.pool(), user, id).await?;
        let cover: Option<String> =
            sqlx::query_scalar("SELECT cover FROM reading_list_cover WHERE list_id = ?")
                .bind(id)
                .fetch_optional(self.db.pool())
                .await?;
        let deleted = sqlx::query("DELETE FROM reading_list WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        if deleted.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        if let Some(cover) = cover {
            self.drop_unused_cover(&cover).await?;
        }
        Ok(())
    }

    /// Puts the books named at the end of a list of `user`'s, in the order
    /// they are named, and returns how many that added: a book the list
    /// already holds stays where it is. See [`Catalog::identities`] for the
    /// order and for what is refused.
    pub async fn add_to_list(
        &self,
        user: UserId,
        id: i64,
        which: &BooksToMark,
    ) -> Result<u64, CatalogError> {
        let identities = self.identities(user, which).await?;
        let mut tx = self.db.begin_write().await?;
        theirs(&mut *tx, user, id).await?;
        let added = append(&mut tx, id, &identities).await?;
        tx.commit().await?;
        Ok(added)
    }

    /// Takes a book out of a list of `user`'s. The others keep their order.
    pub async fn remove_from_list(
        &self,
        user: UserId,
        id: i64,
        book_id: i64,
    ) -> Result<(), CatalogError> {
        theirs(self.db.pool(), user, id).await?;
        let identity = self.book(user, book_id).await?.content_hash;
        let removed =
            sqlx::query("DELETE FROM reading_list_book WHERE list_id = ? AND content_hash = ?")
                .bind(id)
                .bind(identity)
                .execute(self.db.pool())
                .await?;
        if removed.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(())
    }

    /// Moves a book of a list of `user`'s to `position` among the books
    /// they are shown, from 0. A position past the end is the end. The
    /// books not shown, whose files are gone for now or are of a library
    /// they no longer read, keep their neighbours.
    pub async fn move_in_list(
        &self,
        user: UserId,
        id: i64,
        book_id: i64,
        position: i64,
    ) -> Result<(), CatalogError> {
        let position = usize::try_from(position).map_err(|_| {
            CatalogError::Invalid("A position in a reading list is counted from 0.")
        })?;
        let moved = self
            .book(user, book_id)
            .await?
            .content_hash
            .ok_or(CatalogError::NotFound)?;
        let mut tx = self.db.begin_write().await?;
        theirs(&mut *tx, user, id).await?;
        // Every book of the list where it is now, and whether `user` is
        // shown it.
        let query = format!(
            "SELECT lb.content_hash, lb.position, EXISTS (SELECT 1 FROM book b \
             WHERE b.content_hash = lb.content_hash AND {}) FROM reading_list_book lb \
             WHERE lb.list_id = ?2 ORDER BY lb.position",
            may_read("b.library_id")
        );
        let was: Vec<(String, i64, bool)> = sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_all(&mut *tx)
            .await?;
        if !was.iter().any(|(identity, ..)| *identity == moved) {
            return Err(CatalogError::NotFound);
        }
        let mut order: Vec<&String> = Vec::with_capacity(was.len());
        // Where the books shown are among the others, in the list's order.
        let mut shown: Vec<usize> = Vec::new();
        for (identity, _, is_shown) in &was {
            if *identity != moved {
                if *is_shown {
                    shown.push(order.len());
                }
                order.push(identity);
            }
        }
        // Before the book shown at that position, or after the last one.
        let to = match shown.get(position) {
            Some(place) => *place,
            None => shown.last().map_or(order.len(), |last| last + 1),
        };
        order.insert(to, &moved);
        let before: HashMap<&String, i64> = was
            .iter()
            .map(|(identity, at, _)| (identity, *at))
            .collect();
        for (place, identity) in order.into_iter().enumerate() {
            let place = place as i64;
            // Most books stay where they were.
            if before.get(identity) == Some(&place) {
                continue;
            }
            sqlx::query(
                "UPDATE reading_list_book SET position = ? WHERE list_id = ? AND content_hash = ?",
            )
            .bind(place)
            .bind(id)
            .bind(identity)
            .execute(&mut *tx)
            .await?;
        }
        Ok(tx.commit().await?)
    }
}
