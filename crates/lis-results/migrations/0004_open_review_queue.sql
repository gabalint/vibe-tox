-- Indexed current-state projection for the technical-review queue (REQ-OPS-004,
-- NFR-05). Clinical review status and gates remain computed by lis-results.

CREATE TABLE res_unreviewed_results (
    result_id   TEXT NOT NULL PRIMARY KEY REFERENCES res_results (id),
    specimen_id TEXT NOT NULL REFERENCES spc_specimens (id),
    since       TEXT NOT NULL
) STRICT;

CREATE INDEX res_unreviewed_results_queue
    ON res_unreviewed_results (specimen_id, since);

CREATE INDEX res_result_revisions_specimen_result
    ON res_result_revisions (specimen_id, result_id);

CREATE INDEX res_results_current_revision
    ON res_results (current_revision_id);

CREATE INDEX res_review_items_revision
    ON res_review_items (result_revision_id, review_id);

INSERT INTO res_unreviewed_results (result_id, specimen_id, since)
SELECT result.id, current.specimen_id, current.created_at
FROM res_results result
JOIN res_result_revisions current ON current.id = result.current_revision_id
WHERE NOT EXISTS (
    SELECT 1
    FROM res_result_revisions reviewed
    JOIN res_review_items item ON item.result_revision_id = reviewed.id
    WHERE reviewed.result_id = result.id
);

CREATE TRIGGER res_results_track_unreviewed
AFTER UPDATE OF current_revision_id ON res_results
WHEN NOT EXISTS (
    SELECT 1
    FROM res_result_revisions reviewed
    JOIN res_review_items item ON item.result_revision_id = reviewed.id
    WHERE reviewed.result_id = NEW.id
)
BEGIN
    INSERT INTO res_unreviewed_results (result_id, specimen_id, since)
    SELECT NEW.id, current.specimen_id, current.created_at
    FROM res_result_revisions current
    WHERE current.id = NEW.current_revision_id
    ON CONFLICT (result_id) DO UPDATE SET
        specimen_id = excluded.specimen_id,
        since = excluded.since;
END;

CREATE TRIGGER res_review_items_leave_unreviewed_queue
AFTER INSERT ON res_review_items
BEGIN
    DELETE FROM res_unreviewed_results
    WHERE result_id = (
        SELECT result_id FROM res_result_revisions
        WHERE id = NEW.result_revision_id
    );
END;
