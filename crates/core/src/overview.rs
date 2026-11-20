//! A series, or a group of one, as a whole: what its books say about it
//! together, and how far one reader is through it.

use std::collections::HashMap;

use crate::accounts::UserId;
use crate::catalog::{Book, Catalog, CatalogError, Group, Series};
use crate::people::{Credited, Credits};

/// A series seen whole, for one reader.
#[derive(Clone, Debug)]
pub struct SeriesOverview {
    pub series: Series,
    /// Its groups, in the order they are read. Empty when it has none.
    pub groups: Vec<Group>,
    /// What its books say together, and how far the reader is through them.
    pub about: BooksOverview,
}

/// A group of a series seen whole, for one reader.
#[derive(Clone, Debug)]
pub struct GroupOverview {
    pub group: Group,
    /// The series it is a part of.
    pub series: Series,
    /// What its books say together, and how far the reader is through
    /// them. What to open next follows what they did last in this group,
    /// whatever they read elsewhere.
    pub about: BooksOverview,
}

/// Some books seen whole, for one reader: the books of a series, or of a
/// group of one.
#[derive(Clone, Debug)]
pub struct BooksOverview {
    /// How far the reader is through them.
    pub read: ReadSoFar,
    /// The publisher most of the books name.
    pub publisher: Option<String>,
    /// The earliest year the books name.
    pub first_year: Option<i64>,
    /// The latest year the books name.
    pub last_year: Option<i64>,
    /// Who the books name as writers, the one most books name first.
    pub writers: Vec<Credited>,
    /// Who the books name as pencillers, the one most books name first.
    pub pencillers: Vec<Credited>,
    pub summary: Option<Summary>,
}

/// How far one reader is through some books: a series, or a group of one.
#[derive(Clone, Debug)]
pub struct ReadSoFar {
    /// How many of the books can be read. The rest are files the server
    /// could not understand.
    pub readable_count: i64,
    /// Pages in all the books.
    pub page_count: i64,
    /// Books the reader read to the end, out of those that can be read.
    pub books_read: i64,
    /// Pages the reader has been through, over all the books.
    pub pages_read: i64,
    /// How long the pages not yet read will take the reader, in seconds, at
    /// the pace they read at: see [`Catalog::pace`]. Missing until they
    /// have read enough to tell.
    pub seconds_left: Option<i64>,
    /// The book to open to go on with them: see [`next`]. Missing when none
    /// of the books can be read.
    pub next: Option<Book>,
}

impl ReadSoFar {
    /// `books` are in reading order, with the reader's progress, and `pace`
    /// is the seconds the reader takes over a page.
    fn of(books: &[Book], pace: Option<f64>) -> Self {
        let page_count: i64 = books.iter().map(|book| book.page_count).sum();
        let pages_read: i64 = books.iter().map(pages_read).sum();
        Self {
            readable_count: books.iter().filter(|book| is_readable(book)).count() as i64,
            page_count,
            books_read: books.iter().filter(|book| is_read(book)).count() as i64,
            pages_read,
            seconds_left: pace.map(|pace| ((page_count - pages_read) as f64 * pace).round() as i64),
            next: next(books).cloned(),
        }
    }
}

/// What some books are about. No file describes a series or a group, so
/// these are the words of the first of the books, in reading order, that
/// describes itself. A reading list has words of its own.
#[derive(Clone, Debug)]
pub struct Summary {
    pub text: String,
    /// The book the words are from. Missing when they are the words of what
    /// holds the books.
    pub book: Option<Book>,
}

impl BooksOverview {
    /// `books` are in reading order, with the reader's progress, `credits`
    /// are who they name and `pace` is the seconds the reader takes over a
    /// page.
    fn of(books: &[Book], credits: Credits, pace: Option<f64>) -> Self {
        let years = || books.iter().filter_map(Book::year);
        Self {
            read: ReadSoFar::of(books, pace),
            // One name, commas and all: "Small Press, Inc." is one publisher.
            publisher: most_named(
                books
                    .iter()
                    .filter_map(|book| book.info.publisher.as_deref()),
            ),
            first_year: years().min(),
            last_year: years().max(),
            writers: credits.writers,
            pencillers: credits.pencillers,
            summary: books.iter().find_map(|book| {
                Some(Summary {
                    text: book.info.summary.clone()?,
                    book: Some(book.clone()),
                })
            }),
        }
    }
}

/// A file that could not be read has no identity and no pages.
fn is_readable(book: &Book) -> bool {
    book.content_hash.is_some()
}

fn is_read(book: &Book) -> bool {
    book.progress
        .as_ref()
        .is_some_and(|progress| progress.completed)
}

fn pages_read(book: &Book) -> i64 {
    match &book.progress {
        Some(progress) if progress.completed => book.page_count,
        Some(progress) => progress.page + 1,
        None => 0,
    }
}

/// The book to open to go on with some books, from the books in reading order.
///
/// It follows what the reader did last: the first unfinished book from the
/// one they read last onwards, which is that book itself when they left it
/// unfinished. At the end of the books it goes back to the first unfinished
/// one anywhere. Someone who read nothing, or everything, starts at the first
/// book. Files that could not be read are passed over.
fn next(books: &[Book]) -> Option<&Book> {
    let readable: Vec<&Book> = books.iter().filter(|book| is_readable(book)).collect();
    // Of two books read in the same second, the later one in the series.
    let last = readable
        .iter()
        .enumerate()
        .filter_map(|(place, book)| Some((book.progress.as_ref()?.updated_at, place)))
        .max();
    let unfinished = last.and_then(|(_, last)| {
        let (before, from) = readable.split_at(last);
        from.iter()
            .chain(before)
            .copied()
            .find(|book| !is_read(book))
    });
    unfinished.or_else(|| readable.first().copied())
}

/// What a book is read as a part of: reading goes on from it to the book
/// that follows there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Followed {
    /// A reading list.
    List,
    /// The book's group of its series: the reading stops at the group's end.
    Group,
    /// The book's series, from one group to the next.
    Series,
}

/// What comes after a book where it is read.
#[derive(Clone, Debug, Default)]
pub struct After {
    /// What the book is read as a part of. Missing when it is a part of
    /// nothing: it is in no series and was opened from no list, or it is
    /// not in the list it was said to be read from.
    pub followed: Option<Followed>,
    /// The book that follows there. Missing at the end, and whenever
    /// nothing is followed.
    pub next: Option<Book>,
}

/// The book that comes after `book` among `books`, which are in the order
/// they are read: the first one after it that can be read. `None` when the
/// book is not among them, and `Some(None)` when nothing follows it.
///
/// A book the server holds twice is the same book, whichever of its files
/// was opened: a list holds it once.
fn after<'a>(books: &'a [Book], book: &Book) -> Option<Option<&'a Book>> {
    let at = books.iter().position(|one| {
        one.id == book.id || (is_readable(one) && one.content_hash == book.content_hash)
    })?;
    Some(books[at + 1..].iter().find(|one| is_readable(one)))
}

/// The book to offer someone who finished a book of these and opened none
/// since, from the books in reading order: the one [`next`] gives, when they
/// have read from these books and have not started that one. So it is
/// missing when the book they read last is still unfinished, when the one
/// that follows is, and when nothing is left.
///
/// A book they are in the middle of is theirs to continue, not to be
/// offered: it is on the list of [`Catalog::continue_reading`].
pub(crate) fn up_next(books: &[Book]) -> Option<&Book> {
    let started = books.iter().any(|book| book.progress.is_some());
    next(books).filter(|book| started && book.progress.is_none())
}

/// What was named most often, and the first in alphabetical order among
/// those named as often.
fn most_named<'a>(mentions: impl Iterator<Item = &'a str>) -> Option<String> {
    let mut times: HashMap<&str, usize> = HashMap::new();
    for name in mentions {
        *times.entry(name).or_default() += 1;
    }
    times
        .into_iter()
        .max_by(|(a, a_times), (b, b_times)| a_times.cmp(b_times).then(b.cmp(a)))
        .map(|(name, _)| name.to_owned())
}

impl Catalog {
    /// Some books seen whole: `books` are in reading order, with the
    /// progress of `user`, who reads them.
    pub(crate) async fn books_overview(
        &self,
        user: UserId,
        books: &[Book],
    ) -> Result<BooksOverview, CatalogError> {
        let (credits, pace) = tokio::try_join!(self.credited(books), self.pace(user))?;
        Ok(BooksOverview::of(books, credits, pace))
    }

    /// What comes after a book for `user`, where they read it: in the
    /// reading list `list` when they opened it from one, in its group of
    /// its series when they opened it from the group (`in_group`), and
    /// otherwise in its series. So every client goes on to the same book.
    pub async fn book_after(
        &self,
        user: UserId,
        book_id: i64,
        list: Option<i64>,
        in_group: bool,
    ) -> Result<After, CatalogError> {
        let book = self.book(user, book_id).await?;
        let (followed, books) = match (list, book.group_id, book.series_id) {
            (Some(list), ..) => (Followed::List, self.books_in_list(user, list).await?),
            (None, Some(group), _) if in_group => {
                (Followed::Group, self.books_in_group(user, group).await?)
            }
            (None, _, Some(series)) => (Followed::Series, self.books_in(user, series).await?),
            _ => return Ok(After::default()),
        };
        Ok(
            after(&books, &book).map_or_else(After::default, |next| After {
                followed: Some(followed),
                next: next.cloned(),
            }),
        )
    }

    /// A series seen whole, as `user` reads it.
    pub async fn series_overview(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<SeriesOverview, CatalogError> {
        let series = self.series(user, id).await?;
        let groups = self.groups_in(user, id).await?;
        let books = self.books_in(user, id).await?;
        Ok(SeriesOverview {
            series,
            groups,
            about: self.books_overview(user, &books).await?,
        })
    }

    /// A group of a series seen whole, as `user` reads it.
    pub async fn group_overview(
        &self,
        user: UserId,
        id: i64,
    ) -> Result<GroupOverview, CatalogError> {
        let group = self.group(user, id).await?;
        let series = self.series(user, group.series_id).await?;
        let books = self.books_in_group(user, id).await?;
        Ok(GroupOverview {
            group,
            series,
            about: self.books_overview(user, &books).await?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::comicinfo::ComicInfo;
    use crate::reading::Progress;

    /// A readable book of ten pages that nobody has opened.
    fn book(id: i64) -> Book {
        Book {
            id,
            library_id: 1,
            series_id: Some(1),
            series_name: Some("Night Runner".into()),
            group_id: None,
            group_kind: None,
            number: None,
            published: None,
            name: format!("Night Runner {id:03}"),
            page_count: 10,
            file_size: 1000,
            added_at: 0,
            content_hash: Some(format!("{id:064}")),
            problem: None,
            cover: None,
            cover_chosen: false,
            progress: None,
            info: ComicInfo::default(),
        }
    }

    /// The book left on `page`, `at` seconds since 1970.
    fn left_on(mut book: Book, page: i64, at: i64) -> Book {
        book.progress = Some(Progress {
            page,
            completed: page == book.page_count - 1,
            updated_at: at,
        });
        book
    }

    fn finished(book: Book, at: i64) -> Book {
        let last = book.page_count - 1;
        left_on(book, last, at)
    }

    fn unreadable(mut book: Book) -> Book {
        book.content_hash = None;
        book.page_count = 0;
        book.problem = Some("not a comic".to_owned());
        book
    }

    fn next_id(books: &[Book]) -> Option<i64> {
        next(books).map(|book| book.id)
    }

    #[test]
    fn the_book_after_one_is_the_next_that_can_be_read() {
        let books = [book(1), unreadable(book(2)), book(3)];
        let after_id = |book: &Book| after(&books, book).map(|next| next.map(|next| next.id));
        assert_eq!(after_id(&books[0]), Some(Some(3)));
        // Nothing follows the last book, and a book that is not among them
        // is followed by nothing at all.
        assert_eq!(after_id(&books[2]), Some(None));
        assert_eq!(after_id(&book(9)), None);

        // Another file with the same pages is the same book.
        let mut copy = book(8);
        copy.content_hash.clone_from(&books[0].content_hash);
        assert_eq!(after_id(&copy), Some(Some(3)));
        // Two files that could not be read have nothing in common.
        assert_eq!(after_id(&unreadable(book(7))), None);
    }

    #[test]
    fn a_series_nobody_started_begins_at_its_first_readable_book() {
        assert_eq!(next_id(&[]), None);
        assert_eq!(next_id(&[unreadable(book(1))]), None);
        assert_eq!(next_id(&[unreadable(book(1)), book(2), book(3)]), Some(2));
    }

    #[test]
    fn an_unfinished_book_is_the_one_to_go_on_with() {
        let books = [finished(book(1), 100), left_on(book(2), 4, 200), book(3)];
        assert_eq!(next_id(&books), Some(2));
    }

    #[test]
    fn after_a_finished_book_comes_the_next_unfinished_one() {
        // The book after the one read last, not the first one never opened.
        let books = [book(1), finished(book(2), 100), book(3), book(4)];
        assert_eq!(next_id(&books), Some(3));

        // What was read last decides, not what was left unfinished before.
        let books = [left_on(book(1), 2, 100), finished(book(2), 200), book(3)];
        assert_eq!(next_id(&books), Some(3));

        // Books already finished, and files that cannot be read, are passed.
        let books = [
            finished(book(1), 300),
            finished(book(2), 100),
            unreadable(book(3)),
            book(4),
        ];
        assert_eq!(next_id(&books), Some(4));
    }

    #[test]
    fn at_the_end_of_the_series_it_goes_back_to_what_was_skipped() {
        let books = [finished(book(1), 100), book(2), finished(book(3), 200)];
        assert_eq!(next_id(&books), Some(2));

        // With everything read, the series starts again.
        let books = [finished(book(1), 100), finished(book(2), 200)];
        assert_eq!(next_id(&books), Some(1));
    }

    #[test]
    fn the_book_after_a_finished_one_is_offered_until_it_is_opened() {
        let offered = |books: &[Book]| up_next(books).map(|book| book.id);

        assert_eq!(offered(&[book(1), book(2)]), None);
        let books = [finished(book(1), 100), unreadable(book(2)), book(3)];
        assert_eq!(offered(&books), Some(3));

        // A book left open is to be continued, not offered: the one read
        // last, or the one that follows it.
        let books = [finished(book(1), 100), left_on(book(2), 4, 200), book(3)];
        assert_eq!(offered(&books), None);
        let books = [finished(book(1), 200), left_on(book(2), 4, 100), book(3)];
        assert_eq!(offered(&books), None);

        // What was skipped is offered at the end, and nothing once all is read.
        let books = [finished(book(1), 100), book(2), finished(book(3), 200)];
        assert_eq!(offered(&books), Some(2));
        let books = [finished(book(1), 100), finished(book(2), 200)];
        assert_eq!(offered(&books), None);
    }

    #[test]
    fn reading_is_counted_in_books_and_pages() {
        let books = [
            finished(book(1), 100),
            left_on(book(2), 3, 200),
            book(3),
            unreadable(book(4)),
        ];
        let read = ReadSoFar::of(&books, None);
        assert_eq!(read.readable_count, 3);
        assert_eq!(read.page_count, 30);
        assert_eq!(read.books_read, 1);
        assert_eq!(read.pages_read, 14);
        assert_eq!(read.seconds_left, None);
        assert_eq!(read.next.map(|book| book.id), Some(2));

        // Sixteen pages are left, at a page every twenty and a half seconds.
        assert_eq!(ReadSoFar::of(&books, Some(20.5)).seconds_left, Some(328));
    }

    #[test]
    fn what_the_books_say_is_put_together() {
        let said = |id: i64, info: ComicInfo| Book { info, ..book(id) };
        let credit = |name: &str| Some(name.to_owned());
        let books = [
            book(1),
            said(
                2,
                ComicInfo {
                    year: Some(1991),
                    writer: credit("Ben Okri, Ana Pérez"),
                    penciller: credit("Cy Lee"),
                    publisher: credit("Small Press, Inc."),
                    summary: credit("It gets dark."),
                    ..ComicInfo::default()
                },
            ),
            said(
                3,
                ComicInfo {
                    year: Some(1988),
                    writer: credit(" Ana Pérez ,, "),
                    publisher: credit("Small Press, Inc."),
                    summary: credit("It gets darker."),
                    ..ComicInfo::default()
                },
            ),
            said(
                4,
                ComicInfo {
                    writer: credit("Zed Abel"),
                    publisher: credit("Another House"),
                    ..ComicInfo::default()
                },
            ),
        ];
        let overview = BooksOverview::of(&books, Credits::default(), None);
        assert_eq!(overview.first_year, Some(1988));
        assert_eq!(overview.last_year, Some(1991));
        assert_eq!(overview.publisher.as_deref(), Some("Small Press, Inc."));
        let summary = overview.summary.unwrap();
        // The words of the first book, in reading order, that has any.
        assert_eq!(
            (summary.text.as_str(), summary.book.map(|book| book.id)),
            ("It gets dark.", Some(2))
        );

        let silent = BooksOverview::of(&[book(1)], Credits::default(), None);
        assert_eq!(silent.first_year, None);
        assert_eq!(silent.publisher, None);
        assert!(silent.summary.is_none());
    }
}
