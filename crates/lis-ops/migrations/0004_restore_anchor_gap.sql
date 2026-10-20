-- A restore test's `anchor_gap` finding gets its own row like every other §3.6 code
-- (M4 design brief §3.6; REQ-OPS-002, NFR-30). SQLite cannot change a CHECK constraint,
-- so the findings table is rebuilt with the same columns, rows and triggers; only the
-- code list gains `anchor_gap`. Nothing references this table, and the append-only
-- triggers go with the old table, so the copy needs no exception to them.

CREATE TABLE ops_restore_test_findings_new (
    test_id TEXT    NOT NULL REFERENCES ops_restore_tests (id),
    code    TEXT    NOT NULL CHECK (code IN ('manifest_unreadable', 'key_mismatch',
        'database_digest_mismatch', 'integrity_failed', 'schema_newer_than_app', 'open_failed',
        'chain_broken', 'head_mismatch', 'seal_inconsistent', 'anchor_mismatch',
        'local_events_missing', 'lost_interval', 'anchor_gap', 'artifact_missing',
        'artifact_corrupt', 'referenced_artifact_missing')),
    count   INTEGER NOT NULL CHECK (count >= 1),
    PRIMARY KEY (test_id, code)
) STRICT;

INSERT INTO ops_restore_test_findings_new (test_id, code, count)
SELECT test_id, code, count FROM ops_restore_test_findings;

DROP TABLE ops_restore_test_findings;

ALTER TABLE ops_restore_test_findings_new RENAME TO ops_restore_test_findings;

CREATE TRIGGER ops_restore_test_findings_insert BEFORE INSERT ON ops_restore_test_findings
WHEN (SELECT state FROM ops_restore_tests WHERE id = NEW.test_id) IS NOT 'running'
BEGIN
    SELECT RAISE(ABORT, 'findings are recorded while their test is running');
END;

CREATE TRIGGER ops_restore_test_findings_no_update BEFORE UPDATE ON ops_restore_test_findings
BEGIN
    SELECT RAISE(ABORT, 'restore test findings are append-only');
END;

CREATE TRIGGER ops_restore_test_findings_no_delete BEFORE DELETE ON ops_restore_test_findings
BEGIN
    SELECT RAISE(ABORT, 'restore test findings are append-only');
END;
