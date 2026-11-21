-- The pages a person marked while reading are called favorites everywhere:
-- in the apps, in the API and here. They were "kept pages" (0019).
ALTER TABLE kept_page RENAME TO favorite_page;
-- When the page became a favorite, in seconds since 1970.
ALTER TABLE favorite_page RENAME COLUMN kept_at TO added_at;
