//! Reading comic files: CBZ (zip), CBR (rar) and PDF.
//!
//! What a file is, is decided by how it starts and not by its name: a `.cbr`
//! that is really a zip is common.

mod pdf;
mod rar;
mod zip;

use std::fs::File;
use std::io::{self, Cursor, Read, Seek};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::path::Path;

use image::{ImageError, ImageReader};

use crate::comicinfo::ComicInfo;
use crate::natural::natural_cmp;

/// File extensions of the images that count as pages, each with its media
/// type.
const PAGE_TYPES: [(&str, &str); 6] = [
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("png", "image/png"),
    ("webp", "image/webp"),
    ("gif", "image/gif"),
    ("avif", "image/avif"),
];

/// Largest page read into memory. Real pages are a few megabytes.
const MAX_PAGE_BYTES: u64 = 256 * 1024 * 1024;

/// The file a tagging tool puts in an archive to say what the book is.
const COMIC_INFO: &str = "ComicInfo.xml";

/// Largest `ComicInfo.xml` read. Real ones are a few kilobytes.
const MAX_INFO_BYTES: u64 = 1024 * 1024;

#[derive(Debug, thiserror::Error)]
pub enum ArchiveError {
    #[error("the file could not be read: {0}")]
    Io(#[from] io::Error),
    #[error("the file is not a readable zip archive: {0}")]
    Zip(#[from] ::zip::result::ZipError),
    #[error("the file is not a readable rar archive: {0}")]
    Rar(rars::Error),
    #[error("the file is not a readable pdf")]
    Pdf,
    #[error("the file is protected with a password")]
    Protected,
    #[error("the file has no page images in it")]
    NoPages,
    #[error("a page is too large to read")]
    PageTooLarge,
    #[error("a page of the pdf could not be drawn")]
    NotDrawn,
    #[error("the file no longer holds what its index named")]
    NoSuchEntry,
    #[error("reading the file crashed")]
    Crashed,
}

impl ArchiveError {
    /// What to tell the library's owner about a file with this problem. The
    /// cause goes to the log.
    pub fn for_people(&self) -> &'static str {
        match self {
            Self::Io(_) | Self::NoSuchEntry | Self::Crashed => "This file could not be read.",
            Self::Zip(_) => "This file is not a readable CBZ archive.",
            Self::Rar(_) => "This file is not a readable CBR archive.",
            Self::Pdf => "This file is not a readable PDF.",
            Self::Protected => "This file is protected with a password.",
            Self::NoPages => "This file has no page images in it.",
            Self::PageTooLarge => "A page in this file is too large to read.",
            Self::NotDrawn => "A page in this file could not be drawn.",
        }
    }
}

/// Whether a file is a comic this version can read, judged by its name.
pub fn is_comic(path: &Path) -> bool {
    has_extension(path, &["cbz", "cbr", "pdf"])
}

fn has_extension(path: &Path, wanted: &[&str]) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| wanted.iter().any(|w| extension.eq_ignore_ascii_case(w)))
}

/// One file of an archive as the archive's own index describes it. For a
/// PDF, one page of the document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PageEntry {
    /// Name of the entry inside the archive.
    pub name: String,
    /// Size of the image once unpacked.
    pub byte_size: u64,
    crc32: u32,
}

/// What a book contains, read from the archive's index without unpacking it.
#[derive(Clone, Debug)]
pub struct Contents {
    /// Pages in reading order.
    pub pages: Vec<PageEntry>,
    /// The book's identity. See [`identity`].
    pub content_hash: String,
}

/// What the first pass of a scan takes from a file: its pages and identity,
/// and what the file says about itself.
pub fn read_book(path: &Path) -> Result<(Contents, ComicInfo), ArchiveError> {
    let mut archive = Archive::open(path)?;
    let files = archive.files()?;
    let info = files
        .iter()
        .find(|file| file.name.eq_ignore_ascii_case(COMIC_INFO) && file.byte_size <= MAX_INFO_BYTES)
        .and_then(|file| archive.read(&file.name).ok())
        .map(|xml| ComicInfo::parse(&xml))
        .unwrap_or_default();
    Ok((contents_of(files)?, info))
}

/// Runs a piece of reading and answers `crashed` if it panics. The readers
/// take whatever is in a library, and a file that trips one of them up is
/// that file's problem: it must not end a scan or a request.
fn guarded<T>(read: impl FnOnce() -> T, crashed: impl FnOnce() -> T) -> T {
    catch_unwind(AssertUnwindSafe(read)).unwrap_or_else(|_| crashed())
}

/// Picks the pages out of an archive's files, in reading order, and computes
/// the book's identity from them.
fn contents_of(files: Vec<PageEntry>) -> Result<Contents, ArchiveError> {
    let mut pages: Vec<PageEntry> = files
        .into_iter()
        .filter(|file| is_page(&file.name))
        .collect();
    if pages.is_empty() {
        return Err(ArchiveError::NoPages);
    }
    pages.sort_by(|a, b| natural_cmp(&a.name, &b.name));
    let content_hash = identity(&pages);
    Ok(Contents {
        pages,
        content_hash,
    })
}

/// Whether an archive entry is a page: an image that is not hidden and not
/// one of the resource files macOS adds.
fn is_page(name: &str) -> bool {
    let hidden = name
        .split('/')
        .any(|part| part.starts_with('.') || part == "__MACOSX");
    !hidden && media_type(name).is_some()
}

/// The media type of an image that counts as a page, judged by its name.
/// `None` for any other file.
pub fn media_type(name: &str) -> Option<&'static str> {
    let extension = Path::new(name).extension()?.to_str()?;
    PAGE_TYPES
        .iter()
        .find(|(known, _)| extension.eq_ignore_ascii_case(known))
        .map(|(_, media_type)| *media_type)
}

/// A book's identity: a hash over the checksum and size of each page image,
/// in reading order. A zip and a rar store both for every file; for a PDF
/// they are computed from each page.
///
/// It depends only on the page images. Moving or renaming the file, adding
/// a metadata file inside it, or repacking the same images as another kind
/// of archive leaves it unchanged, so reading progress and everything
/// derived from the book can hang on it.
fn identity(pages: &[PageEntry]) -> String {
    let mut hasher = blake3::Hasher::new_derive_key("binge-server book identity v1");
    for page in pages {
        hasher.update(&page.crc32.to_le_bytes());
        hasher.update(&page.byte_size.to_le_bytes());
    }
    hasher.finalize().to_hex().to_string()
}

/// The kinds of file a comic comes as.
enum Format {
    Zip,
    Rar,
    Pdf,
}

impl Format {
    /// Told by how the file starts. A file that starts like none of them is
    /// taken for what its name says, and fails to open as that.
    fn of(path: &Path, file: &mut File) -> io::Result<Self> {
        let mut start = [0; 8];
        let read = file.read(&mut start)?;
        file.rewind()?;
        let start = &start[..read];
        Ok(if start.starts_with(b"PK") {
            Self::Zip
        } else if start.starts_with(b"Rar!") {
            Self::Rar
        } else if start.starts_with(b"%PDF") || has_extension(path, &["pdf"]) {
            Self::Pdf
        } else if has_extension(path, &["cbr"]) {
            Self::Rar
        } else {
            Self::Zip
        })
    }
}

/// An open comic file, for reading pages out of it.
pub struct Archive {
    inner: Inner,
}

enum Inner {
    Zip(zip::Zip),
    Rar(rar::Rar),
    Pdf(pdf::Pdf),
}

impl Archive {
    pub fn open(path: &Path) -> Result<Self, ArchiveError> {
        let mut file = File::open(path)?;
        let format = Format::of(path, &mut file)?;
        guarded(
            || {
                let inner = match format {
                    Format::Zip => Inner::Zip(zip::Zip::open(file)?),
                    Format::Rar => Inner::Rar(rar::Rar::open(file)?),
                    Format::Pdf => Inner::Pdf(pdf::Pdf::open(file)?),
                };
                Ok(Self { inner })
            },
            || Err(ArchiveError::Crashed),
        )
    }

    /// Lists the pages and computes the book's identity. For a zip or a rar
    /// that takes the archive's index alone; a PDF has been read whole by
    /// now.
    pub fn contents(&self) -> Result<Contents, ArchiveError> {
        contents_of(self.files()?)
    }

    /// Every file in the archive; for a PDF, its pages.
    fn files(&self) -> Result<Vec<PageEntry>, ArchiveError> {
        guarded(
            || match &self.inner {
                Inner::Zip(zip) => zip.files(),
                Inner::Rar(rar) => rar.files(),
                Inner::Pdf(pdf) => Ok(pdf.files()),
            },
            || Err(ArchiveError::Crashed),
        )
    }

    /// Reads one page image whole.
    pub fn read(&mut self, entry: &str) -> Result<Vec<u8>, ArchiveError> {
        guarded(
            || match &mut self.inner {
                Inner::Zip(zip) => zip.read(entry),
                Inner::Rar(rar) => rar.read(entry),
                Inner::Pdf(pdf) => pdf.read(entry),
            },
            || Err(ArchiveError::Crashed),
        )
    }

    /// Width and height of a page, reading only as much of it as needed.
    /// `None` when the image cannot be understood.
    pub fn dimensions(&mut self, entry: &str) -> Option<(u32, u32)> {
        guarded(
            || match &mut self.inner {
                Inner::Zip(zip) => zip.dimensions(entry),
                Inner::Rar(rar) => rar.dimensions(entry),
                Inner::Pdf(pdf) => pdf.dimensions(entry),
            },
            || None,
        )
    }
}

fn image_dimensions(bytes: &[u8]) -> Result<(u32, u32), ImageError> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()?
        .into_dimensions()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    fn contents_at(path: &Path) -> Result<Contents, ArchiveError> {
        Ok(read_book(path)?.0)
    }

    #[test]
    fn pages_come_in_reading_order_and_other_files_are_ignored() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        fixtures::write_cbz(
            &path,
            &[
                ("p10.png", fixtures::page(4, 6, 10)),
                ("ComicInfo.xml", b"<ComicInfo/>".to_vec()),
                ("p2.png", fixtures::page(4, 6, 2)),
                ("__MACOSX/._p2.png", vec![0; 8]),
                (".hidden.png", fixtures::page(4, 6, 0)),
                ("P1.PNG", fixtures::page(4, 6, 1)),
            ],
        );

        let contents = contents_at(&path).unwrap();
        let names: Vec<_> = contents
            .pages
            .iter()
            .map(|page| page.name.as_str())
            .collect();
        assert_eq!(names, ["P1.PNG", "p2.png", "p10.png"]);
        assert_eq!(contents.content_hash.len(), 64);
    }

    #[test]
    fn identity_follows_the_pages_and_nothing_else() {
        let dir = tempfile::tempdir().unwrap();
        let pages = vec![
            ("01.png", fixtures::page(4, 6, 1)),
            ("02.png", fixtures::page(4, 6, 2)),
        ];
        let original = dir.path().join("original.cbz");
        fixtures::write_cbz(&original, &pages);

        // Same pages under another file name, with metadata added.
        let mut tagged_pages = pages.clone();
        tagged_pages.push(("ComicInfo.xml", b"<ComicInfo/>".to_vec()));
        let tagged = dir.path().join("renamed and tagged.cbz");
        fixtures::write_cbz(&tagged, &tagged_pages);

        // One page differs.
        let edited = dir.path().join("edited.cbz");
        fixtures::write_cbz(
            &edited,
            &[
                ("01.png", fixtures::page(4, 6, 1)),
                ("02.png", fixtures::page(4, 6, 99)),
            ],
        );

        let identity = |path| contents_at(path).unwrap().content_hash;
        assert_eq!(identity(&original), identity(&tagged));
        assert_ne!(identity(&original), identity(&edited));
    }

    #[test]
    fn files_that_are_not_comics_are_refused() {
        let dir = tempfile::tempdir().unwrap();

        let empty = dir.path().join("empty.cbz");
        fixtures::write_cbz(&empty, &[("notes.txt", b"hello".to_vec())]);
        assert!(matches!(contents_at(&empty), Err(ArchiveError::NoPages)));

        let broken = dir.path().join("broken.cbz");
        std::fs::write(&broken, b"this is not a zip file").unwrap();
        assert!(matches!(contents_at(&broken), Err(ArchiveError::Zip(_))));
    }

    #[test]
    fn dimensions_and_bytes_can_be_read_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("book.cbz");
        let image = fixtures::page(30, 45, 7);
        fixtures::write_cbz(
            &path,
            &[
                ("01.png", image.clone()),
                ("02.png", b"not an image".to_vec()),
            ],
        );

        let mut archive = Archive::open(&path).unwrap();
        assert_eq!(archive.dimensions("01.png"), Some((30, 45)));
        assert_eq!(archive.dimensions("02.png"), None);
        assert_eq!(archive.dimensions("missing.png"), None);
        assert_eq!(archive.read("01.png").unwrap(), image);
    }

    #[test]
    fn media_types_follow_the_page_names() {
        assert_eq!(media_type("a/01.JPG"), Some("image/jpeg"));
        assert_eq!(media_type("01.jpeg"), Some("image/jpeg"));
        assert_eq!(media_type("01.png"), Some("image/png"));
        assert_eq!(media_type("01.webp"), Some("image/webp"));
        assert_eq!(media_type("01.gif"), Some("image/gif"));
        assert_eq!(media_type("01.avif"), Some("image/avif"));
    }

    #[test]
    fn comics_are_told_by_their_extension() {
        assert!(is_comic(Path::new("a/Batman 001.cbz")));
        assert!(is_comic(Path::new("a/Batman 001.CBZ")));
        assert!(is_comic(Path::new("a/Batman 001.cbr")));
        assert!(is_comic(Path::new("a/Batman 001.Pdf")));
        assert!(!is_comic(Path::new("a/Batman 001.zip")));
        assert!(!is_comic(Path::new("a/cbz")));
    }

    #[test]
    fn a_cbr_is_read_like_a_cbz_of_the_same_pages() {
        let dir = tempfile::tempdir().unwrap();
        let first = fixtures::page(30, 45, 1);
        let entries = vec![
            ("pages\\p10.png", fixtures::page(4, 6, 10)),
            ("ComicInfo.xml", fixtures::comic_info(&[("Title", "Ten")])),
            ("pages\\p2.png", fixtures::page(4, 6, 2)),
            ("pages\\P1.PNG", first.clone()),
            ("notes.txt", b"hello".to_vec()),
        ];
        let cbr = dir.path().join("book.cbr");
        fixtures::write_cbr(&cbr, &entries);

        let (contents, info) = read_book(&cbr).unwrap();
        let names: Vec<_> = contents
            .pages
            .iter()
            .map(|page| page.name.as_str())
            .collect();
        assert_eq!(names, ["pages/P1.PNG", "pages/p2.png", "pages/p10.png"]);
        assert_eq!(info.title.as_deref(), Some("Ten"));

        let mut archive = Archive::open(&cbr).unwrap();
        assert_eq!(archive.read("pages/P1.PNG").unwrap(), first);
        assert_eq!(archive.dimensions("pages/P1.PNG"), Some((30, 45)));
        assert_eq!(archive.dimensions("missing.png"), None);

        // The same images in a zip are the same book.
        let cbz = dir.path().join("book.cbz");
        let flat: Vec<(&str, Vec<u8>)> = vec![
            ("1.png", entries[3].1.clone()),
            ("2.png", entries[2].1.clone()),
            ("10.png", entries[0].1.clone()),
        ];
        fixtures::write_cbz(&cbz, &flat);
        assert_eq!(
            contents_at(&cbz).unwrap().content_hash,
            contents.content_hash
        );
    }

    #[test]
    fn a_file_is_what_it_starts_like_not_what_it_is_called() {
        let dir = tempfile::tempdir().unwrap();
        let pages = vec![("01.png", fixtures::page(4, 6, 1))];

        let zip_named_cbr = dir.path().join("really a zip.cbr");
        fixtures::write_cbz(&zip_named_cbr, &pages);
        let rar_named_cbz = dir.path().join("really a rar.cbz");
        fixtures::write_cbr(&rar_named_cbz, &pages);

        let identity = contents_at(&zip_named_cbr).unwrap().content_hash;
        assert_eq!(contents_at(&rar_named_cbz).unwrap().content_hash, identity);

        let nothing = dir.path().join("nothing.cbr");
        std::fs::write(&nothing, b"this is not a rar file").unwrap();
        assert!(matches!(contents_at(&nothing), Err(ArchiveError::Rar(_))));
    }

    #[test]
    fn a_reader_that_crashes_is_an_error_about_the_file() {
        let crashed: Result<(), ArchiveError> =
            guarded(|| panic!("a reader gave up"), || Err(ArchiveError::Crashed));
        assert_eq!(
            crashed.unwrap_err().for_people(),
            "This file could not be read."
        );
        assert_eq!(guarded(|| 1, || 0), 1);
    }

    #[test]
    fn a_cbz_behind_a_password_says_so() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("locked.cbz");
        fixtures::write_cbz(&path, &[("01.png", fixtures::page(4, 6, 1))]);
        // Mark the one file as encrypted, in its own header and in the
        // archive's index: the first of the flags that follow each signature.
        let mut bytes = std::fs::read(&path).unwrap();
        for (signature, flags_at) in [(b"PK\x03\x04", 6), (b"PK\x01\x02", 8)] {
            let at = bytes
                .windows(4)
                .position(|window| window == signature)
                .unwrap();
            bytes[at + flags_at] |= 1;
        }
        std::fs::write(&path, bytes).unwrap();

        assert!(matches!(contents_at(&path), Err(ArchiveError::Protected)));
    }

    #[test]
    fn a_cbr_behind_a_password_says_so() {
        let dir = tempfile::tempdir().unwrap();
        // With the names of its files in the clear and, where this writer
        // can, with those hidden too.
        for (version, hidden_names) in [
            (rars::ArchiveVersion::Rar29, false),
            (rars::ArchiveVersion::Rar50, false),
            (rars::ArchiveVersion::Rar50, true),
        ] {
            let mut rar = rars::Builder::new(version)
                .password(Some(b"made up".to_vec()))
                .header_encryption(hidden_names);
            rar.add_bytes(b"01.png".to_vec(), fixtures::page(4, 6, 1), None, None)
                .unwrap();
            let path = dir.path().join("locked.cbr");
            std::fs::write(&path, rar.to_bytes().unwrap()).unwrap();

            let refused = contents_at(&path).unwrap_err();
            assert!(
                matches!(refused, ArchiveError::Protected),
                "{version:?}, names hidden: {hidden_names}: {refused}"
            );
            assert_eq!(
                refused.for_people(),
                "This file is protected with a password."
            );
        }
    }

    #[test]
    fn a_scanned_pdf_gives_its_pictures_untouched() {
        let dir = tempfile::tempdir().unwrap();
        let (one, two) = (
            fixtures::jpeg_page(30, 45, 1),
            fixtures::jpeg_page(40, 60, 2),
        );
        let pdf = dir.path().join("book.pdf");
        fixtures::write_pdf(
            &pdf,
            &[
                fixtures::PdfPage::Scan(one.clone()),
                fixtures::PdfPage::Scan(two.clone()),
            ],
        );

        let (contents, info) = read_book(&pdf).unwrap();
        let names: Vec<_> = contents
            .pages
            .iter()
            .map(|page| page.name.as_str())
            .collect();
        assert_eq!(names, ["00001.jpg", "00002.jpg"]);
        assert_eq!(contents.pages[1].byte_size, two.len() as u64);
        assert!(info.is_empty());

        let mut archive = Archive::open(&pdf).unwrap();
        assert_eq!(archive.read("00001.jpg").unwrap(), one);
        assert_eq!(archive.read("00002.jpg").unwrap(), two);
        assert_eq!(archive.dimensions("00002.jpg"), Some((40, 60)));
        assert!(matches!(
            archive.read("00003.jpg"),
            Err(ArchiveError::NoSuchEntry)
        ));

        // The same pictures in a zip are the same book.
        let cbz = dir.path().join("book.cbz");
        fixtures::write_cbz(&cbz, &[("1.jpg", one), ("2.jpg", two)]);
        assert_eq!(
            contents_at(&cbz).unwrap().content_hash,
            contents.content_hash
        );
    }

    #[test]
    fn a_pdf_page_that_is_not_one_picture_is_drawn() {
        let dir = tempfile::tempdir().unwrap();
        let pdf = dir.path().join("drawn.pdf");
        fixtures::write_pdf(&pdf, &[fixtures::PdfPage::Drawing(200, 400)]);

        let mut archive = Archive::open(&pdf).unwrap();
        let contents = archive.contents().unwrap();
        assert_eq!(contents.pages.len(), 1);
        assert_eq!(archive.dimensions("00001.jpg"), Some((1100, 2200)));

        let drawn = image::load_from_memory(&archive.read("00001.jpg").unwrap())
            .unwrap()
            .to_rgb8();
        assert_eq!(drawn.dimensions(), (1100, 2200));
        // Blue where the rectangle is, at the bottom left; white elsewhere.
        let [red, _, blue] = drawn.get_pixel(100, 2000).0;
        assert!(red < 100 && blue > 150, "{red} {blue}");
        assert!(
            drawn
                .get_pixel(1000, 100)
                .0
                .iter()
                .all(|&value| value > 240)
        );

        // Another drawing is another book.
        let other = dir.path().join("other.pdf");
        fixtures::write_pdf(&other, &[fixtures::PdfPage::Drawing(200, 300)]);
        assert_ne!(
            contents_at(&other).unwrap().content_hash,
            contents.content_hash
        );
    }

    #[test]
    fn a_pdf_that_cannot_be_understood_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let broken = dir.path().join("broken.pdf");
        std::fs::write(&broken, b"%PDF-1.4\nnothing follows").unwrap();
        assert!(matches!(contents_at(&broken), Err(ArchiveError::Pdf)));

        let empty = dir.path().join("empty.pdf");
        fixtures::write_pdf(&empty, &[]);
        assert!(contents_at(&empty).is_err());
    }
}
