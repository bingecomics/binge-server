//! Cover thumbnails, kept in the data directory.
//!
//! The cover the server makes from a book's first page is kept under the
//! book's identity. Because the name is the identity, a moved or duplicated
//! book reuses its cover, and a cover never has to be invalidated: a changed
//! book has a new identity and so a new file. Such a cover is made once and
//! never deleted: a book whose file or library goes and comes back finds it
//! again.
//!
//! A cover someone chose is kept apart, under the hash of the cover itself:
//! it cannot be made again from the library, and no book names it.

use std::fs;
use std::io::{self, Cursor};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use image::codecs::jpeg::JpegEncoder;

/// Covers are scaled down to fit this, enough for a sharp grid on dense
/// screens.
const MAX_WIDTH: u32 = 480;
const MAX_HEIGHT: u32 = 720;
const JPEG_QUALITY: u8 = 82;

/// A page as a small picture fits this: enough for a strip of pages to
/// jump by, and for a card that shows one kept page.
const PAGE_WIDTH: u32 = 320;
const PAGE_HEIGHT: u32 = 480;
const PAGE_QUALITY: u8 = 78;

#[derive(Debug, thiserror::Error)]
pub enum ThumbnailError {
    #[error("the cover could not be written: {0}")]
    Io(#[from] io::Error),
    #[error("the cover image could not be read: {0}")]
    Image(#[from] image::ImageError),
    #[error("not a book identity")]
    BadIdentity,
}

#[derive(Clone, Debug)]
pub struct Thumbnails {
    dir: Arc<Path>,
}

impl Thumbnails {
    /// The covers made from the first page of each book.
    pub fn new(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("thumbnails").into(),
        }
    }

    /// The covers people chose.
    pub fn chosen(data_dir: &Path) -> Self {
        Self {
            dir: data_dir.join("covers").into(),
        }
    }

    /// Where the cover of this name is kept, whether or not it exists. The
    /// name is a book's identity, or for a chosen cover its own hash.
    ///
    /// `None` unless the name is a well-formed hash, so text from a request
    /// can never name another file.
    pub fn path(&self, content_hash: &str) -> Option<PathBuf> {
        let well_formed = content_hash.len() == 64
            && content_hash
                .bytes()
                .all(|byte| matches!(byte, b'0'..=b'9' | b'a'..=b'f'));
        well_formed.then(|| {
            self.dir
                .join(&content_hash[..2])
                .join(format!("{content_hash}.jpg"))
        })
    }

    pub fn exists(&self, content_hash: &str) -> bool {
        self.path(content_hash).is_some_and(|path| path.is_file())
    }

    /// Makes the cover of a book from a page image.
    pub fn make(&self, content_hash: &str, image: &[u8]) -> Result<(), ThumbnailError> {
        let path = self.path(content_hash).ok_or(ThumbnailError::BadIdentity)?;
        write(&path, &cover_of(image)?)
    }

    /// Makes a cover from a picture and keeps it under its own hash, which
    /// is returned: the name to ask for it by.
    pub fn keep(&self, image: &[u8]) -> Result<String, ThumbnailError> {
        let cover = cover_of(image)?;
        let name = blake3::hash(&cover).to_hex().to_string();
        let path = self.path(&name).ok_or(ThumbnailError::BadIdentity)?;
        // The same picture chosen again is the same file.
        if !path.is_file() {
            write(&path, &cover)?;
        }
        Ok(name)
    }
}

/// A picture as a cover: scaled down to fit, as a JPEG file.
fn cover_of(image: &[u8]) -> Result<Vec<u8>, ThumbnailError> {
    scaled(image, MAX_WIDTH, MAX_HEIGHT, JPEG_QUALITY)
}

/// A page as a small picture, as a JPEG file. It is made from the page each
/// time and kept nowhere: whoever asks for it keeps it, since a page never
/// changes under its book's identity and its number.
pub(crate) fn small_page(image: &[u8]) -> Result<Vec<u8>, ThumbnailError> {
    scaled(image, PAGE_WIDTH, PAGE_HEIGHT, PAGE_QUALITY)
}

/// A picture scaled down to fit a width and a height, as a JPEG file. One
/// that fits already keeps its size.
fn scaled(image: &[u8], width: u32, height: u32, quality: u8) -> Result<Vec<u8>, ThumbnailError> {
    let image = image::load_from_memory(image)?;
    let small = if image.width() > width || image.height() > height {
        image.thumbnail(width, height)
    } else {
        image
    };
    let mut file = Cursor::new(Vec::new());
    JpegEncoder::new_with_quality(&mut file, quality).encode_image(&small.into_rgb8())?;
    Ok(file.into_inner())
}

/// Writes a cover beside its final place and renames it, so nothing ever
/// serves half a file.
fn write(path: &Path, cover: &[u8]) -> Result<(), ThumbnailError> {
    if let Some(folder) = path.parent() {
        fs::create_dir_all(folder)?;
    }
    // The counter keeps two writers of one cover apart.
    static WRITES: AtomicU64 = AtomicU64::new(0);
    let partial = path.with_extension(format!("part{}", WRITES.fetch_add(1, Ordering::Relaxed)));
    let written = fs::write(&partial, cover).and_then(|()| fs::rename(&partial, path));
    if written.is_err() {
        let _ = fs::remove_file(&partial);
    }
    Ok(written?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fixtures;

    const IDENTITY: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    #[test]
    fn large_pages_are_scaled_down_and_small_ones_kept() {
        let dir = tempfile::tempdir().unwrap();
        let thumbnails = Thumbnails::new(dir.path());

        thumbnails
            .make(IDENTITY, &fixtures::page(1000, 1500, 9))
            .unwrap();
        let cover = image::open(thumbnails.path(IDENTITY).unwrap()).unwrap();
        assert_eq!((cover.width(), cover.height()), (480, 720));

        thumbnails
            .make(IDENTITY, &fixtures::page(40, 60, 9))
            .unwrap();
        let cover = image::open(thumbnails.path(IDENTITY).unwrap()).unwrap();
        assert_eq!((cover.width(), cover.height()), (40, 60));
        assert!(thumbnails.exists(IDENTITY));
    }

    #[test]
    fn a_chosen_cover_is_kept_under_its_own_hash() {
        let dir = tempfile::tempdir().unwrap();
        let chosen = Thumbnails::chosen(dir.path());

        let name = chosen.keep(&fixtures::page(1000, 1500, 9)).unwrap();
        let path = chosen.path(&name).unwrap();
        assert!(path.starts_with(dir.path().join("covers")));
        let cover = image::open(&path).unwrap();
        assert_eq!((cover.width(), cover.height()), (480, 720));
        assert_eq!(
            name,
            blake3::hash(&fs::read(&path).unwrap()).to_hex().as_str()
        );

        // The same picture again is the same cover, another one is not.
        assert_eq!(chosen.keep(&fixtures::page(1000, 1500, 9)).unwrap(), name);
        assert_ne!(chosen.keep(&fixtures::page(1000, 1500, 3)).unwrap(), name);
        assert!(chosen.keep(b"not an image").is_err());
    }

    #[test]
    fn a_page_is_made_small_and_a_wide_one_keeps_its_shape() {
        let small = image::load_from_memory(&small_page(&fixtures::page(1000, 1500, 9)).unwrap());
        let small = small.unwrap();
        assert_eq!((small.width(), small.height()), (320, 480));

        // Two pages drawn as one.
        let wide = image::load_from_memory(&small_page(&fixtures::page(2000, 1500, 9)).unwrap());
        let wide = wide.unwrap();
        assert_eq!((wide.width(), wide.height()), (320, 240));
        assert!(small_page(b"not an image").is_err());
    }

    #[test]
    fn only_identities_name_a_file() {
        let thumbnails = Thumbnails::new(Path::new("/data"));
        assert!(thumbnails.path(IDENTITY).is_some());
        for bad in [
            "",
            "abc",
            "../../etc/passwd",
            &IDENTITY.to_uppercase(),
            &format!("{IDENTITY}0"),
        ] {
            assert!(thumbnails.path(bad).is_none(), "{bad}");
        }
    }

    #[test]
    fn an_unreadable_image_leaves_nothing_behind() {
        let dir = tempfile::tempdir().unwrap();
        let thumbnails = Thumbnails::new(dir.path());
        assert!(thumbnails.make(IDENTITY, b"not an image").is_err());
        assert!(!thumbnails.exists(IDENTITY));
    }
}
