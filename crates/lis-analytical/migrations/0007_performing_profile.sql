-- RPT-05: retain the performing site version at run creation. Legacy runs
-- remain explicitly unknown; wall-clock timestamps cannot establish causality.
ALTER TABLE ana_runs ADD COLUMN laboratory_profile_id TEXT REFERENCES idn_laboratory_profiles(id);

CREATE TRIGGER ana_runs_profile_immutable BEFORE UPDATE OF laboratory_profile_id ON ana_runs
WHEN NEW.laboratory_profile_id IS NOT OLD.laboratory_profile_id
BEGIN
    SELECT RAISE(ABORT, 'run performing profile is immutable');
END;

-- WFREV-01: consumers reconcile all candidate positions in stable run order,
-- including unfinished analytes on closed runs, without the run-list page cap.
DROP VIEW ana_v_positions;
CREATE VIEW ana_v_positions AS
SELECT p.id AS position_id, p.run_id, r.state AS run_state, r.instrument_id,
       r.method_version_id, p.role, p.execution_id, e.container_id, p.sample_label,
       e.prep_batch_id, p.lot_id, e.attempt_kind, e.state AS execution_state,
       r.created_at AS run_created_at
FROM ana_run_positions p
JOIN ana_runs r ON r.id = p.run_id
LEFT JOIN ana_executions e ON e.id = p.execution_id;

-- Factual recovery for an unknown historical site, never a replacement of a pin.
CREATE TABLE ana_run_performing_site_resolutions (
    run_id TEXT NOT NULL PRIMARY KEY REFERENCES ana_runs(id),
    laboratory_profile_id TEXT NOT NULL REFERENCES idn_laboratory_profiles(id),
    run_revision INTEGER NOT NULL CHECK (run_revision >= 1),
    source_reference TEXT NOT NULL,
    reason TEXT NOT NULL,
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL
) STRICT;
CREATE TRIGGER ana_run_performing_site_resolution_unpinned
BEFORE INSERT ON ana_run_performing_site_resolutions
WHEN (SELECT laboratory_profile_id FROM ana_runs WHERE id = NEW.run_id) IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'known performing sites cannot be replaced'); END;
CREATE TRIGGER ana_run_performing_site_resolutions_no_update
BEFORE UPDATE ON ana_run_performing_site_resolutions
BEGIN SELECT RAISE(ABORT, 'performing site resolutions are append-only'); END;
CREATE TRIGGER ana_run_performing_site_resolutions_no_delete
BEFORE DELETE ON ana_run_performing_site_resolutions
BEGIN SELECT RAISE(ABORT, 'performing site resolutions are append-only'); END;
