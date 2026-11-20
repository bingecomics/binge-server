-- Reading lists: books in an order someone chose, across series. A list
-- belongs to the server, not to a person (ADR 0014): the owner makes it and
-- everyone reads it.
CREATE TABLE reading_list (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    name       TEXT NOT NULL,
    -- The name as names are compared (0006): no two lists have the same.
    name_key   TEXT NOT NULL UNIQUE,
    summary    TEXT,
    -- Seconds since 1970.
    created_at INTEGER NOT NULL
) STRICT;

-- A book of a list, by its identity and without a link to `book`, as
-- progress is and for the same reasons (0003): the list keeps the book when
-- its file is moved, renamed, or gone for a while. A list holds a book once.
-- `position` orders the books of one list and says nothing else: numbers
-- may be missing between two books.
CREATE TABLE reading_list_book (
    list_id      INTEGER NOT NULL REFERENCES reading_list (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    position     INTEGER NOT NULL,
    PRIMARY KEY (list_id, content_hash)
) STRICT, WITHOUT ROWID;

CREATE INDEX reading_list_book_order ON reading_list_book (list_id, position);

-- For the lists a book is in.
CREATE INDEX reading_list_book_identity ON reading_list_book (content_hash);

-- The cover chosen for a list, as for a series (0011).
CREATE TABLE reading_list_cover (
    list_id INTEGER PRIMARY KEY REFERENCES reading_list (id) ON DELETE CASCADE,
    cover   TEXT NOT NULL
) STRICT;

CREATE INDEX reading_list_cover_file ON reading_list_cover (cover);

-- What the owner told the server while fixing the import of a CBL file, kept
-- for the next import: which series a name in such files means, or which
-- book one of their entries is.
--
-- `entry_key` is what the rule is for: the series' name as names are
-- compared, the volume as the file writes it, and for a rule about one
-- entry its number. A rule gives a series or a book, never both. The book is
-- kept by its identity.
CREATE TABLE cbl_rule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    entry_key    TEXT NOT NULL UNIQUE,
    series_id    INTEGER REFERENCES series (id) ON DELETE CASCADE,
    content_hash TEXT,
    CHECK ((series_id IS NULL) <> (content_hash IS NULL))
) STRICT;
