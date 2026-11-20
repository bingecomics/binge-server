-- A rule about a series goes when its series does: without this, every
-- series deleted reads all the rules to find its own.
CREATE INDEX cbl_rule_series ON cbl_rule (series_id);
