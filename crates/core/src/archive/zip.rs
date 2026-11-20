//! CBZ: a zip of page images.

use std::fs::File;
use std::io::{BufReader, Read};

use image::ImageError;
use zip::ZipArchive;
use zip::result::ZipError;

use super::{ArchiveError, MAX_PAGE_BYTES, PageEntry, image_dimensions};

/// How much of a page is read to find its width and height. Image headers
/// are almost always within this; when one is not, the rest is read too.
const HEADER_BYTES: u64 = 64 * 1024;

pub(super) struct Zip {
    zip: ZipArchive<BufReader<File>>,
}

impl Zip {
    pub(super) fn open(file: File) -> Result<Self, ArchiveError> {
        let zip = ZipArchive::new(BufReader::new(file))?;
        Ok(Self { zip })
    }

    /// Every file in the archive, from its index alone: nothing is read from
    /// where the files are.
    pub(super) fn files(&self) -> Result<Vec<PageEntry>, ArchiveError> {
        let mut files = Vec::new();
        for index in 0..self.zip.len() {
            let entry = self.zip.by_index_data(index)?;
            if entry.is_file() {
                if entry.encrypted() {
                    return Err(ArchiveError::Protected);
                }
                files.push(PageEntry {
                    name: entry.name()?.into_owned(),
                    byte_size: entry.size(),
                    crc32: entry.crc32(),
                });
            }
        }
        Ok(files)
    }

    pub(super) fn read(&mut self, entry: &str) -> Result<Vec<u8>, ArchiveError> {
        let file = self.zip.by_name(entry).map_err(|error| match error {
            ZipError::FileNotFound => ArchiveError::NoSuchEntry,
            other => other.into(),
        })?;
        if file.size() > MAX_PAGE_BYTES {
            return Err(ArchiveError::PageTooLarge);
        }
        let mut bytes = Vec::with_capacity(file.size() as usize);
        file.take(MAX_PAGE_BYTES).read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    /// Reads only as much of the page as its width and height need.
    pub(super) fn dimensions(&mut self, entry: &str) -> Option<(u32, u32)> {
        let mut file = self.zip.by_name(entry).ok()?;
        let mut bytes = Vec::new();
        (&mut file)
            .take(HEADER_BYTES)
            .read_to_end(&mut bytes)
            .ok()?;
        let cut_short = bytes.len() as u64 == HEADER_BYTES;
        match image_dimensions(&bytes) {
            Ok(size) => Some(size),
            // A kind of image this build cannot read: more of it will not help.
            Err(ImageError::Unsupported(_)) => None,
            // The header did not fit, for example behind a large colour profile.
            Err(_) if cut_short => {
                (&mut file)
                    .take(MAX_PAGE_BYTES)
                    .read_to_end(&mut bytes)
                    .ok()?;
                image_dimensions(&bytes).ok()
            }
            Err(_) => None,
        }
    }
}
