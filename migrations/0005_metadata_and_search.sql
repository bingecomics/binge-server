-- What a book's file says about itself, from the ComicInfo.xml inside it. A
-- book whose file says nothing has no row. It belongs to the file, not to
-- the book's identity: retagging a file changes this and nothing else.
CREATE TABLE book_info (
    book_id   INTEGER PRIMARY KEY REFERENCES book (id) ON DELETE CASCADE,
    title     TEXT,
    series    TEXT,
    number    TEXT,
    volume    INTEGER,
    year      INTEGER,
    month     INTEGER,
    day       INTEGER,
    writer    TEXT,
    penciller TEXT,
    publisher TEXT,
    summary   TEXT,
    language  TEXT
) STRICT;

-- What search looks through: the words a series or a book can be found by,
-- in lower case and without accents. Written by the scan.
ALTER TABLE series ADD COLUMN search_key TEXT NOT NULL DEFAULT '';
ALTER TABLE book ADD COLUMN search_key TEXT NOT NULL DEFAULT '';

-- Makes the next scan read every file again, to fill in what is new here.
-- Pages, covers and progress stay as they are.
UPDATE book SET modified_at = -1;
