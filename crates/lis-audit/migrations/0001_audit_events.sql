-- Append-only, hash-chained audit trail (REQ-OPS-001, NFR-18, ADR 0008).
-- Each stream has its own sequence and chain. The hash input format is
-- documented in crates/lis-audit/src/chain.rs; never change it in place.
CREATE TABLE audit_events (
    stream         TEXT    NOT NULL CHECK (stream IN ('clinical', 'security', 'access')),
    seq            INTEGER NOT NULL CHECK (seq >= 1),
    prev_hash      TEXT    NOT NULL CHECK (length(prev_hash) = 64),
    hash           TEXT    NOT NULL CHECK (length(hash) = 64),
    hash_alg       TEXT    NOT NULL,
    occurred_at    TEXT    NOT NULL,
    recorded_at    TEXT    NOT NULL,
    -- 'partner' (interface partners, security-audit-logging.md) has no Actor yet;
    -- listing it now avoids rebuilding this append-only table later.
    principal_kind TEXT    NOT NULL CHECK (principal_kind IN ('user', 'instrument', 'partner', 'system')),
    principal_id   TEXT    NOT NULL,
    session_id     TEXT,
    workstation_id TEXT,
    action         TEXT    NOT NULL,
    outcome        TEXT    NOT NULL CHECK (outcome IN ('succeeded', 'denied', 'failed')),
    record_type    TEXT,
    record_id      TEXT,
    old_revision   INTEGER,
    new_revision   INTEGER,
    reason         TEXT,
    authority      TEXT,
    correlation_id TEXT    NOT NULL,
    details        TEXT    NOT NULL,
    PRIMARY KEY (stream, seq)
) STRICT;

-- Stops application bugs, not the OS administrator; the chain and off-host
-- anchors (M4) cover that.
CREATE TRIGGER audit_events_no_update BEFORE UPDATE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only');
END;

CREATE TRIGGER audit_events_no_delete BEFORE DELETE ON audit_events
BEGIN
    SELECT RAISE(ABORT, 'audit_events is append-only');
END;
