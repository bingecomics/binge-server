//! Import: working out where a book goes when a library first sees it.
//!
//! A book's place is its series, its group in that series, and its number.
//! Each part is taken from the file's tags, else from the file's name, else
//! from its folder. This is done once per book; after that the place is
//! Binge's own record and the file is not asked again.
//!
//! The file name patterns are the ones comic files are commonly named by:
//! `Teen Titans v1 001 (1966-02)`,
//! `Batgirl Vol.2000 #57`, `Saga 001 (2012) (Digital)`.

use std::ops::Range;

use crate::catalog::GroupKind;
use crate::comicinfo::{ComicInfo, MOST_VOLUMES};
use crate::layout;
use crate::search::fold;

/// Where import puts a book.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Placed {
    /// The series' name, as the tag, the file or the folder writes it.
    pub series: String,
    /// The group of the series the book is in, as its kind and number: the
    /// volume with that number, or for a book that is no issue of a run the
    /// group [`kind_of_special`] names. Annuals take their volume's number
    /// when the book gives it; the other kinds have none. Missing for a book
    /// that sits directly in its series.
    pub group: Option<(GroupKind, Option<i64>)>,
    /// The issue's number as written, without leading zeros: `12`, `0.5`.
    pub number: Option<String>,
    /// The date published: see [`published`].
    pub published: Option<i64>,
}

/// Where the book in the file called `name`, in `folder`, goes. `folder` is
/// relative to the library and empty for the library's own folder.
pub(crate) fn place(name: &str, folder: &str, info: &ComicInfo) -> Placed {
    let named = read_name(name);
    // What a tag says is used when a place may hold it: the owner must be
    // able to give a book the place import gave it.
    let said = info.number.as_deref();
    let number = said
        .and_then(|number| layout::issue_number(number).ok())
        .map(written)
        .or(named.number);
    let mut volume = info.volume.or(named.volume);
    // A special's file is often named after the special, not the series, so
    // its folder is asked before its name.
    let from_name = named.series;
    let said = info.series.as_deref();
    let mut series = said
        .and_then(|series| layout::series_name(series).ok())
        .map(str::to_owned)
        .or_else(|| from_name.clone().filter(|_| !named.special));
    // The folders, the nearest first. One named only after a volume, such as
    // `Volume 2`, gives the volume and leaves the series to the one above.
    for folder in folder.rsplit('/').filter(|folder| !folder.is_empty()) {
        if series.is_some() {
            break;
        }
        let read = read_name(folder);
        if read.volume.is_some() && read.number.is_none() {
            volume = volume.or(read.volume);
            series = read.series;
        } else {
            series = Some(folder.to_owned());
        }
    }
    // A file in the library's own folder that names no series is one itself.
    let series = series
        .or(from_name)
        .map(|series| series.trim().to_owned())
        .filter(|series| !series.is_empty())
        .unwrap_or_else(|| unbracketed(name).unwrap_or_else(|| name.to_owned()));
    // A file's or a folder's name may be longer than a series' name.
    let series = series.chars().take(layout::MAX_NAME).collect();
    let said = info.format.as_deref().and_then(kind_of_special);
    let special = named.special || said.is_some() || (volume.is_none() && number.is_none());
    let group = if special {
        // With no tag to say what kind of special it is, it is a book read
        // by itself. Whether it is an appearance in another title is not
        // the file's to say, and no word in its name is taken for it.
        match said.unwrap_or(GroupKind::OneShot) {
            GroupKind::Annual => Some((GroupKind::Annual, volume)),
            kind => Some((kind, None)),
        }
    } else {
        volume.map(|volume| (GroupKind::Volume, Some(volume)))
    };

    Placed {
        series,
        group,
        number,
        published: published(info),
    }
}

/// The date a file gives, as [`date_number`] writes it. Missing without a
/// year.
fn published(info: &ComicInfo) -> Option<i64> {
    let year = info.year?;
    // A day means nothing without its month, and one the month does not
    // have is a mistake in the tag: the rest of the date is kept.
    let day = info
        .month
        .zip(info.day)
        .filter(|&(month, day)| day <= days_in(year, month))
        .map(|(_, day)| day);
    Some(date_number(year, info.month, day))
}

/// How many days a month has.
pub(crate) fn days_in(year: i64, month: i64) -> i64 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// A date as one number that sorts by time: year * 10000 + month * 100 + day,
/// with 99 for a month or a day that is not known, so that a book that gives
/// less comes after one that gives more.
pub(crate) fn date_number(year: i64, month: Option<i64>, day: Option<i64>) -> i64 {
    year * 10_000 + month.unwrap_or(99) * 100 + day.unwrap_or(99)
}

/// What a series is known by when names are compared: its letters and digits
/// in lower case and without accents. `Spider-Man`, `Spider Man` and
/// `SPIDERMAN` are one series.
pub(crate) fn name_key(name: &str) -> String {
    let folded = fold(name);
    let key: String = folded.chars().filter(|c| c.is_alphanumeric()).collect();
    // A name of signs only is still a name.
    if key.is_empty() {
        folded.trim().to_owned()
    } else {
        key
    }
}

/// What a `Format` tag has to hold for the book to be no issue of a run,
/// and the kind of group such a book goes in. Each format is written as
/// its [`name_key`], so that `One-Shot` and `one shot` are one.
const SPECIAL_FORMATS: &[(&str, GroupKind)] = &[
    ("annual", GroupKind::Annual),
    ("absolute", GroupKind::Collected),
    ("boxset", GroupKind::Collected),
    ("compendium", GroupKind::Collected),
    ("gn", GroupKind::Collected),
    ("graphicnovel", GroupKind::Collected),
    ("omnibus", GroupKind::Collected),
    ("tpb", GroupKind::Collected),
    ("tradepaperback", GroupKind::Collected),
    ("anthology", GroupKind::OneShot),
    ("directorscut", GroupKind::OneShot),
    ("epilogue", GroupKind::OneShot),
    ("fcbd", GroupKind::OneShot),
    ("giantsize", GroupKind::OneShot),
    ("oneshot", GroupKind::OneShot),
    ("prologue", GroupKind::OneShot),
    ("reference", GroupKind::OneShot),
    ("special", GroupKind::OneShot),
];

/// The kind of group a `Format` tag puts its book in. Missing when the
/// format is that of an issue of a run, or says nothing Binge knows.
fn kind_of_special(format: &str) -> Option<GroupKind> {
    let key = name_key(format);
    SPECIAL_FORMATS
        .iter()
        .find(|(known, _)| *known == key)
        .map(|(_, kind)| *kind)
}

/// What a file's or a folder's name says.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Named {
    /// The words before the volume or the number. Missing when the name
    /// gives neither, or nothing but a word such as "Chapter" before it.
    series: Option<String>,
    volume: Option<i64>,
    number: Option<String>,
    /// Whether the name carries the marker for a special, `SP01`.
    special: bool,
}

/// Words that come before a number without being a series' name.
const NOT_A_NAME: &[&str] = &[
    "book", "chapter", "ep", "episode", "issue", "no", "number", "part", "tome", "vol", "volume",
];

/// Words that say the number after them is the issue's.
const BEFORE_A_NUMBER: &[&str] = &["chapter", "ch", "ch.", "ep", "ep.", "episode", "issue"];

/// Words that say the number after them is the volume's.
const BEFORE_A_VOLUME: &[&str] = &["v", "v.", "vol", "vol.", "volume", "tome"];

fn read_name(name: &str) -> Named {
    let (outside, counted) = without_brackets(name);
    let words: Vec<&str> = outside.split_whitespace().collect();

    // Where the series' name ends: at the first word that is about something
    // else.
    let mut end = words.len();
    let mut ends_at = |word: usize| end = end.min(word);

    let mut named = Named::default();
    let marker = words.iter().position(|word| is_special_marker(word));
    if let Some(marker) = marker {
        named.special = true;
        ends_at(marker);
    }

    // The words that give the volume: none when the name gives no volume.
    let mut volume_words = 0..0;
    if let Some((words, volume)) = volume_in(&words) {
        named.volume = Some(volume);
        ends_at(words.start);
        volume_words = words;
    }

    // The number. A special's marker counts the specials, not the issues, so
    // a name that carries one gives no number.
    if marker.is_none() {
        named.number = counted.or_else(|| {
            let (at, number) = number_in(&words, &volume_words)?;
            ends_at(at);
            Some(number)
        });
    }

    if named.volume.is_some() || named.number.is_some() || named.special {
        let series = words[..end].join(" ");
        let series = series.trim_matches(|c: char| c.is_whitespace() || "-,:#".contains(c));
        if !series.is_empty() && !NOT_A_NAME.contains(&name_key(series).as_str()) {
            named.series = Some(series.to_owned());
        }
    }
    named
}

/// Finds the volume among a name's words: the words that give it, and the
/// volume. `v1`, `Vol.2000`, `Volume 3`.
fn volume_in(words: &[&str]) -> Option<(Range<usize>, i64)> {
    words.iter().enumerate().find_map(|(at, word)| {
        let lower = word.to_lowercase();
        let joined = BEFORE_A_VOLUME
            .iter()
            .filter_map(|before| lower.strip_prefix(before))
            .find_map(volume);
        if let Some(found) = joined {
            return Some((at..at + 1, found));
        }
        let found = volume(words.get(at + 1)?)?;
        BEFORE_A_VOLUME
            .contains(&lower.as_str())
            .then_some((at..at + 2, found))
    })
}

/// Finds the issue's number among a name's words: where the words about it
/// start, and the number. `volume_words` are the words that give the volume,
/// which are not looked at; empty when the name gives none.
fn number_in(words: &[&str], volume_words: &Range<usize>) -> Option<(usize, String)> {
    let at = |word: usize| {
        let text = words.get(word).filter(|_| !volume_words.contains(&word))?;
        Some((word, number(text)?))
    };

    // Said outright: `#57`, `Issue 12`, `Chapter 01`.
    let said = words.iter().enumerate().find_map(|(word, text)| {
        if let Some(found) = text.strip_prefix('#').and_then(number) {
            return Some((word, found));
        }
        let next = words.get(word + 1)?;
        let found = number(next.strip_prefix('#').unwrap_or(next))?;
        BEFORE_A_NUMBER
            .contains(&text.to_lowercase().as_str())
            .then_some((word, found))
    });
    // Right after the volume: `Teen Titans v1 001`.
    let volumes = || {
        (!volume_words.is_empty())
            .then(|| at(volume_words.end))
            .flatten()
    };
    // What comes before the volume is the series' name, numbers and all:
    // `Earth 2 v1`. Without a volume, any word may be the number.
    let rest = volume_words.end..words.len();
    // Before a dash: `Invincible 070.5 - Invincible Returns 1`.
    let dashed = || {
        rest.clone()
            .filter(|word| words.get(word + 1) == Some(&"-"))
            .find_map(at)
    };
    // Written with zeros in front, as issues are: `Blade Runner 2049 001`.
    let padded = || {
        rest.clone()
            .filter(|word| words[*word].starts_with('0') && words[*word].len() > 1)
            .find_map(at)
    };
    // The last number, unless it starts the name: `100 Bullets` has none.
    let last = || {
        rest.clone()
            .rev()
            .filter(|word| *word > 0 || words.len() == 1)
            .find_map(at)
    };
    said.or_else(volumes)
        .or_else(dashed)
        .or_else(padded)
        .or_else(last)
}

/// The marker some libraries put in the name of a special: `SP01`.
fn is_special_marker(word: &str) -> bool {
    let lower = word.to_lowercase();
    lower
        .strip_prefix("sp")
        .is_some_and(|rest| !rest.is_empty() && rest.chars().all(|c| c.is_ascii_digit()))
}

/// A volume's number: digits only, and no more than a tag may give.
fn volume(text: &str) -> Option<i64> {
    let digits = text.strip_prefix('.').unwrap_or(text);
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    digits
        .parse()
        .ok()
        .filter(|volume| (1..=MOST_VOLUMES).contains(volume))
}

/// An issue's number, as [`written`]: `12`, `070.5`, or a run of issues in
/// one file, `001-006`.
fn number(text: &str) -> Option<String> {
    let is_one = |part: &str| {
        let (whole, decimals) = part.split_once('.').unwrap_or((part, "0"));
        let digits = |text: &str| !text.is_empty() && text.chars().all(|c| c.is_ascii_digit());
        digits(whole) && digits(decimals)
    };
    let (first, last) = text
        .split_once('-')
        .map_or((text, None), |(first, last)| (first, Some(last)));
    (is_one(first) && last.is_none_or(is_one)).then(|| written(text))
}

/// A number as Binge writes it: as the file does, without zeros in front.
/// `001` is `1`, `000` is `0`, `070.5` is `70.5`.
pub(crate) fn written(number: &str) -> String {
    let unpadded = |part: &str| {
        let kept = part.trim_start_matches('0');
        if kept.is_empty() || kept.starts_with('.') {
            format!("0{kept}")
        } else {
            kept.to_owned()
        }
    };
    let starts_with_digit = |part: &str| part.starts_with(|c: char| c.is_ascii_digit());
    match number.split_once('-') {
        Some((first, last)) if starts_with_digit(first) && starts_with_digit(last) => {
            format!("{}-{}", unpadded(first), unpadded(last))
        }
        _ if starts_with_digit(number) => unpadded(number),
        _ => number.to_owned(),
    }
}

/// A name without what it holds in round and square brackets, where scans
/// carry their year and who made them, and with spaces for its underscores.
/// Also the issue's number when a bracket counts the issues: `(1 of 3)`.
fn without_brackets(name: &str) -> (String, Option<String>) {
    let mut outside = String::new();
    let mut inside = String::new();
    let mut counted = None;
    let mut depth = 0_u32;
    for letter in name.chars() {
        match if letter == '_' { ' ' } else { letter } {
            '(' | '[' => depth += 1,
            ')' | ']' if depth > 0 => {
                depth -= 1;
                if depth == 0 {
                    let mut words = inside.split_whitespace();
                    if let (Some(first), Some("of"), Some(_)) =
                        (words.next(), words.next(), words.next())
                    {
                        counted = counted.or_else(|| number(first));
                    }
                    inside.clear();
                    outside.push(' ');
                }
            }
            letter if depth > 0 => inside.push(letter),
            letter => outside.push(letter),
        }
    }
    (outside, counted)
}

/// A name without its brackets, when something is left of it.
pub(crate) fn unbracketed(name: &str) -> Option<String> {
    let (outside, _) = without_brackets(name);
    let words: Vec<&str> = outside.split_whitespace().collect();
    (!words.is_empty()).then(|| words.join(" "))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What a file's name alone says: series, volume, number, special.
    fn read(name: &str) -> (Option<String>, Option<i64>, Option<String>, bool) {
        let named = read_name(name);
        (named.series, named.volume, named.number, named.special)
    }

    fn says(
        series: &str,
        volume: Option<i64>,
        number: &str,
    ) -> (Option<String>, Option<i64>, Option<String>, bool) {
        (
            Some(series.to_owned()),
            volume,
            Some(number.to_owned()),
            false,
        )
    }

    #[test]
    fn a_name_gives_its_series_volume_and_number() {
        for (name, expected) in [
            (
                "Teen Titans v1 001 (1966-02) (digital) (Scanner-Group)",
                says("Teen Titans", Some(1), "1"),
            ),
            (
                "Batgirl Vol.2000 #57 (December, 2004)",
                says("Batgirl", Some(2000), "57"),
            ),
            (
                "Night Runner Volume 3 012",
                says("Night Runner", Some(3), "12"),
            ),
            ("Saga 001 (2012) (Digital)", says("Saga", None, "1")),
            ("Saga_010", says("Saga", None, "10")),
            (
                "Batman & Wildcat (1 of 3)",
                says("Batman & Wildcat", None, "1"),
            ),
            (
                "Batman Beyond 04 (of 6) (1999)",
                says("Batman Beyond", None, "4"),
            ),
            (
                "Batman Beyond 2.0 001 (2013)",
                says("Batman Beyond 2.0", None, "1"),
            ),
            (
                "Batman & Robin the Teen Wonder #0",
                says("Batman & Robin the Teen Wonder", None, "0"),
            ),
            ("Batman 2016 - Chapter 01", says("Batman 2016", None, "1")),
            ("Batman 2016 - Issue #12", says("Batman 2016", None, "12")),
            (
                "Invincible 070.5 - Invincible Returns 1 (2010)",
                says("Invincible", None, "70.5"),
            ),
            (
                "Batman & Catwoman - Trail of the Gun 01",
                says("Batman & Catwoman - Trail of the Gun", None, "1"),
            ),
            (
                "Batman & Grendel (1996) 01 - Devil's Bones",
                says("Batman & Grendel", None, "1"),
            ),
            (
                "Blade Runner 2049 001",
                says("Blade Runner 2049", None, "1"),
            ),
            ("100 Bullets 45", says("100 Bullets", None, "45")),
            ("Night Runner 001-006", says("Night Runner", None, "1-6")),
            ("Earth 2 v1 005", says("Earth 2", Some(1), "5")),
        ] {
            assert_eq!(read(name), expected, "{name}");
        }
    }

    #[test]
    fn a_name_that_gives_less_says_less() {
        // No number and no volume: nothing says where the series' name ends.
        assert_eq!(read("Watchmen"), (None, None, None, false));
        assert_eq!(read("100 Bullets"), (None, None, None, false));
        assert_eq!(read("The First Frieze (2019)"), (None, None, None, false));
        // A number alone, or after a word that is no name.
        assert_eq!(read("007"), (None, None, Some("7".to_owned()), false));
        assert_eq!(
            read("Chapter 12"),
            (None, None, Some("12".to_owned()), false)
        );
        assert_eq!(read("Vol 1"), (None, Some(1), None, false));
        assert_eq!(read("v2 003"), (None, Some(2), Some("3".to_owned()), false));
        // A volume and no number: a collected edition. A number before the
        // volume is part of the name.
        assert_eq!(
            read("Saga Vol. 2 (2013)"),
            (Some("Saga".to_owned()), Some(2), None, false)
        );
        assert_eq!(
            read("Earth 2 v1"),
            (Some("Earth 2".to_owned()), Some(1), None, false)
        );
    }

    #[test]
    fn a_special_is_marked_in_its_name() {
        assert_eq!(
            read("Odd Squad SP03 - The Lost Year 2"),
            (Some("Odd Squad".to_owned()), None, None, true)
        );
        assert_eq!(
            read("Odd Squad v2 SP01"),
            (Some("Odd Squad".to_owned()), Some(2), None, true)
        );
        // Letters after "sp" are a word, not the marker.
        assert!(!read("Spawn 012").3);
        assert!(!read("Wasp12 003").3);
    }

    #[test]
    fn a_number_is_written_without_what_pads_it() {
        for (number, expected) in [
            ("001", "1"),
            ("000", "0"),
            ("070.5", "70.5"),
            ("0.5", "0.5"),
            ("001-006", "1-6"),
            ("Annual 1", "Annual 1"),
            ("1/2", "1/2"),
        ] {
            assert_eq!(written(number), expected);
        }
    }

    fn tagged(fields: &[(&str, &str)]) -> ComicInfo {
        ComicInfo::parse(&crate::fixtures::comic_info(fields))
    }

    const ONE_SHOTS: Option<(GroupKind, Option<i64>)> = Some((GroupKind::OneShot, None));

    fn in_volume(number: i64) -> Option<(GroupKind, Option<i64>)> {
        Some((GroupKind::Volume, Some(number)))
    }

    #[test]
    fn the_tag_comes_first_then_the_name_then_the_folder() {
        let untagged = ComicInfo::default();

        // All three agree to differ: the tag wins, part by part.
        let placed = place(
            "Runner v2 007",
            "Scans/Night",
            &tagged(&[("Series", "Night Runner"), ("Number", "8")]),
        );
        assert_eq!(placed.series, "Night Runner");
        assert_eq!(
            (placed.group, placed.number.as_deref()),
            (in_volume(2), Some("8"))
        );

        // No tag: the name.
        let placed = place("Runner v2 007", "Scans/Night", &untagged);
        assert_eq!(placed.series, "Runner");
        assert_eq!(
            (placed.group, placed.number.as_deref()),
            (in_volume(2), Some("7"))
        );

        // A name that gives no series: the folder.
        let placed = place("007", "Scans/Night", &untagged);
        assert_eq!(placed.series, "Night");
        assert_eq!((placed.group, placed.number.as_deref()), (None, Some("7")));

        // A folder named after a volume gives the volume, and the one above
        // it the series.
        let placed = place("007", "Night Runner/Volume 2", &untagged);
        assert_eq!(placed.series, "Night Runner");
        assert_eq!(placed.group, in_volume(2));
        let placed = place("007", "Night Runner v3", &untagged);
        assert_eq!(
            (placed.series.as_str(), placed.group),
            ("Night Runner", in_volume(3))
        );
    }

    #[test]
    fn a_book_with_no_number_and_no_volume_is_a_one_shot_of_its_folder() {
        let untagged = ComicInfo::default();
        let placed = place("The Doll's House", "Sandman", &untagged);
        assert_eq!(placed.series, "Sandman");
        assert_eq!((placed.group, placed.number), (ONE_SHOTS, None));

        // In the library's own folder it is a series of its own.
        let placed = place("Watchmen (1987) (Digital)", "", &untagged);
        assert_eq!(placed.series, "Watchmen");
        assert_eq!(placed.group, ONE_SHOTS);
    }

    #[test]
    fn a_special_is_in_no_volume() {
        let placed = place("Odd Squad v2 SP01", "", &ComicInfo::default());
        assert_eq!(
            (placed.series.as_str(), placed.group),
            ("Odd Squad", ONE_SHOTS)
        );

        // Marked as a special and named after something else: its folder
        // says whose special it is. That it is an appearance in another
        // title is for the owner to say.
        let placed = place("Strange Tales 080 SP01", "Odd Squad", &ComicInfo::default());
        assert_eq!(
            (placed.series.as_str(), placed.group),
            ("Odd Squad", ONE_SHOTS)
        );
        // Nor does a word in the name say what kind of special it is.
        let placed = place("Odd Squad Annual 001 SP02", "", &ComicInfo::default());
        assert_eq!(placed.group, ONE_SHOTS);
    }

    #[test]
    fn the_format_tag_says_what_kind_of_special_a_book_is() {
        let of = |fields: &[(&str, &str)]| {
            let mut all = vec![("Series", "Odd Squad")];
            all.extend_from_slice(fields);
            place("Odd Squad 001", "", &tagged(&all))
        };

        // An annual is one of its volume's annuals, and keeps its number.
        let placed = of(&[("Volume", "2"), ("Number", "1"), ("Format", "Annual")]);
        assert_eq!(
            (placed.group, placed.number.as_deref()),
            (Some((GroupKind::Annual, Some(2))), Some("1"))
        );
        // With no volume, one of the series' annuals.
        let placed = of(&[("Format", "Annual")]);
        assert_eq!(placed.group, Some((GroupKind::Annual, None)));

        // A collected edition is in no volume, whatever its tag says.
        let placed = of(&[("Volume", "2"), ("Format", "TPB")]);
        assert_eq!(placed.group, Some((GroupKind::Collected, None)));
        let placed = of(&[("Format", "Director's Cut")]);
        assert_eq!(placed.group, ONE_SHOTS);

        // However the tag writes it.
        for (format, kind) in [
            ("One-Shot", GroupKind::OneShot),
            ("one shot", GroupKind::OneShot),
            ("Trade Paperback", GroupKind::Collected),
            (" TPB ", GroupKind::Collected),
            ("ANNUAL", GroupKind::Annual),
        ] {
            assert_eq!(kind_of_special(format), Some(kind), "{format}");
        }
        assert_eq!(kind_of_special("Digital"), None);
        // An issue of a run goes where its volume and number say.
        let placed = of(&[("Volume", "2"), ("Format", "Digital")]);
        assert_eq!(placed.group, in_volume(2));

        // A volume and no number is a book of that volume, not a special.
        let placed = place("Saga Vol. 2", "", &ComicInfo::default());
        assert_eq!(placed.group, in_volume(2));
    }

    #[test]
    fn a_date_sorts_by_time_and_what_gives_less_comes_later() {
        let date = |fields: &[(&str, &str)]| published(&tagged(fields));
        assert_eq!(date(&[("Month", "3")]), None);
        let full = date(&[("Year", "1964"), ("Month", "3"), ("Day", "5")]).unwrap();
        let month = date(&[("Year", "1964"), ("Month", "3")]).unwrap();
        let year = date(&[("Year", "1964")]).unwrap();
        let next = date(&[("Year", "1964"), ("Month", "4")]).unwrap();
        assert_eq!(full, 19_640_305);
        assert!(full < month && month < next && next < year);
    }

    #[test]
    fn a_tag_gives_only_what_a_place_may_hold() {
        // A day the month does not have, or one with no month, is dropped.
        let date = |fields: &[(&str, &str)]| published(&tagged(fields));
        let february = [("Year", "2015"), ("Month", "2"), ("Day", "29")];
        assert_eq!(date(&february), Some(20_150_299));
        assert_eq!(date(&[("Year", "2015"), ("Day", "7")]), Some(20_159_999));

        // A number or a series too long for a place is not the tag's to
        // give: the file's name is asked.
        let long = "9".repeat(40);
        let name = "n".repeat(300);
        let info = tagged(&[("Number", &long), ("Series", &name)]);
        let placed = place("Saga 003", "", &info);
        assert_eq!(placed.number.as_deref(), Some("3"));
        assert_eq!(placed.series, "Saga");
        // And a file's own name is cut to what a series' name may be.
        let placed = place(&name, "", &ComicInfo::default());
        assert_eq!(placed.series.chars().count(), layout::MAX_NAME);
    }

    #[test]
    fn names_are_compared_by_their_letters_and_digits() {
        assert_eq!(name_key("Spider-Man"), name_key("spider man"));
        assert_eq!(name_key("Élodie & Co."), "elodieco");
        assert_ne!(name_key("Batman"), name_key("Batman 2016"));
        assert_eq!(name_key("!?"), "!?");
    }
}
