//! Asking the place releases are published at, and fetching from it.
//!
//! With reading the server a person is leaving (`crate::bring`), which
//! happens only when asked, this is the only part of the server that calls
//! an address outside it.
//! What it sends is the request for a file and the server's version, as
//! the name of the program that asks. Nothing of a library or of a person.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::Path;
use std::time::Duration;

use flate2::read::GzDecoder;
use ureq::Agent;

use super::UpdateError;
use super::release::{Build, Keys, MANIFEST, Release, SIGNATURE, Version, checksum_of};
use super::store::BINARY;

/// Where Binge Server's releases are published.
const RELEASES: &str = "https://github.com/bingecomics/binge-server/releases";

/// The longest a release's file that names its builds is, and its
/// signature, in bytes. They are a few hundred.
const MOST_SAID: u64 = 64 * 1024;

/// The largest archive that is fetched, in bytes. One is about 20 MB.
const MOST_FETCHED: u64 = 256 * 1024 * 1024;

/// The name the archive has while it is being fetched.
const ARCHIVE: &str = "archive";

/// A place releases are published at.
pub struct Source {
    releases: String,
    agent: Agent,
}

impl Source {
    /// Where Binge publishes them.
    pub fn published() -> Self {
        Self::at(RELEASES, true)
    }

    /// The releases under an address laid out like GitHub's. `encrypted`
    /// says that nothing is fetched over a connection that is not.
    pub fn at(releases: &str, encrypted: bool) -> Self {
        let config = Agent::config_builder()
            .https_only(encrypted)
            .user_agent(format!("binge-server/{}", binge_core::VERSION))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(30)))
            // An archive over a slow line.
            .timeout_global(Some(Duration::from_secs(15 * 60)))
            .build();
        Self {
            releases: releases.trim_end_matches('/').to_owned(),
            agent: config.new_agent(),
        }
    }

    /// The page that says what changed in a version.
    pub fn notes(&self, version: Version) -> String {
        format!("{}/tag/v{version}", self.releases)
    }

    /// The latest release, once one of `keys` is known to have signed it.
    pub fn latest(&self, keys: &Keys) -> Result<Release, UpdateError> {
        // A release published between the two requests makes them disagree.
        // The signature is then refused, and the next look is right again.
        let latest = format!("{}/latest/download", self.releases);
        let manifest = self.small(&format!("{latest}/{MANIFEST}"))?;
        let signature = self.small(&format!("{latest}/{SIGNATURE}"))?;
        let signature = String::from_utf8(signature)
            .map_err(|_| UpdateError::NotSigned("the signature is not text".to_owned()))?;
        Release::read(manifest, signature, keys)
    }

    /// Fetches the binary of `build` into the folder `into`, as
    /// [`BINARY`], and makes sure it is the one the release names.
    pub fn fetch(&self, release: &Release, build: &Build, into: &Path) -> Result<(), UpdateError> {
        let address = format!(
            "{}/download/v{}/{}",
            self.releases, release.version, build.archive
        );
        let archive = into.join(ARCHIVE);
        let body = self.get(&address)?;
        let body = body.into_with_config().limit(MOST_FETCHED).reader();
        save(body, &archive)?;
        if checksum_of(&archive)? != build.archive_sha256 {
            return Err(UpdateError::WrongFile);
        }

        let binary = into.join(BINARY);
        unpack(&archive, &binary, build.binary_size)?;
        if checksum_of(&binary)? != build.binary_sha256 {
            return Err(UpdateError::WrongFile);
        }
        fs::remove_file(&archive)?;
        Ok(())
    }

    /// The whole of a small file.
    fn small(&self, address: &str) -> Result<Vec<u8>, UpdateError> {
        let body = self.get(address)?;
        body.into_with_config()
            .limit(MOST_SAID)
            .read_to_vec()
            .map_err(unreachable)
    }

    fn get(&self, address: &str) -> Result<ureq::Body, UpdateError> {
        match self.agent.get(address).call() {
            Ok(response) => Ok(response.into_body()),
            Err(ureq::Error::StatusCode(404)) => Err(UpdateError::NothingPublished),
            Err(error) => Err(unreachable(error)),
        }
    }
}

fn unreachable(error: ureq::Error) -> UpdateError {
    UpdateError::Unreachable(error.to_string())
}

/// Writes all of `from` to a new file. What goes wrong reading is the
/// connection's failure; what goes wrong writing is the disk's. The file is
/// an archive that is unpacked and removed, so nothing waits for the disk
/// to have it.
fn save(mut from: impl Read, to: &Path) -> Result<(), UpdateError> {
    let mut file = File::create(to)?;
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = match from.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(UpdateError::Unreachable(error.to_string())),
        };
        file.write_all(&buffer[..read])?;
    }
    Ok(())
}

/// Takes the binary out of a release's archive, which is a `.tar.gz` with
/// the binary at its top, and makes it a file that can be run. No more
/// than `size` bytes and one are written, however much the archive holds.
fn unpack(archive: &Path, binary: &Path, size: u64) -> Result<(), UpdateError> {
    let unreadable =
        |error: io::Error| UpdateError::Unreadable(format!("the archive does not unpack: {error}"));
    let mut archive = tar::Archive::new(GzDecoder::new(File::open(archive)?));
    for entry in archive.entries().map_err(unreadable)? {
        let entry = entry.map_err(unreadable)?;
        let path = entry.path().map_err(unreadable)?;
        // With or without the `./` some programs put before a name.
        if path.strip_prefix(".").unwrap_or(&path) != Path::new(BINARY) {
            continue;
        }
        drop(path);
        let mut file = File::create(binary)?;
        io::copy(&mut entry.take(size + 1), &mut file)?;
        file.sync_all()?;
        drop(file);
        make_runnable(binary)?;
        return Ok(());
    }
    Err(UpdateError::Unreadable(format!(
        "the archive has no {BINARY} in it"
    )))
}

#[cfg(unix)]
fn make_runnable(binary: &Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(binary, fs::Permissions::from_mode(0o755))
}

#[cfg(not(unix))]
fn make_runnable(_: &Path) -> io::Result<()> {
    Ok(())
}

#[cfg(test)]
pub mod testing {
    //! A place that publishes releases, on this machine, for a test.

    use std::collections::HashMap;
    use std::sync::{Arc, Mutex};

    use axum::Router;
    use axum::extract::{Path as Named, State};
    use axum::http::StatusCode;
    use axum::routing::get;
    use flate2::Compression;
    use flate2::write::GzEncoder;
    use tokio::net::TcpListener;

    use super::*;
    use crate::update::release::testing::{Signer, archive_name, manifest};

    type Files = Arc<Mutex<HashMap<String, Vec<u8>>>>;

    /// Releases served from memory, laid out like GitHub's.
    pub struct Published {
        files: Files,
        pub address: String,
    }

    impl Published {
        /// Starts answering on a free port of this machine.
        pub async fn start() -> Self {
            let files = Files::default();
            let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = format!("http://{}/releases", listener.local_addr().unwrap());
            let app = Router::new()
                .route("/releases/{*file}", get(file))
                .with_state(files.clone());
            tokio::spawn(async move { axum::serve(listener, app).await });
            Self { files, address }
        }

        pub fn source(&self) -> Source {
            Source::at(&self.address, false)
        }

        /// Publishes `version` as the latest, with a binary that is the
        /// shell script `binary`, signed by `signer`.
        pub fn release(&self, signer: &Signer, version: &str, binary: &str) {
            let archive = archive_of(binary.as_bytes());
            let said = manifest(version, &archive, binary.as_bytes());
            let name = archive_name(version);
            let mut files = self.files.lock().unwrap();
            files.insert(format!("download/v{version}/{name}"), archive);
            files.insert(
                format!("latest/download/{SIGNATURE}"),
                signer.sign(&said).into(),
            );
            files.insert(format!("latest/download/{MANIFEST}"), said);
        }

        /// Puts `bytes` where a file is published, in its place.
        pub fn replace(&self, file: &str, bytes: Vec<u8>) {
            self.files.lock().unwrap().insert(file.to_owned(), bytes);
        }
    }

    async fn file(
        State(files): State<Files>,
        Named(file): Named<String>,
    ) -> Result<Vec<u8>, StatusCode> {
        let files = files.lock().unwrap();
        files.get(&file).cloned().ok_or(StatusCode::NOT_FOUND)
    }

    /// A release's archive: a `.tar.gz` with the binary and a file beside it.
    pub fn archive_of(binary: &[u8]) -> Vec<u8> {
        let mut archive = tar::Builder::new(GzEncoder::new(Vec::new(), Compression::fast()));
        for (name, bytes) in [(BINARY, binary), ("LICENSE", b"the licence")] {
            let mut header = tar::Header::new_gnu();
            header.set_size(bytes.len() as u64);
            header.set_mode(0o644);
            header.set_cksum();
            archive.append_data(&mut header, name, bytes).unwrap();
        }
        archive.into_inner().unwrap().finish().unwrap()
    }

    /// A program that says it is `version` of the server, as the real one
    /// does when asked for its version.
    pub fn says_version(version: &str) -> String {
        format!("#!/bin/sh\necho \"binge-server {version}\"\n")
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{Published, archive_of, says_version};
    use super::*;
    use crate::update::release::testing::{Signer, archive_name};

    /// Runs what blocks while it waits for an answer off the test's thread.
    async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> T {
        tokio::task::spawn_blocking(work).await.unwrap()
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_latest_release_is_fetched_and_checked() {
        let published = Published::start().await;
        let signer = Signer::new();
        let script = says_version("9.9.9");
        published.release(&signer, "9.9.9", &script);
        let folder = tempfile::tempdir().unwrap();
        let into = folder.path().to_owned();

        let (source, keys) = (published.source(), signer.keys());
        let fetched = blocking(move || {
            let release = source.latest(&keys)?;
            let build = release.build.clone().ok_or(UpdateError::NoBuild)?;
            source.fetch(&release, &build, &into)?;
            Ok::<_, UpdateError>(release.version.to_string())
        })
        .await;

        assert_eq!(fetched.unwrap(), "9.9.9");
        let binary = folder.path().join(BINARY);
        assert_eq!(fs::read_to_string(&binary).unwrap(), script);
        assert!(!folder.path().join(ARCHIVE).exists());
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&binary).unwrap().permissions().mode() & 0o777,
                0o755
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn an_archive_other_than_the_one_signed_for_is_refused() {
        let published = Published::start().await;
        let signer = Signer::new();
        published.release(&signer, "9.9.9", &says_version("9.9.9"));
        published.replace(
            &format!("download/v9.9.9/{}", archive_name("9.9.9")),
            archive_of(b"#!/bin/sh\necho something else\n"),
        );
        let folder = tempfile::tempdir().unwrap();
        let into = folder.path().to_owned();

        let (source, keys) = (published.source(), signer.keys());
        let fetched = blocking(move || {
            let release = source.latest(&keys)?;
            let build = release.build.clone().ok_or(UpdateError::NoBuild)?;
            source.fetch(&release, &build, &into)
        })
        .await;

        assert!(matches!(fetched, Err(UpdateError::WrongFile)));
        assert!(!folder.path().join(BINARY).exists());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn what_cannot_be_fetched_says_why() {
        let published = Published::start().await;
        let signer = Signer::new();
        let (source, keys) = (published.source(), signer.keys());
        let nothing = blocking(move || source.latest(&keys)).await;
        assert!(matches!(nothing, Err(UpdateError::NothingPublished)));

        // Signed by a key the server does not trust.
        published.release(&Signer::new(), "9.9.9", &says_version("9.9.9"));
        let (source, keys) = (published.source(), signer.keys());
        let unsigned = blocking(move || source.latest(&keys)).await;
        assert!(matches!(unsigned, Err(UpdateError::NotSigned(_))));

        // Nobody answers there.
        let nowhere = Source::at("http://127.0.0.1:1/releases", false);
        let keys = signer.keys();
        let unanswered = blocking(move || nowhere.latest(&keys)).await;
        assert!(matches!(unanswered, Err(UpdateError::Unreachable(_))));

        // An address that is not encrypted, asked of a source that has to be.
        let plain = Source::at(&published.address, true);
        let keys = signer.keys();
        let refused = blocking(move || plain.latest(&keys)).await;
        assert!(matches!(refused, Err(UpdateError::Unreachable(_))));
    }

    /// The file a release is made with is written by a script and read
    /// here. Nothing else holds the two to each other before a release.
    #[cfg(unix)]
    #[test]
    fn what_the_release_script_writes_is_what_the_server_reads() {
        use crate::update::release::{checksum, platform};

        let folder = tempfile::tempdir().unwrap();
        let binary = says_version("9.9.9");
        let archive = archive_of(binary.as_bytes());
        for platform in ["linux-x86_64", "linux-aarch64", "macos"] {
            let name = format!("binge-server-9.9.9-{platform}.tar.gz");
            fs::write(folder.path().join(name), &archive).unwrap();
        }
        let script = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../scripts/update-manifest.sh"
        );
        let made = std::process::Command::new(script)
            .arg("9.9.9")
            .arg(folder.path())
            .output()
            .unwrap();
        assert!(
            made.status.success(),
            "{}",
            String::from_utf8_lossy(&made.stderr)
        );

        let said = fs::read(folder.path().join(MANIFEST)).unwrap();
        let signer = Signer::new();
        let release = Release::read(said.clone(), signer.sign(&said), &signer.keys()).unwrap();
        assert_eq!(release.version.to_string(), "9.9.9");
        let build = release.build.expect("a build for where the tests run");
        assert_eq!(
            build.archive,
            format!("binge-server-9.9.9-{}.tar.gz", platform().unwrap())
        );
        assert_eq!(build.archive_sha256, checksum(&archive[..]).unwrap());
        assert_eq!(build.binary_sha256, checksum(binary.as_bytes()).unwrap());
        assert_eq!(build.binary_size, binary.len() as u64);
    }
}
