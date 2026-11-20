# 0008. One reader until accounts exist

## Status

Accepted on 2026-11-16. Built in iteration 2. Superseded by
[0009](0009-first-run-setup-makes-the-owner.md) when accounts arrived in
iteration 3: progress is now each reader's own. What this record says about
progress hanging on a book's identity still holds.

## Context

Reading progress belongs to a book's identity
([0004](0004-identify-books-by-page-content.md)) and, in the end, to a
person: the plan's data model keys it by user and book.

Progress is built in iteration 2. Accounts are built in iteration 3. For one
iteration something has to say whose progress it is.

## Decision

Until accounts exist, the server has one reader.

- The `progress` table is keyed by the book's identity alone. Everyone who
  opens the server reads and moves the same place in each book.
- Progress has no link to a row of `book`. A scan that removes a book, or
  removing a whole library, leaves its progress where it is, so it is there
  again when the same pages come back: after a move, a rename, a drive that
  was away, or a library added again.
- When accounts arrive, a new migration adds the reader to the key and gives
  the rows that exist to the owner created at first-run setup.
- Progress is one page number, the last page that was on screen, and whether
  that was the book's last page. Opening a book saves nothing; turning a page
  does.
- How the reader looks (fit, one or two pages, reading direction) is not
  progress. It is kept in the browser, for every book, until there is an
  account to keep it for and metadata that says which way a book reads.

## Consequences

- Two people who share a server before iteration 3 share one place in each
  book.
- The data directory now holds something a scan cannot rebuild. Deleting it
  loses reading progress.
- Progress of books that are in no library any more stays in the database.
  The rows are small. Clearing them needs its own rule, because "gone" and
  "on a drive that is not connected" look the same to the server.
- Two files with the same pages share progress, as 0004 already says.
- The API saves progress through a book (`PUT /books/{id}/progress`) and
  never names a reader, so it does not change shape when accounts arrive:
  the reader will come from who is signed in.

## Alternatives considered

- **Keep progress in the browser until accounts exist.** No table to migrate
  later, but a book would not resume in another browser, and the iOS reader
  in iteration 4 needs progress on the server anyway.
- **Create a placeholder user now.** A `user` table with nobody able to sign
  in reads like accounts that half exist. Adding the column later is one
  migration either way.
- **Key progress by the book's row.** Simple to join, but a moved file is a
  new row, so moving a file would lose its progress. That is what 0004
  exists to prevent.
