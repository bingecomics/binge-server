-- What a person chose in an app, kept by the server so that it follows
-- them to another browser or device: the size of the covers, the rows of
-- their home page. The server does not read the values; the app that
-- wrote one knows what it means.
CREATE TABLE user_setting (
    user_id    INTEGER NOT NULL REFERENCES user (id) ON DELETE CASCADE,
    key        TEXT NOT NULL,
    value      TEXT NOT NULL,
    -- Seconds since 1970.
    updated_at INTEGER NOT NULL,
    PRIMARY KEY (user_id, key)
) STRICT, WITHOUT ROWID;
