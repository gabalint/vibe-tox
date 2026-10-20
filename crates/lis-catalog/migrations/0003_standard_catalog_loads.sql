-- Standard Catalog loads (REQ-OPS-014, proposed). Each load records the file
-- version and digest, who loaded it and every record it created, so the
-- "starter content, not reviewed" banner can count the loaded versions still
-- in draft. Loaded content itself lives in the ordinary catalog tables,
-- written through the ordinary commands. Both tables are append-only.

CREATE TABLE cat_standard_loads (
    id              TEXT NOT NULL PRIMARY KEY,
    catalog_version TEXT NOT NULL,
    -- Lowercase hex SHA-256 of the embedded file's bytes.
    file_digest     TEXT NOT NULL CHECK (length(file_digest) = 64),
    loaded_by       TEXT NOT NULL,
    loaded_at       TEXT NOT NULL
) STRICT;

CREATE TABLE cat_standard_load_items (
    load_id     TEXT NOT NULL REFERENCES cat_standard_loads (id),
    record_type TEXT NOT NULL CHECK (record_type IN ('specimen_type', 'analyte', 'method',
        'method_version', 'orderable', 'orderable_version')),
    record_id   TEXT NOT NULL,
    PRIMARY KEY (load_id, record_type, record_id)
) STRICT;

CREATE INDEX cat_standard_load_items_record ON cat_standard_load_items (record_type, record_id);

CREATE TRIGGER cat_standard_loads_no_update BEFORE UPDATE ON cat_standard_loads
BEGIN
    SELECT RAISE(ABORT, 'standard catalog loads are append-only');
END;

CREATE TRIGGER cat_standard_loads_no_delete BEFORE DELETE ON cat_standard_loads
BEGIN
    SELECT RAISE(ABORT, 'standard catalog loads are append-only');
END;

CREATE TRIGGER cat_standard_load_items_no_update BEFORE UPDATE ON cat_standard_load_items
BEGIN
    SELECT RAISE(ABORT, 'standard catalog loads are append-only');
END;

CREATE TRIGGER cat_standard_load_items_no_delete BEFORE DELETE ON cat_standard_load_items
BEGIN
    SELECT RAISE(ABORT, 'standard catalog loads are append-only');
END;
