-- Historical physical row order is not evidence of clinical commit order.
-- Freeze the migration boundary and exact audit evidence without changing any
-- review, timestamp, audit event or issued report. NULL is explicit uncertainty.
CREATE TABLE res_legacy_review_evidence (
    review_id TEXT NOT NULL PRIMARY KEY REFERENCES res_technical_reviews(id),
    audit_match_count INTEGER NOT NULL CHECK (audit_match_count >= 0),
    clinical_seq INTEGER,
    CHECK (clinical_seq IS NULL OR (audit_match_count = 1 AND clinical_seq > 0))
) STRICT;

INSERT INTO res_legacy_review_evidence(review_id, audit_match_count, clinical_seq)
SELECT review.id, count(event.seq),
       CASE WHEN count(event.seq) = 1
                  AND min(event.principal_kind) = 'user'
                  AND min(event.principal_id) = review.reviewed_by
                  AND min(event.occurred_at) = review.reviewed_at
            THEN min(event.seq) ELSE NULL END
FROM res_technical_reviews review
LEFT JOIN audit_events event
  ON event.stream = 'clinical' AND event.action = 'result.submit_review'
 AND event.outcome = 'succeeded' AND event.record_type = 'technical_review'
 AND event.record_id = review.id
GROUP BY review.id;

CREATE TRIGGER res_legacy_review_evidence_no_insert BEFORE INSERT ON res_legacy_review_evidence
BEGIN SELECT RAISE(ABORT, 'legacy review evidence is frozen'); END;
CREATE TRIGGER res_legacy_review_evidence_no_update BEFORE UPDATE ON res_legacy_review_evidence
BEGIN SELECT RAISE(ABORT, 'legacy review evidence is frozen'); END;
CREATE TRIGGER res_legacy_review_evidence_no_delete BEFORE DELETE ON res_legacy_review_evidence
BEGIN SELECT RAISE(ABORT, 'legacy review evidence is frozen'); END;

-- A review absent from the frozen legacy set was committed after migration.
-- Its existing writer-assigned sequence orders it after every legacy review.
-- Within legacy data only the unambiguous clinical audit sequence is used.
CREATE VIEW res_v_review_causal_order AS
SELECT review.id AS review_id,
       legacy.review_id IS NULL AS after_migration,
       CASE WHEN legacy.review_id IS NULL THEN review.sequence
            ELSE legacy.clinical_seq END AS causal_order
FROM res_technical_reviews review
LEFT JOIN res_legacy_review_evidence legacy ON legacy.review_id = review.id;

DROP VIEW res_v_reviewed_lines;
DROP VIEW res_v_latest_reviews;
CREATE VIEW res_v_latest_reviews AS
SELECT revision.result_id, item.result_revision_id,
       review.id AS review_id, review.decision, review.reviewed_at
FROM res_review_items item
JOIN res_result_revisions revision ON revision.id = item.result_revision_id
JOIN res_technical_reviews review ON review.id = item.review_id
JOIN res_v_review_causal_order ordering ON ordering.review_id = review.id
WHERE ordering.causal_order IS NOT NULL AND NOT EXISTS (
    SELECT 1
    FROM res_review_items later_item
    JOIN res_result_revisions later_revision ON later_revision.id = later_item.result_revision_id
    JOIN res_v_review_causal_order later ON later.review_id = later_item.review_id
    WHERE later_revision.result_id = revision.result_id
      AND (later.after_migration > ordering.after_migration
        OR (later.after_migration = ordering.after_migration
          AND (later.causal_order > ordering.causal_order
            OR later.causal_order IS NULL)))
);

CREATE VIEW res_v_reviewed_lines AS
SELECT revision.specimen_id, result.id AS result_id,
       result.order_component_id, result.analyte_id,
       revision.id AS revision_id, review.review_id, review.reviewed_at
FROM res_results result
JOIN res_result_revisions revision ON revision.id = result.current_revision_id
JOIN res_v_latest_reviews review
  ON review.result_id = result.id AND review.result_revision_id = revision.id
WHERE review.decision = 'approved';
