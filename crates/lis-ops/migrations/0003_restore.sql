-- Restore tests, restores onto a new installation, the recovery reconciliation and exit
-- exports (M4 design brief §2.2, §3.6–§3.8, §5; REQ-OPS-002, REQ-OPS-003, REQ-OPS-007,
-- REQ-OPS-010, NFR-04, NFR-12, NFR-30). Notes and reasons are audit event reasons and are
-- not stored twice.

-- A restore test of one target's set into a scratch folder that is deleted afterwards.
-- `set_id` is the requested set (NULL: the newest); `tested_set_id` the set it found.
CREATE TABLE ops_restore_tests (
    id                    TEXT    NOT NULL PRIMARY KEY,
    target_id             TEXT    NOT NULL REFERENCES ops_targets (id),
    set_id                TEXT,
    state                 TEXT    NOT NULL
        CHECK (state IN ('requested', 'running', 'passed', 'failed', 'interrupted')),
    requested_by          TEXT    NOT NULL,
    requested_at          TEXT    NOT NULL,
    started_at            TEXT,
    finished_at           TEXT,
    tested_set_id         TEXT,
    set_created_at        TEXT,
    elapsed_ms            INTEGER CHECK (elapsed_ms >= 0),
    failure_code          TEXT    CHECK (failure_code IN ('key_unavailable', 'target_unavailable',
        'set_not_found', 'scratch_failed')),
    anchors_compared      INTEGER CHECK (anchors_compared >= 0),
    anchors_matched       INTEGER CHECK (anchors_matched >= 0),
    anchors_lost_interval INTEGER CHECK (anchors_lost_interval >= 0),
    anchors_unverified    INTEGER CHECK (anchors_unverified >= 0),
    anchors_mismatched    INTEGER CHECK (anchors_mismatched >= 0),
    anchors_missing       INTEGER CHECK (anchors_missing >= 0),
    anchor_conflicts      INTEGER CHECK (anchor_conflicts >= 0),
    anchor_gaps           INTEGER CHECK (anchor_gaps >= 0),
    anchors_unanchored    INTEGER CHECK (anchors_unanchored >= 0),
    revision              INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (failure_code IS NULL OR state = 'failed'),
    CHECK ((state IN ('requested', 'running')) = (finished_at IS NULL)),
    CHECK (state IN ('requested', 'interrupted') OR started_at IS NOT NULL),
    -- The anchor counts are all present (compared) or all absent (not compared).
    CHECK ((anchors_compared IS NULL) = (anchors_matched IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchors_lost_interval IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchors_unverified IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchors_mismatched IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchors_missing IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchor_conflicts IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchor_gaps IS NULL)),
    CHECK ((anchors_compared IS NULL) = (anchors_unanchored IS NULL))
) STRICT;

CREATE INDEX ops_restore_tests_state ON ops_restore_tests (state, requested_at);

CREATE TRIGGER ops_restore_tests_insert BEFORE INSERT ON ops_restore_tests
WHEN NEW.state <> 'requested' OR NEW.revision <> 1
BEGIN
    SELECT RAISE(ABORT, 'a restore test starts requested at revision 1');
END;

CREATE TRIGGER ops_restore_tests_transition BEFORE UPDATE ON ops_restore_tests
WHEN NOT ((OLD.state = 'requested' AND NEW.state IN ('running', 'interrupted'))
        OR (OLD.state = 'running' AND NEW.state IN ('passed', 'failed', 'interrupted')))
    OR NEW.id IS NOT OLD.id OR NEW.target_id IS NOT OLD.target_id
    OR NEW.set_id IS NOT OLD.set_id OR NEW.requested_by IS NOT OLD.requested_by
    OR NEW.requested_at IS NOT OLD.requested_at
    OR (OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at)
    OR NEW.revision IS NOT OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'invalid restore test transition');
END;

CREATE TRIGGER ops_restore_tests_no_delete BEFORE DELETE ON ops_restore_tests
BEGIN
    SELECT RAISE(ABORT, 'restore tests are never deleted');
END;

-- The §3.6 finding codes of a test, with how often each was found.
CREATE TABLE ops_restore_test_findings (
    test_id TEXT    NOT NULL REFERENCES ops_restore_tests (id),
    code    TEXT    NOT NULL CHECK (code IN ('manifest_unreadable', 'key_mismatch',
        'database_digest_mismatch', 'integrity_failed', 'schema_newer_than_app', 'open_failed',
        'chain_broken', 'head_mismatch', 'seal_inconsistent', 'anchor_mismatch',
        'local_events_missing', 'lost_interval', 'artifact_missing', 'artifact_corrupt',
        'referenced_artifact_missing')),
    count   INTEGER NOT NULL CHECK (count >= 1),
    PRIMARY KEY (test_id, code)
) STRICT;

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

-- A restore applied onto this installation, recorded in the restored database at the
-- first start after it was applied. Each row opens a recovery reconciliation. The
-- `applied_event_*` columns name its own `ops.restore_applied` event on the security
-- stream, which anchor verification needs to see covered before trusting the restore.
CREATE TABLE ops_restores (
    id                       TEXT    NOT NULL PRIMARY KEY,
    set_id                   TEXT    NOT NULL,
    set_created_at           TEXT    NOT NULL,
    set_kind                 TEXT    NOT NULL CHECK (set_kind IN ('scheduled', 'manual', 'pre_update')),
    snapshot_sha256          TEXT    NOT NULL CHECK (length(snapshot_sha256) = 64),
    backup_app_version       TEXT    NOT NULL,
    staged_at                TEXT    NOT NULL,
    applied_at               TEXT    NOT NULL,
    recorded_at              TEXT    NOT NULL,
    replaced_database_sha256 TEXT    CHECK (length(replaced_database_sha256) = 64),
    anchors_compared         INTEGER NOT NULL CHECK (anchors_compared >= 0),
    anchors_matched          INTEGER NOT NULL CHECK (anchors_matched >= 0),
    anchors_lost_interval    INTEGER NOT NULL CHECK (anchors_lost_interval >= 0),
    anchors_unverified       INTEGER NOT NULL CHECK (anchors_unverified >= 0),
    anchors_mismatched       INTEGER NOT NULL CHECK (anchors_mismatched >= 0),
    anchors_missing          INTEGER NOT NULL CHECK (anchors_missing >= 0),
    anchor_conflicts         INTEGER NOT NULL CHECK (anchor_conflicts >= 0),
    anchor_gaps              INTEGER NOT NULL CHECK (anchor_gaps >= 0),
    anchors_unanchored       INTEGER NOT NULL CHECK (anchors_unanchored >= 0),
    applied_event_seq        INTEGER NOT NULL CHECK (applied_event_seq >= 1),
    applied_event_hash       TEXT    NOT NULL CHECK (length(applied_event_hash) = 64),
    accession_floor          TEXT    NOT NULL,
    container_floor          TEXT    NOT NULL
) STRICT;

CREATE TRIGGER ops_restores_no_update BEFORE UPDATE ON ops_restores
BEGIN
    SELECT RAISE(ABORT, 'restores are append-only');
END;

CREATE TRIGGER ops_restores_no_delete BEFORE DELETE ON ops_restores
BEGIN
    SELECT RAISE(ABORT, 'restores are append-only');
END;

-- The audit chain heads the restored set held; anchors newer than these are the lost
-- interval.
CREATE TABLE ops_restore_heads (
    restore_id TEXT    NOT NULL REFERENCES ops_restores (id),
    stream     TEXT    NOT NULL CHECK (stream IN ('clinical', 'security', 'access')),
    seq        INTEGER NOT NULL CHECK (seq >= 0),
    hash       TEXT    NOT NULL CHECK (length(hash) = 64),
    PRIMARY KEY (restore_id, stream)
) STRICT;

CREATE TRIGGER ops_restore_heads_no_update BEFORE UPDATE ON ops_restore_heads
BEGIN
    SELECT RAISE(ABORT, 'restore heads are append-only');
END;

CREATE TRIGGER ops_restore_heads_no_delete BEFORE DELETE ON ops_restore_heads
BEGIN
    SELECT RAISE(ABORT, 'restore heads are append-only');
END;

-- The checklist items attested when a recovery reconciliation is closed (§3.8).
CREATE TABLE ops_recovery_attestations (
    restore_id TEXT NOT NULL REFERENCES ops_restores (id),
    item_code  TEXT NOT NULL CHECK (item_code IN ('production_computer', 'lost_interval_listed',
        'numbers_checked', 'analyzer_results_resent', 'reports_reconciled', 'orders_reentered',
        'billing_reconciled', 'accounts_reapplied', 'configuration_reapplied', 'anchors_reviewed',
        'labels_waiting')),
    PRIMARY KEY (restore_id, item_code)
) STRICT;

-- The closure of a reconciliation; the closing note is the audit event's reason.
CREATE TABLE ops_recovery_closures (
    restore_id TEXT NOT NULL PRIMARY KEY REFERENCES ops_restores (id),
    closed_by  TEXT NOT NULL,
    closed_at  TEXT NOT NULL
) STRICT;

CREATE TRIGGER ops_recovery_attestations_open BEFORE INSERT ON ops_recovery_attestations
WHEN EXISTS (SELECT 1 FROM ops_recovery_closures WHERE restore_id = NEW.restore_id)
BEGIN
    SELECT RAISE(ABORT, 'a closed recovery reconciliation takes no more attestations');
END;

CREATE TRIGGER ops_recovery_closures_complete BEFORE INSERT ON ops_recovery_closures
WHEN (SELECT count(*) FROM ops_recovery_attestations WHERE restore_id = NEW.restore_id) <> 11
BEGIN
    SELECT RAISE(ABORT, 'every checklist item is attested before closing');
END;

CREATE TRIGGER ops_recovery_attestations_no_update BEFORE UPDATE ON ops_recovery_attestations
BEGIN
    SELECT RAISE(ABORT, 'recovery attestations are append-only');
END;

CREATE TRIGGER ops_recovery_attestations_no_delete BEFORE DELETE ON ops_recovery_attestations
BEGIN
    SELECT RAISE(ABORT, 'recovery attestations are append-only');
END;

CREATE TRIGGER ops_recovery_closures_no_update BEFORE UPDATE ON ops_recovery_closures
BEGIN
    SELECT RAISE(ABORT, 'recovery closures are append-only');
END;

CREATE TRIGGER ops_recovery_closures_no_delete BEFORE DELETE ON ops_recovery_closures
BEGIN
    SELECT RAISE(ABORT, 'recovery closures are append-only');
END;

-- Restores whose reconciliation is still open: the outbound gate (§3.8) reads this.
CREATE VIEW ops_v_recovery_open AS
SELECT r.id AS restore_id, r.applied_at, r.set_created_at
FROM ops_restores r
WHERE NOT EXISTS (SELECT 1 FROM ops_recovery_closures c WHERE c.restore_id = r.id);

-- One exit export (§5). The chosen folder's path is never stored; `folder_label` is its
-- last name component, for display. The reason is the request event's reason.
-- `artifact_problems` counts artifacts the package lacks (listed in its manifest).
CREATE TABLE ops_exit_exports (
    id              TEXT    NOT NULL PRIMARY KEY,
    state           TEXT    NOT NULL
        CHECK (state IN ('requested', 'running', 'succeeded', 'failed', 'interrupted')),
    requested_by    TEXT    NOT NULL,
    requested_at    TEXT    NOT NULL,
    folder_label    TEXT    NOT NULL,
    package_name    TEXT,
    manifest_sha256 TEXT    CHECK (length(manifest_sha256) = 64),
    database_bytes  INTEGER CHECK (database_bytes >= 0),
    artifact_count  INTEGER CHECK (artifact_count >= 0),
    artifact_problems INTEGER CHECK (artifact_problems >= 0),
    failure_code    TEXT    CHECK (failure_code IN ('folder_unusable', 'snapshot_failed',
        'snapshot_not_intact', 'artifact_listing_failed', 'write_failed', 'readback_mismatch')),
    finished_at     TEXT,
    revision        INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'failed') = (failure_code IS NOT NULL)),
    CHECK (state <> 'succeeded' OR manifest_sha256 IS NOT NULL),
    CHECK ((state IN ('requested', 'running')) = (finished_at IS NULL))
) STRICT;

CREATE INDEX ops_exit_exports_state ON ops_exit_exports (state, requested_at);

CREATE TRIGGER ops_exit_exports_insert BEFORE INSERT ON ops_exit_exports
WHEN NEW.state <> 'requested' OR NEW.revision <> 1
BEGIN
    SELECT RAISE(ABORT, 'an exit export starts requested at revision 1');
END;

CREATE TRIGGER ops_exit_exports_transition BEFORE UPDATE ON ops_exit_exports
WHEN NOT ((OLD.state = 'requested' AND NEW.state IN ('running', 'interrupted'))
        OR (OLD.state = 'running' AND NEW.state IN ('succeeded', 'failed', 'interrupted')))
    OR NEW.id IS NOT OLD.id OR NEW.requested_by IS NOT OLD.requested_by
    OR NEW.requested_at IS NOT OLD.requested_at OR NEW.folder_label IS NOT OLD.folder_label
    OR NEW.revision IS NOT OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'invalid exit export transition');
END;

CREATE TRIGGER ops_exit_exports_no_delete BEFORE DELETE ON ops_exit_exports
BEGIN
    SELECT RAISE(ABORT, 'exit exports are never deleted');
END;
