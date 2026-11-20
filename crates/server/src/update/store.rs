//! The versions the server fetched, kept in `updates` in its data directory.
//!
//! Each has a folder named after it, with the binary and the two files of
//! its release that prove the binary is Binge's. The proof is checked again
//! every time a version is about to be started: the folder can be written
//! by whoever can write the data directory.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::UpdateError;
use super::release::{Keys, MANIFEST, Release, SIGNATURE, Version, checksum_of};

/// The name of the binary in a version's folder, and in a release's archive.
pub const BINARY: &str = "binge-server";

/// The folder a release is fetched into before it is kept.
const INCOMING: &str = "incoming";

/// The versions kept in one data directory.
pub struct Store {
    dir: PathBuf,
}

/// A version that is kept and proven to be Binge's.
#[derive(Debug)]
pub struct Kept {
    pub version: Version,
    pub binary: PathBuf,
}

impl Store {
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("updates"),
        }
    }

    /// The versions that have a folder, whatever is in it.
    fn versions(&self) -> Vec<Version> {
        let Ok(entries) = fs::read_dir(&self.dir) else {
            // No folder yet: nothing was ever fetched.
            return Vec::new();
        };
        entries
            .filter_map(|entry| entry.ok()?.file_name().to_str()?.parse().ok())
            .collect()
    }

    /// The newest version that has a folder here and is newer than `than`,
    /// proven or not.
    pub fn newer_than(&self, than: Version) -> Option<Version> {
        self.versions()
            .into_iter()
            .filter(|kept| *kept > than)
            .max()
    }

    /// The newest version kept here that is newer than `than` and proven.
    /// One that is not proven is passed over and reported.
    pub fn newest(&self, keys: &Keys, than: Version) -> Option<Kept> {
        let mut versions = self.versions();
        versions.sort_unstable_by(|a, b| b.cmp(a));
        versions
            .into_iter()
            .filter(|version| *version > than)
            .find_map(|version| match self.proven(version, keys) {
                Ok(kept) => Some(kept),
                Err(error) => {
                    tracing::warn!(
                        %version, %error,
                        "a version kept in the data directory is not started"
                    );
                    None
                }
            })
    }

    /// The kept `version`, once its release is known to be signed and its
    /// binary to be the one the release names.
    fn proven(&self, version: Version, keys: &Keys) -> Result<Kept, UpdateError> {
        let dir = self.dir.join(version.to_string());
        let manifest = fs::read(dir.join(MANIFEST))?;
        let signature = fs::read_to_string(dir.join(SIGNATURE))?;
        let release = Release::read(manifest, signature, keys)?;
        if release.version != version {
            return Err(UpdateError::Unreadable(format!(
                "the folder of {version} holds the release {}",
                release.version
            )));
        }
        let build = release.build.ok_or(UpdateError::NoBuild)?;
        let binary = dir.join(BINARY);
        if checksum_of(&binary)? != build.binary_sha256 {
            return Err(UpdateError::WrongFile);
        }
        Ok(Kept { version, binary })
    }

    /// An empty folder to fetch a release into.
    pub fn incoming(&self) -> io::Result<PathBuf> {
        let incoming = self.dir.join(INCOMING);
        remove_folder(&incoming)?;
        fs::create_dir_all(&incoming)?;
        Ok(incoming)
    }

    /// Keeps the release whose binary was fetched into [`Store::incoming`],
    /// in place of what was kept of that version before.
    pub fn keep(&self, release: &Release, keys: &Keys) -> Result<Kept, UpdateError> {
        let incoming = self.dir.join(INCOMING);
        fs::write(incoming.join(MANIFEST), release.manifest())?;
        fs::write(incoming.join(SIGNATURE), release.signature())?;
        let place = self.dir.join(release.version.to_string());
        remove_folder(&place)?;
        fs::rename(&incoming, &place)?;
        // The same check a start makes, so that what is kept will start.
        self.proven(release.version, keys)
    }

    /// Removes the kept versions that `gone` names. One that cannot be
    /// removed is reported and stays.
    pub fn forget(&self, gone: impl Fn(Version) -> bool) {
        for version in self.versions().into_iter().filter(|version| gone(*version)) {
            match remove_folder(&self.dir.join(version.to_string())) {
                Ok(()) => {
                    tracing::info!(%version, "removed an older version from the data directory")
                }
                Err(error) => tracing::warn!(%version, %error, "could not remove an older version"),
            }
        }
    }
}

/// Removes a folder and what is in it. None there is no error.
fn remove_folder(folder: &Path) -> io::Result<()> {
    match fs::remove_dir_all(folder) {
        Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
        _ => Ok(()),
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;
    use crate::update::release::testing::{Signer, manifest};

    /// Keeps a version whose binary is `binary`, signed by `signer`.
    pub fn keep(store: &Store, signer: &Signer, version: &str, binary: &[u8]) -> Kept {
        let said = manifest(version, b"archive", binary);
        let release = Release::read(said.clone(), signer.sign(&said), &signer.keys()).unwrap();
        let incoming = store.incoming().unwrap();
        fs::write(incoming.join(BINARY), binary).unwrap();
        store.keep(&release, &signer.keys()).unwrap()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::keep;
    use super::*;
    use crate::update::release::testing::Signer;

    fn version(text: &str) -> Version {
        text.parse().unwrap()
    }

    #[test]
    fn the_newest_proven_version_is_the_one_to_start() {
        let data = tempfile::tempdir().unwrap();
        let store = Store::new(data.path());
        let signer = Signer::new();
        let keys = signer.keys();
        assert!(store.newest(&keys, version("0.1.0")).is_none());

        keep(&store, &signer, "0.2.0", b"two");
        keep(&store, &signer, "0.10.0", b"ten");
        keep(&store, &signer, "0.3.0", b"three");
        // Not a version: left alone, and not looked at.
        fs::create_dir(data.path().join("updates/notes")).unwrap();

        let newest = store.newest(&keys, version("0.1.0")).unwrap();
        assert_eq!(newest.version, version("0.10.0"));
        assert_eq!(fs::read(&newest.binary).unwrap(), b"ten");
        // Nothing kept is newer than what runs.
        assert!(store.newest(&keys, version("0.10.0")).is_none());
        assert!(store.newest(&keys, version("1.0.0")).is_none());
    }

    #[test]
    fn a_version_that_is_not_proven_is_passed_over() {
        let data = tempfile::tempdir().unwrap();
        let store = Store::new(data.path());
        let signer = Signer::new();
        let keys = signer.keys();
        keep(&store, &signer, "0.2.0", b"two");
        let three = keep(&store, &signer, "0.3.0", b"three");
        let four = keep(&store, &signer, "0.4.0", b"four");

        // A binary that was changed after it was kept.
        fs::write(&four.binary, b"something else").unwrap();
        assert_eq!(
            store.newest(&keys, version("0.1.0")).unwrap().version,
            version("0.3.0")
        );

        // A release moved into the folder of a newer version.
        let moved = data.path().join("updates/0.9.0");
        fs::rename(three.binary.parent().unwrap(), &moved).unwrap();
        assert_eq!(
            store.newest(&keys, version("0.1.0")).unwrap().version,
            version("0.2.0")
        );

        // Signed, but not by a key this server trusts.
        assert!(
            store
                .newest(&Signer::new().keys(), version("0.1.0"))
                .is_none()
        );
    }

    #[test]
    fn older_versions_are_removed_and_nothing_else() {
        let data = tempfile::tempdir().unwrap();
        let store = Store::new(data.path());
        let signer = Signer::new();
        for kept in ["0.2.0", "0.3.0", "0.4.0"] {
            keep(&store, &signer, kept, b"binary");
        }
        let notes = data.path().join("updates/notes");
        fs::create_dir(&notes).unwrap();

        store.forget(|kept| kept < version("0.4.0"));
        let mut left = store.versions();
        left.sort_unstable();
        assert_eq!(left, [version("0.4.0")]);
        assert!(notes.is_dir());
    }

    #[test]
    fn keeping_a_version_again_replaces_it() {
        let data = tempfile::tempdir().unwrap();
        let store = Store::new(data.path());
        let signer = Signer::new();
        keep(&store, &signer, "0.2.0", b"first");
        let again = keep(&store, &signer, "0.2.0", b"second");
        assert_eq!(fs::read(again.binary).unwrap(), b"second");
        assert!(!data.path().join("updates/incoming").exists());
    }
}
