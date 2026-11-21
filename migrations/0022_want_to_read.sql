-- A series someone wants to read. It is each person's own: nobody else is
-- shown it, and it goes with its person. It holds the record of the series,
-- as a collection does (0021): the series stays on it under a new name, and
-- leaves it when the series is deleted or its library is removed.
CREATE TABLE want_to_read (
    user_id   INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    -- Seconds since 1970. What was put there last is shown first.
    wanted_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, series_id)
) STRICT, WITHOUT ROWID;

-- For a series that is deleted: its rows go with it.
CREATE INDEX want_to_read_series ON want_to_read (series_id);
