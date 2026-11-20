# 0005. Library folders are read-only

## Status

Accepted on 2026-11-15. In effect since scanning was built in iteration 1.

## Context

A comic library is years of someone's collecting. Other tools read and write
the same folders: taggers, download managers, another server. A program that
renames or rewrites files there on its own can do damage that is hard to see
and hard to undo.

At the same time, automatic organization is one of the features Binge wants
to offer.

## Decision

- The server never writes, renames or deletes anything inside a library
  folder.
- Folders are a hint, not the truth. The folder layout gives the first guess
  at series and order. The real organization (series, order, metadata) lives
  in the database and can be corrected there without touching a file.
- Everything the server derives lives in its own data directory: the
  database, thumbnails, and later page maps and translation tracks.
- Reorganizing files on disk is a later, separate feature. It will be opt-in
  per library, show what it would do before doing it, record every change,
  and be undoable.

## Consequences

- Pointing the server at a library is safe to try. Removing the server
  leaves the library exactly as it was.
- A library can be mounted read-only, which is common on a NAS.
- Corrections made in Binge are not visible to other tools, because they are
  not written into the files.
- Metadata edits do not go into `ComicInfo.xml`. Writing metadata back would
  be part of the opt-in feature.
- The database is the only copy of the user's corrections, so it is worth
  backing up.

## Alternatives considered

- **Write metadata into the archives**, as tagging tools do. It keeps
  metadata with the file, but it changes files the user did not ask to
  change and conflicts with other tools doing the same.
- **Make the folder layout the truth**, so reorganizing means moving files.
  Simple to explain, but every correction becomes a file operation.
