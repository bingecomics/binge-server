-- A series someone took off their "Up next" on the home page.
--
-- It stays off until they read from the series again: the row is not
-- deleted then, it is only older than their reading, and counts no more.
CREATE TABLE off_up_next (
    user_id      INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    series_id    INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    -- Seconds since 1970.
    taken_off_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, series_id)
) STRICT, WITHOUT ROWID;
