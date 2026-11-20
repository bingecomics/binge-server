-- What one scan changed is listed in the order it was written, a page at a
-- time: this reads a page without sorting the whole scan.
CREATE INDEX scan_change_in_order ON scan_change (scan_id, id);

-- Who is reading now is asked by when a sitting ended, whoever's it is.
CREATE INDEX sitting_ended ON sitting (ended_at);
