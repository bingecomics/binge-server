-- Reading lists and collections a library makes from what its files say
-- (ADR 0024).

-- A run of books a file says its book is part of, with the book's number in
-- it: a story arc, or another series it is also an issue of. It belongs to
-- the file, as `book_info` does.
CREATE TABLE book_arc (
    book_id  INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    -- The name as names are compared (0006): a book is in a run once.
    name_key TEXT NOT NULL,
    name     TEXT NOT NULL,
    -- Where the book comes in the run. A file may give none.
    number   REAL,
    -- Whether the book has still to be put in the list of the run. Every
    -- file that is read says it again, so it waits again.
    waiting  INTEGER NOT NULL DEFAULT 1 CHECK (waiting IN (0, 1)),
    PRIMARY KEY (book_id, name_key)
) STRICT, WITHOUT ROWID;

-- For the books of a run.
CREATE INDEX book_arc_name ON book_arc (name_key);
-- For what a scan has to put in lists.
CREATE INDEX book_arc_waiting ON book_arc (book_id) WHERE waiting;

-- A group of series a file says its book's series is part of.
CREATE TABLE book_series_group (
    book_id  INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    name_key TEXT NOT NULL,
    name     TEXT NOT NULL,
    -- Whether the book's series has still to be put in the collection of
    -- the group.
    waiting  INTEGER NOT NULL DEFAULT 1 CHECK (waiting IN (0, 1)),
    PRIMARY KEY (book_id, name_key)
) STRICT, WITHOUT ROWID;

-- For the books of a group.
CREATE INDEX book_series_group_name ON book_series_group (name_key);
-- For what a scan has to put in collections.
CREATE INDEX book_series_group_waiting ON book_series_group (book_id) WHERE waiting;

-- Whether a library makes reading lists and collections from that. The
-- owner turns each on: a library there before this makes none.
ALTER TABLE library ADD COLUMN lists_from_files INTEGER NOT NULL DEFAULT 0
    CHECK (lists_from_files IN (0, 1));
ALTER TABLE library ADD COLUMN collections_from_files INTEGER NOT NULL DEFAULT 0
    CHECK (collections_from_files IN (0, 1));

-- The name, as names are compared, of what the files made a list or a
-- collection from. It stays when its owner gives it another name, so the
-- files go on filling the same one.
ALTER TABLE reading_list ADD COLUMN from_files TEXT;
ALTER TABLE collection ADD COLUMN from_files TEXT;

CREATE UNIQUE INDEX reading_list_from_files ON reading_list (user_id, from_files)
    WHERE from_files IS NOT NULL;
CREATE UNIQUE INDEX collection_from_files ON collection (user_id, from_files)
    WHERE from_files IS NOT NULL;

-- Makes the next scan read every book's file again, to fill in what is new
-- here. Pages, covers and progress stay as they are. A file that could not
-- be read says nothing, and is left as it is: read again, one that was set
-- aside would be listed again.
UPDATE book SET modified_at = -1 WHERE content_hash IS NOT NULL;
