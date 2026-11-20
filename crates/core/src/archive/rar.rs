//! CBR: a rar of page images.

use std::fs::File;

use rars::{ArchiveMemberDetail, ArchiveReadOptions, ArchiveReader, ErrorKind};

use super::{ArchiveError, MAX_PAGE_BYTES, PageEntry, image_dimensions, is_page};

pub(super) struct Rar {
    archive: rars::Archive,
    files: Vec<Member>,
}

/// A file in the archive, as its headers describe it.
struct Member {
    /// With `/` between folders, whichever way the archive wrote them.
    name: String,
    /// Place among everything in the archive, folders included.
    index: usize,
    byte_size: u64,
    /// Missing in the oldest archives, which keep a shorter checksum.
    crc32: Option<u32>,
}

impl From<rars::Error> for ArchiveError {
    fn from(error: rars::Error) -> Self {
        match error.kind() {
            ErrorKind::PasswordRequired | ErrorKind::BadPassword => Self::Protected,
            _ => Self::Rar(error),
        }
    }
}

impl Rar {
    /// Reads the archive's headers. Nothing is unpacked.
    pub(super) fn open(file: File) -> Result<Self, ArchiveError> {
        let archive = ArchiveReader::read_reader(file)?;
        let mut files = Vec::new();
        for (index, member) in archive.members().enumerate() {
            let meta = &member.meta;
            if meta.is_directory || meta.is_redirection {
                continue;
            }
            if meta.is_encrypted {
                return Err(ArchiveError::Protected);
            }
            let crc32 = match &member.detail {
                ArchiveMemberDetail::Rar15To40 { crc32, .. } => Some(*crc32),
                ArchiveMemberDetail::Rar50Plus { crc32, .. } => *crc32,
                _ => None,
            };
            files.push(Member {
                name: String::from_utf8_lossy(&meta.name).replace('\\', "/"),
                index,
                byte_size: meta.unpacked_size,
                crc32,
            });
        }
        Ok(Self { archive, files })
    }

    /// Every file in the archive. A page whose checksum the archive does not
    /// store is unpacked to compute it.
    pub(super) fn files(&self) -> Result<Vec<PageEntry>, ArchiveError> {
        self.files
            .iter()
            .map(|member| {
                let crc32 = match member.crc32 {
                    Some(stored) => stored,
                    None if is_page(&member.name) => crc32fast::hash(&self.unpack(member)?),
                    None => 0,
                };
                Ok(PageEntry {
                    name: member.name.clone(),
                    byte_size: member.byte_size,
                    crc32,
                })
            })
            .collect()
    }

    pub(super) fn read(&self, entry: &str) -> Result<Vec<u8>, ArchiveError> {
        let member = self
            .files
            .iter()
            .rfind(|member| member.name == entry)
            .ok_or(ArchiveError::NoSuchEntry)?;
        self.unpack(member)
    }

    /// Unpacks one file, and checks it against the checksum the archive
    /// keeps for it.
    fn unpack(&self, member: &Member) -> Result<Vec<u8>, ArchiveError> {
        if member.byte_size > MAX_PAGE_BYTES {
            return Err(ArchiveError::PageTooLarge);
        }
        let options = ArchiveReadOptions::new().with_max_member_output_bytes(MAX_PAGE_BYTES);
        self.archive
            .read_member_at_with_options(member.index, options)?
            .ok_or(ArchiveError::NoSuchEntry)
    }

    /// A rar cannot be read in part, so the page is unpacked whole.
    pub(super) fn dimensions(&self, entry: &str) -> Option<(u32, u32)> {
        image_dimensions(&self.read(entry).ok()?).ok()
    }
}
