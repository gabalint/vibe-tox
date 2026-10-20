-- The runtime area of the operations module (M4 design brief §2.2, §2.3, §6, §7, §8):
-- operations settings, app runs and listening gaps, health history and attestations,
-- update approvals and version acceptances, and media disposals (REQ-OPS-002, 003, 006,
-- 008, 013; NFR-24, 25, 26). Notes and reasons that are not stored here go in the audit
-- event's reason, so they are not stored twice.

-- Laboratory-set operations values, one row per version (D12). The defaults in
-- `policy.rs` apply until version 1 exists; every later version carries a reason. The
-- CHECKs repeat the ranges of `policy.rs`, the one home of the values, as a store guard.
CREATE TABLE ops_settings_versions (
    version                 INTEGER NOT NULL PRIMARY KEY CHECK (version >= 1),
    backup_interval_minutes INTEGER NOT NULL CHECK (backup_interval_minutes BETWEEN 5 AND 1440),
    keep_recent             INTEGER NOT NULL CHECK (keep_recent BETWEEN 2 AND 96),
    keep_daily              INTEGER NOT NULL CHECK (keep_daily BETWEEN 0 AND 90),
    keep_weekly             INTEGER NOT NULL CHECK (keep_weekly BETWEEN 0 AND 104),
    backup_warn_minutes     INTEGER NOT NULL CHECK (backup_warn_minutes BETWEEN 10 AND 1440),
    backup_alert_minutes    INTEGER NOT NULL CHECK (backup_alert_minutes BETWEEN 30 AND 10080),
    offhost_warn_hours      INTEGER NOT NULL CHECK (offhost_warn_hours BETWEEN 1 AND 168),
    offline_warn_days       INTEGER NOT NULL CHECK (offline_warn_days BETWEEN 1 AND 90),
    restore_test_warn_days  INTEGER NOT NULL CHECK (restore_test_warn_days BETWEEN 7 AND 365),
    kit_confirm_warn_days   INTEGER NOT NULL CHECK (kit_confirm_warn_days BETWEEN 7 AND 365),
    anchor_warn_minutes     INTEGER NOT NULL CHECK (anchor_warn_minutes BETWEEN 10 AND 1440),
    anchor_incident_hours   INTEGER NOT NULL CHECK (anchor_incident_hours BETWEEN 1 AND 168),
    disk_free_warn_gib      INTEGER NOT NULL CHECK (disk_free_warn_gib BETWEEN 1 AND 1024),
    os_support_warn_days    INTEGER NOT NULL CHECK (os_support_warn_days BETWEEN 7 AND 365),
    clock_jump_seconds      INTEGER NOT NULL CHECK (clock_jump_seconds BETWEEN 10 AND 3600),
    reason                  TEXT CHECK (length(reason) > 0),
    -- The acting principal's ID text.
    set_by                  TEXT    NOT NULL CHECK (length(set_by) > 0),
    set_at                  TEXT    NOT NULL,
    CHECK ((version = 1) = (reason IS NULL)),
    CHECK (backup_alert_minutes > backup_warn_minutes),
    CHECK (anchor_incident_hours * 60 > anchor_warn_minutes)
) STRICT;

CREATE TRIGGER ops_settings_versions_next BEFORE INSERT ON ops_settings_versions
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM ops_settings_versions)
BEGIN
    SELECT RAISE(ABORT, 'settings versions count up from 1 without gaps');
END;

CREATE TRIGGER ops_settings_versions_no_update BEFORE UPDATE ON ops_settings_versions
BEGIN
    SELECT RAISE(ABORT, 'settings versions are append-only');
END;

CREATE TRIGGER ops_settings_versions_no_delete BEFORE DELETE ON ops_settings_versions
BEGIN
    SELECT RAISE(ABORT, 'settings versions are append-only');
END;

-- One run of the app, from start to stop (REQ-OPS-013, REQ-OPS-002). A run that is still
-- open when a later run starts stopped uncleanly; the later run records that, and the
-- gap since the earlier run was last seen. Heartbeats move `last_seen_at` while open; the
-- stop and the gap acknowledgement are each written once.
CREATE TABLE ops_app_runs (
    id                  TEXT    NOT NULL PRIMARY KEY,
    started_at          TEXT    NOT NULL,
    last_seen_at        TEXT    NOT NULL,
    stopped_at          TEXT,
    stop_kind           TEXT CHECK (stop_kind IN ('clean', 'quit_with_warnings')),
    app_version         TEXT    NOT NULL CHECK (length(app_version) BETWEEN 1 AND 64),
    os_text             TEXT    NOT NULL CHECK (length(os_text) BETWEEN 1 AND 200),
    webview_text        TEXT CHECK (length(webview_text) BETWEEN 1 AND 200),
    previous_run_id     TEXT REFERENCES ops_app_runs (id),
    previous_unclean    INTEGER NOT NULL CHECK (previous_unclean IN (0, 1)),
    gap_seconds         INTEGER CHECK (gap_seconds >= 0),
    gap_acknowledged_by TEXT,
    gap_acknowledged_at TEXT,
    CHECK ((stopped_at IS NULL) = (stop_kind IS NULL)),
    CHECK ((gap_acknowledged_by IS NULL) = (gap_acknowledged_at IS NULL)),
    CHECK ((previous_run_id IS NULL) = (gap_seconds IS NULL)),
    CHECK (previous_run_id IS NOT NULL OR previous_unclean = 0)
) STRICT;

CREATE INDEX ops_app_runs_started ON ops_app_runs (started_at);

CREATE TRIGGER ops_app_runs_insert_open BEFORE INSERT ON ops_app_runs
WHEN NEW.stopped_at IS NOT NULL OR NEW.gap_acknowledged_at IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'an app run starts open and unacknowledged');
END;

CREATE TRIGGER ops_app_runs_update BEFORE UPDATE ON ops_app_runs
WHEN NEW.id IS NOT OLD.id OR NEW.started_at IS NOT OLD.started_at
    OR NEW.app_version IS NOT OLD.app_version OR NEW.os_text IS NOT OLD.os_text
    OR NEW.webview_text IS NOT OLD.webview_text OR NEW.previous_run_id IS NOT OLD.previous_run_id
    OR NEW.previous_unclean IS NOT OLD.previous_unclean OR NEW.gap_seconds IS NOT OLD.gap_seconds
    OR (OLD.stopped_at IS NOT NULL AND (NEW.stopped_at IS NOT OLD.stopped_at
        OR NEW.stop_kind IS NOT OLD.stop_kind OR NEW.last_seen_at IS NOT OLD.last_seen_at))
    OR (OLD.gap_acknowledged_at IS NOT NULL
        AND (NEW.gap_acknowledged_at IS NOT OLD.gap_acknowledged_at
        OR NEW.gap_acknowledged_by IS NOT OLD.gap_acknowledged_by))
BEGIN
    SELECT RAISE(ABORT, 'an app run is only seen, stopped once and acknowledged once');
END;

CREATE TRIGGER ops_app_runs_no_delete BEFORE DELETE ON ops_app_runs
BEGIN
    SELECT RAISE(ABORT, 'app runs are never deleted');
END;

-- Health check state changes: a row only when a check's state differs from its last row
-- (REQ-OPS-008 maintenance record). Technical writes, not audited.
CREATE TABLE ops_health_events (
    id          TEXT NOT NULL PRIMARY KEY,
    check_code  TEXT NOT NULL CHECK (check_code IN ('backup_configured', 'backup_age',
        'offhost_copy_age', 'offline_copy_age', 'restore_test_age', 'recovery_kit',
        'anchor_lag', 'audit_chain', 'artifact_store', 'disk_free', 'disk_encryption',
        'os_support', 'webview', 'listeners', 'keep_awake', 'listening_gaps',
        'unclean_shutdowns', 'clock', 'outbox', 'version', 'recovery_reconciliation', 'ups',
        'cloud_services')),
    state       TEXT NOT NULL CHECK (state IN ('ok', 'warning', 'alert', 'unknown', 'attested')),
    detail_code TEXT CHECK (length(detail_code) BETWEEN 1 AND 64),
    -- Machine text such as "35 min"; never PHI.
    value_text  TEXT CHECK (length(value_text) BETWEEN 1 AND 200),
    observed_at TEXT NOT NULL
) STRICT;

CREATE INDEX ops_health_events_check ON ops_health_events (check_code, observed_at);

CREATE TRIGGER ops_health_events_no_update BEFORE UPDATE ON ops_health_events
BEGIN
    SELECT RAISE(ABORT, 'health events are append-only');
END;

CREATE TRIGGER ops_health_events_no_delete BEFORE DELETE ON ops_health_events
BEGIN
    SELECT RAISE(ABORT, 'health events are append-only');
END;

-- A person's attestation of a fact the app cannot read, valid for at most 90 days. The
-- note is the audit event's reason.
CREATE TABLE ops_attestations (
    id          TEXT NOT NULL PRIMARY KEY,
    check_code  TEXT NOT NULL CHECK (check_code IN ('disk_encryption', 'ups', 'time_sync',
        'webview_smoke', 'os_support')),
    attested_by TEXT NOT NULL CHECK (length(attested_by) > 0),
    attested_at TEXT NOT NULL,
    valid_until TEXT NOT NULL,
    CHECK (valid_until > attested_at),
    -- At most 90 days (policy.rs ATTESTATION_MAX_DAYS), with a tolerance for julianday().
    CHECK (julianday(valid_until) - julianday(attested_at) <= 90.00001)
) STRICT;

CREATE INDEX ops_attestations_code ON ops_attestations (check_code, attested_at);

CREATE TRIGGER ops_attestations_no_update BEFORE UPDATE ON ops_attestations
BEGIN
    SELECT RAISE(ABORT, 'attestations are append-only');
END;

CREATE TRIGGER ops_attestations_no_delete BEFORE DELETE ON ops_attestations
BEGIN
    SELECT RAISE(ABORT, 'attestations are append-only');
END;

-- An approval to install an application version (REQ-OPS-006, NFR-24). The package digest
-- is computed by the engine from the chosen file. Withdrawn once, before installation.
CREATE TABLE ops_update_approvals (
    id                TEXT    NOT NULL PRIMARY KEY,
    version           TEXT    NOT NULL CHECK (length(version) BETWEEN 5 AND 64),
    package_file_name TEXT    NOT NULL CHECK (length(package_file_name) BETWEEN 1 AND 255),
    package_sha256    TEXT    NOT NULL CHECK (length(package_sha256) = 64),
    risk_class_text   TEXT    NOT NULL CHECK (length(risk_class_text) BETWEEN 1 AND 200),
    pre_update_run_id TEXT    NOT NULL REFERENCES ops_backup_runs (id),
    approved_by       TEXT    NOT NULL CHECK (length(approved_by) > 0),
    approved_at       TEXT    NOT NULL,
    withdrawn_by      TEXT,
    withdrawn_at      TEXT,
    withdraw_reason   TEXT CHECK (length(withdraw_reason) > 0),
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((withdrawn_by IS NULL) = (withdrawn_at IS NULL)),
    CHECK ((withdrawn_at IS NULL) = (withdraw_reason IS NULL))
) STRICT;

CREATE UNIQUE INDEX ops_update_approvals_current ON ops_update_approvals (version)
    WHERE withdrawn_at IS NULL;

-- Succeeded pre-update backup runs with at least one verified copy (§7).
CREATE VIEW ops_v_verified_pre_update_runs AS
SELECT r.id AS run_id, r.finished_at
FROM ops_backup_runs r
WHERE r.kind = 'pre_update' AND r.state = 'succeeded'
    AND EXISTS (SELECT 1 FROM ops_backup_copies c WHERE c.run_id = r.id AND c.state = 'succeeded');

-- Store guard for the rule `updates.rs` checks first: the pre-update backup finished at
-- most 24 hours before the approval (policy.rs PRE_UPDATE_MAX_AGE_HOURS). The small
-- tolerance absorbs the floating-point day arithmetic of julianday().
CREATE TRIGGER ops_update_approvals_insert BEFORE INSERT ON ops_update_approvals
WHEN NEW.withdrawn_at IS NOT NULL OR NEW.revision <> 1
    OR NOT EXISTS (
        SELECT 1 FROM ops_v_verified_pre_update_runs v
        WHERE v.run_id = NEW.pre_update_run_id AND v.finished_at <= NEW.approved_at
            AND julianday(NEW.approved_at) - julianday(v.finished_at) <= 1.00001)
BEGIN
    SELECT RAISE(ABORT, 'an approval needs a verified pre-update backup from the last 24 hours');
END;

CREATE TRIGGER ops_update_approvals_withdraw_once BEFORE UPDATE ON ops_update_approvals
WHEN OLD.withdrawn_at IS NOT NULL OR NEW.withdrawn_at IS NULL
    OR NEW.id IS NOT OLD.id OR NEW.version IS NOT OLD.version
    OR NEW.package_file_name IS NOT OLD.package_file_name
    OR NEW.package_sha256 IS NOT OLD.package_sha256
    OR NEW.risk_class_text IS NOT OLD.risk_class_text
    OR NEW.pre_update_run_id IS NOT OLD.pre_update_run_id
    OR NEW.approved_by IS NOT OLD.approved_by OR NEW.approved_at IS NOT OLD.approved_at
    OR NEW.revision IS NOT OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'an update approval can only be withdrawn, once');
END;

CREATE TRIGGER ops_update_approvals_no_delete BEFORE DELETE ON ops_update_approvals
BEGIN
    SELECT RAISE(ABORT, 'update approvals are never deleted');
END;

-- Acceptance of an installed version for clinical use (REQ-OPS-006). The first installed
-- version needs no approval.
CREATE TABLE ops_version_acceptances (
    version     TEXT NOT NULL PRIMARY KEY CHECK (length(version) BETWEEN 1 AND 64),
    approval_id TEXT REFERENCES ops_update_approvals (id),
    accepted_by TEXT NOT NULL CHECK (length(accepted_by) > 0),
    accepted_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER ops_version_acceptances_no_update BEFORE UPDATE ON ops_version_acceptances
BEGIN
    SELECT RAISE(ABORT, 'version acceptances are append-only');
END;

CREATE TRIGGER ops_version_acceptances_no_delete BEFORE DELETE ON ops_version_acceptances
BEGIN
    SELECT RAISE(ABORT, 'version acceptances are append-only');
END;

-- Disposal or re-use of a host disk or backup medium (REQ-OPS-003). The medium is the
-- laboratory's PHI-free description; the note is the audit event's reason.
CREATE TABLE ops_media_disposals (
    id          TEXT NOT NULL PRIMARY KEY,
    medium      TEXT NOT NULL CHECK (length(medium) BETWEEN 1 AND 200),
    method      TEXT NOT NULL CHECK (method IN ('wiped', 'destroyed', 'reused_after_wipe',
        'returned')),
    identifier  TEXT CHECK (length(identifier) BETWEEN 1 AND 100),
    recorded_by TEXT NOT NULL CHECK (length(recorded_by) > 0),
    recorded_at TEXT NOT NULL
) STRICT;

CREATE INDEX ops_media_disposals_recorded ON ops_media_disposals (recorded_at);

CREATE TRIGGER ops_media_disposals_no_update BEFORE UPDATE ON ops_media_disposals
BEGIN
    SELECT RAISE(ABORT, 'media disposals are append-only');
END;

CREATE TRIGGER ops_media_disposals_no_delete BEFORE DELETE ON ops_media_disposals
BEGIN
    SELECT RAISE(ABORT, 'media disposals are append-only');
END;
