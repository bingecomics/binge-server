-- A scan that changed what a library holds. A scan that found everything
-- as it was leaves no record.
CREATE TABLE scan_record (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    library_id INTEGER NOT NULL REFERENCES library (id) ON DELETE CASCADE,
    -- Seconds since 1970.
    started_at INTEGER NOT NULL,
    ended_at   INTEGER NOT NULL
) STRICT;

CREATE INDEX scan_record_library ON scan_record (library_id, id);

-- One file a scan found changed. `kind` says how:
--   `arrived`  a book the library never held
--   `back`     a book the library held before, gone since, and here again
--   `moved`    a book whose file is at another path; `from_path` is the old
--   `gone`     a book whose file is no longer there
-- Paths are relative to the library's folder. `content_hash` is the book's
-- identity, missing for a file that could not be read.
CREATE TABLE scan_change (
    id           INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id      INTEGER NOT NULL REFERENCES scan_record (id) ON DELETE CASCADE,
    kind         TEXT NOT NULL,
    path         TEXT NOT NULL,
    from_path    TEXT,
    content_hash TEXT
) STRICT;

CREATE INDEX scan_change_scan ON scan_change (scan_id, kind, id);

-- When the owner set aside a file that could not be read: seen, and not to
-- be listed with the others. Seconds since 1970. A file that is read again
-- and still cannot be is listed again.
ALTER TABLE book ADD COLUMN set_aside_at INTEGER;
