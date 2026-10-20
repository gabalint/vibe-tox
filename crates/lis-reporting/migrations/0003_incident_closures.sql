-- Closing a preservation incident (NFR-07, REQ-OPS-004): who, when, a
-- resolution code and the reason as entered. Append-only; at most one
-- closure per incident, so a closed incident stays readable as it was.
CREATE TABLE rpt_preservation_incident_closures (
    incident_id TEXT NOT NULL PRIMARY KEY REFERENCES rpt_preservation_incidents (id),
    resolution  TEXT NOT NULL
        CHECK (resolution IN ('artifact_restored_from_backup', 'revision_reissued',
                              'accepted_as_lost')),
    reason      TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    closed_by   TEXT NOT NULL REFERENCES auth_users (id),
    closed_at   TEXT NOT NULL
) STRICT;

CREATE TRIGGER rpt_preservation_incident_closures_no_update
BEFORE UPDATE ON rpt_preservation_incident_closures
BEGIN SELECT RAISE(ABORT, 'rpt_preservation_incident_closures is append-only'); END;
CREATE TRIGGER rpt_preservation_incident_closures_no_delete
BEFORE DELETE ON rpt_preservation_incident_closures
BEGIN SELECT RAISE(ABORT, 'rpt_preservation_incident_closures is append-only'); END;
