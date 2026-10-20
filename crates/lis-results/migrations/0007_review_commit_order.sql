-- A host-clock correction must not change which technical review is current.
-- Freeze existing insertion order in an explicit column before VACUUM or export
-- can renumber rowids; future reviews allocate the next sequence in the writer
-- transaction. Timestamps remain the recorded observation time.
ALTER TABLE res_technical_reviews ADD COLUMN sequence INTEGER NOT NULL DEFAULT 0;

DROP TRIGGER res_technical_reviews_no_update;
UPDATE res_technical_reviews SET sequence = rowid;
CREATE TRIGGER res_technical_reviews_no_update BEFORE UPDATE ON res_technical_reviews
BEGIN SELECT RAISE(ABORT, 'technical reviews are append-only'); END;

CREATE UNIQUE INDEX res_technical_reviews_sequence ON res_technical_reviews (sequence);
CREATE TRIGGER res_technical_reviews_sequence BEFORE INSERT ON res_technical_reviews
WHEN NEW.sequence <= (SELECT coalesce(max(sequence), 0) FROM res_technical_reviews)
BEGIN SELECT RAISE(ABORT, 'technical review sequence must increase'); END;

-- This is the one structural definition of the latest review of a result.
-- Dynamic dependencies and audit attribution are still checked in Rust.
CREATE VIEW res_v_latest_reviews AS
SELECT revision.result_id,
       item.result_revision_id,
       review.id AS review_id,
       review.decision,
       review.reviewed_at
FROM res_review_items item
JOIN res_result_revisions revision ON revision.id = item.result_revision_id
JOIN res_technical_reviews review ON review.id = item.review_id
WHERE NOT EXISTS (
    SELECT 1
    FROM res_review_items later_item
    JOIN res_result_revisions later_revision
      ON later_revision.id = later_item.result_revision_id
    JOIN res_technical_reviews later_review ON later_review.id = later_item.review_id
    WHERE later_revision.result_id = revision.result_id
      AND later_review.sequence > review.sequence
);

DROP VIEW res_v_reviewed_lines;
CREATE VIEW res_v_reviewed_lines AS
SELECT revision.specimen_id,
       result.id AS result_id,
       result.order_component_id,
       result.analyte_id,
       revision.id AS revision_id,
       review.review_id,
       review.reviewed_at
FROM res_results result
JOIN res_result_revisions revision ON revision.id = result.current_revision_id
JOIN res_v_latest_reviews review
  ON review.result_id = result.id AND review.result_revision_id = revision.id
WHERE review.decision = 'approved';
