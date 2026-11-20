-- A page someone kept while reading, to find it again: a mark on the page,
-- theirs alone.
--
-- Keyed by the book's identity and the page's number and without a link to
-- `book`, as progress is and for the same reasons (0003): the mark stays
-- with the page when the book's file is moved or renamed, and while its
-- drive is away.
CREATE TABLE kept_page (
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    -- The page, from 0.
    page         INTEGER NOT NULL,
    -- Seconds since 1970.
    kept_at      INTEGER NOT NULL,
    PRIMARY KEY (user_id, content_hash, page)
) STRICT, WITHOUT ROWID;
