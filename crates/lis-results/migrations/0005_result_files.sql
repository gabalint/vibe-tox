-- Result files from LC-MS/MS software (REQ-ANA-003, REQ-RES-001, REQ-INT-005;
-- result files brief F7, F9–F11, F17, §5 and §13).
--
-- SQLite cannot widen a CHECK constraint in place, so the import attempts,
-- measurements and quarantine items are rebuilt with the documented steps:
-- create `x_next`, copy, drop `x`, rename, then recreate the indexes and
-- triggers of `x`. The store runs this with foreign-key enforcement off and a
-- full foreign-key check before commit. No view or other table's trigger
-- reads these tables.

-- Import attempts: whole-file failure codes, and counts for rows that are not
-- results, rows set aside and measurements kept outside the ordered scope.
CREATE TABLE res_import_attempts_next (
    id                TEXT    NOT NULL PRIMARY KEY,
    receipt_id        TEXT    NOT NULL REFERENCES ifc_receipts (id),
    attempt_no        INTEGER NOT NULL CHECK (attempt_no >= 1),
    instrument_id     TEXT    NOT NULL REFERENCES ifc_instruments (id),
    registration_id   TEXT    REFERENCES ana_source_registrations (id),
    outcome           TEXT    NOT NULL CHECK (outcome IN
                             ('processed', 'processed_with_quarantine', 'quarantined')),
    failure_code      TEXT    CHECK (failure_code IN
                             ('malformed_message', 'unregistered_source', 'sender_mismatch',
                              'undecodable_text', 'layout_mismatch', 'limits_exceeded')),
    rows_total        INTEGER NOT NULL CHECK (rows_total >= 0),
    rows_new          INTEGER NOT NULL CHECK (rows_new >= 0),
    rows_duplicate    INTEGER NOT NULL CHECK (rows_duplicate >= 0),
    rows_reprocessed  INTEGER NOT NULL CHECK (rows_reprocessed >= 0),
    rows_qc           INTEGER NOT NULL CHECK (rows_qc >= 0),
    rows_quarantined  INTEGER NOT NULL CHECK (rows_quarantined >= 0),
    rows_not_result   INTEGER NOT NULL CHECK (rows_not_result >= 0),
    rows_set_aside    INTEGER NOT NULL CHECK (rows_set_aside >= 0),
    rows_retained     INTEGER NOT NULL CHECK (rows_retained >= 0),
    parser_version    TEXT    NOT NULL,
    processed_at      TEXT    NOT NULL,
    UNIQUE (receipt_id, attempt_no),
    CHECK (failure_code IS NULL OR outcome = 'quarantined'),
    CHECK (rows_new + rows_duplicate + rows_reprocessed + rows_qc + rows_quarantined
           + rows_not_result + rows_set_aside + rows_retained <= rows_total)
) STRICT;

INSERT INTO res_import_attempts_next
SELECT id, receipt_id, attempt_no, instrument_id, registration_id, outcome, failure_code,
       rows_total, rows_new, rows_duplicate, rows_reprocessed, rows_qc, rows_quarantined,
       0, 0, 0, parser_version, processed_at
FROM res_import_attempts;

DROP TABLE res_import_attempts;
ALTER TABLE res_import_attempts_next RENAME TO res_import_attempts;

CREATE TRIGGER res_import_attempts_no_update BEFORE UPDATE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;
CREATE TRIGGER res_import_attempts_no_delete BEFORE DELETE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;

-- Measurements: the form `word` keeps an approved value word as received in
-- `value_text`; the result revision derives from its rule (F7).
CREATE TABLE res_measurements_next (
    id                     TEXT NOT NULL PRIMARY KEY,
    import_attempt_id      TEXT NOT NULL REFERENCES res_import_attempts (id),
    processing_revision_id TEXT NOT NULL REFERENCES res_processing_revisions (id),
    position_id            TEXT NOT NULL REFERENCES ana_run_positions (id),
    analyte_id             TEXT NOT NULL REFERENCES cat_analytes (id),
    source_test_code       TEXT NOT NULL,
    value_text             TEXT,
    units_text             TEXT,
    flags_text             TEXT,
    status_text            TEXT,
    operator_text          TEXT,
    completed_text         TEXT,
    record_path            TEXT NOT NULL,
    form                   TEXT NOT NULL CHECK (form IN
                             ('quantity', 'censored', 'qualitative', 'not_performed', 'word')),
    comparator             TEXT CHECK (comparator IN ('<', '<=', '>', '>=')),
    magnitude              TEXT,
    code_text              TEXT,
    recorded_at            TEXT NOT NULL,
    CHECK ((form = 'quantity' AND comparator IS NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (form = 'censored' AND comparator IS NOT NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (form = 'qualitative' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NOT NULL)
        OR (form = 'not_performed' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NULL)
        OR (form = 'word' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NULL
            AND value_text IS NOT NULL))
) STRICT;

INSERT INTO res_measurements_next
SELECT id, import_attempt_id, processing_revision_id, position_id, analyte_id,
       source_test_code, value_text, units_text, flags_text, status_text, operator_text,
       completed_text, record_path, form, comparator, magnitude, code_text, recorded_at
FROM res_measurements;

DROP TABLE res_measurements;
ALTER TABLE res_measurements_next RENAME TO res_measurements;

CREATE INDEX res_measurements_position_analyte
    ON res_measurements (position_id, analyte_id, recorded_at);

-- Defense in depth: payload content never decides whether a row is patient data.
CREATE TRIGGER res_measurements_patient_only BEFORE INSERT ON res_measurements
WHEN coalesce((SELECT role FROM ana_v_positions WHERE position_id = NEW.position_id), 'not_patient') <> 'patient'
BEGIN SELECT RAISE(ABORT, 'measurements attach only to patient positions'); END;
CREATE TRIGGER res_measurements_no_update BEFORE UPDATE ON res_measurements
BEGIN SELECT RAISE(ABORT, 'measurements are append-only'); END;
CREATE TRIGGER res_measurements_no_delete BEFORE DELETE ON res_measurements
BEGIN SELECT RAISE(ABORT, 'measurements are append-only'); END;

-- Quarantine items: a record whose field count differs from the header, a
-- sample type that disagrees with the run position's role, and a dilution
-- factor that differs from the execution's (F9, F11).
CREATE TABLE res_quarantine_items_next (
    id                         TEXT    NOT NULL PRIMARY KEY,
    import_attempt_id          TEXT    NOT NULL REFERENCES res_import_attempts (id),
    record_path                TEXT    NOT NULL,
    sample_label               TEXT,
    source_test_code           TEXT,
    value_text                 TEXT,
    units_text                 TEXT,
    flags_text                 TEXT,
    status_text                TEXT,
    operator_text              TEXT,
    completed_text             TEXT,
    position_id                TEXT    REFERENCES ana_run_positions (id),
    analyte_id                 TEXT    REFERENCES cat_analytes (id),
    reason                     TEXT    NOT NULL CHECK (reason IN
                                  ('unknown_sample', 'ambiguous_sample', 'run_closed',
                                   'execution_failed', 'unknown_analyte', 'not_in_ordered_scope',
                                   'incompatible_unit', 'malformed_value', 'scale_mismatch',
                                   'conflicting_duplicate', 'dilution_not_supported',
                                   'malformed_row', 'role_mismatch', 'dilution_mismatch')),
    state                      TEXT    NOT NULL CHECK (state IN
                                  ('open', 'accepted_as_reprocessing', 'rejected',
                                   'superseded_by_retry')),
    resolved_by                TEXT,
    resolved_at                TEXT,
    resolution_reason          TEXT,
    revision                   INTEGER NOT NULL CHECK (revision >= 1),
    conflicting_measurement_id TEXT REFERENCES res_measurements (id),
    CHECK ((state = 'open') = (resolved_at IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolved_by IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolution_reason IS NULL))
) STRICT;

INSERT INTO res_quarantine_items_next
SELECT id, import_attempt_id, record_path, sample_label, source_test_code,
       value_text, units_text, flags_text, status_text, operator_text,
       completed_text, position_id, analyte_id, reason, state, resolved_by,
       resolved_at, resolution_reason, revision, conflicting_measurement_id
FROM res_quarantine_items;

DROP TABLE res_quarantine_items;
ALTER TABLE res_quarantine_items_next RENAME TO res_quarantine_items;

CREATE INDEX res_quarantine_open_position_analyte
    ON res_quarantine_items (position_id, analyte_id) WHERE state = 'open';

CREATE TRIGGER res_quarantine_resolve_only BEFORE UPDATE ON res_quarantine_items
WHEN OLD.state <> 'open' OR NEW.state = 'open' OR NEW.revision <> OLD.revision + 1
  OR NEW.id IS NOT OLD.id OR NEW.import_attempt_id IS NOT OLD.import_attempt_id
  OR NEW.record_path IS NOT OLD.record_path OR NEW.sample_label IS NOT OLD.sample_label
  OR NEW.source_test_code IS NOT OLD.source_test_code OR NEW.value_text IS NOT OLD.value_text
  OR NEW.units_text IS NOT OLD.units_text OR NEW.flags_text IS NOT OLD.flags_text
  OR NEW.status_text IS NOT OLD.status_text OR NEW.operator_text IS NOT OLD.operator_text
  OR NEW.completed_text IS NOT OLD.completed_text
  OR NEW.position_id IS NOT OLD.position_id OR NEW.analyte_id IS NOT OLD.analyte_id
  OR NEW.reason IS NOT OLD.reason
  OR NEW.conflicting_measurement_id IS NOT OLD.conflicting_measurement_id
BEGIN SELECT RAISE(ABORT, 'quarantine items only move from open to a resolution'); END;

CREATE TRIGGER res_quarantine_no_delete BEFORE DELETE ON res_quarantine_items
BEGIN SELECT RAISE(ABORT, 'quarantine items are never deleted'); END;

CREATE TRIGGER res_quarantine_conflicting_measurement_immutable
BEFORE UPDATE OF conflicting_measurement_id ON res_quarantine_items
BEGIN SELECT RAISE(ABORT, 'quarantine conflict context is immutable'); END;

-- A revision derived from an approved value word cites its rule: the
-- registration version and the word (F7). Only censored and not-performed
-- results come from a word.
ALTER TABLE res_result_revisions ADD COLUMN value_rule_registration_id TEXT
    REFERENCES ana_source_registrations (id);
ALTER TABLE res_result_revisions ADD COLUMN value_rule_word TEXT
    CHECK ((value_rule_word IS NULL) = (value_rule_registration_id IS NULL)
       AND (value_rule_word IS NULL OR outcome IN ('censored', 'not_performed')));

-- Rows an import attempt lists without quarantining them (F9, F17): a
-- calibrator or blank row whose label the LIS does not know, and a patient
-- row outside the ordered scope whose value cannot be kept, with the check
-- it failed.
CREATE TABLE res_set_aside_rows (
    id                TEXT NOT NULL PRIMARY KEY,
    import_attempt_id TEXT NOT NULL REFERENCES res_import_attempts (id),
    record_path       TEXT NOT NULL,
    sample_label      TEXT,
    source_test_code  TEXT,
    value_text        TEXT,
    type_word         TEXT,
    kind              TEXT NOT NULL CHECK (kind IN ('batch_row_not_placed', 'not_ordered_unusable')),
    reason            TEXT CHECK (reason IN
                         ('incompatible_unit', 'malformed_value', 'dilution_mismatch')),
    CHECK ((kind = 'not_ordered_unusable') = (reason IS NOT NULL))
) STRICT;

CREATE INDEX res_set_aside_rows_attempt ON res_set_aside_rows (import_attempt_id);

CREATE TRIGGER res_set_aside_rows_no_update BEFORE UPDATE ON res_set_aside_rows
BEGIN SELECT RAISE(ABORT, 'set-aside rows are append-only'); END;
CREATE TRIGGER res_set_aside_rows_no_delete BEFORE DELETE ON res_set_aside_rows
BEGIN SELECT RAISE(ABORT, 'set-aside rows are append-only'); END;

-- The specimen's measured-not-ordered list finds measurements no revision cites.
CREATE INDEX res_revision_evidence_measurement ON res_revision_evidence (measurement_id);
