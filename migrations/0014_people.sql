-- People: who the books name as their writers and pencillers (ADR 0016).
-- Files spell one person several ways, so a person is the names they go
-- by. Making two people one moves the names of one to the other; nothing a
-- file says is changed.
CREATE TABLE person (
    id   INTEGER PRIMARY KEY AUTOINCREMENT,
    -- The name the person is shown by: one of their names.
    name TEXT NOT NULL
) STRICT;

-- One way of writing a person's name. A name stays when no file spells it
-- any more, so that what the owner said about it is there when one does.
CREATE TABLE person_name (
    id         INTEGER PRIMARY KEY AUTOINCREMENT,
    person_id  INTEGER NOT NULL REFERENCES person (id) ON DELETE CASCADE,
    name       TEXT NOT NULL,
    -- The name as names are compared (0006): no two people go by the same.
    name_key   TEXT NOT NULL UNIQUE,
    -- What search looks through (0005).
    search_key TEXT NOT NULL
) STRICT;

CREATE INDEX person_name_person ON person_name (person_id);

-- Who a book's file names, and for what. It belongs to the file, as
-- `book_info` does, and is written with it. It holds the name as the file
-- spells it: whose name that is can change without touching this.
CREATE TABLE book_credit (
    book_id INTEGER NOT NULL REFERENCES book (id) ON DELETE CASCADE,
    role    TEXT NOT NULL CHECK (role IN ('writer', 'penciller')),
    name_id INTEGER NOT NULL REFERENCES person_name (id) ON DELETE CASCADE,
    PRIMARY KEY (book_id, role, name_id)
) STRICT, WITHOUT ROWID;

-- For the books of a person.
CREATE INDEX book_credit_name ON book_credit (name_id, role);

-- For narrowing a library by publisher.
CREATE INDEX book_info_publisher ON book_info (publisher);
