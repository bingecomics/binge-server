-- How far a book has been read.
--
-- Keyed by the book's identity (`book.content_hash`), not by a book row, and
-- on purpose without a foreign key: a file that is moved or renamed is one
-- book removed and one added, a drive can be away for a while, and the same
-- pages may sit in two libraries. Progress outlives all of that.
--
-- There are no accounts yet, so there is one reader and one row per book.
-- When accounts arrive, a later migration adds the reader to the key and
-- gives these rows to the owner.
CREATE TABLE progress (
    content_hash TEXT PRIMARY KEY NOT NULL,
    -- The last page shown when the book was last read, from 0.
    page         INTEGER NOT NULL,
    -- Whether that reading reached the last page.
    completed    INTEGER NOT NULL DEFAULT 0,
    -- Seconds since 1970.
    updated_at   INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

-- Continue Reading asks for the unfinished books, most recent first.
CREATE INDEX progress_recent ON progress (completed, updated_at);

-- Recently Added asks for the newest books.
CREATE INDEX book_added ON book (added_at);
