-- What was read from the server a person is leaving, kept as it was read
-- so that it can be shown first and applied after, as their books become
-- known here. Nothing in these tables is shown as reading, as a list or as
-- a collection: they are what the other server said.
--
-- Ids named `their_…` are the other server's own. Nothing here is the key
-- that server was read with: it is used for the reading and not kept.

-- One reading of another server, by one person with their key there. A
-- person has one at most: reading again takes the place of the last.
CREATE TABLE bringing (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id    INTEGER NOT NULL UNIQUE REFERENCES user (id) ON DELETE CASCADE,
    -- Which kind of server it was: 'kavita'.
    source     TEXT NOT NULL,
    -- Where it was asked, as it was given, so that reading it again is
    -- known to be reading the same server. The key is not here.
    address    TEXT NOT NULL,
    version    TEXT NOT NULL,
    -- The name the person goes by there.
    their_name TEXT NOT NULL,
    -- Whether the key was of someone who manages that server, and so its
    -- people were read too.
    manages    INTEGER NOT NULL,
    -- Seconds since 1970.
    taken_at   INTEGER NOT NULL
) STRICT;

-- A library there. `library_id` is the one here that holds the same
-- folder: the one it was taken for, or with `said` the one the owner
-- named, or said there is none. What was said holds when the same server
-- is read again.
CREATE TABLE brought_library (
    bringing_id INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    library_id  INTEGER REFERENCES library (id) ON DELETE SET NULL,
    said        INTEGER NOT NULL DEFAULT 0 CHECK (said IN (0, 1)),
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

-- A folder of a library there, as that server sees it.
CREATE TABLE brought_folder (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_library INTEGER NOT NULL,
    path          TEXT NOT NULL,
    PRIMARY KEY (bringing_id, their_library, path)
) STRICT, WITHOUT ROWID;

-- A series there.
CREATE TABLE brought_series (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id      INTEGER NOT NULL,
    their_library INTEGER NOT NULL,
    name          TEXT NOT NULL,
    -- Whether the person wants to read it.
    wanted        INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

-- A file there. What that server reads as one thing, `their_chapter`, is
-- one file nearly always; when it is several, `first_page` says where this
-- file's pages start among them.
CREATE TABLE brought_book (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id      INTEGER NOT NULL,
    their_chapter INTEGER NOT NULL,
    their_series  INTEGER NOT NULL,
    their_library INTEGER NOT NULL,
    -- Where the file is under its library's folder, with `/` between the
    -- folders, and the whole path when it is under none of them.
    path          TEXT NOT NULL,
    file_size     INTEGER NOT NULL,
    page_count    INTEGER NOT NULL,
    first_page    INTEGER NOT NULL DEFAULT 0,
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

CREATE INDEX brought_book_chapter ON brought_book (bringing_id, their_chapter);

-- How far the person is in a chapter there: the pages they read of the
-- pages it has, and when they last read it, in seconds since 1970.
CREATE TABLE brought_reading (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_chapter INTEGER NOT NULL,
    pages_read    INTEGER NOT NULL,
    page_count    INTEGER NOT NULL,
    read_at       INTEGER,
    PRIMARY KEY (bringing_id, their_chapter)
) STRICT, WITHOUT ROWID;

-- One stretch of reading there. `measured` says that server timed it; when
-- it did not, the times are the day it was read and say nothing about how
-- long it took.
CREATE TABLE brought_sitting (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_chapter INTEGER NOT NULL,
    started_at    INTEGER NOT NULL,
    ended_at      INTEGER NOT NULL,
    first_page    INTEGER NOT NULL,
    last_page     INTEGER NOT NULL,
    measured      INTEGER NOT NULL
) STRICT;

CREATE INDEX brought_sitting_of ON brought_sitting (bringing_id, their_chapter);

-- A reading list there. `theirs` says the person made it; `shared` that it
-- was shown to everyone.
CREATE TABLE brought_list (
    bringing_id INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    summary     TEXT NOT NULL DEFAULT '',
    theirs      INTEGER NOT NULL,
    shared      INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE brought_list_book (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_list    INTEGER NOT NULL,
    position      INTEGER NOT NULL,
    their_chapter INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_list, position)
) STRICT, WITHOUT ROWID;

-- A collection there, and the series in it.
CREATE TABLE brought_collection (
    bringing_id INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    summary     TEXT NOT NULL DEFAULT '',
    theirs      INTEGER NOT NULL,
    shared      INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

CREATE TABLE brought_collection_series (
    bringing_id      INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_collection INTEGER NOT NULL,
    their_series     INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_collection, their_series)
) STRICT, WITHOUT ROWID;

-- A page the person marked there, from 0 in its chapter.
CREATE TABLE brought_page (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_chapter INTEGER NOT NULL,
    page          INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_chapter, page)
) STRICT, WITHOUT ROWID;

-- Someone who uses that server, read when the key was of one who manages
-- it. `limited` says they were kept from some of what is there by age.
CREATE TABLE brought_person (
    bringing_id INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_id    INTEGER NOT NULL,
    name        TEXT NOT NULL,
    manages     INTEGER NOT NULL,
    limited     INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_id)
) STRICT, WITHOUT ROWID;

-- The libraries there that such a person reads.
CREATE TABLE brought_person_library (
    bringing_id   INTEGER NOT NULL REFERENCES bringing (id) ON DELETE CASCADE,
    their_person  INTEGER NOT NULL,
    their_library INTEGER NOT NULL,
    PRIMARY KEY (bringing_id, their_person, their_library)
) STRICT, WITHOUT ROWID;
