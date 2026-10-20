-- A patient row outside the ordered scope whose value is an approved word on
-- a dilution execution is set aside with the reason `dilution_not_supported`
-- (result files brief F7 "Dilutions", F17; wave-10 review H1): the method
-- version's limit is not the limit of a diluted injection.
--
-- SQLite cannot widen a CHECK constraint in place, so the table is rebuilt
-- with the documented steps: create `x_next`, copy, drop `x`, rename, then
-- recreate its index and triggers. No view or other table's trigger reads it.
CREATE TABLE res_set_aside_rows_next (
    id                TEXT NOT NULL PRIMARY KEY,
    import_attempt_id TEXT NOT NULL REFERENCES res_import_attempts (id),
    record_path       TEXT NOT NULL,
    sample_label      TEXT,
    source_test_code  TEXT,
    value_text        TEXT,
    type_word         TEXT,
    kind              TEXT NOT NULL CHECK (kind IN ('batch_row_not_placed', 'not_ordered_unusable')),
    reason            TEXT CHECK (reason IN
                         ('incompatible_unit', 'malformed_value', 'dilution_mismatch',
                          'dilution_not_supported')),
    CHECK ((kind = 'not_ordered_unusable') = (reason IS NOT NULL))
) STRICT;

INSERT INTO res_set_aside_rows_next
SELECT id, import_attempt_id, record_path, sample_label, source_test_code, value_text,
       type_word, kind, reason
FROM res_set_aside_rows;

DROP TABLE res_set_aside_rows;
ALTER TABLE res_set_aside_rows_next RENAME TO res_set_aside_rows;

CREATE INDEX res_set_aside_rows_attempt ON res_set_aside_rows (import_attempt_id);

CREATE TRIGGER res_set_aside_rows_no_update BEFORE UPDATE ON res_set_aside_rows
BEGIN SELECT RAISE(ABORT, 'set-aside rows are append-only'); END;
CREATE TRIGGER res_set_aside_rows_no_delete BEFORE DELETE ON res_set_aside_rows
BEGIN SELECT RAISE(ABORT, 'set-aside rows are append-only'); END;
