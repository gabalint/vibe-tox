-- Forward-only review-safety additions for M2b findings 6 and 7, plus the
-- M3 release-queue discovery view (REQ-ANA-001, REQ-ANA-004, REQ-RES-005,
-- REQ-RPT-001, NFR-01).

-- SQLite cannot widen a CHECK constraint in place. Rebuild the quarantine
-- table so failed executions have their own durable reason. Preserve the
-- conflict context introduced by migration 0002.
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
                                   'conflicting_duplicate', 'dilution_not_supported')),
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

-- A technical review binds the database-wide analytical deviation sequence
-- watermark. Missing watermarks on reviews created before this migration are
-- treated as stale by the query code.
CREATE TABLE res_review_dependencies_next (
    review_id TEXT NOT NULL REFERENCES res_technical_reviews (id),
    kind      TEXT NOT NULL CHECK (kind IN
                  ('qc_assessment', 'prep_assessment', 'validity_assessment',
                   'deviation_watermark')),
    ref_id    TEXT NOT NULL,
    PRIMARY KEY (review_id, kind, ref_id)
) STRICT;

INSERT INTO res_review_dependencies_next
SELECT review_id, kind, ref_id FROM res_review_dependencies;

DROP TABLE res_review_dependencies;
ALTER TABLE res_review_dependencies_next RENAME TO res_review_dependencies;

CREATE TRIGGER res_review_dependencies_no_update BEFORE UPDATE ON res_review_dependencies
BEGIN SELECT RAISE(ABORT, 'review dependencies are append-only'); END;
CREATE TRIGGER res_review_dependencies_no_delete BEFORE DELETE ON res_review_dependencies
BEGIN SELECT RAISE(ABORT, 'review dependencies are append-only'); END;

-- Structural discovery for release queues. Dynamic QC, preparation,
-- deviation, validity and specimen gates stay in Rust and are rechecked in
-- the reporting transaction through specimen_release_lines (NFR-01).
CREATE VIEW res_v_reviewed_lines AS
SELECT rr.specimen_id,
       result.id AS result_id,
       result.order_component_id,
       result.analyte_id,
       rr.id AS revision_id,
       review.id AS review_id,
       review.reviewed_at
FROM res_results result
JOIN res_result_revisions rr ON rr.id = result.current_revision_id
JOIN res_review_items item ON item.result_revision_id = rr.id
JOIN res_technical_reviews review ON review.id = item.review_id
WHERE review.decision = 'approved'
  AND NOT EXISTS (
      SELECT 1
      FROM res_review_items later_item
      JOIN res_result_revisions later_revision
        ON later_revision.id = later_item.result_revision_id
      JOIN res_technical_reviews later_review
        ON later_review.id = later_item.review_id
      WHERE later_revision.result_id = result.id
        AND (later_review.reviewed_at > review.reviewed_at
          OR (later_review.reviewed_at = review.reviewed_at
              AND later_review.id > review.id))
  );
