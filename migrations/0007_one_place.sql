-- A book's place is kept in one spot: `book_place`. The copy of it on each
-- file's row goes, so that moving a book is one write.
--
-- A file that could not be read has no identity, so it has no place and is
-- in no series. A library lists such files apart.

DROP INDEX book_series;
DROP INDEX book_group;
ALTER TABLE book DROP COLUMN series_id;
ALTER TABLE book DROP COLUMN group_id;
ALTER TABLE book DROP COLUMN number;
ALTER TABLE book DROP COLUMN published;

-- Lists go from a place to the files that hold that book.
CREATE INDEX book_identity ON book (library_id, content_hash);

-- What was made only for files that could not be read holds no place now.
DELETE FROM series_group
WHERE NOT EXISTS (SELECT 1 FROM book_place WHERE book_place.group_id = series_group.id);
DELETE FROM series
WHERE NOT EXISTS (SELECT 1 FROM book_place WHERE book_place.series_id = series.id);
