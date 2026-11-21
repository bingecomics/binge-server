//! Bringing what a person has on the server they are leaving.
//!
//! Besides the look for a newer version, this is the one part of the
//! server that calls an address outside it, and only when someone asks it
//! to, with an address and a key they give. It reads that server and
//! changes nothing there. What it read is kept by the core, which shows it
//! before anything is made of it; the key is used for the reading and
//! not kept.

mod kavita;

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};

use binge_core::{Catalog, CatalogError, UserId};
use serde::Deserialize;
use utoipa::ToSchema;

#[derive(Debug, thiserror::Error)]
pub enum BringError {
    #[error("the address is not one a server is asked at")]
    BadAddress,
    #[error("no answer from the other server")]
    Unreachable,
    #[error("what answers at the address is not the kind of server asked for")]
    NotThatServer,
    #[error("the other server does not take the key")]
    WrongKey,
    #[error("the other server refused what the key asked for")]
    Refused,
    #[error("the other server answered {0}")]
    Answered(u16),
    #[error("the other server's answer does not read")]
    Unreadable,
    #[error("that person is reading another server already")]
    Busy,
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error("a background task failed: {0}")]
    Task(#[from] tokio::task::JoinError),
}

impl BringError {
    /// What to tell the person who asked. The cause goes to the log.
    pub fn for_people(&self) -> &str {
        match self {
            Self::BadAddress => {
                "That address does not read. Give it as you open the other server in a browser, \
                 such as http://192.168.1.20:5000."
            }
            Self::Unreachable => {
                "This server got no answer at that address. Check the address, and that this \
                 server can reach the other one."
            }
            Self::NotThatServer => "What answers at that address is not a Kavita server.",
            Self::WrongKey => {
                "The other server does not take that key. Copy it again from your account \
                 there."
            }
            Self::Refused => {
                "The other server refused part of what was asked with that key. Nothing was \
                 kept."
            }
            Self::Answered(_) | Self::Unreadable => {
                "The other server answered in a way this version does not read. Nothing was \
                 kept."
            }
            Self::Busy => "The other server is being read already.",
            Self::Catalog(error) => error.for_people(),
            Self::Task(_) => binge_core::FAILED_FOR_PEOPLE,
        }
    }
}

/// Which kind of server is read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
#[schema(as = BringFrom)]
pub enum Source {
    Kavita,
}

impl Source {
    /// The name it is kept under.
    fn name(self) -> &'static str {
        match self {
            Self::Kavita => "kavita",
        }
    }
}

/// How far a reading is: the series whose files were asked for, of all.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Read {
    pub series: u32,
    pub of: u32,
}

/// The address as it is asked: with `http://` when it names no way to
/// ask, and without a `/` at its end. One that names a person or a
/// password is refused: nothing of the kind is kept or sent.
fn base_of(address: &str) -> Result<String, BringError> {
    let address = address.trim().trim_end_matches('/');
    let address = match address.split_once("://") {
        Some(("http" | "https", _)) => address.to_owned(),
        Some(_) => return Err(BringError::BadAddress),
        None => format!("http://{address}"),
    };
    let named = address.split_once("://").map_or("", |(_, rest)| rest);
    if named.is_empty() || named.contains(char::is_whitespace) || named.contains('@') {
        return Err(BringError::BadAddress);
    }
    Ok(address)
}

/// Where one person's reading of another server stands.
#[derive(Clone, Debug, Default)]
pub struct Reading {
    /// Whether it is under way.
    pub under_way: bool,
    pub read: Read,
    /// Why the last one stopped, when it did.
    pub problem: Option<String>,
}

/// The readings of other servers, under way and just ended.
#[derive(Clone)]
pub struct Bringing {
    catalog: Catalog,
    readings: Arc<Mutex<HashMap<UserId, Reading>>>,
}

impl Bringing {
    pub fn new(catalog: Catalog) -> Self {
        Self {
            catalog,
            readings: Arc::default(),
        }
    }

    fn readings(&self) -> MutexGuard<'_, HashMap<UserId, Reading>> {
        // What a reading holds is whole after any line that changes it.
        self.readings.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Where `user`'s reading of another server stands.
    pub fn reading(&self, user: UserId) -> Reading {
        self.readings().get(&user).cloned().unwrap_or_default()
    }

    /// Starts reading the server at `address` for `user`, with their key
    /// there, and returns at once. What it reads takes the place of what
    /// they read before, once all of it is read.
    pub fn begin(
        &self,
        user: UserId,
        source: Source,
        address: String,
        key: String,
    ) -> Result<(), BringError> {
        // An address that does not read is refused at once, not after.
        let base = base_of(&address)?;
        {
            let mut readings = self.readings();
            let reading = readings.entry(user).or_default();
            if reading.under_way {
                return Err(BringError::Busy);
            }
            *reading = Reading {
                under_way: true,
                ..Reading::default()
            };
        }
        let bringing = self.clone();
        let said = self.clone();
        let reading = tokio::spawn(async move {
            let catalog = said.catalog.clone();
            let taken = tokio::task::spawn_blocking(move || {
                let so_far = |read: Read| {
                    if let Some(reading) = said.readings().get_mut(&user) {
                        reading.read = read;
                    }
                };
                let mut taken = match source {
                    Source::Kavita => kavita::read(&base, &key, &so_far)?,
                };
                taken.source = source.name().to_owned();
                taken.address = base;
                Ok::<_, BringError>(taken)
            })
            .await??;
            Ok::<(), BringError>(catalog.keep_brought(user, &taken).await?)
        });
        // It is over however it ends, a panic included.
        tokio::spawn(async move {
            let kept = reading
                .await
                .map_err(BringError::from)
                .and_then(|kept| kept);
            let problem = kept.err().map(|error| {
                tracing::warn!(%error, "could not read the other server");
                error.for_people().to_owned()
            });
            let mut readings = bringing.readings();
            let reading = readings.entry(user).or_default();
            reading.under_way = false;
            reading.problem = problem;
        });
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_address_is_asked_as_it_is_meant() {
        assert_eq!(
            base_of(" nas.local:5000/ ").unwrap(),
            "http://nas.local:5000"
        );
        assert_eq!(base_of("https://k.example/").unwrap(), "https://k.example");
        assert!(matches!(base_of("ftp://nas"), Err(BringError::BadAddress)));
        assert!(matches!(base_of("  "), Err(BringError::BadAddress)));
        assert!(matches!(base_of("me:pw@nas"), Err(BringError::BadAddress)));
    }
}
