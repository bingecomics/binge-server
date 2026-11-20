//! Starting another binary of the server in the place of the one that runs.
//!
//! The process stays the same and its program is replaced, so whatever
//! started the server (a terminal, Docker, a service) keeps the process it
//! knows. That is why this is for Linux and macOS only: Windows has no such
//! call.

use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;

use super::UpdateError;
use super::release::Version;

/// Set for a server that another binary started from the data directory,
/// to the path of that binary: the one that was installed.
const STARTED_BY: &str = "BINGE_STARTED_BY";

/// The binary that was installed, if this server is not it.
fn started_by() -> Option<OsString> {
    std::env::var_os(STARTED_BY).filter(|path| !path.is_empty())
}

/// Whether this server is a version from the data directory, started by
/// the binary that was installed.
pub fn was_started_by_another() -> bool {
    started_by().is_some()
}

/// The binary whoever runs the server starts: this one, or the one that
/// started this one.
fn installed() -> io::Result<PathBuf> {
    match started_by() {
        Some(path) => Ok(path.into()),
        None => std::env::current_exe(),
    }
}

/// Makes sure `binary` runs on this machine and is `version` of the server,
/// by asking it. A binary for another system, or one in a folder programs
/// may not be run from, is found here and not when the server is already
/// stopped.
pub fn starts_here(binary: &Path, version: Version) -> Result<(), UpdateError> {
    let said =
        asked_its_version(binary).map_err(|error| UpdateError::DoesNotStart(error.to_string()))?;
    let said = String::from_utf8_lossy(&said.stdout);
    if said.trim() == format!("binge-server {version}") {
        Ok(())
    } else {
        Err(UpdateError::DoesNotStart(format!(
            "asked for its version, it said {:?}",
            said.trim()
        )))
    }
}

/// Runs `binary --version`.
///
/// Linux refuses to run a file that any process still has open for
/// writing. The server closed it, but a process another thread is starting
/// at that moment holds a copy of every open file until it is under way.
/// That passes in a moment, so the question is asked again.
fn asked_its_version(binary: &Path) -> io::Result<std::process::Output> {
    let mut tries = 0;
    loop {
        match Command::new(binary).arg("--version").output() {
            Err(error) if error.kind() == io::ErrorKind::ExecutableFileBusy && tries < 20 => {
                tries += 1;
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            said => return said,
        }
    }
}

/// Starts `binary`, a version from the data directory, in this process's
/// place. Returns only when that could not be done.
pub fn start(binary: &Path) -> io::Error {
    match installed() {
        Ok(installed) => take_place(binary, Some(&installed)),
        Err(error) => error,
    }
}

/// Starts the installed binary again in this process's place, which then
/// starts the newest version it finds. Returns only when that could not be
/// done.
pub fn start_again() -> io::Error {
    match installed() {
        Ok(installed) => take_place(&installed, None),
        Err(error) => error,
    }
}

/// Runs `binary` in this process's place, with what this one was started
/// with. `started_by` is the installed binary when `binary` is not it.
#[cfg(unix)]
fn take_place(binary: &Path, started_by: Option<&Path>) -> io::Error {
    use std::os::unix::process::CommandExt;
    let mut command = Command::new(binary);
    command.args(std::env::args_os().skip(1));
    match started_by {
        Some(installed) => command.env(STARTED_BY, installed),
        None => command.env_remove(STARTED_BY),
    };
    command.exec()
}

#[cfg(not(unix))]
fn take_place(_: &Path, _: Option<&Path>) -> io::Error {
    io::Error::from(io::ErrorKind::Unsupported)
}

#[cfg(all(test, unix))]
mod tests {
    use std::fs;
    use std::os::unix::fs::PermissionsExt;

    use super::*;

    fn script(folder: &Path, text: &str, mode: u32) -> PathBuf {
        let path = folder.join("binge-server");
        fs::write(&path, text).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(mode)).unwrap();
        path
    }

    #[test]
    fn a_binary_is_tried_before_it_is_kept() {
        let folder = tempfile::tempdir().unwrap();
        let version: Version = "9.9.9".parse().unwrap();

        let good = script(
            folder.path(),
            "#!/bin/sh\necho \"binge-server 9.9.9\"\n",
            0o755,
        );
        starts_here(&good, version).unwrap();

        let other = script(
            folder.path(),
            "#!/bin/sh\necho \"binge-server 9.9.8\"\n",
            0o755,
        );
        assert!(matches!(
            starts_here(&other, version),
            Err(UpdateError::DoesNotStart(_))
        ));

        // Not a program this machine can run.
        let not_run = script(
            folder.path(),
            "#!/bin/sh\necho \"binge-server 9.9.9\"\n",
            0o644,
        );
        assert!(matches!(
            starts_here(&not_run, version),
            Err(UpdateError::DoesNotStart(_))
        ));
    }
}
