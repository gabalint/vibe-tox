-- Two health checks (2026-10-13, adverse-scenario defects D-1 and D-2; M4 brief §6.1):
-- `backup_failures` and `database_integrity`. SQLite cannot change a CHECK constraint in
-- place, so the history table is rebuilt with the same rows, index and triggers.

CREATE TABLE ops_health_events_next (
    id          TEXT NOT NULL PRIMARY KEY,
    check_code  TEXT NOT NULL CHECK (check_code IN ('backup_configured', 'backup_age',
        'backup_failures', 'offhost_copy_age', 'offline_copy_age', 'restore_test_age',
        'recovery_kit', 'anchor_lag', 'audit_chain', 'artifact_store', 'database_integrity',
        'disk_free', 'disk_encryption', 'os_support', 'webview', 'listeners', 'keep_awake',
        'listening_gaps', 'unclean_shutdowns', 'clock', 'outbox', 'version',
        'recovery_reconciliation', 'ups', 'cloud_services')),
    state       TEXT NOT NULL CHECK (state IN ('ok', 'warning', 'alert', 'unknown', 'attested')),
    detail_code TEXT CHECK (length(detail_code) BETWEEN 1 AND 64),
    -- Machine text such as "35 min"; never PHI.
    value_text  TEXT CHECK (length(value_text) BETWEEN 1 AND 200),
    observed_at TEXT NOT NULL
) STRICT;

INSERT INTO ops_health_events_next (id, check_code, state, detail_code, value_text, observed_at)
SELECT id, check_code, state, detail_code, value_text, observed_at
FROM ops_health_events;

DROP TABLE ops_health_events;
ALTER TABLE ops_health_events_next RENAME TO ops_health_events;

CREATE INDEX ops_health_events_check ON ops_health_events (check_code, observed_at);

CREATE TRIGGER ops_health_events_no_update BEFORE UPDATE ON ops_health_events
BEGIN
    SELECT RAISE(ABORT, 'health events are append-only');
END;

CREATE TRIGGER ops_health_events_no_delete BEFORE DELETE ON ops_health_events
BEGIN
    SELECT RAISE(ABORT, 'health events are append-only');
END;
