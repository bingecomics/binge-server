//! Search: finding series and books by the words in their names and in what
//! books say about themselves, reading lists by their names, and people by
//! the names they go by.
//!
//! Every series and book carries a key: its words in lower case and without
//! accents. A search is folded the same way, and finds whatever holds each
//! of its words somewhere in the key.

use sqlx::FromRow;
use sqlx::sqlite::SqliteRow;

use crate::accounts::UserId;
use crate::catalog::{self, Book, Catalog, CatalogError, PartOf, Series, Span};
use crate::comicinfo::ComicInfo;
use crate::import;
use crate::lists::{self, ReadingList};
use crate::natural::folded;
use crate::people::{self, Person};

/// Words taken from one search. More than a title has.
const MAX_WORDS: usize = 8;

/// Letters taken from one word. Longer than any word, and far below what
/// the database accepts as a pattern.
const MAX_WORD_CHARS: usize = 64;

/// What a search found, among what the user may read.
#[derive(Clone, Debug, Default)]
pub struct Found {
    /// In name order.
    pub series: Vec<Series>,
    /// Whether more series hold the words than are here.
    pub more_series: bool,
    /// In name order.
    pub books: Vec<Book>,
    /// Whether more books hold the words than are here.
    pub more_books: bool,
    /// Reading lists found by their name, in name order.
    pub lists: Vec<ReadingList>,
    pub more_lists: bool,
    /// People found by any name they go by, the ones most books name first.
    pub people: Vec<Person>,
    pub more_people: bool,
}

/// Folds text so that case and accents do not matter: `Élodie` and `elodie`
/// become the same.
pub(crate) fn fold(text: &str) -> String {
    folded(text).collect()
}

/// The given texts, folded, with one space between words.
pub(crate) fn key<'a>(texts: impl IntoIterator<Item = &'a str>) -> String {
    let folded: Vec<String> = texts.into_iter().map(fold).collect();
    let words: Vec<&str> = folded
        .iter()
        .flat_map(|text| text.split_whitespace())
        .collect();
    words.join(" ")
}

/// What a series is found by: its name.
pub(crate) fn series_key(name: &str) -> String {
    key([name])
}

/// What a book is found by: its file's name, and who and what its file says
/// it is.
pub(crate) fn book_key(name: &str, info: &ComicInfo) -> String {
    let said = [
        &info.title,
        &info.series,
        &info.writer,
        &info.penciller,
        &info.publisher,
    ];
    key(std::iter::once(name).chain(said.into_iter().flatten().map(String::as_str)))
}

/// A `LIKE` pattern, escaped with `\`, for keys that hold `word`, or the
/// start of it when it is longer than any word is.
fn holding(word: &str) -> String {
    let mut pattern = String::from("%");
    for letter in word.chars().take(MAX_WORD_CHARS) {
        if matches!(letter, '%' | '_' | '\\') {
            pattern.push('\\');
        }
        pattern.push(letter);
    }
    pattern.push('%');
    pattern
}

/// SQL that holds for a row whose key, the expression `key`, matches every
/// one of that many patterns: each as ` AND …`. The patterns are the values
/// a statement takes after the user, from `?2`.
fn every(key: &str, patterns: usize) -> String {
    (0..patterns)
        .map(|pattern| format!(" AND {key} LIKE ?{} ESCAPE '\\'", pattern + 2))
        .collect()
}

/// What `people::people` ends with to keep those one of whose names
/// matches every one of that many patterns.
fn named(patterns: usize) -> String {
    format!(
        "AND EXISTS (SELECT 1 FROM person_name sn WHERE sn.person_id = p.id{})",
        every("sn.search_key", patterns)
    )
}

/// The people most books name first.
const MOST_NAMED_FIRST: &str = "ORDER BY book_count DESC, p.name COLLATE natsort, p.id";

/// The words of a text as patterns for keys: see [`holding`]. `kept` is
/// how the keys keep a word.
fn patterns_of(text: &str, kept: impl Fn(&str) -> String) -> Vec<String> {
    let folded = fold(text);
    let words = folded.split_whitespace().take(MAX_WORDS);
    words.map(|word| holding(&kept(word))).collect()
}

/// Most of each kind of thing one search finds.
pub const MOST_FOUND: u32 = 50;

impl Catalog {
    /// The series, books, reading lists and people that hold every word of
    /// `text`, among what `user` may read: at most `limit` of each, and
    /// [`MOST_FOUND`] when it is not said or more is asked for. A search
    /// with no words finds nothing.
    pub async fn search(
        &self,
        user: UserId,
        text: &str,
        limit: Option<u32>,
    ) -> Result<Found, CatalogError> {
        let limit = limit.unwrap_or(MOST_FOUND).min(MOST_FOUND);
        let patterns = patterns_of(text, str::to_owned);
        if patterns.is_empty() {
            return Ok(Found::default());
        }
        // A list's name is kept as names are compared: its letters and digits.
        let in_names = patterns_of(text, import::name_key);
        // What each statement ends with. The limit follows the patterns.
        let (words, at) = (patterns.len(), patterns.len() + 2);
        let series = catalog::series(&format!(
            "{} ORDER BY s.name COLLATE natsort, s.id LIMIT ?{at}",
            every("s.search_key", words)
        ));
        let books = catalog::books(&format!(
            "{} ORDER BY b.name COLLATE natsort, b.id LIMIT ?{at}",
            every("b.search_key", words)
        ));
        let lists = lists::lists(&format!(
            "{} ORDER BY rl.name COLLATE natsort, rl.id LIMIT ?{at}",
            every("rl.name_key", words)
        ));
        // By any one of their names, the people most books name first.
        let people = people::people(&format!("{} {MOST_NAMED_FIRST} LIMIT ?{at}", named(words)));
        let ((series, more_series), (books, more_books), (rows, more_lists), (people, more_people)) =
            tokio::try_join!(
                self.first(&series, user, &patterns, limit),
                self.first(&books, user, &patterns, limit),
                self.first(&lists, user, &in_names, limit),
                self.first(&people, user, &patterns, limit),
            )?;
        Ok(Found {
            series,
            more_series,
            books,
            more_books,
            lists: self.with_books(user, rows).await?,
            more_lists,
            people,
            more_people,
        })
    }

    /// A part of the people the books `user` may read name, the ones most
    /// books name first: all of them, or with `text` those one of whose
    /// names holds every word of it.
    pub async fn people(
        &self,
        user: UserId,
        text: &str,
        span: Span,
    ) -> Result<PartOf<Person>, CatalogError> {
        let patterns = patterns_of(text, str::to_owned);
        let (words, at) = (patterns.len(), patterns.len() + 2);
        let named = named(words);
        let part = people::people(&format!(
            "{named} {MOST_NAMED_FIRST} LIMIT ?{at} OFFSET ?{}",
            at + 1
        ));
        let count = people::people_counted(&named);
        let (mut items, mut total) = (sqlx::query_as(&part), sqlx::query_scalar(&count));
        (items, total) = (items.bind(user), total.bind(user));
        for pattern in &patterns {
            (items, total) = (items.bind(pattern), total.bind(pattern));
        }
        let (items, total) = tokio::try_join!(
            items
                .bind(span.limit)
                .bind(span.offset)
                .fetch_all(self.db.pool()),
            total.fetch_one(self.db.pool()),
        )?;
        Ok(PartOf { items, total })
    }

    /// The first `limit` rows of a statement that takes the user, the
    /// patterns and one more than the limit, and whether there are more.
    async fn first<T>(
        &self,
        query: &str,
        user: UserId,
        patterns: &[String],
        limit: u32,
    ) -> Result<(Vec<T>, bool), CatalogError>
    where
        T: for<'row> FromRow<'row, SqliteRow> + Send + Unpin,
    {
        let mut found = sqlx::query_as(query).bind(user);
        for pattern in patterns {
            found = found.bind(pattern);
        }
        // One more than asked for says whether there are more.
        let mut rows: Vec<T> = found
            .bind(limit.saturating_add(1))
            .fetch_all(self.db.pool())
            .await?;
        let more = rows.len() > limit as usize;
        rows.truncate(limit as usize);
        Ok((rows, more))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_ignore_case_accents_and_spacing() {
        assert_eq!(key(["  Élodie   à PARIS "]), "elodie a paris");
        assert_eq!(key(["Batman 001", "Año Uno"]), "batman 001 ano uno");
        assert_eq!(key(["ﬁn"]), "fin");
        assert_eq!(key(["", "   "]), "");
    }

    #[test]
    fn a_book_is_found_by_its_name_and_by_what_its_file_says() {
        let info = ComicInfo {
            title: Some("The Long Night".into()),
            writer: Some("Élodie Durand".into()),
            summary: Some("Not searched.".into()),
            ..ComicInfo::default()
        };
        assert_eq!(
            book_key("Saga 001", &info),
            "saga 001 the long night elodie durand"
        );
        assert_eq!(book_key("Saga 001", &ComicInfo::default()), "saga 001");
        assert_eq!(series_key("  Año Uno "), "ano uno");
    }

    #[test]
    fn a_word_is_matched_as_written() {
        assert_eq!(holding("100%"), "%100\\%%");
        assert_eq!(holding("a_b\\c"), "%a\\_b\\\\c%");
        assert_eq!(holding(&"a".repeat(1000)).len(), MAX_WORD_CHARS + 2);
    }
}
