# 0004. Identify books by page content

## Status

Accepted on 2026-11-15. Built in iteration 1. Amended on 2026-11-16: the
second, whole-file hash was dropped (see the decision and the alternatives).
Amended on 2026-11-17: the rule for CBR and PDF, which iteration 4 added.

## Context

Reading progress, and later page maps and translation tracks, belong to a
book. Something has to say which book.

Servers that use the file path lose that link when a file is moved or
renamed, and people reorganize comic folders often. Binge also plans to
reorganize them on the user's behalf.

Hashing the whole file is not stable either. Tagging tools add or rewrite a
metadata file (`ComicInfo.xml`) inside the archive, which changes the hash
although no page changed.

## Decision

A book's identity is computed from its page images only.

- A comic archive already stores a checksum (CRC32) and a size for every
  file in it. The identity is a BLAKE3 hash over those values for the image
  files, in page order.
- Progress and every piece of derived data attach to that identity, never to
  the path.
- The hash is keyed to this purpose and versioned (`binge-server book
  identity v1`), so a later rule cannot be confused with this one.
- To notice that a file changed and needs to be looked at again, the server
  keeps the file's size and modification time. It does not hash the file.
- The rule is the same for every format; only where the checksum and size
  come from differs. A zip and a rar store them. The oldest rar archives
  store a shorter checksum, so theirs is computed by unpacking the page. A
  PDF stores neither, so both are computed: over the JPEG when a page is
  one scanned picture, and otherwise over the page's drawing instructions
  and the pictures and forms they name.

## Consequences

- Moving, renaming, or adding metadata to a file loses nothing.
- Computing the identity does not need to decompress the images, so it is
  fast even for a large library.
- Two files with the same pages are the same book and share progress. That
  is correct for duplicates.
- Two scans or encodings of the same issue are different books. A
  translation made for one does not apply to the other.
- Replacing, adding or removing a page gives a new identity. Carrying
  progress and tracks over in that case needs its own rule later.
- The same pictures are the same book in a CBZ, a CBR and a scanned PDF,
  because all three come down to the checksum and size of the same image
  files. Converting a book between them keeps its progress.
- Finding a PDF's identity means reading the whole file, where a zip or a
  rar needs only its index.
- A PDF page that is drawn is identified by what it is made from, not by
  the picture the server draws, so a newer drawing library does not change
  which book it is.

## Alternatives considered

- **File path.** Breaks on every move or rename.
- **Hash of the whole file.** Breaks when metadata inside the archive
  changes, and needs reading every byte.
- **Hash of the whole file as well, only to detect changes.** The first
  version of this record had it. Dropped when scanning was built: it reads
  every byte of the library, slowly over a network share, to learn what size
  and modification time already say. A change that keeps both is missed
  until the file is touched; that is rare, and the price was every scan.
- **A random ID stored in the database per path.** Simple, but moving a file
  cannot be told apart from deleting one and adding another.
