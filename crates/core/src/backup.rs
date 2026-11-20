//! Copies of the database, kept in the data directory.
//!
//! A copy is the whole database as it was at one moment, in one file that
//! opens like the database itself. One is made a day while the schedule is
//! on, and the oldest make room for the new. They guard against a mistake
//! or a database that broke, not against a disk that died: they sit on the
//! same disk.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::UNIX_EPOCH;

use crate::catalog::now;
use crate::db::Database;

/// How many copies are kept. Making one more removes the oldest.
pub const COPIES_KEPT: usize = 7;

/// How long after a copy the schedule makes the next, in seconds: a day.
const BETWEEN_COPIES: i64 = 24 * 60 * 60;

/// The setting that says the schedule is off.
const OFF: &str = "backups.off";

/// What a copy's file name starts and ends with. Only files named so are
/// listed, and only they are ever removed.
const STARTS: &str = "binge-";
const ENDS: &str = ".db";

/// What went wrong. `Display` is for the log; [`BackupError::for_people`]
/// is what to tell the owner.
#[derive(Debug, thiserror::Error)]
pub enum BackupError {
    #[error("the copy could not be written: {0}")]
    Io(#[from] std::io::Error),
    #[error("the database failed: {0}")]
    Database(#[from] sqlx::Error),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl BackupError {
    /// What to tell the owner. The cause goes to the log.
    pub fn for_people(&self) -> &str {
        match self {
            Self::Io(_) => {
                "The copy could not be written. Check that the data directory has room \
                 and can be written to."
            }
            Self::Database(_) | Self::Task(_) => crate::FAILED_FOR_PEOPLE,
        }
    }
}

/// One copy of the database.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DatabaseCopy {
    /// The file's name in the folder of copies.
    pub name: String,
    /// In bytes.
    pub size: u64,
    /// Seconds since 1970.
    pub made_at: i64,
}

/// The copies of the database and the schedule that makes them.
///
/// Cheap to clone; clones make one copy at a time between them.
#[derive(Clone)]
pub struct Backups {
    db: Database,
    dir: Arc<Path>,
    making: Arc<tokio::sync::Mutex<()>>,
}

impl Backups {
    /// The copies are kept in `backups` under `data_dir`.
    pub fn new(db: Database, data_dir: &Path) -> Self {
        Self {
            db,
            dir: data_dir.join("backups").into(),
            making: Arc::default(),
        }
    }

    /// The folder the copies are kept in.
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    /// The copies there are, the latest first.
    pub async fn copies(&self) -> Result<Vec<DatabaseCopy>, BackupError> {
        let dir = self.dir.clone();
        Ok(tokio::task::spawn_blocking(move || copies_in(&dir)).await??)
    }

    /// Whether a copy is made every day.
    pub async fn scheduled(&self) -> Result<bool, BackupError> {
        Ok(!self.db.has_mark(OFF).await?)
    }

    /// Turns the daily copy on or off. The copies there are stay.
    pub async fn schedule(&self, on: bool) -> Result<(), BackupError> {
        Ok(self.db.set_mark(OFF, !on).await?)
    }

    /// Makes a copy now, and removes the ones too old to keep.
    pub async fn make(&self) -> Result<DatabaseCopy, BackupError> {
        let _one_at_a_time = self.making.lock().await;
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || std::fs::create_dir_all(&dir)).await??;

        // Named by the moment it is made, to the second, in UTC.
        let stamp: String = sqlx::query_scalar("SELECT strftime('%Y%m%d-%H%M%S', 'now')")
            .fetch_one(self.db.pool())
            .await?;
        let name = format!("{STARTS}{stamp}{ENDS}");
        let file = self.dir.join(&name);
        // Written under another name first: a copy cut short is never
        // taken for a whole one.
        let unfinished = self.dir.join(format!("{name}.part"));
        let written = self.write(&unfinished, &file).await;
        if written.is_err() {
            let _ = std::fs::remove_file(&unfinished);
        }
        written?;

        let dir = self.dir.clone();
        let copies = tokio::task::spawn_blocking(move || -> std::io::Result<_> {
            let copies = copies_in(&dir)?;
            for old in copies.iter().skip(COPIES_KEPT) {
                std::fs::remove_file(dir.join(&old.name))?;
            }
            Ok(copies)
        })
        .await??;
        copies
            .into_iter()
            .find(|copy| copy.name == name)
            .ok_or_else(|| std::io::Error::from(std::io::ErrorKind::NotFound).into())
    }

    async fn write(&self, unfinished: &Path, file: &Path) -> Result<(), BackupError> {
        // What an earlier try that was cut short left behind.
        match std::fs::remove_file(unfinished) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            _ => {}
        }
        sqlx::query("VACUUM INTO ?")
            .bind(unfinished.to_string_lossy().into_owned())
            .execute(self.db.pool())
            .await?;
        // Two copies asked for within one second are the same copy.
        std::fs::rename(unfinished, file)?;
        Ok(())
    }

    /// Makes a copy when the schedule is on and the latest is a day old or
    /// there is none. Gives back the copy it made.
    pub async fn make_when_due(&self) -> Result<Option<DatabaseCopy>, BackupError> {
        if !self.scheduled().await? {
            return Ok(None);
        }
        let latest = self.copies().await?.into_iter().next();
        if latest.is_some_and(|copy| now() - copy.made_at < BETWEEN_COPIES) {
            return Ok(None);
        }
        self.make().await.map(Some)
    }
}

/// The copies in a folder, the latest first. A folder that is not there
/// holds none.
fn copies_in(dir: &Path) -> std::io::Result<Vec<DatabaseCopy>> {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut copies = Vec::new();
    for entry in entries {
        let entry = entry?;
        let path: PathBuf = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if !(name.starts_with(STARTS) && name.ends_with(ENDS)) {
            continue;
        }
        let about = entry.metadata()?;
        if !about.is_file() {
            continue;
        }
        let made_at = about
            .modified()?
            .duration_since(UNIX_EPOCH)
            .map_or(0, |since| since.as_secs() as i64);
        copies.push(DatabaseCopy {
            name: name.to_owned(),
            size: about.len(),
            made_at,
        });
    }
    // The name holds the moment, so the names order them too.
    copies.sort_by(|a, b| b.name.cmp(&a.name));
    Ok(copies)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_copy_opens_like_the_database_and_the_oldest_make_room() {
        let dir = tempfile::tempdir().unwrap();
        let db = Database::open(&dir.path().join("binge.db")).await.unwrap();
        sqlx::query("INSERT INTO setting (key, value) VALUES ('kept', 'yes')")
            .execute(db.pool())
            .await
            .unwrap();
        let backups = Backups::new(db, dir.path());
        assert!(backups.copies().await.unwrap().is_empty());

        // The schedule is on until it is turned off, and makes one a day.
        assert!(backups.scheduled().await.unwrap());
        let first = backups.make_when_due().await.unwrap().unwrap();
        assert_eq!(backups.make_when_due().await.unwrap(), None);
        assert_eq!(
            backups.copies().await.unwrap(),
            std::slice::from_ref(&first)
        );
        assert!(first.size > 0);

        // The copy holds what the database held.
        let copy = Database::open(&backups.dir().join(&first.name))
            .await
            .unwrap();
        let kept: String = sqlx::query_scalar("SELECT value FROM setting WHERE key = 'kept'")
            .fetch_one(copy.pool())
            .await
            .unwrap();
        assert_eq!(kept, "yes");

        // Older copies than are kept go; other files in the folder stay.
        std::fs::write(backups.dir().join("notes.txt"), "mine").unwrap();
        for day in 0..COPIES_KEPT + 2 {
            let old = format!("{STARTS}2020010{day}-000000{ENDS}");
            std::fs::write(backups.dir().join(old), "old").unwrap();
        }
        std::fs::remove_file(backups.dir().join(&first.name)).unwrap();
        let made = backups.make().await.unwrap();
        let names: Vec<String> = backups
            .copies()
            .await
            .unwrap()
            .into_iter()
            .map(|copy| copy.name)
            .collect();
        assert_eq!(names.len(), COPIES_KEPT);
        assert_eq!(names[0], made.name);
        assert_eq!(names.last().unwrap(), "binge-20200103-000000.db");
        assert!(backups.dir().join("notes.txt").exists());

        // Off, nothing is made however old the latest is.
        backups.schedule(false).await.unwrap();
        assert!(!backups.scheduled().await.unwrap());
        for name in &names {
            std::fs::remove_file(backups.dir().join(name)).unwrap();
        }
        assert_eq!(backups.make_when_due().await.unwrap(), None);
        backups.schedule(true).await.unwrap();
        assert!(backups.make_when_due().await.unwrap().is_some());
    }
}
