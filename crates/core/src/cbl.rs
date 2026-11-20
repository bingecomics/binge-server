//! Importing a CBL file: a reading list as ComicRack writes it, which is
//! how reading orders are passed around.
//!
//! A CBL file names books by their series, volume and number, not by any
//! file. [`Catalog::plan_cbl`] works out which book of the server each entry
//! means and says why, and makes nothing: the list is made afterwards, from
//! the books the owner settled on, like any other list.
//!
//! An entry's series is looked for by its name, then by its name with the
//! volume, then without its article, then without the words that say what
//! kind of publication it is. Each of these is tried in turn until one
//! finds the issue. What the owner fixes by hand is kept as a rule, and a
//! rule comes before all of them the next time.

use std::collections::{HashMap, HashSet};

use crate::accounts::UserId;
use crate::catalog::{Book, Catalog, CatalogError, GroupKind, PLACED, may_read};
use crate::import::{self, name_key, written};
use crate::lists::{UNREADABLE, list_with_key};
use crate::reading::list;

/// The biggest CBL file read, in bytes: 5 MB. One entry is a line of text.
pub const BIGGEST_FILE: usize = 5 * 1024 * 1024;

/// What a file bigger than [`BIGGEST_FILE`] is refused with.
pub fn too_big() -> CatalogError {
    CatalogError::Invalid("That file is too big. The server reads a CBL file of 5 MB at most.")
}

/// Most entries read from one file.
const MOST_ENTRIES: usize = 5000;

/// What a file that is not a reading list is answered with.
const NOT_A_LIST: &str = "That is not a CBL reading list the server can read.";

/// One book as a CBL file names it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct CblEntry {
    pub series: String,
    /// The issue's number, as the file writes it.
    pub number: Option<String>,
    /// The volume, as the file writes it: most files give the year the
    /// series started.
    pub volume: Option<String>,
    /// The year the issue came out.
    pub year: Option<i64>,
}

/// How the series of an entry was found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CblHow {
    /// By what the owner chose for it before.
    Rule,
    /// A series has exactly that name.
    Name,
    /// A series is called by the name and the volume: "Batman (2016)".
    NameAndVolume,
    /// The names are the same without "The", "A" or "An" in front.
    WithoutArticle,
    /// The names are the same without the words that say what kind of
    /// publication it is, such as "Annual" or "TPB".
    WithoutWords,
}

/// What was found for an entry.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CblFound {
    /// One book.
    Book,
    /// No series goes by the entry's name.
    NoSeries,
    /// The series is there, and no book of it has the number.
    NoIssue,
    /// Several books fit, and nothing in the entry tells them apart.
    Several,
}

/// One entry of a CBL file, with what the server found for it.
#[derive(Clone, Debug)]
pub struct CblLine {
    pub entry: CblEntry,
    pub found: CblFound,
    /// How its series was found. Missing when none was.
    pub how: Option<CblHow>,
    /// The rule that decided, to forget when it decided wrong.
    pub rule_id: Option<i64>,
    /// The book the entry means.
    pub book: Option<Book>,
    /// The books that fit, when there are several.
    pub candidates: Vec<Book>,
    /// The series that was looked in: its id and its name.
    pub series: Option<(i64, String)>,
}

/// What a list made from a CBL file would hold.
#[derive(Clone, Debug)]
pub struct CblPlan {
    /// The name the file gives the list.
    pub name: Option<String>,
    /// The id of the list that already has that name.
    pub name_taken_by: Option<i64>,
    /// The file's entries, in its order.
    pub lines: Vec<CblLine>,
}

/// What a rule says an entry of a CBL file means.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CblRuleFor {
    /// Its series, by that name and volume, is this series: its issues are
    /// looked for there.
    Series(i64),
    /// The entry is this book.
    Book(i64),
}

/// A CBL file, read.
#[derive(Debug, PartialEq, Eq)]
struct Cbl {
    name: Option<String>,
    entries: Vec<CblEntry>,
}

fn parse(file: &[u8]) -> Result<Cbl, CatalogError> {
    if file.len() > BIGGEST_FILE {
        return Err(too_big());
    }
    let text = String::from_utf8_lossy(file);
    let text = text.trim_start_matches('\u{feff}');
    let document =
        roxmltree::Document::parse(text).map_err(|_| CatalogError::Invalid(NOT_A_LIST))?;
    let root = document.root_element();
    if root.tag_name().name() != "ReadingList" {
        return Err(CatalogError::Invalid(NOT_A_LIST));
    }
    let child = |name: &str| {
        root.children()
            .find(|node| node.is_element() && node.tag_name().name() == name)
    };
    let said = |text: Option<&str>| {
        text.map(str::trim)
            .filter(|text| !text.is_empty())
            .map(str::to_owned)
    };
    let entries: Vec<CblEntry> = child("Books")
        .into_iter()
        .flat_map(|books| books.children())
        .filter(|node| node.is_element() && node.tag_name().name() == "Book")
        .filter_map(|book| {
            Some(CblEntry {
                series: said(book.attribute("Series"))?,
                number: said(book.attribute("Number")),
                volume: said(book.attribute("Volume")),
                year: said(book.attribute("Year"))
                    .and_then(|year| year.parse().ok())
                    .filter(|year| (1..=9999).contains(year)),
            })
        })
        .collect();
    if entries.is_empty() {
        return Err(CatalogError::Invalid("That file lists no books."));
    }
    if entries.len() > MOST_ENTRIES {
        return Err(CatalogError::Invalid(
            "That file lists more than 5,000 books, which is more than one reading list takes.",
        ));
    }
    Ok(Cbl {
        name: said(child("Name").and_then(|name| name.text())),
        entries,
    })
}

/// A name without "The", "A" or "An" in front of it.
fn without_article(name: &str) -> &str {
    let name = name.trim();
    ["the ", "a ", "an "]
        .iter()
        .find_map(|article| {
            let (start, rest) = name.split_at_checked(article.len())?;
            let rest = rest.trim_start();
            (start.eq_ignore_ascii_case(article) && !rest.is_empty()).then_some(rest)
        })
        .unwrap_or(name)
}

/// Words that say what kind of publication a name is of, each as its
/// [`name_key`], and the kind of group such books sit in. Only words that
/// are no part of a title: "Special" and "Absolute" start real series.
const FORMAT_WORDS: &[(&str, GroupKind)] = &[
    ("annual", GroupKind::Annual),
    ("annuals", GroupKind::Annual),
    ("gn", GroupKind::Collected),
    ("hc", GroupKind::Collected),
    ("omnibus", GroupKind::Collected),
    ("tpb", GroupKind::Collected),
];

/// A name without what it holds in brackets, without its article and
/// without the words of [`FORMAT_WORDS`], and the kind of group the first
/// such word names. "Batman Annual (2016)" is "Batman", among the annuals.
fn without_words(name: &str) -> (String, Option<GroupKind>) {
    let outside = import::unbracketed(name).unwrap_or_else(|| name.to_owned());
    let mut kind = None;
    let kept: Vec<&str> = without_article(&outside)
        .split_whitespace()
        .filter(|word| {
            let key = name_key(word);
            let format = FORMAT_WORDS.iter().find(|(known, _)| *known == key);
            if let Some((_, named)) = format {
                kind.get_or_insert(*named);
            }
            format.is_none()
        })
        .collect();
    if kept.is_empty() {
        (outside, None)
    } else {
        (kept.join(" "), kind)
    }
}

/// Parts of a rule's key are told apart by a sign no name holds.
const BETWEEN: char = '\u{1f}';

/// What a rule about an entry's series is kept under.
fn series_rule_key(entry: &CblEntry) -> String {
    let volume = entry.volume.as_deref().unwrap_or_default();
    format!("s{BETWEEN}{}{BETWEEN}{volume}", name_key(&entry.series))
}

/// What a rule about one entry is kept under.
fn book_rule_key(entry: &CblEntry) -> String {
    let volume = entry.volume.as_deref().unwrap_or_default();
    let number = entry.number.as_deref().map(written).unwrap_or_default();
    format!(
        "b{BETWEEN}{}{BETWEEN}{volume}{BETWEEN}{number}",
        name_key(&entry.series)
    )
}

/// A rule the owner made.
#[derive(sqlx::FromRow)]
struct Rule {
    id: i64,
    entry_key: String,
    series_id: Option<i64>,
    content_hash: Option<String>,
}

/// The series a user may read, by each way a name is compared.
#[derive(Default)]
struct Names {
    /// By name.
    exact: HashMap<String, Vec<i64>>,
    /// By name without its article.
    bare: HashMap<String, Vec<i64>>,
    /// By name without its article and its words.
    plain: HashMap<String, Vec<i64>>,
    called: HashMap<i64, String>,
}

impl Names {
    fn of(series: Vec<(i64, String)>) -> Self {
        let mut names = Self::default();
        for (id, name) in series {
            names.exact.entry(name_key(&name)).or_default().push(id);
            let bare = name_key(without_article(&name));
            names.bare.entry(bare).or_default().push(id);
            let plain = name_key(&without_words(&name).0);
            names.plain.entry(plain).or_default().push(id);
            names.called.insert(id, name);
        }
        names
    }
}

/// Some series to look for an entry's issue in, and why those.
struct Attempt {
    how: CblHow,
    rule: Option<i64>,
    series: Vec<i64>,
}

/// Where to look for an entry's issue, in the order to try, and the kind of
/// group the entry's name says the book is in.
fn attempts(
    entry: &CblEntry,
    names: &Names,
    rules: &HashMap<String, Rule>,
) -> (Vec<Attempt>, Option<GroupKind>) {
    let (plain, kind) = without_words(&entry.series);
    // What the owner said the name means is all there is to try.
    let rule = rules.get(&series_rule_key(entry));
    if let Some((rule, series)) = rule.and_then(|rule| Some((rule, rule.series_id?))) {
        let attempt = Attempt {
            how: CblHow::Rule,
            rule: Some(rule.id),
            series: vec![series],
        };
        return (vec![attempt], kind);
    }
    let with_volume = entry
        .volume
        .as_ref()
        .map(|volume| name_key(&format!("{} {volume}", entry.series)));
    let ways = [
        (CblHow::Name, names.exact.get(&name_key(&entry.series))),
        (
            CblHow::NameAndVolume,
            with_volume.and_then(|key| names.exact.get(&key)),
        ),
        (
            CblHow::WithoutArticle,
            names.bare.get(&name_key(without_article(&entry.series))),
        ),
        (CblHow::WithoutWords, names.plain.get(&name_key(&plain))),
    ];
    let mut tries: Vec<Attempt> = Vec::new();
    for (how, series) in ways {
        // The same series found another way hold nothing new.
        if let Some(series) = series
            && !tries.iter().any(|tried| tried.series == *series)
        {
            tries.push(Attempt {
                how,
                rule: None,
                series: series.clone(),
            });
        }
    }
    (tries, kind)
}

/// A book where it sits, as far as matching asks.
#[derive(sqlx::FromRow)]
struct Placed {
    id: i64,
    content_hash: String,
    series_id: i64,
    number: Option<String>,
    published: Option<i64>,
    kind: Option<GroupKind>,
    group_number: Option<i64>,
}

/// The books among `placed` that an entry may mean: the ones with its
/// number, told apart as far as the entry tells them apart, by the kind of
/// group its name says, by its volume, and by its year. A book the server
/// holds twice is there once.
fn pick<'a>(entry: &CblEntry, kind: Option<GroupKind>, placed: &[&'a Placed]) -> Vec<&'a Placed> {
    let number = entry.number.as_deref().map(written);
    let once = |books: &mut dyn Iterator<Item = &'a Placed>| -> Vec<&'a Placed> {
        let mut seen = HashSet::new();
        books
            .filter(|book| seen.insert(book.content_hash.as_str()))
            .collect()
    };
    let mut found = once(&mut placed.iter().copied().filter(|book| book.number == number));
    // A series that is one book with no number is that book, whatever number
    // the file gives it: a one-shot is "#1" to a CBL file.
    if found.is_empty() {
        let all = once(&mut placed.iter().copied());
        if let [only] = all.as_slice()
            && only.number.is_none()
        {
            found = all;
        }
    }
    // Each is asked only while several fit, and only if it leaves any.
    let mut narrow = |fits: &dyn Fn(&Placed) -> bool| {
        if found.len() > 1 && found.iter().any(|book| fits(book)) {
            found.retain(|book| fits(book));
        }
    };
    match kind {
        Some(kind) => narrow(&|book| book.kind == Some(kind)),
        // With no word for it, an issue of a run before a special.
        None => narrow(&|book| {
            matches!(
                book.kind,
                None | Some(GroupKind::Volume | GroupKind::Limited)
            )
        }),
    }
    if let Some(volume) = entry.volume.as_deref().and_then(|v| v.parse::<i64>().ok()) {
        narrow(&|book| book.group_number == Some(volume));
    }
    if let Some(year) = entry.year {
        narrow(&|book| book.published.is_some_and(|date| date / 10_000 == year));
    }
    found
}

/// What was settled for an entry, by ids.
struct Chosen {
    found: CblFound,
    how: Option<CblHow>,
    rule_id: Option<i64>,
    book: Option<i64>,
    candidates: Vec<i64>,
    series: Option<i64>,
}

/// Settles one entry. `ruled` are the books that rules name, by identity;
/// `placed` the books of every series the attempts look in.
fn choose(
    entry: &CblEntry,
    (tries, kind): &(Vec<Attempt>, Option<GroupKind>),
    rules: &HashMap<String, Rule>,
    ruled: &HashMap<String, i64>,
    placed: &HashMap<i64, Vec<Placed>>,
) -> Chosen {
    let said = rules.get(&book_rule_key(entry)).and_then(|rule| {
        let book = ruled.get(rule.content_hash.as_ref()?)?;
        Some((rule.id, *book))
    });
    if let Some((rule, book)) = said {
        return Chosen {
            found: CblFound::Book,
            how: Some(CblHow::Rule),
            rule_id: Some(rule),
            book: Some(book),
            candidates: Vec::new(),
            series: None,
        };
    }
    for attempt in tries {
        let in_series: Vec<&Placed> = attempt
            .series
            .iter()
            .filter_map(|id| placed.get(id))
            .flatten()
            .collect();
        let fit = pick(entry, *kind, &in_series);
        let (found, book, candidates) = match fit.as_slice() {
            [] => continue,
            [one] => (CblFound::Book, Some(one.id), Vec::new()),
            several => (
                CblFound::Several,
                None,
                several.iter().map(|book| book.id).collect(),
            ),
        };
        return Chosen {
            found,
            how: Some(attempt.how),
            rule_id: attempt.rule,
            book,
            candidates,
            series: fit.first().map(|book| book.series_id),
        };
    }
    // Nothing fits. The first series found by name is where the issue is
    // missing.
    let looked = tries.first();
    Chosen {
        found: if looked.is_some() {
            CblFound::NoIssue
        } else {
            CblFound::NoSeries
        },
        how: looked.map(|attempt| attempt.how),
        rule_id: looked.and_then(|attempt| attempt.rule),
        book: None,
        candidates: Vec::new(),
        series: looked.and_then(|attempt| attempt.series.first().copied()),
    }
}

impl Catalog {
    /// Reads a CBL file and says, for each of its entries, which book among
    /// those `user` may read it means and how that was found. Makes nothing.
    pub async fn plan_cbl(&self, user: UserId, file: &[u8]) -> Result<CblPlan, CatalogError> {
        let cbl = parse(file)?;
        let query = format!(
            "SELECT s.id, s.name FROM series s WHERE {}",
            may_read("s.library_id")
        );
        let series: Vec<(i64, String)> = sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        let names = Names::of(series);
        let rules: Vec<Rule> =
            sqlx::query_as("SELECT id, entry_key, series_id, content_hash FROM cbl_rule")
                .fetch_all(self.db.pool())
                .await?;
        let rules: HashMap<String, Rule> = rules
            .into_iter()
            .map(|rule| (rule.entry_key.clone(), rule))
            .collect();

        let tries: Vec<_> = cbl
            .entries
            .iter()
            .map(|entry| attempts(entry, &names, &rules))
            .collect();
        let looked_in: HashSet<i64> = tries
            .iter()
            .flat_map(|(tries, _)| tries.iter().flat_map(|attempt| attempt.series.iter()))
            .copied()
            .collect();
        let placed = self.placed_in(user, looked_in).await?;
        let ruled = self.books_ruled(user, &rules).await?;

        let chosen: Vec<Chosen> = cbl
            .entries
            .iter()
            .zip(&tries)
            .map(|(entry, tries)| choose(entry, tries, &rules, &ruled, &placed))
            .collect();

        // The books themselves, to show what was found.
        let ids: Vec<i64> = chosen
            .iter()
            .flat_map(|chosen| chosen.book.iter().chain(&chosen.candidates))
            .copied()
            .collect();
        let by_id = self.books_by_id(user, &ids).await?;
        let book = |id: &i64| by_id.get(id).cloned();

        let name_taken_by = match &cbl.name {
            Some(name) => list_with_key(self.db.pool(), &name_key(name)).await?,
            None => None,
        };
        let lines = cbl
            .entries
            .into_iter()
            .zip(chosen)
            .map(|(entry, chosen)| CblLine {
                entry,
                found: chosen.found,
                how: chosen.how,
                rule_id: chosen.rule_id,
                book: chosen.book.as_ref().and_then(book),
                candidates: chosen.candidates.iter().filter_map(book).collect(),
                series: chosen
                    .series
                    .and_then(|id| Some((id, names.called.get(&id)?.clone()))),
            })
            .collect();
        Ok(CblPlan {
            name: cbl.name,
            name_taken_by,
            lines,
        })
    }

    /// The books of some series that `user` may read, by series.
    async fn placed_in(
        &self,
        user: UserId,
        series: HashSet<i64>,
    ) -> Result<HashMap<i64, Vec<Placed>>, CatalogError> {
        let mut by_series: HashMap<i64, Vec<Placed>> = HashMap::new();
        if series.is_empty() {
            return Ok(by_series);
        }
        let ids: Vec<i64> = series.into_iter().collect();
        let query = format!(
            "SELECT b.id, b.content_hash, bp.series_id, bp.number, bp.published, g.kind, \
             g.number AS group_number FROM {PLACED} \
             LEFT JOIN series_group g ON g.id = bp.group_id \
             WHERE bp.series_id IN ({}) AND {} ORDER BY b.id",
            list(&ids),
            may_read("b.library_id")
        );
        let placed: Vec<Placed> = sqlx::query_as(&query)
            .persistent(false)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        for book in placed {
            by_series.entry(book.series_id).or_default().push(book);
        }
        Ok(by_series)
    }

    /// The books that rules name and `user` may read: the id of one file of
    /// each, by the book's identity.
    async fn books_ruled(
        &self,
        user: UserId,
        rules: &HashMap<String, Rule>,
    ) -> Result<HashMap<String, i64>, CatalogError> {
        if !rules.values().any(|rule| rule.content_hash.is_some()) {
            return Ok(HashMap::new());
        }
        let query = format!(
            "SELECT b.content_hash, MIN(b.id) FROM book b \
             WHERE b.content_hash IN (SELECT content_hash FROM cbl_rule) AND {} \
             GROUP BY b.content_hash",
            may_read("b.library_id")
        );
        let found: Vec<(String, i64)> = sqlx::query_as(&query)
            .bind(user)
            .fetch_all(self.db.pool())
            .await?;
        Ok(found.into_iter().collect())
    }

    /// Keeps what the owner says an entry of a CBL file means, for this
    /// import and the ones after it, and returns the rule's id. It takes the
    /// place of what was said before about the same entry, or about the same
    /// series for a rule about a series.
    pub async fn keep_cbl_rule(
        &self,
        owner: UserId,
        entry: &CblEntry,
        means: CblRuleFor,
    ) -> Result<i64, CatalogError> {
        if entry.series.trim().is_empty() {
            return Err(CatalogError::Invalid(
                "Say which series of the file the rule is for.",
            ));
        }
        let missing = |error| match error {
            CatalogError::NotFound => {
                CatalogError::Invalid("The series or the book chosen is not on the server.")
            }
            other => other,
        };
        let (key, series, identity) = match means {
            CblRuleFor::Series(id) => {
                self.series(owner, id).await.map_err(missing)?;
                (series_rule_key(entry), Some(id), None)
            }
            CblRuleFor::Book(id) => {
                let book = self.book(owner, id).await.map_err(missing)?;
                let identity = book.content_hash.ok_or(CatalogError::Invalid(UNREADABLE))?;
                (book_rule_key(entry), None, Some(identity))
            }
        };
        Ok(sqlx::query_scalar(
            "INSERT INTO cbl_rule (entry_key, series_id, content_hash) VALUES (?, ?, ?) \
             ON CONFLICT (entry_key) DO UPDATE SET series_id = excluded.series_id, \
             content_hash = excluded.content_hash RETURNING id",
        )
        .bind(key)
        .bind(series)
        .bind(identity)
        .fetch_one(self.db.pool())
        .await?)
    }

    /// Forgets a rule.
    pub async fn forget_cbl_rule(&self, id: i64) -> Result<(), CatalogError> {
        let forgotten = sqlx::query("DELETE FROM cbl_rule WHERE id = ?")
            .bind(id)
            .execute(self.db.pool())
            .await?;
        if forgotten.rows_affected() == 0 {
            return Err(CatalogError::NoSuchRule);
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn entry(series: &str, number: &str, volume: &str, year: i64) -> CblEntry {
        let said = |text: &str| (!text.is_empty()).then(|| text.to_owned());
        CblEntry {
            series: series.to_owned(),
            number: said(number),
            volume: said(volume),
            year: (year > 0).then_some(year),
        }
    }

    #[test]
    fn a_cbl_file_is_read_entry_by_entry() {
        let file = br#"<?xml version="1.0"?>
            <ReadingList xmlns:xsd="http://www.w3.org/2001/XMLSchema">
              <Name> Night of the Runner </Name>
              <Books>
                <Book Series="Night Runner" Number="1" Volume="1987" Year="1987">
                  <Id>3f0c</Id>
                </Book>
                <Book Series="The Lamplighters" Number="12" Volume="" Year="none" />
                <Book Number="3" />
              </Books>
              <Matchers />
            </ReadingList>"#;
        let read = parse(file).unwrap();
        assert_eq!(read.name.as_deref(), Some("Night of the Runner"));
        assert_eq!(
            read.entries,
            [
                entry("Night Runner", "1", "1987", 1987),
                entry("The Lamplighters", "12", "", 0),
            ]
        );

        for not_one in [
            &b"not xml"[..],
            b"<ComicInfo><Series>Night Runner</Series></ComicInfo>",
            b"<ReadingList><Name>Empty</Name><Books /></ReadingList>",
        ] {
            assert!(matches!(parse(not_one), Err(CatalogError::Invalid(_))));
        }
    }

    #[test]
    fn names_lose_their_article_and_the_words_for_a_format() {
        assert_eq!(without_article("The Lamplighters"), "Lamplighters");
        assert_eq!(without_article("an Orchard"), "Orchard");
        assert_eq!(without_article("Theatre"), "Theatre");
        assert_eq!(without_article("The "), "The");

        assert_eq!(
            without_words("The Night Runner Annual (1987)"),
            ("Night Runner".to_owned(), Some(GroupKind::Annual))
        );
        assert_eq!(
            without_words("Lamplighters TPB"),
            ("Lamplighters".to_owned(), Some(GroupKind::Collected))
        );
        // A word that starts real titles stays.
        assert_eq!(
            without_words("Absolute Night"),
            ("Absolute Night".to_owned(), None)
        );
        assert_eq!(without_words("Annual"), ("Annual".to_owned(), None));
    }

    fn placed(id: i64, number: &str, group: Option<(GroupKind, i64)>, year: i64) -> Placed {
        Placed {
            id,
            content_hash: format!("{id:064}"),
            series_id: 1,
            number: (!number.is_empty()).then(|| number.to_owned()),
            published: (year > 0).then_some(year * 10_000 + 9999),
            kind: group.map(|(kind, _)| kind),
            group_number: group.map(|(_, number)| number).filter(|number| *number > 0),
        }
    }

    fn picked(entry: &CblEntry, kind: Option<GroupKind>, books: &[Placed]) -> Vec<i64> {
        let books: Vec<&Placed> = books.iter().collect();
        pick(entry, kind, &books).iter().map(|b| b.id).collect()
    }

    #[test]
    fn an_issue_is_told_apart_by_kind_volume_and_year() {
        let books = [
            placed(1, "1", Some((GroupKind::Volume, 1987)), 1987),
            placed(2, "1", Some((GroupKind::Volume, 2011)), 2011),
            placed(3, "1", Some((GroupKind::Annual, 0)), 1988),
            placed(4, "2", Some((GroupKind::Volume, 1987)), 1987),
        ];
        // Padded as files pad them, the number is the same.
        assert_eq!(picked(&entry("N", "001", "2011", 0), None, &books), [2]);
        assert_eq!(picked(&entry("N", "1", "", 1987), None, &books), [1]);
        // An issue of a run comes before a special, and a word for the
        // special turns that around.
        assert_eq!(picked(&entry("N", "1", "", 0), None, &books), [1, 2]);
        let annual = Some(GroupKind::Annual);
        assert_eq!(picked(&entry("N", "1", "", 0), annual, &books), [3]);
        assert_eq!(picked(&entry("N", "2", "1999", 1999), None, &books), [4]);
        assert!(picked(&entry("N", "7", "", 0), None, &books).is_empty());
    }

    #[test]
    fn a_series_of_one_book_without_a_number_is_that_book() {
        let one_shot = [placed(1, "", Some((GroupKind::OneShot, 0)), 1988)];
        assert_eq!(picked(&entry("N", "1", "", 0), None, &one_shot), [1]);
        // One issue of a run is not any issue of it.
        let third = [placed(1, "3", None, 1988)];
        assert!(picked(&entry("N", "1", "", 0), None, &third).is_empty());
        // The same pages in two files are one book.
        let mut twice = [placed(1, "1", None, 0), placed(2, "1", None, 0)];
        twice[1].content_hash = twice[0].content_hash.clone();
        assert_eq!(picked(&entry("N", "1", "", 0), None, &twice), [1]);
    }
}
