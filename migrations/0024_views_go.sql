-- Views go, and a collection can be a row on Home (ADR 0025).

-- A view was a library narrowed and ordered, kept under a name (0015). A
-- library is still narrowed, with what is asked in its address: nothing of
-- that is kept by the server any more.
DROP TABLE kept_view_value;
DROP TABLE kept_view;

-- Whether a collection is a row on Home: its maker says so, and it is a row
-- for everyone who sees the collection. Where the row sits, and whether it
-- is put away, is each person's own (`home.rows`, ADR 0018).
ALTER TABLE collection ADD COLUMN on_home INTEGER NOT NULL DEFAULT 0 CHECK (on_home IN (0, 1));
