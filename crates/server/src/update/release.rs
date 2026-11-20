//! What a release says of itself, and the proof that Binge made it.
//!
//! A release has a file, `update.json`, that names its version and, for
//! each system it has a build for, the archive to fetch and the checksum of
//! the binary inside. Beside it is a signature over that file, made when
//! the release was built, with a key only Binge has. A binary is trusted
//! when the file names its checksum and one of the keys built into the
//! server signed the file.

use std::fmt;
use std::fs::File;
use std::io::{self, Read};
use std::path::Path;
use std::str::FromStr;

use minisign_verify::{PublicKey, Signature};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use super::UpdateError;

/// The file of a release that names its builds.
pub const MANIFEST: &str = "update.json";

/// The signature over that file.
pub const SIGNATURE: &str = "update.json.minisig";

/// A version of the server: three numbers with dots between.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Version {
    major: u64,
    minor: u64,
    patch: u64,
}

impl Version {
    /// The version of the server that runs.
    pub fn running() -> Self {
        binge_core::VERSION
            .parse()
            .expect("the server's own version is three numbers")
    }
}

impl FromStr for Version {
    type Err = NotAVersion;

    fn from_str(text: &str) -> Result<Self, NotAVersion> {
        let mut parts = text.split('.');
        let mut number = || -> Result<u64, NotAVersion> {
            let part = parts.next().ok_or(NotAVersion)?;
            // Digits alone: `parse` would also take "+1".
            if part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()) {
                return Err(NotAVersion);
            }
            part.parse().map_err(|_| NotAVersion)
        };
        let version = Self {
            major: number()?,
            minor: number()?,
            patch: number()?,
        };
        if parts.next().is_some() {
            return Err(NotAVersion);
        }
        Ok(version)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// A text that is not three numbers with dots between.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct NotAVersion;

/// The name releases give the system this server was built for, when they
/// have a build for it.
pub fn platform() -> Option<&'static str> {
    match (std::env::consts::OS, std::env::consts::ARCH) {
        ("linux", "x86_64") => Some("linux-x86_64"),
        ("linux", "aarch64") => Some("linux-aarch64"),
        // One binary for both kinds of Mac.
        ("macos", _) => Some("macos"),
        _ => None,
    }
}

/// The keys whose signature makes a release Binge's.
pub struct Keys(Vec<PublicKey>);

impl Keys {
    /// The keys built into the server: the one releases are signed with,
    /// and a spare that is kept apart from it, to sign with should the
    /// first be lost.
    pub fn built_in() -> Self {
        let files = [
            include_str!("../../update-keys/release.pub"),
            include_str!("../../update-keys/spare.pub"),
        ];
        let keys: Result<_, _> = files.into_iter().map(PublicKey::decode).collect();
        Self(keys.expect("the keys built into the server read"))
    }

    /// One key, as the text `minisign` prints for it.
    pub fn one(key: &str) -> Result<Self, UpdateError> {
        let key = PublicKey::from_base64(key.trim())
            .map_err(|error| UpdateError::NotSigned(format!("the key does not read: {error}")))?;
        Ok(Self(vec![key]))
    }

    /// Whether one of the keys made `signature` over `signed`.
    fn check(&self, signed: &[u8], signature: &str) -> Result<(), UpdateError> {
        let signature = Signature::decode(signature).map_err(|error| {
            UpdateError::NotSigned(format!("the signature does not read: {error}"))
        })?;
        let made_by = |key: &PublicKey| key.verify(signed, &signature, false).is_ok();
        if self.0.iter().any(made_by) {
            Ok(())
        } else {
            Err(UpdateError::NotSigned(
                "none of the server's keys made the signature".to_owned(),
            ))
        }
    }
}

/// `update.json`, as it is written.
#[derive(Debug, Deserialize)]
struct Manifest {
    version: String,
    files: Vec<Build>,
}

/// The build of a release for one system.
#[derive(Clone, Debug, Deserialize)]
pub struct Build {
    platform: String,
    /// The name of the file to fetch.
    pub archive: String,
    /// The checksum of that file, in hex.
    pub archive_sha256: String,
    /// The checksum of the binary inside it, in hex.
    pub binary_sha256: String,
    /// The size of that binary, in bytes.
    pub binary_size: u64,
}

/// A release whose signature was checked.
pub struct Release {
    pub version: Version,
    /// Its build for the system this server runs on, when it has one.
    pub build: Option<Build>,
    manifest: Vec<u8>,
    signature: String,
}

impl Release {
    /// Reads what a release says of itself, once one of `keys` is known to
    /// have signed it.
    pub fn read(manifest: Vec<u8>, signature: String, keys: &Keys) -> Result<Self, UpdateError> {
        keys.check(&manifest, &signature)?;
        let said: Manifest = serde_json::from_slice(&manifest)
            .map_err(|error| UpdateError::Unreadable(error.to_string()))?;
        let version = said.version.parse().map_err(|NotAVersion| {
            UpdateError::Unreadable(format!("{:?} is not a version", said.version))
        })?;
        let ours = platform();
        let build = said
            .files
            .into_iter()
            .find(|build| Some(build.platform.as_str()) == ours);
        Ok(Self {
            version,
            build,
            manifest,
            signature,
        })
    }

    /// The file that names the builds, as it was signed.
    pub fn manifest(&self) -> &[u8] {
        &self.manifest
    }

    /// The signature over it, as it was fetched.
    pub fn signature(&self) -> &str {
        &self.signature
    }
}

/// Reads all of `from` and gives its SHA-256 in hex.
pub fn checksum(mut from: impl Read) -> io::Result<String> {
    let mut sum = Sha256::new();
    let mut buffer = [0; 64 * 1024];
    loop {
        let read = from.read(&mut buffer)?;
        if read == 0 {
            return Ok(hex::encode(sum.finalize()));
        }
        sum.update(&buffer[..read]);
    }
}

/// The SHA-256 of a file, in hex.
pub fn checksum_of(file: &Path) -> io::Result<String> {
    checksum(File::open(file)?)
}

#[cfg(test)]
pub mod testing {
    //! Releases signed with a key made for a test.

    use std::io::Cursor;

    use minisign::KeyPair;

    use super::*;

    /// A key that signs, and what verifies its signatures.
    pub struct Signer {
        pair: KeyPair,
    }

    impl Signer {
        pub fn new() -> Self {
            let pair = KeyPair::generate_unencrypted_keypair().unwrap();
            Self { pair }
        }

        pub fn keys(&self) -> Keys {
            Keys::one(&self.pair.pk.to_base64()).unwrap()
        }

        /// The signature over `text`, as `minisign` writes it to a file.
        pub fn sign(&self, text: &[u8]) -> String {
            minisign::sign(None, &self.pair.sk, Cursor::new(text), None, None)
                .unwrap()
                .into_string()
        }
    }

    /// The name of the archive of `version` in a release made for a test.
    pub fn archive_name(version: &str) -> String {
        format!("binge-server-{version}-test.tar.gz")
    }

    /// `update.json` for one build for the system the tests run on.
    pub fn manifest(version: &str, archive: &[u8], binary: &[u8]) -> Vec<u8> {
        serde_json::json!({
            "version": version,
            "files": [{
                "platform": platform().expect("the tests run where releases have a build"),
                "archive": archive_name(version),
                "archive_sha256": checksum(archive).unwrap(),
                "binary_sha256": checksum(binary).unwrap(),
                "binary_size": binary.len(),
            }],
        })
        .to_string()
        .into_bytes()
    }
}

#[cfg(test)]
mod tests {
    use super::testing::{Signer, manifest};
    use super::*;

    #[test]
    fn a_version_is_three_numbers() {
        let version = |text: &str| text.parse::<Version>();
        assert_eq!(version("1.2.3").unwrap().to_string(), "1.2.3");
        assert!(version("0.10.0").unwrap() > version("0.9.9").unwrap());
        assert!(version("1.0.0").unwrap() > version("0.99.99").unwrap());
        assert!(version("0.1.1").unwrap() > version("0.1.0").unwrap());
        for not_one in [
            "", "1", "1.2", "1.2.3.4", "1.2.x", "v1.2.3", "1.2.+3", "1..3",
        ] {
            assert_eq!(version(not_one), Err(NotAVersion), "{not_one:?}");
        }
        // What the server says of itself has to read, or it could not tell
        // a newer version from its own.
        assert_eq!(Version::running().to_string(), binge_core::VERSION);
    }

    #[test]
    fn the_keys_built_in_read() {
        assert_eq!(Keys::built_in().0.len(), 2);
    }

    #[test]
    fn a_release_is_read_only_when_a_known_key_signed_it() {
        let signer = Signer::new();
        let said = manifest("9.9.9", b"archive", b"binary");
        let signature = signer.sign(&said);

        let release = Release::read(said.clone(), signature.clone(), &signer.keys()).unwrap();
        assert_eq!(release.version.to_string(), "9.9.9");
        let build = release.build.unwrap();
        assert_eq!(build.binary_size, 6);
        assert_eq!(build.archive, "binge-server-9.9.9-test.tar.gz");

        // Another key's signature, a changed file, and no signature at all.
        let other = Signer::new();
        let refused = [
            Release::read(said.clone(), other.sign(&said), &signer.keys()),
            Release::read(
                manifest("9.9.10", b"archive", b"binary"),
                signature,
                &signer.keys(),
            ),
            Release::read(said.clone(), String::new(), &signer.keys()),
            Release::read(said.clone(), signer.sign(&said), &Keys::built_in()),
        ];
        for read in refused {
            assert!(matches!(read, Err(UpdateError::NotSigned(_))));
        }
    }

    #[test]
    fn a_signed_release_that_does_not_read_is_refused() {
        let signer = Signer::new();
        for said in [&b"not json"[..], br#"{"version": "latest", "files": []}"#] {
            let read = Release::read(said.to_vec(), signer.sign(said), &signer.keys());
            assert!(matches!(read, Err(UpdateError::Unreadable(_))));
        }
        // A release with no build for this system reads, and says so.
        let said = br#"{"version": "2.0.0", "files": []}"#;
        let release = Release::read(said.to_vec(), signer.sign(said), &signer.keys()).unwrap();
        assert!(release.build.is_none());
    }
}
