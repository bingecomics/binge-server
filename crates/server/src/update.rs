//! Bringing the server to a newer version from the web app.
//!
//! The owner asks; the server fetches the release, makes sure Binge signed
//! it, keeps its binary in the data directory and starts again. A server
//! that starts looks in its data directory first and, when a newer version
//! than itself is kept there and proven, runs that one in its own place.
//!
//! So the binary that was installed is never replaced, which is what makes
//! this work in Docker, where a container cannot change its image: the
//! image's binary starts the newer one from the volume.
//!
//! See `docs/adr/0020-the-server-updates-itself-from-signed-releases.md`.

mod fetch;
mod handover;
mod release;
mod store;

use std::io;
use std::path::Path;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use binge_core::catalog::now;
use binge_core::{Database, DbError};
use serde::Serialize;
use tokio::sync::watch;
use utoipa::ToSchema;

use self::fetch::Source;
pub use self::handover::start_again;
use self::release::{Keys, Version};
use self::store::{BINARY, Store};

/// The mark the database keeps while the server looks for a newer version
/// by itself.
const LOOKS: &str = "updates.look";

/// How long after a look the server looks again by itself, in seconds: a day.
const BETWEEN_LOOKS: i64 = 24 * 60 * 60;

/// How often the server finds out whether a look is due.
const BETWEEN_CHECKS: Duration = Duration::from_secs(60 * 60);

/// How long after it starts the server first finds that out.
const BEFORE_FIRST_CHECK: Duration = Duration::from_secs(60);

/// What went wrong. `Display` is for the log; [`UpdateError::for_people`]
/// is what to tell the owner.
#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("this server does not update itself: {0:?}")]
    Cannot(CannotUpdate),
    #[error("an update is already under way")]
    Busy,
    #[error("the version asked for is not the newer one the last look found")]
    NotTheNewest,
    #[error("the newest version is {0}, not the one asked for")]
    Another(Version),
    #[error("nothing is published where releases are looked for")]
    NothingPublished,
    #[error("could not reach where releases are published: {0}")]
    Unreachable(String),
    #[error("the release is not signed by a key of this server: {0}")]
    NotSigned(String),
    #[error("the release does not read: {0}")]
    Unreadable(String),
    #[error("the release has no build for this system")]
    NoBuild,
    #[error("the file is not the one the release names")]
    WrongFile,
    #[error("the new version does not start here: {0}")]
    DoesNotStart(String),
    #[error("could not write to the data directory: {0}")]
    Io(#[from] io::Error),
    #[error(transparent)]
    Database(#[from] DbError),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl UpdateError {
    /// What to tell the owner. The cause goes to the log.
    pub fn for_people(&self) -> &str {
        match self {
            Self::Cannot(why) => why.for_people(),
            Self::Busy => "An update is already under way.",
            Self::NotTheNewest => {
                "That is not the newest version this server knows of. Look for a new version \
                 again."
            }
            Self::Another(_) => {
                "The newest version is not that one any more. Look at what is out now before \
                 you update."
            }
            Self::NothingPublished => {
                "No version was found where Binge publishes them. Try again later."
            }
            Self::Unreachable(_) => {
                "The server could not reach GitHub, where new versions are published. Check \
                 that it has a connection to the internet."
            }
            Self::NotSigned(_) | Self::WrongFile => {
                "What the server fetched is not a version made by Binge, so it was not used. \
                 Try again later."
            }
            Self::Unreadable(_) => "The new version could not be read. Try again later.",
            Self::NoBuild => "The new version has no build for the system this server runs on.",
            Self::DoesNotStart(_) => {
                "The new version does not start on this machine, so the server stays on this \
                 one. Its log has the details."
            }
            Self::Io(_) => {
                "The new version could not be written. Check that the data directory has room \
                 and can be written to."
            }
            Self::Database(_) | Self::Task(_) => binge_core::FAILED_FOR_PEOPLE,
        }
    }
}

/// Why a server does not bring itself to a newer version.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum CannotUpdate {
    /// It was started with `--no-updates`. It does not look either.
    TurnedOff,
    /// Releases have no build that can replace this one while it runs:
    /// Windows, or a system the server was built for by hand.
    NoBuild,
    /// It is a build for development.
    Development,
}

impl CannotUpdate {
    /// The same in a sentence for the owner.
    pub fn for_people(self) -> &'static str {
        match self {
            Self::TurnedOff => {
                "Updates are turned off where this server is started, so its version is \
                 decided there."
            }
            Self::NoBuild => {
                "This server cannot update itself on this system. Download the new version \
                 and put it in place of this one."
            }
            Self::Development => "This is a build for development. It does not update itself.",
        }
    }
}

/// Where an update asked for stands.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum UpdateStage {
    /// None is under way.
    #[default]
    Idle,
    /// The new version is being fetched and checked.
    Fetching,
    /// The new version is kept, and the server is about to start again as
    /// it. Requests fail for a moment.
    Restarting,
}

/// Why a server does not bring itself to a newer version, for a program
/// and for a person.
#[derive(Debug, Serialize, ToSchema)]
pub struct NotUpdating {
    pub why: CannotUpdate,
    /// The same in a sentence for the owner.
    pub message: &'static str,
}

/// A version newer than the one that runs.
#[derive(Debug, Serialize, ToSchema)]
pub struct NewerVersion {
    pub version: String,
    /// The page that says what changed in it.
    pub notes: String,
}

/// Where the server stands with its own updates.
#[derive(Debug, Serialize, ToSchema)]
pub struct UpdateStatus {
    /// The version that runs.
    pub version: &'static str,
    /// Whether the server looks for a newer version by itself, once a day.
    /// Looking asks GitHub, so it is off until the owner turns it on.
    pub looks: bool,
    /// Why this server does not bring itself to a newer version. Missing
    /// when it does.
    pub cannot: Option<NotUpdating>,
    /// When the server last looked, in seconds since 1970. Missing when it
    /// has not looked since it started.
    pub looked_at: Option<i64>,
    /// The newer version the last look found.
    pub newer: Option<NewerVersion>,
    /// What went wrong the last time the server looked or tried to update,
    /// in a sentence for the owner.
    pub problem: Option<String>,
    /// A newer version that is kept in the data directory and was not
    /// started when this server was, in a sentence for the owner. Missing
    /// when there is none. It stays until the server starts again.
    pub not_started: Option<String>,
    pub stage: UpdateStage,
}

/// Where releases come from, what proves them and where they are kept:
/// decided once, when the server starts.
pub struct Setup {
    cannot: Option<CannotUpdate>,
    source: Source,
    keys: Keys,
    store: Store,
    /// The newest version that is kept and was not started when this
    /// server was: see [`Setup::start_newest`].
    not_started: Option<Version>,
}

impl Setup {
    /// `turned_off` is what `--no-updates` says.
    pub fn new(turned_off: bool, data_dir: &Path) -> Self {
        let (source, keys, development) = for_this_build();
        let cannot = if turned_off {
            Some(CannotUpdate::TurnedOff)
        } else if development {
            Some(CannotUpdate::Development)
        } else if release::platform().is_none() {
            Some(CannotUpdate::NoBuild)
        } else {
            None
        };
        Self {
            cannot,
            source,
            keys,
            store: Store::new(data_dir),
            not_started: None,
        }
    }

    /// Runs the newest proven version kept in the data directory in this
    /// process's place, when one is newer than this server. Returns when
    /// there is none, or when it was not started: not proven, or it could
    /// not be run. The log says which, and the owner is told that a newer
    /// version is kept and does not run.
    pub fn start_newest(&mut self) {
        let running = Version::running();
        if handover::was_started_by_another() {
            // This is a version from the data directory, and it runs: the
            // ones before it are of no use any more.
            self.store.forget(|kept| kept < running);
        } else if self.cannot.is_some() {
            return;
        } else {
            // What the installed binary has caught up with.
            self.store.forget(|kept| kept <= running);
            if let Some(newest) = self.store.newest(&self.keys, running) {
                tracing::info!(
                    version = %newest.version,
                    binary = %newest.binary.display(),
                    "starting the newer version kept in the data directory"
                );
                let error = handover::start(&newest.binary);
                tracing::error!(
                    version = %newest.version, %error,
                    "could not start the newer version"
                );
            }
        }
        // Whatever is still newer than this server was passed over, here or
        // by the binary that started this one.
        self.not_started = self.store.newer_than(running);
    }

    /// Fetches the latest release, which has to be `wanted`, and keeps it
    /// once it is proven and known to start here.
    fn fetch_newer(&self, wanted: Version) -> Result<Version, UpdateError> {
        let release = self.source.latest(&self.keys)?;
        if release.version != wanted {
            return Err(UpdateError::Another(release.version));
        }
        let build = release.build.as_ref().ok_or(UpdateError::NoBuild)?;
        let incoming = self.store.incoming()?;
        self.source.fetch(&release, build, &incoming)?;
        handover::starts_here(&incoming.join(BINARY), release.version)?;
        self.store.keep(&release, &self.keys)?;
        Ok(release.version)
    }
}

/// Where this build looks for releases, the keys it trusts, and whether it
/// is a build for development that does not update itself.
///
/// A build for development can be pointed at releases made for a test and
/// at the key that signed them, to try an update from end to end. A build
/// that is released reads neither setting.
fn for_this_build() -> (Source, Keys, bool) {
    if cfg!(debug_assertions) {
        let source = std::env::var("BINGE_UPDATE_TEST_SOURCE");
        let key = std::env::var("BINGE_UPDATE_TEST_KEY");
        if let (Ok(source), Ok(key)) = (source, key) {
            match Keys::one(&key) {
                Ok(keys) => return (Source::at(&source, false), keys, false),
                Err(error) => tracing::warn!(%error, "BINGE_UPDATE_TEST_KEY is not used"),
            }
        }
    }
    (
        Source::published(),
        Keys::built_in(),
        cfg!(debug_assertions),
    )
}

/// What is known since the server started.
#[derive(Default)]
struct Known {
    looked_at: Option<i64>,
    newer: Option<Version>,
    problem: Option<String>,
    stage: UpdateStage,
}

/// The server's own updates: looking for a newer version, fetching it, and
/// asking for the start that runs it.
///
/// Cheap to clone; clones share what is known.
#[derive(Clone)]
pub struct Updates {
    inner: Arc<Inner>,
}

struct Inner {
    db: Database,
    setup: Setup,
    known: Mutex<Known>,
    /// Becomes true when a new version is kept and the server is to start
    /// again as it.
    restart: watch::Sender<bool>,
}

impl Updates {
    pub fn new(db: Database, setup: Setup) -> Self {
        Self {
            inner: Arc::new(Inner {
                db,
                setup,
                known: Mutex::new(Known::default()),
                restart: watch::Sender::new(false),
            }),
        }
    }

    fn known(&self) -> MutexGuard<'_, Known> {
        // Nothing panics while it holds this.
        self.inner.known.lock().expect("no holder panicked")
    }

    /// Where the server stands with its updates.
    pub async fn status(&self) -> Result<UpdateStatus, UpdateError> {
        let looks = self.inner.db.is_marked(LOOKS).await?;
        let known = self.known();
        Ok(UpdateStatus {
            version: binge_core::VERSION,
            looks,
            cannot: self.inner.setup.cannot.map(|why| NotUpdating {
                why,
                message: why.for_people(),
            }),
            looked_at: known.looked_at,
            newer: known.newer.map(|version| NewerVersion {
                version: version.to_string(),
                notes: self.inner.setup.source.notes(version),
            }),
            problem: known.problem.clone(),
            not_started: self.inner.setup.not_started.map(|kept| {
                format!(
                    "Version {kept} is in the data directory but was not started, so the \
                     server runs {}. Its log says why.",
                    binge_core::VERSION
                )
            }),
            stage: known.stage,
        })
    }

    /// Turns the daily look on or off. Turning it on looks at once.
    pub async fn set_looks(&self, on: bool) -> Result<(), UpdateError> {
        self.may_look()?;
        self.inner.db.mark(LOOKS, on).await?;
        if on {
            self.look().await?;
        }
        Ok(())
    }

    fn may_look(&self) -> Result<(), UpdateError> {
        match self.inner.setup.cannot {
            Some(CannotUpdate::TurnedOff) => Err(UpdateError::Cannot(CannotUpdate::TurnedOff)),
            _ => Ok(()),
        }
    }

    /// Asks whether a newer version is published. What it finds, or why it
    /// could not find out, is in [`Updates::status`] afterwards: a look
    /// that fails is not an error of the one who asked.
    pub async fn look(&self) -> Result<(), UpdateError> {
        self.may_look()?;
        let inner = self.inner.clone();
        let latest =
            tokio::task::spawn_blocking(move || inner.setup.source.latest(&inner.setup.keys))
                .await?;
        let mut known = self.known();
        known.looked_at = Some(now());
        match latest {
            Ok(release) => {
                known.newer = (release.version > Version::running()).then_some(release.version);
                known.problem = None;
            }
            Err(error) => {
                tracing::warn!(%error, "could not look for a newer version");
                known.problem = Some(error.for_people().to_owned());
            }
        }
        Ok(())
    }

    /// Starts fetching `version`, which has to be the newer one the last
    /// look found: what the owner was shown. Returns at once. When the
    /// version is kept, the server starts again as it; when it cannot be,
    /// the reason is in [`Updates::status`].
    pub fn begin(&self, version: &str) -> Result<(), UpdateError> {
        if let Some(why) = self.inner.setup.cannot {
            return Err(UpdateError::Cannot(why));
        }
        let wanted = {
            let mut known = self.known();
            if known.stage != UpdateStage::Idle {
                return Err(UpdateError::Busy);
            }
            let wanted = known
                .newer
                .filter(|newer| newer.to_string() == version)
                .ok_or(UpdateError::NotTheNewest)?;
            known.stage = UpdateStage::Fetching;
            known.problem = None;
            wanted
        };
        let updates = self.clone();
        tokio::spawn(async move {
            let inner = updates.inner.clone();
            let fetched = tokio::task::spawn_blocking(move || inner.setup.fetch_newer(wanted))
                .await
                .map_err(UpdateError::from)
                .and_then(|fetched| fetched);
            let mut known = updates.known();
            match fetched {
                Ok(version) => {
                    tracing::info!(%version, "a newer version is kept; starting again as it");
                    known.stage = UpdateStage::Restarting;
                    known.newer = None;
                    drop(known);
                    updates.inner.restart.send_replace(true);
                }
                Err(error) => {
                    tracing::warn!(%error, "could not update");
                    known.stage = UpdateStage::Idle;
                    if let UpdateError::Another(latest) = error {
                        // What there is to update to now.
                        known.newer = (latest > Version::running()).then_some(latest);
                    }
                    known.problem = Some(error.for_people().to_owned());
                }
            }
        });
        Ok(())
    }

    /// Resolves once a new version is kept and the server is to start again.
    pub async fn restart_asked(&self) {
        let mut asked = self.inner.restart.subscribe();
        // The sender lives as long as `self`, so this only ends with a yes.
        let _ = asked.wait_for(|asked| *asked).await;
    }

    /// Looks once a day while the owner has that on, for as long as the
    /// server runs.
    pub async fn look_on_schedule(self) {
        if self.may_look().is_err() {
            return;
        }
        tokio::time::sleep(BEFORE_FIRST_CHECK).await;
        loop {
            if let Err(error) = self.look_when_due().await {
                tracing::warn!(%error, "could not find out whether a look is due");
            }
            tokio::time::sleep(BETWEEN_CHECKS).await;
        }
    }

    async fn look_when_due(&self) -> Result<(), UpdateError> {
        if !self.inner.db.is_marked(LOOKS).await? {
            return Ok(());
        }
        let looked_at = self.known().looked_at;
        if looked_at.is_none_or(|at| now() - at >= BETWEEN_LOOKS) {
            self.look().await?;
        }
        Ok(())
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;
    pub use crate::update::fetch::testing::{Published, says_version};
    pub use crate::update::release::testing::Signer;

    impl Setup {
        /// Releases from `published`, proven by `signer`, kept in `data_dir`.
        pub fn of(
            published: &Published,
            signer: &Signer,
            data_dir: &Path,
            cannot: Option<CannotUpdate>,
        ) -> Self {
            Self {
                cannot,
                source: published.source(),
                keys: signer.keys(),
                store: Store::new(data_dir),
                not_started: None,
            }
        }

        /// A server that never has anything to fetch: nobody answers where
        /// it looks.
        pub fn nowhere(data_dir: &Path) -> Self {
            Self {
                cannot: None,
                source: Source::at("http://127.0.0.1:1/releases", false),
                keys: Keys::built_in(),
                store: Store::new(data_dir),
                not_started: None,
            }
        }
    }
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;

    use super::testing::{Published, Signer, says_version};
    use super::*;

    struct Server {
        updates: Updates,
        published: Published,
        signer: Signer,
        data: tempfile::TempDir,
    }

    async fn server(cannot: Option<CannotUpdate>) -> Server {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(&data.path().join("binge.db")).await.unwrap();
        let published = Published::start().await;
        let signer = Signer::new();
        let setup = Setup::of(&published, &signer, data.path(), cannot);
        Server {
            updates: Updates::new(db, setup),
            published,
            signer,
            data,
        }
    }

    /// Waits for an update that was begun to end one way or the other.
    async fn fetched(updates: &Updates) -> UpdateStatus {
        loop {
            let status = updates.status().await.unwrap();
            if status.stage != UpdateStage::Fetching {
                return status;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_look_finds_a_newer_version_and_only_a_newer_one() {
        let server = server(None).await;
        let updates = &server.updates;
        let before = updates.status().await.unwrap();
        assert_eq!(before.version, binge_core::VERSION);
        assert!(!before.looks && before.looked_at.is_none() && before.newer.is_none());

        // Nothing published yet: the look happened, and says what it found.
        updates.look().await.unwrap();
        let status = updates.status().await.unwrap();
        assert!(status.looked_at.is_some() && status.newer.is_none());
        assert_eq!(
            status.problem.as_deref(),
            Some(UpdateError::NothingPublished.for_people())
        );

        server
            .published
            .release(&server.signer, "0.0.1", &says_version("0.0.1"));
        updates.look().await.unwrap();
        let status = updates.status().await.unwrap();
        assert!(status.newer.is_none() && status.problem.is_none());

        server
            .published
            .release(&server.signer, "9.9.9", &says_version("9.9.9"));
        updates.look().await.unwrap();
        let newer = updates.status().await.unwrap().newer.unwrap();
        assert_eq!(newer.version, "9.9.9");
        assert_eq!(
            newer.notes,
            format!("{}/tag/v9.9.9", server.published.address)
        );
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn turning_the_daily_look_on_looks_at_once() {
        let server = server(None).await;
        server
            .published
            .release(&server.signer, "9.9.9", &says_version("9.9.9"));

        server.updates.set_looks(true).await.unwrap();
        let status = server.updates.status().await.unwrap();
        assert!(status.looks);
        assert_eq!(status.newer.unwrap().version, "9.9.9");

        server.updates.set_looks(false).await.unwrap();
        assert!(!server.updates.status().await.unwrap().looks);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_keeps_the_new_version_and_asks_for_a_restart() {
        let server = server(None).await;
        let script = says_version("9.9.9");
        server.published.release(&server.signer, "9.9.9", &script);

        // Only what a look found can be asked for.
        assert!(matches!(
            server.updates.begin("9.9.9"),
            Err(UpdateError::NotTheNewest)
        ));
        server.updates.look().await.unwrap();
        assert!(matches!(
            server.updates.begin("9.9.8"),
            Err(UpdateError::NotTheNewest)
        ));
        server.updates.begin("9.9.9").unwrap();
        let status = fetched(&server.updates).await;
        assert_eq!(status.stage, UpdateStage::Restarting);
        assert!(status.problem.is_none());
        tokio::time::timeout(Duration::from_secs(5), server.updates.restart_asked())
            .await
            .expect("the restart was asked for");

        // What a start would run: proven, and the binary that was published.
        let setup = &server.updates.inner.setup;
        let kept = setup.store.newest(&setup.keys, Version::running()).unwrap();
        assert_eq!(kept.version.to_string(), "9.9.9");
        assert_eq!(fs::read_to_string(kept.binary).unwrap(), script);

        // One at a time.
        assert!(matches!(
            server.updates.begin("9.9.9"),
            Err(UpdateError::Busy)
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_update_that_cannot_be_made_says_why_and_keeps_nothing() {
        let server = server(None).await;
        let updates = &server.updates;
        let kept = server.data.path().join("updates");

        // Nothing newer than what runs.
        server
            .published
            .release(&server.signer, "0.0.1", &says_version("0.0.1"));
        updates.look().await.unwrap();
        assert!(matches!(
            updates.begin("0.0.1"),
            Err(UpdateError::NotTheNewest)
        ));

        // A binary that is not the version its release names.
        server
            .published
            .release(&server.signer, "9.9.9", &says_version("9.9.8"));
        updates.look().await.unwrap();
        updates.begin("9.9.9").unwrap();
        let status = fetched(updates).await;
        assert_eq!(status.stage, UpdateStage::Idle);
        assert_eq!(
            status.problem.as_deref(),
            Some(UpdateError::DoesNotStart(String::new()).for_people())
        );

        // A release someone else signed.
        server
            .published
            .release(&Signer::new(), "9.9.9", &says_version("9.9.9"));
        updates.begin("9.9.9").unwrap();
        let status = fetched(updates).await;
        assert_eq!(
            status.problem.as_deref(),
            Some(UpdateError::NotSigned(String::new()).for_people())
        );

        // A version that came out after the one the owner was shown: it is
        // not fetched in the other's name, and is what is shown now.
        server
            .published
            .release(&server.signer, "9.9.10", &says_version("9.9.10"));
        updates.begin("9.9.9").unwrap();
        let status = fetched(updates).await;
        assert_eq!(status.stage, UpdateStage::Idle);
        assert_eq!(
            status.problem.as_deref(),
            Some(UpdateError::Another(Version::running()).for_people())
        );
        assert_eq!(status.newer.unwrap().version, "9.9.10");

        let setup = &updates.inner.setup;
        assert!(
            setup
                .store
                .newest(&setup.keys, Version::running())
                .is_none()
        );
        assert!(!kept.join("9.9.9").exists() && !kept.join("9.9.10").exists());
        // The restart was never asked for.
        let asked = tokio::time::timeout(Duration::from_millis(50), updates.restart_asked());
        assert!(asked.await.is_err());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_server_that_does_not_update_itself_refuses() {
        let turned_off = server(Some(CannotUpdate::TurnedOff)).await;
        assert!(matches!(
            turned_off.updates.begin("9.9.9"),
            Err(UpdateError::Cannot(CannotUpdate::TurnedOff))
        ));
        // It does not look either: looking is what it was told not to do.
        assert!(matches!(
            turned_off.updates.look().await,
            Err(UpdateError::Cannot(CannotUpdate::TurnedOff))
        ));
        assert!(matches!(
            turned_off.updates.set_looks(true).await,
            Err(UpdateError::Cannot(CannotUpdate::TurnedOff))
        ));

        // Where there is no build to start, it still says what is new.
        let no_build = server(Some(CannotUpdate::NoBuild)).await;
        no_build
            .published
            .release(&no_build.signer, "9.9.9", &says_version("9.9.9"));
        no_build.updates.look().await.unwrap();
        let status = no_build.updates.status().await.unwrap();
        assert_eq!(status.cannot.unwrap().why, CannotUpdate::NoBuild);
        assert_eq!(status.newer.unwrap().version, "9.9.9");
        assert!(matches!(
            no_build.updates.begin("9.9.9"),
            Err(UpdateError::Cannot(CannotUpdate::NoBuild))
        ));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_start_removes_the_versions_it_has_caught_up_with() {
        let data = tempfile::tempdir().unwrap();
        let (published, signer) = (Published::start().await, Signer::new());
        let mut setup = Setup::of(&published, &signer, data.path(), None);
        let running = binge_core::VERSION;
        store::testing::keep(&setup.store, &signer, "0.0.1", b"older");
        store::testing::keep(&setup.store, &signer, running, b"the same");

        // Nothing newer is kept, so the server stays what it is.
        setup.start_newest();
        assert!(setup.not_started.is_none());
        let kept = data.path().join("updates");
        assert!(!kept.join("0.0.1").exists());
        assert!(!kept.join(running).exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_newer_version_that_is_not_started_is_told_to_the_owner() {
        let data = tempfile::tempdir().unwrap();
        let db = Database::open(&data.path().join("binge.db")).await.unwrap();
        let (published, signer) = (Published::start().await, Signer::new());
        let mut setup = Setup::of(&published, &signer, data.path(), None);
        // Kept, and signed by a key this server does not trust.
        store::testing::keep(&setup.store, &Signer::new(), "9.9.9", b"not ours");

        setup.start_newest();
        let status = Updates::new(db, setup).status().await.unwrap();
        let said = status.not_started.unwrap();
        assert!(said.starts_with("Version 9.9.9 is in the data directory"));
        // It is not what a look or an update said, and outlasts them.
        assert!(status.problem.is_none());
    }
}
