-- Immutable result evidence, explicit reprocessing, quarantine, validity and
-- technical review (REQ-ANA-003, REQ-RES-001–005, REQ-INT-003).

CREATE TABLE res_import_attempts (
    id                TEXT    NOT NULL PRIMARY KEY,
    receipt_id        TEXT    NOT NULL REFERENCES ifc_receipts (id),
    attempt_no        INTEGER NOT NULL CHECK (attempt_no >= 1),
    instrument_id     TEXT    NOT NULL REFERENCES ifc_instruments (id),
    registration_id   TEXT    REFERENCES ana_source_registrations (id),
    outcome           TEXT    NOT NULL CHECK (outcome IN
                             ('processed', 'processed_with_quarantine', 'quarantined')),
    failure_code      TEXT    CHECK (failure_code IN
                             ('malformed_message', 'unregistered_source', 'sender_mismatch')),
    rows_total        INTEGER NOT NULL CHECK (rows_total >= 0),
    rows_new          INTEGER NOT NULL CHECK (rows_new >= 0),
    rows_duplicate    INTEGER NOT NULL CHECK (rows_duplicate >= 0),
    rows_reprocessed  INTEGER NOT NULL CHECK (rows_reprocessed >= 0),
    rows_qc           INTEGER NOT NULL CHECK (rows_qc >= 0),
    rows_quarantined  INTEGER NOT NULL CHECK (rows_quarantined >= 0),
    parser_version    TEXT    NOT NULL,
    processed_at      TEXT    NOT NULL,
    UNIQUE (receipt_id, attempt_no),
    CHECK (failure_code IS NULL OR outcome = 'quarantined'),
    CHECK (rows_new + rows_duplicate + rows_reprocessed + rows_qc + rows_quarantined <= rows_total)
) STRICT;

CREATE TABLE res_processing_revisions (
    id                TEXT    NOT NULL PRIMARY KEY,
    run_id            TEXT    NOT NULL REFERENCES ana_runs (id),
    seq               INTEGER NOT NULL CHECK (seq >= 1),
    import_attempt_id TEXT    NOT NULL REFERENCES res_import_attempts (id),
    predecessor_id    TEXT    REFERENCES res_processing_revisions (id),
    created_at        TEXT    NOT NULL,
    UNIQUE (run_id, seq),
    CHECK ((seq = 1) = (predecessor_id IS NULL))
) STRICT;

CREATE TABLE res_measurements (
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
                             ('quantity', 'censored', 'qualitative', 'not_performed')),
    comparator             TEXT CHECK (comparator IN ('<', '<=', '>', '>=')),
    magnitude              TEXT,
    code_text              TEXT,
    recorded_at            TEXT NOT NULL,
    CHECK ((form = 'quantity' AND comparator IS NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (form = 'censored' AND comparator IS NOT NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (form = 'qualitative' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NOT NULL)
        OR (form = 'not_performed' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NULL))
) STRICT;

CREATE INDEX res_measurements_position_analyte
    ON res_measurements (position_id, analyte_id, recorded_at);

-- Defense in depth: payload content never decides whether a row is patient data.
CREATE TRIGGER res_measurements_patient_only BEFORE INSERT ON res_measurements
WHEN coalesce((SELECT role FROM ana_v_positions WHERE position_id = NEW.position_id), 'not_patient') <> 'patient'
BEGIN SELECT RAISE(ABORT, 'measurements attach only to patient positions'); END;

CREATE TABLE res_measurement_supersessions (
    measurement_id         TEXT NOT NULL PRIMARY KEY REFERENCES res_measurements (id),
    superseded_by_id       TEXT NOT NULL UNIQUE REFERENCES res_measurements (id),
    processing_revision_id TEXT NOT NULL REFERENCES res_processing_revisions (id),
    CHECK (measurement_id <> superseded_by_id)
) STRICT;

CREATE TABLE res_quarantine_items (
    id                TEXT    NOT NULL PRIMARY KEY,
    import_attempt_id TEXT    NOT NULL REFERENCES res_import_attempts (id),
    record_path       TEXT    NOT NULL,
    sample_label      TEXT,
    source_test_code  TEXT,
    value_text        TEXT,
    units_text        TEXT,
    flags_text        TEXT,
    status_text       TEXT,
    operator_text     TEXT,
    completed_text    TEXT,
    position_id       TEXT    REFERENCES ana_run_positions (id),
    analyte_id        TEXT    REFERENCES cat_analytes (id),
    reason            TEXT    NOT NULL CHECK (reason IN
                         ('unknown_sample', 'ambiguous_sample', 'run_closed', 'unknown_analyte',
                          'not_in_ordered_scope', 'incompatible_unit', 'malformed_value',
                          'scale_mismatch', 'conflicting_duplicate', 'dilution_not_supported')),
    state             TEXT    NOT NULL CHECK (state IN
                         ('open', 'accepted_as_reprocessing', 'rejected', 'superseded_by_retry')),
    resolved_by       TEXT,
    resolved_at       TEXT,
    resolution_reason TEXT,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'open') = (resolved_at IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolved_by IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolution_reason IS NULL))
) STRICT;

CREATE INDEX res_quarantine_open_position_analyte
    ON res_quarantine_items (position_id, analyte_id) WHERE state = 'open';

CREATE TABLE res_results (
    id                 TEXT    NOT NULL PRIMARY KEY,
    order_component_id TEXT    NOT NULL REFERENCES ord_order_components (id),
    analyte_id         TEXT    NOT NULL REFERENCES cat_analytes (id),
    purpose            TEXT    NOT NULL CHECK (purpose IN ('analytical', 'validity')),
    current_revision_id TEXT,
    revision           INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE (order_component_id, analyte_id),
    FOREIGN KEY (current_revision_id) REFERENCES res_result_revisions (id)
        DEFERRABLE INITIALLY DEFERRED
) STRICT;

CREATE TABLE res_result_revisions (
    id                 TEXT    NOT NULL PRIMARY KEY,
    result_id          TEXT    NOT NULL REFERENCES res_results (id),
    revision_no        INTEGER NOT NULL CHECK (revision_no >= 1),
    cause              TEXT    NOT NULL CHECK (cause IN
                         ('initial', 'reprocessed', 'selected', 'disposition')),
    outcome            TEXT    NOT NULL CHECK (outcome IN
                         ('quantity', 'censored', 'qualitative', 'not_performed',
                          'invalid', 'indeterminate', 'missing')),
    comparator         TEXT    CHECK (comparator IN ('<', '<=', '>', '>=')),
    magnitude          TEXT,
    units_text         TEXT,
    code_text          TEXT,
    classification     TEXT    NOT NULL CHECK (classification IN
                         ('meets_cutoff', 'does_not_meet_cutoff', 'not_classifiable', 'not_applicable')),
    classification_reason TEXT,
    method_version_id  TEXT    NOT NULL REFERENCES cat_method_versions (id),
    method_certainty   TEXT    NOT NULL CHECK (method_certainty IN ('presumptive', 'definitive')),
    output_scale       TEXT    NOT NULL CHECK (output_scale IN ('qualitative', 'quantitative')),
    sequence_role      TEXT    NOT NULL CHECK (sequence_role IN ('first_line', 'confirmation')),
    specimen_id        TEXT    NOT NULL REFERENCES spc_specimens (id),
    reason             TEXT,
    created_by         TEXT    NOT NULL,
    created_at         TEXT    NOT NULL,
    UNIQUE (result_id, revision_no),
    CHECK (cause IN ('initial', 'reprocessed') OR reason IS NOT NULL),
    CHECK ((outcome = 'quantity' AND comparator IS NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (outcome = 'censored' AND comparator IS NOT NULL AND magnitude IS NOT NULL AND code_text IS NULL)
        OR (outcome = 'qualitative' AND comparator IS NULL AND magnitude IS NULL AND code_text IS NOT NULL)
        OR (outcome IN ('not_performed', 'invalid', 'indeterminate', 'missing')
            AND comparator IS NULL AND magnitude IS NULL AND code_text IS NULL))
) STRICT;

CREATE TABLE res_revision_evidence (
    revision_id  TEXT NOT NULL REFERENCES res_result_revisions (id),
    measurement_id TEXT NOT NULL REFERENCES res_measurements (id),
    PRIMARY KEY (revision_id, measurement_id)
) STRICT;

CREATE TABLE res_technical_reviews (
    id          TEXT NOT NULL PRIMARY KEY,
    specimen_id TEXT NOT NULL REFERENCES spc_specimens (id),
    decision    TEXT NOT NULL CHECK (decision IN ('approved', 'rejected')),
    reason      TEXT,
    reviewed_by TEXT NOT NULL,
    reviewed_at TEXT NOT NULL,
    CHECK (decision = 'approved' OR reason IS NOT NULL)
) STRICT;

CREATE TABLE res_review_items (
    review_id          TEXT NOT NULL REFERENCES res_technical_reviews (id),
    result_revision_id TEXT NOT NULL REFERENCES res_result_revisions (id),
    PRIMARY KEY (review_id, result_revision_id)
) STRICT;

CREATE TABLE res_review_dependencies (
    review_id TEXT NOT NULL REFERENCES res_technical_reviews (id),
    kind      TEXT NOT NULL CHECK (kind IN
                ('qc_assessment', 'prep_assessment', 'validity_assessment')),
    ref_id    TEXT NOT NULL,
    PRIMARY KEY (review_id, kind, ref_id)
) STRICT;

CREATE TABLE res_validity_assessments (
    id                TEXT NOT NULL PRIMARY KEY,
    specimen_id       TEXT NOT NULL REFERENCES spc_specimens (id),
    policy_version_id TEXT NOT NULL REFERENCES cat_validity_policy_versions (id),
    category_code     TEXT NOT NULL,
    supersedes_id     TEXT UNIQUE REFERENCES res_validity_assessments (id),
    reason            TEXT,
    assessed_by       TEXT NOT NULL,
    assessed_at       TEXT NOT NULL,
    CHECK (supersedes_id IS NULL OR reason IS NOT NULL),
    FOREIGN KEY (policy_version_id, category_code)
        REFERENCES cat_validity_categories (policy_version_id, code)
) STRICT;

CREATE TABLE res_validity_evidence (
    assessment_id     TEXT NOT NULL REFERENCES res_validity_assessments (id),
    result_revision_id TEXT NOT NULL REFERENCES res_result_revisions (id),
    PRIMARY KEY (assessment_id, result_revision_id)
) STRICT;

CREATE VIEW res_v_current_results AS
SELECT r.id AS result_id, r.order_component_id, r.analyte_id, r.purpose,
       r.current_revision_id, rr.specimen_id
FROM res_results r
LEFT JOIN res_result_revisions rr ON rr.id = r.current_revision_id;

-- Source evidence and decisions are append-only. Mutable pointers and
-- quarantine resolution have narrower transition triggers below.
CREATE TRIGGER res_import_attempts_no_update BEFORE UPDATE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;
CREATE TRIGGER res_import_attempts_no_delete BEFORE DELETE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;
CREATE TRIGGER res_processing_revisions_no_update BEFORE UPDATE ON res_processing_revisions
BEGIN SELECT RAISE(ABORT, 'processing revisions are append-only'); END;
CREATE TRIGGER res_processing_revisions_no_delete BEFORE DELETE ON res_processing_revisions
BEGIN SELECT RAISE(ABORT, 'processing revisions are append-only'); END;
CREATE TRIGGER res_measurements_no_update BEFORE UPDATE ON res_measurements
BEGIN SELECT RAISE(ABORT, 'measurements are append-only'); END;
CREATE TRIGGER res_measurements_no_delete BEFORE DELETE ON res_measurements
BEGIN SELECT RAISE(ABORT, 'measurements are append-only'); END;
CREATE TRIGGER res_measurement_supersessions_no_update BEFORE UPDATE ON res_measurement_supersessions
BEGIN SELECT RAISE(ABORT, 'measurement supersessions are append-only'); END;
CREATE TRIGGER res_measurement_supersessions_no_delete BEFORE DELETE ON res_measurement_supersessions
BEGIN SELECT RAISE(ABORT, 'measurement supersessions are append-only'); END;
CREATE TRIGGER res_result_revisions_no_update BEFORE UPDATE ON res_result_revisions
BEGIN SELECT RAISE(ABORT, 'result revisions are append-only'); END;
CREATE TRIGGER res_result_revisions_no_delete BEFORE DELETE ON res_result_revisions
BEGIN SELECT RAISE(ABORT, 'result revisions are append-only'); END;
CREATE TRIGGER res_revision_evidence_no_update BEFORE UPDATE ON res_revision_evidence
BEGIN SELECT RAISE(ABORT, 'revision evidence is append-only'); END;
CREATE TRIGGER res_revision_evidence_no_delete BEFORE DELETE ON res_revision_evidence
BEGIN SELECT RAISE(ABORT, 'revision evidence is append-only'); END;
CREATE TRIGGER res_technical_reviews_no_update BEFORE UPDATE ON res_technical_reviews
BEGIN SELECT RAISE(ABORT, 'technical reviews are append-only'); END;
CREATE TRIGGER res_technical_reviews_no_delete BEFORE DELETE ON res_technical_reviews
BEGIN SELECT RAISE(ABORT, 'technical reviews are append-only'); END;
CREATE TRIGGER res_review_items_no_update BEFORE UPDATE ON res_review_items
BEGIN SELECT RAISE(ABORT, 'review items are append-only'); END;
CREATE TRIGGER res_review_items_no_delete BEFORE DELETE ON res_review_items
BEGIN SELECT RAISE(ABORT, 'review items are append-only'); END;
CREATE TRIGGER res_review_dependencies_no_update BEFORE UPDATE ON res_review_dependencies
BEGIN SELECT RAISE(ABORT, 'review dependencies are append-only'); END;
CREATE TRIGGER res_review_dependencies_no_delete BEFORE DELETE ON res_review_dependencies
BEGIN SELECT RAISE(ABORT, 'review dependencies are append-only'); END;
CREATE TRIGGER res_validity_assessments_no_update BEFORE UPDATE ON res_validity_assessments
BEGIN SELECT RAISE(ABORT, 'validity assessments are append-only'); END;
CREATE TRIGGER res_validity_assessments_no_delete BEFORE DELETE ON res_validity_assessments
BEGIN SELECT RAISE(ABORT, 'validity assessments are append-only'); END;
CREATE TRIGGER res_validity_evidence_no_update BEFORE UPDATE ON res_validity_evidence
BEGIN SELECT RAISE(ABORT, 'validity evidence is append-only'); END;
CREATE TRIGGER res_validity_evidence_no_delete BEFORE DELETE ON res_validity_evidence
BEGIN SELECT RAISE(ABORT, 'validity evidence is append-only'); END;

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
BEGIN SELECT RAISE(ABORT, 'quarantine items only move from open to a resolution'); END;
CREATE TRIGGER res_quarantine_no_delete BEFORE DELETE ON res_quarantine_items
BEGIN SELECT RAISE(ABORT, 'quarantine items are never deleted'); END;

CREATE TRIGGER res_results_pointer_only BEFORE UPDATE ON res_results
WHEN NEW.id IS NOT OLD.id OR NEW.order_component_id IS NOT OLD.order_component_id
  OR NEW.analyte_id IS NOT OLD.analyte_id OR NEW.purpose IS NOT OLD.purpose
  OR NEW.current_revision_id IS OLD.current_revision_id OR NEW.revision <> OLD.revision + 1
BEGIN SELECT RAISE(ABORT, 'result lines only advance their current revision'); END;
CREATE TRIGGER res_results_no_delete BEFORE DELETE ON res_results
BEGIN SELECT RAISE(ABORT, 'result lines are never deleted'); END;
