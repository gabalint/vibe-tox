-- Report PDF connections (Interfaces, ticket #4555 step 2). A released report
-- revision becomes a message; its later revisions and its withdrawal notice go
-- first to every connection that had an earlier revision (REQ-RPT-013), and an
-- earlier revision not written yet is superseded instead of written. Rebuild the
-- CHECK-constrained tables with the foreign-key checked migration procedure of
-- 0002 and 0004; existing rows keep their values.

CREATE TABLE ifc_partner_connections_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    partner TEXT NOT NULL,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json', 'report_pdf')),
    protocol TEXT NOT NULL CHECK (protocol IN ('folder')),
    folder TEXT NOT NULL,
    folder_set_by TEXT NOT NULL,
    rules TEXT NOT NULL,
    schedule TEXT NOT NULL CHECK (schedule IN ('immediately', 'every_minutes', 'daily_at')),
    schedule_minutes INTEGER CHECK (schedule_minutes BETWEEN 1 AND 1440),
    schedule_time TEXT,
    file_name_pattern TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((schedule = 'every_minutes') = (schedule_minutes IS NOT NULL)),
    CHECK ((schedule = 'daily_at') = (schedule_time IS NOT NULL))
) STRICT;

INSERT INTO ifc_partner_connections_next
SELECT id, name, partner, message_type, protocol, folder, folder_set_by, rules, schedule,
       schedule_minutes, schedule_time, file_name_pattern, active, revision
FROM ifc_partner_connections;

DROP TABLE ifc_partner_connections;
ALTER TABLE ifc_partner_connections_next RENAME TO ifc_partner_connections;

CREATE UNIQUE INDEX ifc_partner_connections_name
    ON ifc_partner_connections (name COLLATE NOCASE);

-- A source record gives at most one message of each type, so a replay has no
-- second effect (REQ-INT-003). A report's revisions and its withdrawal share
-- the report's ID (`source_group`) and number (`source_label`).
CREATE TABLE ifc_partner_messages_next (
    id TEXT PRIMARY KEY,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json', 'report_pdf')),
    source_kind TEXT NOT NULL
        CHECK (source_kind IN ('billing_handoff', 'report_revision', 'report_withdrawal', 'test')),
    source_id TEXT,
    source_number INTEGER,
    source_group TEXT,
    source_label TEXT,
    artifact_sha256 TEXT NOT NULL,
    routing_facts TEXT NOT NULL,
    checked_against INTEGER NOT NULL,
    unrouted INTEGER NOT NULL CHECK (unrouted IN (0, 1)),
    created_at TEXT NOT NULL,
    CHECK ((source_kind = 'test') = (source_id IS NULL)),
    CHECK ((source_kind IN ('report_revision', 'report_withdrawal'))
        = (source_group IS NOT NULL AND source_label IS NOT NULL)),
    CHECK (source_kind = 'test'
        OR (source_kind = 'billing_handoff') = (message_type = 'billing_json')),
    UNIQUE (message_type, source_kind, source_id)
) STRICT;

INSERT INTO ifc_partner_messages_next (
    id, message_type, source_kind, source_id, source_number, artifact_sha256, routing_facts,
    checked_against, unrouted, created_at
)
SELECT id, message_type, source_kind, source_id, source_number, artifact_sha256, routing_facts,
       checked_against, unrouted, created_at
FROM ifc_partner_messages;

DROP TABLE ifc_partner_messages;
ALTER TABLE ifc_partner_messages_next RENAME TO ifc_partner_messages;

CREATE INDEX ifc_partner_messages_unrouted
    ON ifc_partner_messages (unrouted, created_at);
CREATE INDEX ifc_partner_messages_group
    ON ifc_partner_messages (source_group) WHERE source_group IS NOT NULL;

-- `follow_up`: the connection had an earlier revision of the report. A
-- `superseded` delivery was replaced by a newer revision or a withdrawal before
-- it was written, and is never written.
CREATE TABLE ifc_partner_deliveries_next (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES ifc_partner_messages (id),
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    reason TEXT NOT NULL CHECK (reason IN ('routed', 'follow_up', 'sent_once', 'test')),
    created_at TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('waiting', 'written', 'failed', 'superseded')),
    attempts INTEGER NOT NULL CHECK (attempts >= 0),
    next_attempt_at TEXT,
    last_attempt_at TEXT,
    last_error_code TEXT,
    written_path TEXT,
    written_at TEXT,
    CHECK ((state = 'written') = (written_path IS NOT NULL AND written_at IS NOT NULL)),
    UNIQUE (message_id, connection_id)
) STRICT;

INSERT INTO ifc_partner_deliveries_next
SELECT id, message_id, connection_id, reason, created_at, state, attempts, next_attempt_at,
       last_attempt_at, last_error_code, written_path, written_at
FROM ifc_partner_deliveries;

DROP TABLE ifc_partner_deliveries;
ALTER TABLE ifc_partner_deliveries_next RENAME TO ifc_partner_deliveries;

CREATE INDEX ifc_partner_deliveries_state
    ON ifc_partner_deliveries (state, next_attempt_at);
CREATE INDEX ifc_partner_deliveries_connection
    ON ifc_partner_deliveries (connection_id, last_attempt_at);

CREATE TRIGGER ifc_partner_messages_no_update
BEFORE UPDATE ON ifc_partner_messages
BEGIN
    SELECT RAISE(ABORT, 'messages are append-only');
END;

CREATE TRIGGER ifc_partner_messages_no_delete
BEFORE DELETE ON ifc_partner_messages
BEGIN
    SELECT RAISE(ABORT, 'messages are append-only');
END;

CREATE TRIGGER ifc_partner_deliveries_no_delete
BEFORE DELETE ON ifc_partner_deliveries
BEGIN
    SELECT RAISE(ABORT, 'deliveries are kept');
END;

-- Report revisions that went, or are going, to a connection: Reporting leaves
-- them out of its "Reports without a destination" queue.
CREATE VIEW ifc_v_partner_report_revisions AS
SELECT DISTINCT m.source_id AS revision_id
FROM ifc_partner_messages m
JOIN ifc_partner_deliveries d ON d.message_id = m.id
WHERE m.source_kind = 'report_revision' AND d.state <> 'superseded';
