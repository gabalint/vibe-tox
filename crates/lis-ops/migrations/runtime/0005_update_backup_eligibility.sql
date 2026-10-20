-- A manual backup is retained just like a pre-update backup, so either can serve
-- as the recovery point for an approval (REQ-OPS-006). Scheduled sets may be pruned.
-- Keep the view name so the existing insertion guard uses the relaxed rule.
DROP VIEW ops_v_verified_pre_update_runs;
CREATE VIEW ops_v_verified_pre_update_runs AS
SELECT r.id AS run_id, r.finished_at
FROM ops_backup_runs r
WHERE r.kind IN ('manual', 'pre_update') AND r.state = 'succeeded'
    AND EXISTS (
        SELECT 1 FROM ops_backup_copies c
        WHERE c.run_id = r.id AND c.state = 'succeeded' AND c.pruned_at IS NULL
    );
