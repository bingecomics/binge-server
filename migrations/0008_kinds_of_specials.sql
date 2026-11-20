-- The one bucket of specials becomes three kinds of group: the annuals, the
-- one-shots, and the appearances in other titles.
--
-- A table's CHECK cannot be changed, so `series_group` is made again. Each
-- specials group becomes the one-shots of its series: what its books are is
-- for the owner to say afterwards.

CREATE TABLE series_group_next (
    id        INTEGER PRIMARY KEY AUTOINCREMENT,
    series_id INTEGER NOT NULL REFERENCES series (id) ON DELETE CASCADE,
    kind      TEXT NOT NULL CHECK (
        kind IN ('volume', 'limited', 'annual', 'one_shot', 'appearance', 'collected')
    ),
    number    INTEGER,
    name      TEXT
) STRICT;

INSERT INTO series_group_next (id, series_id, kind, number, name)
SELECT id, series_id, CASE kind WHEN 'specials' THEN 'one_shot' ELSE kind END, number, name
FROM series_group;

-- A group that was deleted keeps its id to itself: the next group made
-- takes the id after the last one ever given.
DELETE FROM sqlite_sequence WHERE name = 'series_group_next';
INSERT INTO sqlite_sequence (name, seq)
SELECT 'series_group_next', seq FROM sqlite_sequence WHERE name = 'series_group';

-- Dropping the old table takes every book out of its group, as deleting the
-- groups would. The groups are set aside first and given back after.
CREATE TABLE place_group AS
SELECT library_id, content_hash, group_id FROM book_place WHERE group_id IS NOT NULL;

DROP TABLE series_group;
ALTER TABLE series_group_next RENAME TO series_group;
CREATE INDEX series_group_series ON series_group (series_id);

UPDATE book_place
SET group_id = kept.group_id
FROM place_group kept
WHERE kept.library_id = book_place.library_id
  AND kept.content_hash = book_place.content_hash;

DROP TABLE place_group;
