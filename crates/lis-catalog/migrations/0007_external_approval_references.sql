-- Optional external laboratory approval evidence, never clinical report content.
-- Legacy approvals remain without a reference; no historical evidence is inferred.
CREATE TABLE cat_approval_references (
    record_kind TEXT NOT NULL CHECK (record_kind IN ('method_version', 'catalog_review')),
    record_id TEXT NOT NULL,
    external_reference TEXT NOT NULL CHECK (length(trim(external_reference)) BETWEEN 1 AND 1000),
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (record_kind, record_id)
) STRICT;
CREATE TRIGGER cat_approval_reference_method_guard BEFORE INSERT ON cat_approval_references
WHEN NEW.record_kind = 'method_version' AND NOT EXISTS (
    SELECT 1 FROM cat_method_versions WHERE id = NEW.record_id AND status = 'approved'
      AND approved_by = NEW.recorded_by AND effective_from = NEW.recorded_at
)
BEGIN SELECT RAISE(ABORT, 'reference must match method approval'); END;
CREATE TRIGGER cat_approval_reference_review_guard BEFORE INSERT ON cat_approval_references
WHEN NEW.record_kind = 'catalog_review' AND NOT EXISTS (
    SELECT 1 FROM cat_catalog_reviews WHERE id = NEW.record_id
      AND reviewed_by = NEW.recorded_by AND reviewed_at = NEW.recorded_at
)
BEGIN SELECT RAISE(ABORT, 'reference must match catalog review'); END;
CREATE TRIGGER cat_approval_references_no_update BEFORE UPDATE ON cat_approval_references
BEGIN SELECT RAISE(ABORT, 'approval references are immutable'); END;
CREATE TRIGGER cat_approval_references_no_delete BEFORE DELETE ON cat_approval_references
BEGIN SELECT RAISE(ABORT, 'approval references are immutable'); END;
