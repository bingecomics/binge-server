-- A collection: series someone picked by hand, from any library, under a
-- name. It belongs to the person who made it and is shared as a reading
-- list is (ADR 0022).
CREATE TABLE collection (
    id              INTEGER PRIMARY KEY AUTOINCREMENT,
    -- Whose it is. A collection goes with its person.
    user_id         INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    name            TEXT NOT NULL,
    -- The name as names are compared (0006).
    name_key        TEXT NOT NULL,
    summary         TEXT,
    -- Whether everyone on the server sees it. The people it is shared with
    -- one by one are in `collection_share`.
    shared_with_all INTEGER NOT NULL DEFAULT 0 CHECK (shared_with_all IN (0, 1)),
    -- Seconds since 1970.
    created_at      INTEGER NOT NULL,
    -- No two collections of one person have the same name.
    UNIQUE (user_id, name_key)
) STRICT;

-- A series of a collection. A series is a record of the layout, so the
-- collection holds that record: it keeps the series through a new name and
-- through books moved in or out of it, and loses it when the series is
-- deleted or its library is removed. A collection holds a series once, and
-- its series have no order of their own.
CREATE TABLE collection_series (
    collection_id INTEGER NOT NULL REFERENCES collection (id) ON DELETE CASCADE,
    series_id     INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, series_id)
) STRICT, WITHOUT ROWID;

-- For the collections a series is in.
CREATE INDEX collection_series_series ON collection_series (series_id);

-- A person a collection is shared with. The one whose it is has no row.
CREATE TABLE collection_share (
    collection_id INTEGER NOT NULL REFERENCES collection (id) ON DELETE CASCADE,
    user_id       INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    PRIMARY KEY (collection_id, user_id)
) STRICT, WITHOUT ROWID;

-- For the collections shared with a person.
CREATE INDEX collection_share_person ON collection_share (user_id);
