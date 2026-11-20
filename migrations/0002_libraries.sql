-- Ids are never given out twice (AUTOINCREMENT): clients keep them in
-- addresses, and a new library must not pass for one that was removed.

-- A folder of comics the server was pointed at.
CREATE TABLE library (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    name            TEXT NOT NULL,
    root_path       TEXT NOT NULL UNIQUE,
    created_at      INTEGER NOT NULL,
    last_scanned_at INTEGER
) STRICT;

-- One folder inside a library that holds comics. `folder` is relative to the
-- library root.
CREATE TABLE series (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    folder     TEXT NOT NULL,
    created_at INTEGER NOT NULL,
    UNIQUE (library_id, folder)
) STRICT;

-- One comic file. `path` is relative to the library root. A file directly in
-- the library root belongs to no series.
--
-- `content_hash` is the book's identity, computed from its page images, so it
-- survives the file being moved, renamed or retagged. `file_size` and
-- `modified_at` (milliseconds) only tell a rescan whether to look again.
-- `problem` says why a file could not be read as a comic.
CREATE TABLE book (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id   INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    series_id    INTEGER REFERENCES series (id) ON DELETE CASCADE,
    name         TEXT NOT NULL,
    path         TEXT NOT NULL,
    file_size    INTEGER NOT NULL,
    modified_at  INTEGER NOT NULL,
    page_count   INTEGER NOT NULL,
    content_hash TEXT,
    problem      TEXT,
    has_cover    INTEGER NOT NULL DEFAULT 0,
    analyzed_at  INTEGER,
    added_at     INTEGER NOT NULL,
    UNIQUE (library_id, path)
) STRICT;

CREATE INDEX book_series ON book (series_id);
CREATE INDEX book_content ON book (content_hash);

-- One page image of a book, in reading order from 0. Width and height are
-- filled in by the background pass of a scan.
CREATE TABLE page (
    book_id   INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    number    INTEGER NOT NULL,
    entry     TEXT NOT NULL,
    byte_size INTEGER NOT NULL,
    width     INTEGER,
    height    INTEGER,
    PRIMARY KEY (book_id, number)
) STRICT, WITHOUT ROWID;
