-- A view: a library narrowed and ordered, kept under a name, as "Unread
-- Vertigo". The owner makes them, or an AI client for the owner, and each
-- can be a row on the home page and an entry in the menu. Whoever may read
-- the library sees them; how far a series was read is each reader's own,
-- so "unread" is not the same series for everyone.
CREATE TABLE kept_view (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    -- The order of its series: what `GET /libraries/{id}/series` takes.
    sort       TEXT NOT NULL,
    reverse    INTEGER NOT NULL,
    seed       INTEGER NOT NULL,
    on_home    INTEGER NOT NULL,
    in_menu    INTEGER NOT NULL,
    -- Seconds since 1970.
    created_at INTEGER NOT NULL
) STRICT;

-- One value a view narrows by. `kind` is what is narrowed: `read`,
-- `publisher`, `year`, `writer` or `penciller`. `value` is written as the
-- address of a library writes it: `unread`, a publisher's name, `1990s`, a
-- person's id. A value is asked for or ruled out, never both.
CREATE TABLE kept_view_value (
    view_id   INTEGER NOT NULL REFERENCES kept_view (id) ON DELETE CASCADE,
    kind      TEXT NOT NULL,
    value     TEXT NOT NULL,
    ruled_out INTEGER NOT NULL,
    PRIMARY KEY (view_id, kind, value)
) STRICT, WITHOUT ROWID;
