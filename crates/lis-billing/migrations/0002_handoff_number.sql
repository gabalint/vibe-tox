-- Human handoff numbers for reconciliation (REQ-BIL-002).
-- Existing immutable rows are numbered in their creation order before the
-- append-only guard is restored.

DROP TRIGGER bil_handoffs_no_update;

ALTER TABLE bil_handoffs ADD COLUMN handoff_no INTEGER;

UPDATE bil_handoffs AS handoff
SET handoff_no = (
    SELECT count(*)
    FROM bil_handoffs AS ordered
    WHERE ordered.created_at < handoff.created_at
       OR (ordered.created_at = handoff.created_at AND ordered.id <= handoff.id)
);

CREATE UNIQUE INDEX bil_handoffs_number ON bil_handoffs (handoff_no);

CREATE TRIGGER bil_handoffs_number_required BEFORE INSERT ON bil_handoffs
WHEN NEW.handoff_no IS NULL OR NEW.handoff_no <>
    (SELECT COALESCE(max(handoff_no), 0) + 1 FROM bil_handoffs)
BEGIN SELECT RAISE(ABORT, 'billing handoffs need the next sequential number'); END;

CREATE TRIGGER bil_handoffs_no_update BEFORE UPDATE ON bil_handoffs
BEGIN SELECT RAISE(ABORT, 'billing handoffs are append-only'); END;
