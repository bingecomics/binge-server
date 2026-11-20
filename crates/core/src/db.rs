//! The server's single SQLite database.

use std::path::Path;

use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqliteSynchronous};
use sqlx::{Sqlite, SqlitePool, Transaction};

use crate::natural::natural_cmp;

static MIGRATOR: Migrator = sqlx::migrate!("../../migrations");

#[derive(Debug, thiserror::Error)]
pub enum DbError {
    #[error("could not open the database: {0}")]
    Open(#[from] sqlx::Error),
    #[error("could not migrate the database: {0}")]
    Migrate(#[from] sqlx::migrate::MigrateError),
    #[error("the database failed: {0}")]
    Failed(#[source] sqlx::Error),
}

/// Handle to the database. Cheap to clone; clones share one connection pool.
#[derive(Clone, Debug)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Opens the database file, creating it if needed, and brings its schema
    /// up to date.
    pub async fn open(path: &Path) -> Result<Self, DbError> {
        let options = SqliteConnectOptions::new()
            .filename(path)
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            // Enough with a write-ahead log: a crash of the server loses
            // nothing, and commits do not wait for the disk each time.
            .synchronous(SqliteSynchronous::Normal)
            // `ORDER BY name COLLATE natsort` puts "Issue 2" before "Issue 10".
            .collation("natsort", natural_cmp);
        let pool = SqlitePool::connect_with(options).await?;
        MIGRATOR.run(&pool).await?;
        Ok(Self { pool })
    }

    pub(crate) fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Starts a transaction that is going to write. The database has one
    /// writer at a time, and this takes that place from the start, waiting
    /// its turn for it. A transaction that reads first and asks to write
    /// later does not wait: it is refused when another wrote in between, as
    /// two libraries scanned together do to each other.
    pub(crate) async fn begin_write(&self) -> Result<Transaction<'static, Sqlite>, sqlx::Error> {
        self.pool.begin_with("BEGIN IMMEDIATE").await
    }

    /// Whether the server keeps a mark under `key`: a yes of its own that
    /// it remembers from one run to the next, such as a schedule turned on.
    pub async fn is_marked(&self, key: &str) -> Result<bool, DbError> {
        self.has_mark(key).await.map_err(DbError::Failed)
    }

    /// Keeps the mark under `key`, or removes it.
    pub async fn mark(&self, key: &str, on: bool) -> Result<(), DbError> {
        self.set_mark(key, on).await.map_err(DbError::Failed)
    }

    /// [`Database::is_marked`], for this crate, whose errors hold the
    /// database's own.
    pub(crate) async fn has_mark(&self, key: &str) -> Result<bool, sqlx::Error> {
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM setting WHERE key = ?)")
            .bind(key)
            .fetch_one(&self.pool)
            .await
    }

    /// [`Database::mark`], for this crate.
    pub(crate) async fn set_mark(&self, key: &str, on: bool) -> Result<(), sqlx::Error> {
        let change = if on {
            "INSERT OR IGNORE INTO setting (key, value) VALUES (?, '1')"
        } else {
            "DELETE FROM setting WHERE key = ?"
        };
        sqlx::query(change).bind(key).execute(&self.pool).await?;
        Ok(())
    }

    /// Closes the database once what is under way in it has ended, so that
    /// its file is whole before another process opens it.
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Whether the database answers a trivial query right now.
    pub async fn is_healthy(&self) -> bool {
        sqlx::query_scalar::<_, i64>("SELECT 1")
            .fetch_one(&self.pool)
            .await
            .is_ok()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    #[tokio::test]
    async fn a_write_waits_its_turn_behind_another() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("binge.db")).await.unwrap();
        let set = "INSERT INTO setting (key, value) VALUES (?, 'x')";

        let mut first = db.begin_write().await.unwrap();
        sqlx::query(set)
            .bind("first")
            .execute(&mut *first)
            .await
            .unwrap();

        // Reads, then writes, while the first is still open.
        let second = tokio::spawn({
            let db = db.clone();
            async move {
                let mut second = db.begin_write().await?;
                let seen: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM setting")
                    .fetch_one(&mut *second)
                    .await?;
                sqlx::query(set)
                    .bind("second")
                    .execute(&mut *second)
                    .await?;
                second.commit().await?;
                Ok::<_, sqlx::Error>(seen)
            }
        });
        tokio::time::sleep(Duration::from_millis(200)).await;
        first.commit().await.unwrap();

        // It waited, so it saw what the first wrote, and was not refused.
        let before = second.await.unwrap().unwrap();
        let after: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM setting")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(after, before + 1);
    }

    #[tokio::test]
    async fn opens_migrates_and_reopens() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binge.db");

        let db = Database::open(&path).await.unwrap();
        assert!(db.is_healthy().await);
        drop(db);

        // Reapplying a migration would fail, so this also shows that opening
        // an existing database leaves its schema alone.
        let db = Database::open(&path).await.unwrap();
        assert!(db.is_healthy().await);
    }

    #[tokio::test]
    async fn the_lists_that_were_there_become_the_owners_shared_with_everyone() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("binge.db");
        // A server from before a list was its maker's (migration 20).
        let options = SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true)
            .foreign_keys(true);
        let pool = SqlitePool::connect_with(options).await.unwrap();
        let earlier = MIGRATOR.iter().filter(|migration| migration.version < 20);
        let before = Migrator {
            migrations: earlier.cloned().collect::<Vec<_>>().into(),
            ..Migrator::DEFAULT
        };
        before.run(&pool).await.unwrap();
        for statement in [
            "INSERT INTO user (id, name, name_key, password_hash, is_owner, created_at) \
             VALUES (7, 'Ana', 'ana', 'x', 1, 0), (8, 'Ben', 'ben', 'x', 0, 0)",
            "INSERT INTO reading_list (id, name, name_key, summary, created_at) \
             VALUES (1, 'One', 'one', 'About it', 5), (2, 'Two', 'two', NULL, 6), \
             (3, 'Gone', 'gone', NULL, 7)",
            "INSERT INTO reading_list_book (list_id, content_hash, position) \
             VALUES (1, 'aa', 0), (1, 'bb', 4), (2, 'aa', 0)",
            "INSERT INTO reading_list_cover (list_id, cover) VALUES (2, 'cc')",
            "INSERT INTO cbl_rule (entry_key, content_hash) VALUES ('k', 'aa')",
            "DELETE FROM reading_list WHERE id = 3",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;

        let db = Database::open(&path).await.unwrap();
        let lists: Vec<(i64, i64, String, Option<String>, bool, i64)> = sqlx::query_as(
            "SELECT id, user_id, name, summary, shared_with_all, created_at \
             FROM reading_list ORDER BY id",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        assert_eq!(
            lists,
            [
                (1, 7, "One".to_owned(), Some("About it".to_owned()), true, 5),
                (2, 7, "Two".to_owned(), None, true, 6),
            ]
        );
        let books: Vec<(i64, String, i64)> = sqlx::query_as(
            "SELECT list_id, content_hash, position FROM reading_list_book \
             ORDER BY list_id, position",
        )
        .fetch_all(db.pool())
        .await
        .unwrap();
        let book = |list, identity: &str, position| (list, identity.to_owned(), position);
        assert_eq!(
            books,
            [book(1, "aa", 0), book(1, "bb", 4), book(2, "aa", 0)]
        );
        let kept: (i64, String, i64, String) = sqlx::query_as(
            "SELECT c.list_id, c.cover, r.user_id, r.entry_key \
             FROM reading_list_cover c, cbl_rule r",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(kept, (2, "cc".to_owned(), 7, "k".to_owned()));
        // The id of the list that was deleted is not given out again.
        let next: i64 = sqlx::query_scalar(
            "INSERT INTO reading_list (user_id, name, name_key, created_at) \
             VALUES (8, 'Two', 'two', 9) RETURNING id",
        )
        .fetch_one(db.pool())
        .await
        .unwrap();
        assert_eq!(next, 4);
    }

    #[tokio::test]
    async fn fails_when_the_folder_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("binge.db");
        assert!(matches!(Database::open(&path).await, Err(DbError::Open(_))));
    }
}
