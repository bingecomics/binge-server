//! Reading a Kavita server, once, with one person's key there.
//!
//! Everything here asks and nothing here changes that server: what is
//! asked for is the libraries and their files, what the person has read
//! and when, their reading lists, collections, want to read and the pages
//! they marked, and, with the key of someone who manages it, who uses it.
//! The key goes to that server and nowhere else: it is not kept, not
//! logged, and not part of any error.

use std::collections::HashSet;
use std::time::Duration;

use binge_core::{
    Taken, TakenBook, TakenCollection, TakenLibrary, TakenList, TakenPage, TakenPerson,
    TakenReading, TakenSeries, TakenSitting,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;
use ureq::Agent;

use super::{BringError, Read};

/// What this server calls itself to that one.
const AS: &str = "Binge Server";

/// How many series, list or history entries are asked for at a time.
const A_PAGE: usize = 200;

/// The most pages one list is asked for: two million entries.
const MOST_PAGES: usize = 10_000;

/// The most one answer may hold, in bytes. A page of history is the
/// longest, at a few megabytes.
const MOST_SAID: u64 = 64 * 1024 * 1024;

/// What asks for everything: no condition, all of them.
const EVERYTHING: &str = r#"{"statements":[],"combination":1,"limitTo":0}"#;

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct SignedIn {
    username: String,
    token: String,
    kavita_version: String,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Myself {
    roles: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Library {
    id: i64,
    name: String,
    folders: Vec<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Series {
    id: i64,
    name: String,
    library_id: i64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Volume {
    chapters: Vec<Chapter>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Chapter {
    id: i64,
    pages: i64,
    pages_read: i64,
    last_reading_progress_utc: Option<String>,
    files: Vec<File>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct File {
    id: i64,
    file_path: String,
    pages: i64,
    bytes: i64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct List {
    id: i64,
    title: String,
    summary: Option<String>,
    promoted: bool,
    owner_user_name: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct ListItem {
    order: i64,
    chapter_id: i64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Collection {
    id: i64,
    title: String,
    summary: Option<String>,
    promoted: bool,
    owner: Option<String>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Bookmark {
    page: i64,
    chapter_id: i64,
}

/// What was read in one go, of one series.
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Session {
    is_generated_session: bool,
    start_time_utc: Option<String>,
    chapters: Vec<SessionChapter>,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct SessionChapter {
    chapter_id: i64,
    start_time_utc: Option<String>,
    end_time_utc: Option<String>,
    duration_seconds: i64,
    start_page: i64,
    end_page: i64,
}

#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase", default)]
struct Member {
    id: i64,
    username: String,
    roles: Vec<String>,
    libraries: Vec<Library>,
    age_restriction: Option<AgeRestriction>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct AgeRestriction {
    /// Below zero when the person is kept from nothing.
    age_rating: i64,
}

/// A Kavita server that took the key.
struct Kavita {
    base: String,
    agent: Agent,
    token: String,
}

/// What went wrong asking, without the address that was asked: it can
/// hold the key.
fn failed(error: &ureq::Error) -> BringError {
    match error {
        ureq::Error::StatusCode(401 | 403) => BringError::Refused,
        ureq::Error::StatusCode(status) => BringError::Answered(*status),
        ureq::Error::BadUri(_) => BringError::BadAddress,
        _ => BringError::Unreachable,
    }
}

/// Seconds since 1970 of a time as Kavita writes it, in UTC: a date, `T`,
/// and a time with or without parts of a second. Nothing for the year 1,
/// which is how it writes "never".
fn seconds(written: &str) -> Option<i64> {
    let number = |text: &str| text.parse::<i64>().ok();
    let (date, time) = written.trim_end_matches('Z').split_once('T')?;
    let mut date = date.splitn(3, '-').map(number);
    let (year, month, day) = (date.next()??, date.next()??, date.next()??);
    let time = time.split(['.', '+']).next()?;
    let mut time = time.splitn(3, ':').map(number);
    let (hour, minute, second) = (time.next()??, time.next()??, time.next()??);
    if year < 1970 || !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    // The days from 1970 to a date, by the months counted from March.
    let years = if month <= 2 { year - 1 } else { year };
    let era = years.div_euclid(400);
    let of_era = years.rem_euclid(400);
    let of_year = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let of_days = of_era * 365 + of_era / 4 - of_era / 100 + of_year;
    let days = era * 146_097 + of_days - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second)
}

impl Kavita {
    /// Shows the key to the server at `base` and keeps what it gives for
    /// it. Returns who the key is of, there.
    fn open(base: &str, key: &str) -> Result<(Self, SignedIn), BringError> {
        let config = Agent::config_builder()
            .user_agent(format!("binge-server/{}", binge_core::VERSION))
            .timeout_connect(Some(Duration::from_secs(15)))
            .timeout_recv_response(Some(Duration::from_secs(60)))
            .timeout_global(Some(Duration::from_secs(5 * 60)))
            .build();
        let agent = config.new_agent();
        let asked = agent
            .post(format!("{base}/api/Plugin/authenticate"))
            .query("apiKey", key.trim())
            .query("pluginName", AS)
            .send_empty();
        let answer = match asked {
            Ok(answer) => answer,
            Err(ureq::Error::StatusCode(401 | 403)) => return Err(BringError::WrongKey),
            // Another kind of server answers that it has no such page.
            Err(ureq::Error::StatusCode(_)) => return Err(BringError::NotThatServer),
            Err(error) => return Err(failed(&error)),
        };
        let signed_in: SignedIn = text_of(answer)
            .and_then(|text| read_as(&text))
            .map_err(|_| BringError::NotThatServer)?;
        if signed_in.token.is_empty() {
            return Err(BringError::NotThatServer);
        }
        let kavita = Self {
            base: base.to_owned(),
            agent,
            token: signed_in.token.clone(),
        };
        Ok((kavita, signed_in))
    }

    /// What a path answers, asked with `body` when there is one.
    fn asked(&self, path: &str, body: Option<&str>) -> Result<String, BringError> {
        let at = format!("{}{path}", self.base);
        let shown = format!("Bearer {}", self.token);
        let answer = match body {
            Some(body) => self
                .agent
                .post(at)
                .header("Authorization", shown)
                .header("Content-Type", "application/json")
                .send(body),
            None => self.agent.get(at).header("Authorization", shown).call(),
        };
        text_of(answer.map_err(|error| failed(&error))?)
    }

    fn get<T: DeserializeOwned>(&self, path: &str) -> Result<T, BringError> {
        read_as(&self.asked(path, None)?)
    }

    fn post<T: DeserializeOwned>(&self, path: &str, body: &str) -> Result<T, BringError> {
        read_as(&self.asked(path, Some(body))?)
    }

    /// Everything a path lists, asked for a page at a time. `path` ends
    /// where the page is named: in `?` or `&`.
    fn all<T: DeserializeOwned>(
        &self,
        path: &str,
        body: Option<&str>,
    ) -> Result<Vec<T>, BringError> {
        let mut all = Vec::new();
        let mut before = String::new();
        for page in 1..=MOST_PAGES {
            let said = self.asked(&format!("{path}PageNumber={page}&PageSize={A_PAGE}"), body)?;
            // A path that answers with all it has, whatever page is asked
            // for, says the same again: it was all there the first time.
            if page > 1 && said == before {
                return Ok(all);
            }
            let some: Vec<T> = read_as(&said)?;
            let last = some.len() < A_PAGE;
            all.extend(some);
            if last {
                return Ok(all);
            }
            before = said;
        }
        Err(BringError::Unreadable)
    }
}

/// What an answer says.
fn text_of(answer: ureq::http::Response<ureq::Body>) -> Result<String, BringError> {
    answer
        .into_body()
        .into_with_config()
        .limit(MOST_SAID)
        .read_to_string()
        .map_err(|_| BringError::Unreadable)
}

/// What an answer said, read as `T`.
fn read_as<T: DeserializeOwned>(said: &str) -> Result<T, BringError> {
    serde_json::from_str(said).map_err(|_| BringError::Unreadable)
}

/// Whether a version of Kavita is from before it timed reading, which it
/// does since 0.8.9. One that does not read as a version is asked.
fn from_before_it_timed(version: &str) -> bool {
    let mut parts = version.split('.').map(|part| part.parse::<u32>().ok());
    let (Some(Some(first)), Some(Some(second)), Some(Some(third))) =
        (parts.next(), parts.next(), parts.next())
    else {
        return false;
    };
    (first, second, third) < (0, 8, 9)
}

/// Reads the Kavita server at `base` with `key`, and returns what it said.
/// `so_far` is told how far the reading is.
pub fn read(base: &str, key: &str, so_far: &dyn Fn(Read)) -> Result<Taken, BringError> {
    let (kavita, signed_in) = Kavita::open(base, key)?;
    let myself: Myself = kavita.get("/api/Account")?;
    let manages = myself.roles.iter().any(|role| role == "Admin");
    let mut taken = Taken {
        version: signed_in.kavita_version,
        their_name: signed_in.username,
        manages,
        ..Taken::default()
    };

    let libraries: Vec<Library> = kavita.get("/api/Library/libraries")?;
    let mut series: Vec<Series> = Vec::new();
    for library in &libraries {
        let only = format!(
            r#"{{"statements":[{{"comparison":0,"field":19,"value":"{}"}}],"combination":1,"limitTo":0,"sortOptions":{{"sortField":1,"isAscending":true}}}}"#,
            library.id
        );
        series.extend(kavita.all::<Series>("/api/Series/all-v2?", Some(&only))?);
    }
    taken.libraries = libraries
        .into_iter()
        .map(|library| TakenLibrary {
            id: library.id,
            name: library.name,
            folders: library.folders,
        })
        .collect();

    let wanted: Vec<Series> = kavita.all("/api/want-to-read/v2?", Some(EVERYTHING))?;
    let wanted: HashSet<i64> = wanted.iter().map(|series| series.id).collect();

    // The files and how far each was read come with a series' volumes:
    // there is no asking for them all at once.
    let of = series.len() as u32;
    for (done, series) in series.iter().enumerate() {
        so_far(Read {
            series: done as u32,
            of,
        });
        let volumes: Vec<Volume> =
            kavita.get(&format!("/api/Series/volumes?seriesId={}", series.id))?;
        for chapter in volumes.into_iter().flat_map(|volume| volume.chapters) {
            let mut first_page = 0;
            for file in chapter.files {
                taken.books.push(TakenBook {
                    id: file.id,
                    chapter: chapter.id,
                    series: series.id,
                    library: series.library_id,
                    path: file.file_path,
                    file_size: file.bytes,
                    page_count: file.pages,
                    first_page,
                });
                first_page += file.pages;
            }
            if chapter.pages_read > 0 {
                taken.reading.push(TakenReading {
                    chapter: chapter.id,
                    pages_read: chapter.pages_read,
                    page_count: chapter.pages,
                    read_at: chapter
                        .last_reading_progress_utc
                        .as_deref()
                        .and_then(seconds),
                });
            }
        }
    }
    so_far(Read { series: of, of });
    taken.series = series
        .into_iter()
        .map(|series| TakenSeries {
            wanted: wanted.contains(&series.id),
            id: series.id,
            library: series.library_id,
            name: series.name,
        })
        .collect();

    // A version that does not time reading knows, of each chapter, the
    // day it was last read, and that came with the chapter.
    if !from_before_it_timed(&taken.version) {
        taken.sittings = sessions(&kavita, &taken.libraries)?;
    }

    let lists: Vec<List> = kavita.all(
        "/api/ReadingList/lists?includePromoted=true&",
        Some(EVERYTHING),
    )?;
    for list in lists {
        let mut items: Vec<ListItem> =
            kavita.get(&format!("/api/ReadingList/items?readingListId={}", list.id))?;
        items.sort_by_key(|item| item.order);
        taken.lists.push(TakenList {
            id: list.id,
            name: list.title,
            summary: list.summary.unwrap_or_default(),
            theirs: list.owner_user_name.as_deref() == Some(taken.their_name.as_str()),
            shared: list.promoted,
            chapters: items.into_iter().map(|item| item.chapter_id).collect(),
        });
    }

    let collections: Vec<Collection> = kavita.get("/api/Collection?ownedOnly=false")?;
    for collection in collections {
        let held: Vec<Series> = kavita.all(
            &format!(
                "/api/Series/series-by-collection?collectionId={}&",
                collection.id
            ),
            None,
        )?;
        taken.collections.push(TakenCollection {
            id: collection.id,
            name: collection.title,
            summary: collection.summary.unwrap_or_default(),
            theirs: collection.owner.as_deref() == Some(taken.their_name.as_str()),
            shared: collection.promoted,
            series: held.into_iter().map(|series| series.id).collect(),
        });
    }

    let marked: Vec<Bookmark> = kavita.post("/api/Reader/all-bookmarks", EVERYTHING)?;
    taken.pages = marked
        .into_iter()
        .map(|mark| TakenPage {
            chapter: mark.chapter_id,
            page: mark.page,
        })
        .collect();

    if manages {
        let members: Vec<Member> = kavita.get("/api/Users?includePending=false")?;
        taken.people = members
            .into_iter()
            .map(|member| TakenPerson {
                id: member.id,
                name: member.username,
                manages: member.roles.iter().any(|role| role == "Admin"),
                limited: member
                    .age_restriction
                    .is_some_and(|limit| limit.age_rating >= 0),
                libraries: member.libraries.iter().map(|library| library.id).collect(),
            })
            .collect();
    }
    Ok(taken)
}

/// The stretches of reading that server kept; none when it turns out to
/// keep none. One it made up, for a book
/// marked as read or for what was read before it timed anything, is kept
/// as the day it stands for: it started at midnight and took what such a
/// book takes on average.
fn sessions(kavita: &Kavita, libraries: &[TakenLibrary]) -> Result<Vec<TakenSitting>, BringError> {
    // It answers for the libraries named, and for none when none is.
    let named: String = libraries
        .iter()
        .map(|library| format!("libraries={}&", library.id))
        .collect();
    let path = format!("/api/Stats/reading-history?{named}");
    let read: Vec<Session> = match kavita.all(&path, None) {
        Ok(read) => read,
        Err(BringError::Answered(404 | 405)) => return Ok(Vec::new()),
        Err(error) => return Err(error),
    };
    let mut sittings = Vec::new();
    for session in read {
        let at_midnight = session
            .start_time_utc
            .as_deref()
            .and_then(seconds)
            .is_some_and(|start| start % 86_400 == 0);
        let measured = !session.is_generated_session && !at_midnight;
        for chapter in session.chapters {
            let Some(started_at) = chapter.start_time_utc.as_deref().and_then(seconds) else {
                continue;
            };
            let ended_at = chapter
                .end_time_utc
                .as_deref()
                .and_then(seconds)
                .unwrap_or(started_at + chapter.duration_seconds);
            sittings.push(TakenSitting {
                chapter: chapter.chapter_id,
                started_at,
                ended_at: if measured {
                    ended_at.max(started_at)
                } else {
                    started_at
                },
                first_page: chapter.start_page,
                last_page: chapter.end_page,
                measured,
            });
        }
    }
    Ok(sittings)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_time_is_read_as_seconds_since_1970() {
        assert_eq!(seconds("1970-01-01T00:00:00"), Some(0));
        assert_eq!(seconds("2024-02-29T12:30:15.1234567"), Some(1_709_209_815));
        assert_eq!(seconds("2026-11-21T16:20:22Z"), Some(1_795_278_022));
        // How "never" is written, and what is no time at all.
        assert_eq!(seconds("0001-01-01T00:00:00"), None);
        assert_eq!(seconds("soon"), None);
    }

    #[test]
    fn only_a_version_from_before_it_timed_reading_is_not_asked_for_it() {
        assert!(from_before_it_timed("0.8.8.3"));
        assert!(from_before_it_timed("0.7.14"));
        assert!(!from_before_it_timed("0.8.9.0"));
        assert!(!from_before_it_timed("0.10.0.1"));
        assert!(!from_before_it_timed("1.0.0"));
        // What does not read as a version is asked, and says itself.
        assert!(!from_before_it_timed(""));
        assert!(!from_before_it_timed("nightly"));
    }
}
