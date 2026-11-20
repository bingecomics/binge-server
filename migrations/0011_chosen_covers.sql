-- A cover someone chose, in place of the one the server shows by itself: a
-- page of a book or a picture they gave, made into a file under `covers/`
-- in the data directory. `cover` is that file's name, which is the hash of
-- what is in it, so two choices of the same picture share one file.

-- A book's hangs on its identity, like progress: every copy of the book has
-- it, and it is still there when the book comes back.
CREATE TABLE book_cover (
    content_hash TEXT PRIMARY KEY,
    cover        TEXT NOT NULL
) STRICT, WITHOUT ROWID;

-- For finding who shows a cover, when its file is asked for.
CREATE INDEX book_cover_file ON book_cover (cover);

CREATE TABLE series_cover (
    series_id INTEGER PRIMARY KEY REFERENCES series (id) ON DELETE CASCADE,
    cover     TEXT NOT NULL
) STRICT;

CREATE INDEX series_cover_file ON series_cover (cover);
