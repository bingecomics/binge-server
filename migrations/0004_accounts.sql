-- Accounts: who may use the server, how they prove it, and what they may read.

-- A person who may use the server.
CREATE TABLE user (
    id            INTEGER PRIMARY KEY AUTOINCREMENT,
    -- What the person signs in with, as they wrote it.
    name          TEXT NOT NULL,
    -- The name in lower case, which is what names are compared by: "Ana" and
    -- "ana" are one name.
    name_key      TEXT NOT NULL UNIQUE,
    -- Argon2id, as a PHC string. The password itself is never stored.
    password_hash TEXT NOT NULL,
    -- The owner reads every library and manages the server.
    is_owner      INTEGER NOT NULL DEFAULT 0,
    created_at    INTEGER NOT NULL
) STRICT;

-- There is one owner, made at first-run setup. This is also what settles two
-- setups that arrive at once.
CREATE UNIQUE INDEX user_one_owner ON user (is_owner) WHERE is_owner;

-- The libraries someone who is not the owner may read.
CREATE TABLE library_access (
    user_id    INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    library_id INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    PRIMARY KEY (user_id, library_id)
) STRICT, WITHOUT ROWID;

-- A browser that signed in with a password. The browser holds a random
-- secret in a cookie; the server keeps only the secret's BLAKE3 hash.
CREATE TABLE session (
    token_hash   TEXT PRIMARY KEY NOT NULL,
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    created_at   INTEGER NOT NULL,
    -- A session not used for a while is over.
    last_used_at INTEGER NOT NULL
) STRICT, WITHOUT ROWID;

CREATE INDEX session_user ON session (user_id);

-- A token an app uses instead of a password. Shown once when it is made;
-- the server keeps only its BLAKE3 hash.
CREATE TABLE api_token (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    -- What the person called it, such as the device it is on.
    name         TEXT NOT NULL,
    token_hash   TEXT NOT NULL UNIQUE,
    created_at   INTEGER NOT NULL,
    last_used_at INTEGER
) STRICT;

CREATE INDEX api_token_user ON api_token (user_id);

-- Progress becomes each reader's own: the reader joins the key.
--
-- What was saved while the server had one reader belongs to the owner, who
-- does not exist until first-run setup. Those rows wait in
-- `progress_before_accounts` and move to the owner when the owner is made.
ALTER TABLE progress RENAME TO progress_before_accounts;
DROP INDEX progress_recent;

-- How far a reader has read a book. Still keyed by the book's identity and
-- still without a link to `book`, for the reasons 0003 gives.
CREATE TABLE progress (
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    content_hash TEXT NOT NULL,
    -- The last page shown when the book was last read, from 0.
    page         INTEGER NOT NULL,
    -- Whether that reading reached the last page.
    completed    INTEGER NOT NULL DEFAULT 0,
    -- Seconds since 1970.
    updated_at   INTEGER NOT NULL,
    PRIMARY KEY (user_id, content_hash)
) STRICT, WITHOUT ROWID;

-- Continue Reading asks for one reader's unfinished books, most recent first.
CREATE INDEX progress_recent ON progress (user_id, completed, updated_at);
