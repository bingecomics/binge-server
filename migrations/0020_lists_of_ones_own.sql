-- A reading list belongs to the person who made it, and is shared with the
-- people they choose or with everyone on the server (ADR 0021, which
-- replaces 0014). The lists there are become the owner's, shared with
-- everyone, so nobody loses one.
--
-- The tables are made again: a list's name is now its own among one
-- person's lists, not among all of them, and that is part of the table.
-- What they held is put aside first, since dropping `reading_list` takes
-- the rows that point at it.
CREATE TABLE reading_list_before AS SELECT * FROM reading_list;
CREATE TABLE reading_list_book_before AS SELECT * FROM reading_list_book;
CREATE TABLE reading_list_cover_before AS SELECT * FROM reading_list_cover;
CREATE TABLE cbl_rule_before AS SELECT * FROM cbl_rule;
-- How far their ids have counted, so that the id of a list or a rule that
-- was deleted is not given out again: a list's id is in addresses.
CREATE TABLE sequence_before AS
SELECT name, seq FROM sqlite_sequence WHERE name IN ('reading_list', 'cbl_rule');

DROP TABLE reading_list_cover;
DROP TABLE reading_list_book;
DROP TABLE reading_list;
DROP TABLE cbl_rule;

CREATE TABLE reading_list (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Whose it is. A list goes with its person, as their progress does.
    user_id         INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    -- The name as names are compared (0006).
    name_key        TEXT NOT NULL,
    summary         TEXT,
    -- Whether everyone on the server sees it. The people it is shared with
    -- one by one are in `reading_list_share`.
    shared_with_all INTEGER NOT NULL DEFAULT 0 CHECK (shared_with_all IN (0, 1)),
    -- Seconds since 1970.
    created_at      INTEGER NOT NULL,
    -- No two lists of one person have the same name.
    UNIQUE (user_id, name_key)
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

-- A person a list is shared with. The one whose list it is has no row.
CREATE TABLE reading_list_share (
    list_id INTEGER NOT NULL REFERENCES reading_list (id) ON DELETE CASCADE,
    user_id INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    PRIMARY KEY (list_id, user_id)
) STRICT, WITHOUT ROWID;

-- For the lists shared with a person.
CREATE INDEX reading_list_share_person ON reading_list_share (user_id);

-- What a person told the server while fixing the import of a CBL file, kept
-- for their next import: which series a name in such files means, or which
-- book one of their entries is. Each person has their own.
--
-- `entry_key` is what the rule is for: the series' name as names are
-- compared, the volume as the file writes it, and for a rule about one
-- entry its number. A rule gives a series or a book, never both. The book is
-- kept by its identity.
CREATE TABLE cbl_rule (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    entry_key    TEXT NOT NULL,
    series_id    INTEGER REFERENCES series (id) ON DELETE CASCADE,
    content_hash TEXT,
    UNIQUE (user_id, entry_key),
    CHECK ((series_id IS NULL) <> (content_hash IS NULL))
) STRICT;

CREATE INDEX cbl_rule_series ON cbl_rule (series_id);

-- Only the owner could make a list or a rule until now, so wherever there
-- is one there is an owner to give it to.
INSERT INTO reading_list (id, user_id, name, name_key, summary, shared_with_all, created_at)
SELECT was.id, u.id, was.name, was.name_key, was.summary, 1, was.created_at
FROM reading_list_before was JOIN user u ON u.is_owner;

INSERT INTO reading_list_book (list_id, content_hash, position)
SELECT was.list_id, was.content_hash, was.position
FROM reading_list_book_before was JOIN reading_list rl ON rl.id = was.list_id;

INSERT INTO reading_list_cover (list_id, cover)
SELECT was.list_id, was.cover
FROM reading_list_cover_before was JOIN reading_list rl ON rl.id = was.list_id;

INSERT INTO cbl_rule (id, user_id, entry_key, series_id, content_hash)
SELECT was.id, u.id, was.entry_key, was.series_id, was.content_hash
FROM cbl_rule_before was JOIN user u ON u.is_owner;

DELETE FROM sqlite_sequence WHERE name IN (SELECT name FROM sequence_before);
INSERT INTO sqlite_sequence (name, seq) SELECT name, seq FROM sequence_before;

DROP TABLE sequence_before;
DROP TABLE reading_list_before;
DROP TABLE reading_list_book_before;
DROP TABLE reading_list_cover_before;
DROP TABLE cbl_rule_before;
