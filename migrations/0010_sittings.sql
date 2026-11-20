-- One sitting with a book: someone reading it with no long pause between
-- two pages. `progress` says where a reader is in a book; this says when
-- they read, which progress forgets each time it moves.
--
-- Keyed by the book's identity and without a link to `book`, as progress
-- is and for the same reasons (0003). Marking a book read or unread writes
-- no sitting and takes none away: a sitting is reading that happened.
CREATE TABLE sitting (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    -- When the first and the last page of the sitting were shown, in seconds
    -- since 1970.
    started_at   INTEGER NOT NULL,
    ended_at     INTEGER NOT NULL,
    -- The page shown first and the one shown last, from 0.
    first_page   INTEGER NOT NULL,
    last_page    INTEGER NOT NULL
) STRICT;

-- A page turn looks for the reader's sitting with that book that is still
-- going on.
CREATE INDEX sitting_of_book ON sitting (user_id, content_hash, ended_at);

-- What someone read, by when.
CREATE INDEX sitting_when ON sitting (user_id, started_at);
