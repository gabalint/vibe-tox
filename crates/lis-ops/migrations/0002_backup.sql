-- Backup targets, backup runs and their copies, and recovery kits (M4 design brief §2.2,
-- §3.1–§3.4; REQ-OPS-007, NFR-04, NFR-11, NFR-20, NFR-30). The restore, recovery and exit
-- export tables come in a later migration of this module.

-- A folder the laboratory chose for encrypted copies and audit anchors. Its
-- `VibeToxLIS/target.json` marker names this row's ID. The path comes from an OS dialog
-- and never goes into audit details or logs. Retired once, never deleted.
CREATE TABLE ops_targets (
    id            TEXT    NOT NULL PRIMARY KEY,
    label         TEXT    NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
    kind          TEXT    NOT NULL CHECK (kind IN ('local_disk', 'removable', 'network_share')),
    purpose       TEXT    NOT NULL CHECK (purpose IN ('backup_and_anchors', 'anchors_only')),
    path          TEXT    NOT NULL CHECK (length(path) > 0),
    created_by    TEXT    NOT NULL,
    created_at    TEXT    NOT NULL,
    retired_by    TEXT,
    retired_at    TEXT,
    retire_reason TEXT,
    revision      INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

CREATE UNIQUE INDEX ops_targets_active_path ON ops_targets (path) WHERE retired_at IS NULL;

CREATE TRIGGER ops_targets_insert_active BEFORE INSERT ON ops_targets
WHEN NEW.retired_at IS NOT NULL OR NEW.revision <> 1
BEGIN
    SELECT RAISE(ABORT, 'a backup target starts active at revision 1');
END;

CREATE TRIGGER ops_targets_retire_once BEFORE UPDATE ON ops_targets
WHEN OLD.retired_at IS NOT NULL OR NEW.retired_at IS NULL
    OR NEW.id IS NOT OLD.id OR NEW.label IS NOT OLD.label OR NEW.kind IS NOT OLD.kind
    OR NEW.purpose IS NOT OLD.purpose OR NEW.path IS NOT OLD.path
    OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
    OR NEW.revision IS NOT OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'a backup target can only be retired, once');
END;

CREATE TRIGGER ops_targets_no_delete BEFORE DELETE ON ops_targets
BEGIN
    SELECT RAISE(ABORT, 'backup targets are never deleted');
END;

-- One backup run: requested (manual, pre-update) or scheduled, then running, then
-- succeeded, failed or interrupted. Scheduled runs start as running.
CREATE TABLE ops_backup_runs (
    id                TEXT    NOT NULL PRIMARY KEY,
    kind              TEXT    NOT NULL CHECK (kind IN ('scheduled', 'manual', 'pre_update')),
    state             TEXT    NOT NULL
        CHECK (state IN ('requested', 'running', 'succeeded', 'failed', 'interrupted')),
    requested_by      TEXT,
    requested_at      TEXT    NOT NULL,
    started_at        TEXT,
    finished_at       TEXT,
    snapshot_sha256   TEXT    CHECK (length(snapshot_sha256) = 64),
    snapshot_bytes    INTEGER CHECK (snapshot_bytes >= 0),
    artifact_count    INTEGER CHECK (artifact_count >= 0),
    artifact_problems INTEGER NOT NULL DEFAULT 0 CHECK (artifact_problems >= 0),
    failure_code      TEXT    CHECK (failure_code IN ('key_unavailable', 'no_target_available',
        'insufficient_space', 'snapshot_failed', 'snapshot_not_intact', 'artifact_listing_failed',
        'all_copies_failed')),
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (kind = 'scheduled' OR requested_by IS NOT NULL),
    CHECK ((state = 'failed') = (failure_code IS NOT NULL)),
    CHECK (state <> 'succeeded' OR snapshot_sha256 IS NOT NULL),
    CHECK (state IN ('requested', 'interrupted') OR started_at IS NOT NULL),
    CHECK ((state IN ('requested', 'running')) = (finished_at IS NULL))
) STRICT;

CREATE INDEX ops_backup_runs_state ON ops_backup_runs (state, requested_at);

CREATE TRIGGER ops_backup_runs_insert BEFORE INSERT ON ops_backup_runs
WHEN NEW.revision <> 1
    OR NEW.state IS NOT (CASE NEW.kind WHEN 'scheduled' THEN 'running' ELSE 'requested' END)
BEGIN
    SELECT RAISE(ABORT, 'scheduled runs start running, requested runs start requested');
END;

CREATE TRIGGER ops_backup_runs_transition BEFORE UPDATE ON ops_backup_runs
WHEN NOT ((OLD.state = 'requested' AND NEW.state IN ('running', 'interrupted'))
        OR (OLD.state = 'running' AND NEW.state IN ('succeeded', 'failed', 'interrupted')))
    OR NEW.id IS NOT OLD.id OR NEW.kind IS NOT OLD.kind
    OR NEW.requested_by IS NOT OLD.requested_by OR NEW.requested_at IS NOT OLD.requested_at
    OR (OLD.started_at IS NOT NULL AND NEW.started_at IS NOT OLD.started_at)
    OR NEW.revision IS NOT OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'invalid backup run transition');
END;

CREATE TRIGGER ops_backup_runs_no_delete BEFORE DELETE ON ops_backup_runs
BEGIN
    SELECT RAISE(ABORT, 'backup runs are never deleted');
END;

-- The audit chain heads a run's snapshot holds, read from the snapshot copy.
CREATE TABLE ops_backup_run_heads (
    run_id TEXT    NOT NULL REFERENCES ops_backup_runs (id),
    stream TEXT    NOT NULL CHECK (stream IN ('clinical', 'security', 'access')),
    seq    INTEGER NOT NULL CHECK (seq >= 0),
    hash   TEXT    NOT NULL CHECK (length(hash) = 64),
    PRIMARY KEY (run_id, stream)
) STRICT;

CREATE TRIGGER ops_backup_run_heads_no_update BEFORE UPDATE ON ops_backup_run_heads
BEGIN
    SELECT RAISE(ABORT, 'backup run heads are append-only');
END;

CREATE TRIGGER ops_backup_run_heads_no_delete BEFORE DELETE ON ops_backup_run_heads
BEGIN
    SELECT RAISE(ABORT, 'backup run heads are append-only');
END;

-- One run's copy to one target, written once the copy is verified or has failed (§3.5).
-- A succeeded copy is pruned at most once.
CREATE TABLE ops_backup_copies (
    run_id        TEXT    NOT NULL REFERENCES ops_backup_runs (id),
    target_id     TEXT    NOT NULL REFERENCES ops_targets (id),
    set_id        TEXT,
    state         TEXT    NOT NULL CHECK (state IN ('succeeded', 'failed')),
    failure_code  TEXT    CHECK (failure_code IN ('write_failed', 'readback_mismatch')),
    written_bytes INTEGER NOT NULL CHECK (written_bytes >= 0),
    verified_at   TEXT,
    pruned_at     TEXT,
    PRIMARY KEY (run_id, target_id),
    CHECK ((state = 'succeeded') = (verified_at IS NOT NULL AND set_id IS NOT NULL)),
    CHECK ((state = 'failed') = (failure_code IS NOT NULL)),
    CHECK (pruned_at IS NULL OR state = 'succeeded')
) STRICT;

CREATE INDEX ops_backup_copies_target ON ops_backup_copies (target_id, state);

CREATE TRIGGER ops_backup_copies_insert BEFORE INSERT ON ops_backup_copies
WHEN NEW.pruned_at IS NOT NULL
    OR (SELECT state FROM ops_backup_runs WHERE id = NEW.run_id) IS NOT 'running'
BEGIN
    SELECT RAISE(ABORT, 'copies are recorded, unpruned, while their run is running');
END;

CREATE TRIGGER ops_backup_copies_prune_once BEFORE UPDATE ON ops_backup_copies
WHEN OLD.pruned_at IS NOT NULL OR NEW.pruned_at IS NULL
    OR NEW.run_id IS NOT OLD.run_id OR NEW.target_id IS NOT OLD.target_id
    OR NEW.set_id IS NOT OLD.set_id OR NEW.state IS NOT OLD.state
    OR NEW.failure_code IS NOT OLD.failure_code OR NEW.written_bytes IS NOT OLD.written_bytes
    OR NEW.verified_at IS NOT OLD.verified_at
BEGIN
    SELECT RAISE(ABORT, 'a backup copy can only be marked pruned, once');
END;

CREATE TRIGGER ops_backup_copies_no_delete BEFORE DELETE ON ops_backup_copies
BEGIN
    SELECT RAISE(ABORT, 'backup copies are never deleted');
END;

-- Each export of the recovery kit. The kit text holds the installation key; this row
-- holds only its key ID and a digest of the backup locations listed in the kit.
CREATE TABLE ops_recovery_kits (
    id                        TEXT    NOT NULL PRIMARY KEY,
    kit_no                    INTEGER NOT NULL UNIQUE CHECK (kit_no >= 1),
    key_id                    TEXT    NOT NULL CHECK (length(key_id) = 32),
    targets_digest            TEXT    NOT NULL CHECK (length(targets_digest) = 64),
    disk_encryption_reference TEXT    NOT NULL CHECK (length(disk_encryption_reference) > 0),
    file_name                 TEXT    NOT NULL CHECK (length(file_name) > 0),
    exported_by               TEXT    NOT NULL,
    exported_at               TEXT    NOT NULL
) STRICT;

CREATE TRIGGER ops_recovery_kits_number BEFORE INSERT ON ops_recovery_kits
WHEN NEW.kit_no <> (SELECT coalesce(max(kit_no), 0) + 1 FROM ops_recovery_kits)
BEGIN
    SELECT RAISE(ABORT, 'kit numbers count up from 1 without gaps');
END;

CREATE TRIGGER ops_recovery_kits_no_update BEFORE UPDATE ON ops_recovery_kits
BEGIN
    SELECT RAISE(ABORT, 'recovery kit exports are append-only');
END;

CREATE TRIGGER ops_recovery_kits_no_delete BEFORE DELETE ON ops_recovery_kits
BEGIN
    SELECT RAISE(ABORT, 'recovery kit exports are append-only');
END;

-- Confirmations that the latest kit is stored off this computer; where it is stored is
-- the audit event's reason.
CREATE TABLE ops_kit_confirmations (
    id           TEXT NOT NULL PRIMARY KEY,
    kit_id       TEXT NOT NULL REFERENCES ops_recovery_kits (id),
    confirmed_by TEXT NOT NULL,
    confirmed_at TEXT NOT NULL
) STRICT;

CREATE INDEX ops_kit_confirmations_kit ON ops_kit_confirmations (kit_id, confirmed_at);

CREATE TRIGGER ops_kit_confirmations_no_update BEFORE UPDATE ON ops_kit_confirmations
BEGIN
    SELECT RAISE(ABORT, 'kit confirmations are append-only');
END;

CREATE TRIGGER ops_kit_confirmations_no_delete BEFORE DELETE ON ops_kit_confirmations
BEGIN
    SELECT RAISE(ABORT, 'kit confirmations are append-only');
END;
