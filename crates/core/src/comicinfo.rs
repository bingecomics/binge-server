//! `ComicInfo.xml`: what a tagging tool wrote about a book, inside its
//! archive.

/// Longest text kept from one field. A summary runs to a few paragraphs.
const MAX_FIELD_CHARS: usize = 4000;

/// What a book's file says about itself. Most files say little or nothing,
/// so every part may be missing.
#[derive(Clone, Debug, Default, PartialEq, Eq, sqlx::FromRow)]
pub struct ComicInfo {
    pub title: Option<String>,
    /// The series the file says it belongs to. Import reads it once, to
    /// place a new book; after that it is only what the file claims.
    pub series: Option<String>,
    /// The issue's number as written: `12`, `0.5`, `Annual 1`.
    pub number: Option<String>,
    pub volume: Option<i64>,
    pub year: Option<i64>,
    pub month: Option<i64>,
    pub day: Option<i64>,
    /// One name, or several separated by commas.
    pub writer: Option<String>,
    /// One name, or several separated by commas.
    pub penciller: Option<String>,
    pub publisher: Option<String>,
    pub summary: Option<String>,
    /// A language code such as `en` or `es`.
    pub language: Option<String>,
    /// The kind of publication, such as `Annual` or `TPB`.
    pub format: Option<String>,
    /// The groups of series the book's series is part of, separated by
    /// commas. This and the four after it are kept apart from the rest,
    /// one row for each thing named: see [`crate::from_files`].
    #[sqlx(skip)]
    pub series_group: Option<String>,
    /// The story arcs the book is part of, separated by commas.
    #[sqlx(skip)]
    pub story_arc: Option<String>,
    /// The book's number in each of those arcs, in the same order.
    #[sqlx(skip)]
    pub story_arc_number: Option<String>,
    /// Another series the book is also an issue of, as in a crossover.
    #[sqlx(skip)]
    pub alternate_series: Option<String>,
    /// The book's number in that series.
    #[sqlx(skip)]
    pub alternate_number: Option<String>,
}

/// The highest volume a book may be in, whoever says so: a tag, a file's
/// name, or the owner.
pub(crate) const MOST_VOLUMES: i64 = u16::MAX as i64;

impl ComicInfo {
    /// Reads a `ComicInfo.xml`. One that cannot be understood says nothing.
    pub fn parse(xml: &[u8]) -> Self {
        let text = String::from_utf8_lossy(xml);
        let text = text.trim_start_matches('\u{feff}');
        let Ok(document) = roxmltree::Document::parse(text) else {
            return Self::default();
        };
        let root = document.root_element();
        let text = |name: &str| -> Option<String> {
            let value = root
                .children()
                .find(|node| node.is_element() && node.tag_name().name() == name)?
                .text()?
                .trim();
            (!value.is_empty()).then(|| value.chars().take(MAX_FIELD_CHARS).collect())
        };
        // Tagging tools write -1 for a number they do not know.
        let number = |name: &str, most: i64| -> Option<i64> {
            text(name)?
                .parse()
                .ok()
                .filter(|value| (1..=most).contains(value))
        };
        Self {
            title: text("Title"),
            series: text("Series"),
            number: text("Number"),
            volume: number("Volume", MOST_VOLUMES),
            year: number("Year", 9999),
            month: number("Month", 12),
            day: number("Day", 31),
            writer: text("Writer"),
            penciller: text("Penciller"),
            publisher: text("Publisher"),
            summary: text("Summary"),
            language: text("LanguageISO"),
            format: text("Format"),
            series_group: text("SeriesGroup"),
            story_arc: text("StoryArc"),
            story_arc_number: text("StoryArcNumber"),
            alternate_series: text("AlternateSeries"),
            alternate_number: text("AlternateNumber"),
        }
    }

    /// Whether the file said nothing.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    /// The writers the file names, each by itself.
    pub fn writers(&self) -> impl Iterator<Item = &str> {
        names(&self.writer)
    }

    /// The pencillers the file names, each by itself.
    pub fn pencillers(&self) -> impl Iterator<Item = &str> {
        names(&self.penciller)
    }

    /// The groups of series the file names, each by itself.
    pub fn series_groups(&self) -> impl Iterator<Item = &str> {
        names(&self.series_group)
    }

    /// The runs of books the file says the book is part of, each with the
    /// book's number in it when the file gives one: its story arcs, which
    /// go with their numbers by the order both are written in, and the
    /// other series it is an issue of.
    pub fn arcs(&self) -> Vec<(&str, Option<f64>)> {
        let numbers: Vec<&str> = self
            .story_arc_number
            .iter()
            .flat_map(|numbers| numbers.split(','))
            .collect();
        // An arc left blank between two commas still has its number.
        let arcs = self.story_arc.iter().flat_map(|arcs| arcs.split(','));
        let mut named: Vec<(&str, Option<f64>)> = arcs
            .enumerate()
            .map(|(at, arc)| (arc.trim(), numbers.get(at).and_then(|text| a_number(text))))
            .filter(|(arc, _)| !arc.is_empty())
            .collect();
        // The name of a series may hold a comma, so it is one name.
        if let Some(series) = self.alternate_series.as_deref() {
            let number = self.alternate_number.as_deref().and_then(a_number);
            named.push((series, number));
        }
        named
    }
}

/// A book's number in a run, as files write it: `3`, `2.5`.
fn a_number(text: &str) -> Option<f64> {
    text.trim()
        .parse()
        .ok()
        .filter(|number: &f64| number.is_finite() && *number >= 0.0)
}

/// The names in a field that holds one, or several separated by commas.
fn names(field: &Option<String>) -> impl Iterator<Item = &str> {
    field
        .iter()
        .flat_map(|names| names.split(','))
        .map(str::trim)
        .filter(|name| !name.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_fields_a_reader_shows_are_taken() {
        let info = ComicInfo::parse(
            "\u{feff}<?xml version=\"1.0\" encoding=\"utf-8\"?>
            <ComicInfo xmlns:xsd=\"http://www.w3.org/2001/XMLSchema\">
              <Title> The Long Night </Title>
              <Series>Night &amp; Day</Series>
              <Number>0.5</Number>
              <Volume>2</Volume>
              <Year>2019</Year><Month>3</Month><Day>-1</Day>
              <Writer>Ana Pérez, Ben Okri</Writer>
              <Penciller>Cy Lee</Penciller>
              <Publisher>Small Press</Publisher>
              <Summary>It gets dark.</Summary>
              <LanguageISO>en</LanguageISO>
              <Format>Annual</Format>
              <PageCount>24</PageCount>
              <Pages><Page Image=\"0\" /></Pages>
            </ComicInfo>"
                .as_bytes(),
        );
        assert_eq!(
            info,
            ComicInfo {
                title: Some("The Long Night".to_owned()),
                series: Some("Night & Day".to_owned()),
                number: Some("0.5".to_owned()),
                volume: Some(2),
                year: Some(2019),
                month: Some(3),
                day: None,
                writer: Some("Ana Pérez, Ben Okri".to_owned()),
                penciller: Some("Cy Lee".to_owned()),
                publisher: Some("Small Press".to_owned()),
                summary: Some("It gets dark.".to_owned()),
                language: Some("en".to_owned()),
                format: Some("Annual".to_owned()),
                ..ComicInfo::default()
            }
        );
        assert!(!info.is_empty());
    }

    #[test]
    fn arcs_go_with_their_numbers_by_the_order_they_are_written_in() {
        let info = ComicInfo::parse(
            b"<ComicInfo>
              <SeriesGroup>Night City, , Small Press Classics</SeriesGroup>
              <StoryArc>The Long Night, , Dawn,Dusk</StoryArc>
              <StoryArcNumber>3, 9, 1.5</StoryArcNumber>
              <AlternateSeries>Night, Day and After</AlternateSeries>
              <AlternateNumber>-1</AlternateNumber>
            </ComicInfo>",
        );
        assert_eq!(
            info.series_groups().collect::<Vec<_>>(),
            ["Night City", "Small Press Classics"]
        );
        assert_eq!(
            info.arcs(),
            [
                ("The Long Night", Some(3.0)),
                ("Dawn", Some(1.5)),
                ("Dusk", None),
                ("Night, Day and After", None),
            ]
        );
        assert!(ComicInfo::default().arcs().is_empty());
    }

    #[test]
    fn several_names_in_one_field_are_told_apart() {
        let info = ComicInfo {
            writer: Some(" Ana Pérez ,, Ben Okri".to_owned()),
            ..ComicInfo::default()
        };
        assert_eq!(
            info.writers().collect::<Vec<_>>(),
            ["Ana Pérez", "Ben Okri"]
        );
        assert_eq!(info.pencillers().count(), 0);
    }

    #[test]
    fn a_file_that_is_not_understood_says_nothing() {
        for broken in [
            &b"not xml at all"[..],
            b"<ComicInfo><Title>cut off",
            b"<ComicInfo></ComicInfo>",
            b"<ComicInfo><Title>   </Title><Year>soon</Year></ComicInfo>",
            b"",
        ] {
            assert!(ComicInfo::parse(broken).is_empty());
        }
    }

    #[test]
    fn a_very_long_field_is_cut() {
        let xml = format!(
            "<ComicInfo><Summary>{}</Summary></ComicInfo>",
            "é".repeat(9000)
        );
        let summary = ComicInfo::parse(xml.as_bytes()).summary.unwrap();
        assert_eq!(summary.chars().count(), MAX_FIELD_CHARS);
    }
}
