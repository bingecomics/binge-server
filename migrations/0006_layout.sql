-- Series and groups become Binge's own record, apart from the folders and
-- from what the files' tags say (ADR 0012).

-- The folders' series go. The next scan imports every book again, by name.
-- Books are let go of first, so that removing a series removes no book.
UPDATE book SET series_id = NULL;
DELETE FROM series;

-- A series is no longer a folder. It is known by its name, compared loosely:
-- `name_key` is the name in lower case, without accents, punctuation or
-- spaces, and one library holds each key once.
ALTER TABLE series RENAME COLUMN folder TO name_key;

-- A part of a series with its own page: a volume, a limited series, the
-- specials, the collected editions. `number` is a volume's number; `name` is
-- a name of its own, for the kinds that have one.
CREATE TABLE series_group (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    kind      TEXT NOT NULL CHECK (kind IN ('volume', 'limited', 'specials', 'collected')),
    number    INTEGER,
    name      TEXT
) STRICT;

CREATE INDEX series_group_series ON series_group (series_id);

-- Where a book sits, by its identity, so that it stays when the file is
-- moved, renamed, retagged or gone for a while. Written once, when the
-- library first sees the book, and after that changed only in Binge.
-- `number` is the issue's number as written. `published` is the date as one
-- number, year * 10000 + month * 100 + day, with 99 for a month or day that
-- is not known.
CREATE TABLE book_place (
    library_id   INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    series_id    INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    group_id     INTEGER REFERENCES series_group (id) ON DELETE SET NULL,
    number       TEXT,
    published    INTEGER,
    PRIMARY KEY (library_id, content_hash)
) STRICT, WITHOUT ROWID;

CREATE INDEX book_place_series ON book_place (series_id);
CREATE INDEX book_place_group ON book_place (group_id);

-- The place in effect for each file, beside `series_id`: a copy of the
-- book's row in `book_place`. A file that could not be read has no identity
-- to keep a place by, so its place is worked out from its name each time.
ALTER TABLE book ADD COLUMN group_id INTEGER REFERENCES series_group (id) ON DELETE SET NULL;
ALTER TABLE book ADD COLUMN number TEXT;
ALTER TABLE book ADD COLUMN published INTEGER;

CREATE INDEX book_group ON book (group_id);

-- One more of the tags: what kind of publication the file says it is, such
-- as "Annual" or "TPB".
ALTER TABLE book_info ADD COLUMN format TEXT;

-- Makes the next scan read every file again, to give each book its place.
-- Pages, covers and progress stay as they are.
UPDATE book SET modified_at = -1;
