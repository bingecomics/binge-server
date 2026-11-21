//! What a person made and shares: a reading list, a collection.
//!
//! Such a thing belongs to the person who made it. Only they change it, and
//! they share it with the people they choose or with everyone on the
//! server. What is here is the part every kind has in common: whose a thing
//! is, and who it is shared with. What someone it is shared with sees of it
//! is each kind's own to say.

use std::collections::{HashMap, HashSet};

use sqlx::{SqliteConnection, SqliteExecutor, SqlitePool};

use crate::accounts::{UserId, UserName};
use crate::catalog::{CatalogError, now};
use crate::layout::a_name;
use crate::reading::list;

/// What sharing with someone who is not there is refused with.
const NOT_A_USER: &str = "One of the people named is not on the server.";

/// How many covers such a thing shows together, as one.
const COVERS_SHOWN: usize = 4;

/// Longest summary such a thing takes, in characters.
const MAX_SUMMARY: usize = 4000;

/// A summary as it is kept: without the spaces around it, and none at all
/// when nothing is left of it.
pub(crate) fn a_summary(summary: Option<&str>) -> Result<Option<&str>, CatalogError> {
    let summary = summary.map(str::trim).filter(|text| !text.is_empty());
    if summary.is_some_and(|text| text.chars().count() > MAX_SUMMARY) {
        return Err(CatalogError::Invalid(
            "A summary is at most 4,000 characters.",
        ));
    }
    Ok(summary)
}

/// The covers a thing shows together as one: the first few of what it
/// holds, each once. Two of what it holds may show the same chosen cover.
pub(crate) fn first_covers<'a>(covers: impl Iterator<Item = &'a str>) -> Vec<String> {
    let mut shown = HashSet::new();
    covers
        .filter(|cover| shown.insert(*cover))
        .take(COVERS_SHOWN)
        .map(str::to_owned)
        .collect()
}

/// Who a thing is shared with from now on. It is all of it: the people not
/// named no longer see it.
#[derive(Clone, Debug, Default)]
pub struct Sharing {
    /// Everyone on the server, those who join later too.
    pub everyone: bool,
    /// The people chosen one by one.
    pub people: Vec<UserId>,
}

/// A kind of thing that is made and shared, by where it is kept.
pub(crate) struct Kind {
    /// The table of the things. Each has an `id`, the `user_id` of the one
    /// who made it, a `name`, a `name_key`, `shared_with_all`, when it was
    /// made (`created_at`), and `from_files`: the name the files fill it
    /// under, for one they made.
    pub table: &'static str,
    /// The table of the people a thing is shared with one by one, each row
    /// with a `user_id`.
    pub shares: &'static str,
    /// The column of `shares` that says which thing.
    pub of: &'static str,
    /// SQL that holds when the user, `?1`, may see the thing whose id is
    /// the expression given.
    pub may_see: fn(&str) -> String,
}

impl Kind {
    /// SQL that holds when the thing in the row `thing` was shared with
    /// the user, `?1`: with everyone, or with them by name.
    pub(crate) fn shared_with_them(&self, thing: &str) -> String {
        format!(
            "({thing}.shared_with_all OR EXISTS (SELECT 1 FROM {} shared \
             WHERE shared.{} = {thing}.id AND shared.user_id = ?1))",
            self.shares, self.of
        )
    }

    /// The thing of `user` that has a name, if any. A name is taken among
    /// one person's things of a kind: two people may each have one called
    /// the same.
    pub(crate) async fn with_key(
        &self,
        db: impl SqliteExecutor<'_>,
        user: UserId,
        name_key: &str,
    ) -> Result<Option<i64>, CatalogError> {
        let query = format!(
            "SELECT id FROM {} WHERE user_id = ? AND name_key = ?",
            self.table
        );
        Ok(sqlx::query_scalar(&query)
            .bind(user)
            .bind(name_key)
            .fetch_optional(db)
            .await?)
    }

    /// The thing of `user` that the files fill under a name, made when it
    /// is not there, and whether it was made just now. It is the one the
    /// files made from that name, whatever it is called now, or else the
    /// one of that name. One that is made is called `name` and shared with
    /// everyone. Nothing when it is not there and `name` cannot be one.
    pub(crate) async fn filled_by_files(
        &self,
        tx: &mut SqliteConnection,
        user: UserId,
        name_key: &str,
        name: &str,
    ) -> Result<Option<(i64, bool)>, CatalogError> {
        let find = format!(
            "SELECT id FROM {} WHERE user_id = ?1 AND (from_files = ?2 OR name_key = ?2) \
             ORDER BY from_files IS ?2 DESC LIMIT 1",
            self.table
        );
        let found: Option<i64> = sqlx::query_scalar(&find)
            .bind(user)
            .bind(name_key)
            .fetch_optional(&mut *tx)
            .await?;
        if let Some(id) = found {
            return Ok(Some((id, false)));
        }
        // What a file says may be too long to be a name.
        let Ok(name) = a_name(name, "") else {
            return Ok(None);
        };
        let make = format!(
            "INSERT INTO {} (user_id, name, name_key, from_files, shared_with_all, created_at) \
             VALUES (?1, ?2, ?3, ?3, 1, ?4) RETURNING id",
            self.table
        );
        let id = sqlx::query_scalar(&make)
            .bind(user)
            .bind(name)
            .bind(name_key)
            .bind(now())
            .fetch_one(&mut *tx)
            .await?;
        Ok(Some((id, true)))
    }

    /// Whether a thing `user` sees is their own. One they may not see is
    /// not there for them.
    pub(crate) async fn is_theirs(
        &self,
        db: impl SqliteExecutor<'_>,
        user: UserId,
        id: i64,
    ) -> Result<bool, CatalogError> {
        let query = format!(
            "SELECT made.user_id = ?1 FROM {} made WHERE made.id = ?2 AND {}",
            self.table,
            (self.may_see)("made.id")
        );
        let mine: Option<bool> = sqlx::query_scalar(&query)
            .bind(user)
            .bind(id)
            .fetch_optional(db)
            .await?;
        mine.ok_or(CatalogError::NotFound)
    }

    /// Refuses unless the thing is `user`'s own: only the one who made it
    /// changes it. One they may not see is not there for them.
    pub(crate) async fn theirs(
        &self,
        db: impl SqliteExecutor<'_>,
        user: UserId,
        id: i64,
    ) -> Result<(), CatalogError> {
        if self.is_theirs(db, user, id).await? {
            Ok(())
        } else {
            Err(CatalogError::NotYours)
        }
    }

    /// Says who a thing of `user`'s is shared with from now on. Someone
    /// named who is not on the server refuses it: the caller then leaves
    /// without committing, and nothing changes. The one whose it is needs
    /// no sharing, and naming them changes nothing.
    pub(crate) async fn share(
        &self,
        tx: &mut SqliteConnection,
        user: UserId,
        id: i64,
        sharing: &Sharing,
    ) -> Result<(), CatalogError> {
        self.theirs(&mut *tx, user, id).await?;
        let people: HashSet<UserId> = sharing
            .people
            .iter()
            .copied()
            .filter(|person| *person != user)
            .collect();
        sqlx::query(&format!(
            "UPDATE {} SET shared_with_all = ? WHERE id = ?",
            self.table
        ))
        .bind(sharing.everyone)
        .bind(id)
        .execute(&mut *tx)
        .await?;
        sqlx::query(&format!(
            "DELETE FROM {} WHERE {} = ?",
            self.shares, self.of
        ))
        .bind(id)
        .execute(&mut *tx)
        .await?;
        let put = format!(
            "INSERT INTO {} ({}, user_id) SELECT ?, u.id FROM user u WHERE u.id = ?",
            self.shares, self.of
        );
        for person in people {
            let put = sqlx::query(&put)
                .bind(id)
                .bind(person)
                .execute(&mut *tx)
                .await?;
            if put.rows_affected() == 0 {
                return Err(CatalogError::Invalid(NOT_A_USER));
            }
        }
        Ok(())
    }

    /// The people each of these things is shared with one by one, in name
    /// order. It is asked only for the things that are the asker's own:
    /// who else a thing was shared with is not said to someone it was
    /// shared with.
    pub(crate) async fn shared_with(
        &self,
        db: &SqlitePool,
        ids: &[i64],
    ) -> Result<HashMap<i64, Vec<UserName>>, CatalogError> {
        if ids.is_empty() {
            return Ok(HashMap::new());
        }
        // The ids are part of the statement, so it is not one to keep.
        let query = format!(
            "SELECT shared.{of}, u.id, u.name FROM {} shared \
             JOIN user u ON u.id = shared.user_id WHERE shared.{of} IN ({}) \
             ORDER BY u.name COLLATE natsort, u.id",
            self.shares,
            list(ids),
            of = self.of
        );
        let shares: Vec<(i64, UserId, String)> = sqlx::query_as(&query)
            .persistent(false)
            .fetch_all(db)
            .await?;
        let mut by_thing: HashMap<i64, Vec<UserName>> = HashMap::new();
        for (thing, id, name) in shares {
            by_thing
                .entry(thing)
                .or_default()
                .push(UserName { id, name });
        }
        Ok(by_thing)
    }
}
