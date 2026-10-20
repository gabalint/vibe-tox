-- RPT-08: laboratory-approved report text is part of the immutable method.
-- Absent legacy information stays absent; no clinical content is synthesized.
ALTER TABLE cat_method_versions ADD COLUMN report_notes TEXT;
ALTER TABLE cat_method_version_analytes ADD COLUMN reference_interval TEXT;
ALTER TABLE cat_method_version_analytes ADD COLUMN range_policy TEXT NOT NULL DEFAULT 'report_only'
    CHECK (range_policy IN ('report_only', 'require_within_range'));

CREATE TRIGGER cat_method_report_notes_frozen BEFORE UPDATE OF report_notes ON cat_method_versions
WHEN OLD.status <> 'draft' AND NEW.report_notes IS NOT OLD.report_notes
BEGIN
    SELECT RAISE(ABORT, 'approved method report notes are immutable');
END;
