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
    async fn fails_when_the_folder_is_missing() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("missing").join("binge.db");
        assert!(matches!(Database::open(&path).await, Err(DbError::Open(_))));
    }
}
