//! Collections: series someone picked by hand, from any library.
//!
//! A collection belongs to the person who made it. Only they change it, and
//! they share it with the people they choose or with everyone on the
//! server. It holds the record of each series, so a series stays in it
//! under a new name and with other books, and leaves it when the series is
//! deleted. Its series have no order of their own: they are shown by name.
//! Everything here is answered for one user: a collection shows them the
//! series they may read, and one that was shared with them and would show
//! them none is not there for them.

use std::collections::{HashMap, HashSet};

use crate::accounts::{UserId, UserName};
use crate::catalog::{Catalog, CatalogError, Series, may_read, now, series};
use crate::import;
use crate::layout::a_name;
use crate::reading::{MOST_NAMED, TOO_MANY_NAMED, list};
use crate::sharing::{Kind, Sharing, a_summary, first_covers};

/// What a request that names a series the collection cannot take is
/// answered with.
const NOT_ON_SERVER: &str = "One of the series named is not on the server.";

/// A collection, as one user sees it.
#[derive(Clone, Debug)]
pub struct Collection {
    pub id: i64,
    pub name: String,
    pub summary: Option<String>,
    /// How many of its series the user may read.
    pub series_count: i64,
    /// How many books those series hold.
    pub book_count: i64,
    /// How many of those books the user has not read to the end.
    pub unread_count: i64,
    /// Name of the collection's cover: that of its first series, by name,
    /// that has one.
    pub cover: Option<String>,
    /// The covers to show together as one: those of its first series, four
    /// different ones at most.
    pub covers: Vec<String>,
    /// Whose collection it is.
    pub made_by: UserName,
    /// Whether it is the collection of the user it is shown to.
    pub mine: bool,
    /// Whether everyone on the server sees it.
    pub shared_with_all: bool,
    /// The people it is shared with one by one, by name. Only the one whose
    /// collection it is is told: for anyone else there are none here.
    pub shared_with: Vec<UserName>,
}

/// SQL that holds when the user, `?1`, may see the collection whose id is
/// the expression `collection`: their own, and of the ones shared with them
/// or with everyone, those that hold a series they may read.
pub(crate) fn may_see(collection: &str) -> String {
    format!(
        "EXISTS (SELECT 1 FROM collection seen WHERE seen.id = {collection} \
         AND (seen.user_id = ?1 OR ({} AND EXISTS (SELECT 1 FROM collection_series cs \
              JOIN series s ON s.id = cs.series_id \
              WHERE cs.collection_id = seen.id AND {}))))",
        COLLECTIONS.shared_with_them("seen"),
        may_read("s.library_id")
    )
}

/// Collections, as things that are made and shared.
const COLLECTIONS: Kind = Kind {
    table: "collection",
    shares: "collection_share",
    of: "collection_id",
    may_see,
};

/// Selects the collections a user may see, without what is worked out from
/// their series. Takes the user, then whatever `rest` asks for, from `?2`.
pub(crate) fn collections(rest: &str) -> String {
    format!(
        "SELECT c.id, c.name, c.summary, c.user_id, maker.name AS made_by, \
         c.user_id = ?1 AS mine, c.shared_with_all \
         FROM collection c JOIN user maker ON maker.id = c.user_id WHERE {} {rest}",
        may_see("c.id")
    )
}

/// What every listing of collections ends with: name order.
const BY_NAME: &str = "ORDER BY c.name COLLATE natsort, c.id";

/// A collection as its own row has it.
#[derive(sqlx::FromRow)]
pub(crate) struct Row {
    id: i64,
    name: String,
    summary: Option<String>,
    user_id: UserId,
    made_by: String,
    mine: bool,
    shared_with_all: bool,
}

/// A series of a collection, with the collection it is in.
#[derive(sqlx::FromRow)]
struct Held {
    collection_id: i64,
    #[sqlx(flatten)]
    series: Series,
}

impl Collection {
    /// `held` are the series of this collection the user may read, in name
    /// order. `shared_with` are the people it is shared with one by one.
    fn of(row: Row, held: &[Series], shared_with: Vec<UserName>) -> Self {
        let covers = first_covers(held.iter().filter_map(|series| series.cover.as_deref()));
        Self {
            id: row.id,
            name: row.name,
            summary: row.summary,
            series_count: held.len() as i64,
            book_count: held.iter().map(|series| series.book_count).sum(),
            unread_count: held.iter().map(|series| series.unread_count).sum(),
            cover: covers.first().cloned(),
            covers,
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

/// A collection's name as it is kept.
fn collection_name(name: &str) -> Result<&str, CatalogError> {
    a_name(name, "A collection needs a name.")
}

/// The series named, each once, when `user` may read every one of them.
/// Refused when one is not there for them: a collection is given exactly
/// the series asked for. Asked inside the write that puts them in, so that
/// none of them goes in between.
async fn series_named(
    tx: &mut sqlx::SqliteConnection,
    user: UserId,
    named: &[i64],
) -> Result<Vec<i64>, CatalogError> {
    if named.len() > MOST_NAMED {
        return Err(CatalogError::Invalid(TOO_MANY_NAMED));
    }
    let mut seen = HashSet::new();
    let asked: Vec<i64> = named
        .iter()
        .copied()
        .filter(|id| seen.insert(*id))
        .collect();
    if asked.is_empty() {
        return Ok(asked);
    }
    // The ids are part of the statement, so it is not one to keep.
    let query = format!(
        "SELECT COUNT(*) FROM series s WHERE s.id IN ({}) AND {}",
        list(&asked),
        may_read("s.library_id")
    );
    let there: i64 = sqlx::query_scalar(&query)
        .persistent(false)
        .bind(user)
        .fetch_one(&mut *tx)
        .await?;
    if there as usize != asked.len() {
        return Err(CatalogError::Invalid(NOT_ON_SERVER));
    }
    Ok(asked)
}

/// Puts series in a collection and returns how many that added. A series
/// the collection already holds is not counted.
async fn put(
    tx: &mut sqlx::SqliteConnection,
    collection_id: i64,
    series: &[i64],
) -> Result<u64, CatalogError> {
    let mut added = 0;
    for series_id in series {
        let put = sqlx::query(
            "INSERT OR IGNORE INTO collection_series (collection_id, series_id) VALUES (?, ?)",
        )
        .bind(collection_id)
        .bind(series_id)
        .execute(&mut *tx)
        .await?;
        added += put.rows_affected();
    }
    Ok(added)
}

impl Catalog {
    /// The collections `user` may see, in name order.
    pub async fn collections(&self, user: UserId) -> Result<Vec<Collection>, CatalogError> {
        let query = collections(BY_NAME);
        let rows = sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        self.with_series(user, rows).await
    }

    pub async fn collection(&self, user: UserId, id: i64) -> Result<Collection, CatalogError> {
        let query = collections("AND c.id = ?2");
        let rows = sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_all(self.db.pool())
            .await?;
        let found = self.with_series(user, rows).await?;
        found.into_iter().next().ok_or(CatalogError::NotFound)
    }

    /// The collections that hold a series, in name order.
    pub async fn collections_of_series(
        &self,
        user: UserId,
        series_id: i64,
    ) -> Result<Vec<Collection>, CatalogError> {
        self.series(user, series_id).await?;
        let query = collections(&format!(
            "AND EXISTS (SELECT 1 FROM collection_series of_it \
             WHERE of_it.collection_id = c.id AND of_it.series_id = ?2) {BY_NAME}"
        ));
        let rows = sqlx::query_as(&query)
            .bind(user)
            .bind(series_id)
            .fetch_all(self.db.pool())
            .await?;
        self.with_series(user, rows).await
    }

    /// Makes collections of their rows: adds what is worked out from the
    /// series `user` may read in each.
    pub(crate) async fn with_series(
        &self,
        user: UserId,
        rows: Vec<Row>,
    ) -> Result<Vec<Collection>, CatalogError> {
        if rows.is_empty() {
            return Ok(Vec::new());
        }
        let ids: Vec<i64> = rows.iter().map(|row| row.id).collect();
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "SELECT cs.collection_id, held.* FROM collection_series cs \
             JOIN ({}) held ON held.id = cs.series_id WHERE cs.collection_id IN ({}) \
             ORDER BY cs.collection_id, held.name COLLATE natsort, held.id",
            series(""),
            list(&ids)
        );
        let held: Vec<Held> = sqlx::query_as(&query)
            .persistent(false)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        let mut by_collection: HashMap<i64, Vec<Series>> = HashMap::new();
        for one in held {
            by_collection
                .entry(one.collection_id)
                .or_default()
                .push(one.series);
        }
        // Who else a collection was shared with is not said to someone it
        // was shared with.
        let mine: Vec<i64> = rows
            .iter()
            .filter(|row| row.mine)
            .map(|row| row.id)
            .collect();
        let mut shared_with = COLLECTIONS.shared_with(self.db.pool(), &mine).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let held = by_collection.remove(&row.id).unwrap_or_default();
                let shared_with = shared_with.remove(&row.id).unwrap_or_default();
                Collection::of(row, &held, shared_with)
            })
            .collect())
    }

    /// The series of a collection `user` may read, in name order.
    pub async fn series_in_collection(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<Vec<Series>, CatalogError> {
        // A collection holds series its reader may read whether or not it
        // is shown to them, so that is asked first.
        COLLECTIONS.is_theirs(self.db.pool(), user, id).await?;
        let query = series(
            "AND s.id IN (SELECT cs.series_id FROM collection_series cs \
             WHERE cs.collection_id = ?2) ORDER BY s.name COLLATE natsort, s.id",
        );
        Ok(sqlx::query_as(&query)
            .bind(user)
            .bind(id)
            .fetch_all(self.db.pool())
            .await?)
    }

    /// Makes a collection of `user`'s own that holds the series named.
    /// Nobody else sees it until it is shared. A name another of their
    /// collections has is refused.
    pub async fn create_collection(
        &self,
        user: UserId,
        name: &str,
        summary: Option<&str>,
        series: &[i64],
    ) -> Result<Collection, CatalogError> {
        let name = collection_name(name)?;
        let summary = a_summary(summary)?;
        let name_key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        let series = series_named(&mut tx, user, series).await?;
        if let Some(taken) = COLLECTIONS.with_key(&mut *tx, user, &name_key).await? {
            return Err(CatalogError::CollectionNameTaken(taken));
        }
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO collection (user_id, name, name_key, summary, created_at) \
             VALUES (?, ?, ?, ?, ?) RETURNING id",
        )
        .bind(user)
        .bind(name)
        .bind(&name_key)
        .bind(summary)
        .bind(now())
        .fetch_one(&mut *tx)
        .await?;
        put(&mut tx, id, &series).await?;
        tx.commit().await?;
        self.collection(user, id).await
    }

    /// Gives a collection of `user`'s another name and summary. Its series
    /// stay. It is all of both: without a summary, the collection has none.
    pub async fn change_collection(
        &self,
        user: UserId,
        id: i64,
        name: &str,
        summary: Option<&str>,
    ) -> Result<Collection, CatalogError> {
        let name = collection_name(name)?;
        let summary = a_summary(summary)?;
        let name_key = import::name_key(name);
        let mut tx = self.db.begin_write().await?;
        COLLECTIONS.theirs(&mut *tx, user, id).await?;
        // Its own name written another way is not taken.
        let taken = COLLECTIONS.with_key(&mut *tx, user, &name_key).await?;
        if let Some(other) = taken.filter(|other| *other != id) {
            return Err(CatalogError::CollectionNameTaken(other));
        }
        sqlx::query("UPDATE collection SET name = ?, name_key = ?, summary = ? WHERE id = ?")
            .bind(name)
            .bind(&name_key)
            .bind(summary)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
        self.collection(user, id).await
    }

    /// Says who a collection of `user`'s is shared with from now on: see
    /// [`Sharing`]. Someone named who is not on the server refuses it, and
    /// nothing changes.
    pub async fn share_collection(
        &self,
        user: UserId,
        id: i64,
        sharing: &Sharing,
    ) -> Result<Collection, CatalogError> {
        let mut tx = self.db.begin_write().await?;
        COLLECTIONS.share(&mut tx, user, id, sharing).await?;
        tx.commit().await?;
        self.collection(user, id).await
    }

    /// Deletes a collection of `user`'s. Its series are not touched.
    pub async fn delete_collection(&self, user: UserId, id: i64) -> Result<(), CatalogError> {
        let mut tx = self.db.begin_write().await?;
        COLLECTIONS.theirs(&mut *tx, user, id).await?;
        sqlx::query("DELETE FROM collection WHERE id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        Ok(tx.commit().await?)
    }

    /// Puts the series named in a collection of `user`'s, and returns how
    /// many that added: a series it already holds is not counted. See
    /// [`series_named`] for what is refused.
    pub async fn add_to_collection(
        &self,
        user: UserId,
        id: i64,
        series: &[i64],
    ) -> Result<u64, CatalogError> {
        let mut tx = self.db.begin_write().await?;
        COLLECTIONS.theirs(&mut *tx, user, id).await?;
        let series = series_named(&mut tx, user, series).await?;
        let added = put(&mut tx, id, &series).await?;
        tx.commit().await?;
        Ok(added)
    }

    /// Takes a series out of a collection of `user`'s.
    pub async fn remove_from_collection(
        &self,
        user: UserId,
        id: i64,
        series_id: i64,
    ) -> Result<(), CatalogError> {
        let mut tx = self.db.begin_write().await?;
        COLLECTIONS.theirs(&mut *tx, user, id).await?;
        let removed =
            sqlx::query("DELETE FROM collection_series WHERE collection_id = ? AND series_id = ?")
                .bind(id)
                .bind(series_id)
                .execute(&mut *tx)
                .await?;
        if removed.rows_affected() == 0 {
            return Err(CatalogError::NotFound);
        }
        Ok(tx.commit().await?)
    }
}
