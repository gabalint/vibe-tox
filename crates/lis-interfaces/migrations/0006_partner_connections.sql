-- Connections that send messages to outside systems (Interfaces menu): the
-- message type, the protocol and its folder, routing rules, schedule, file
-- name pattern, the destination check and every change. Messages, their
-- deliveries per connection and the decisions on unrouted messages are kept so
-- nothing sent or left unsent is lost (REQ-RPT-003, REQ-RPT-011, REQ-RPT-012,
-- REQ-INT-002, REQ-INT-003, REQ-BIL-002).

CREATE TABLE ifc_partner_connections (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    partner TEXT NOT NULL,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json')),
    protocol TEXT NOT NULL CHECK (protocol IN ('folder')),
    folder TEXT NOT NULL,
    -- Who chose the current folder: verifying it is the second step of that duty.
    folder_set_by TEXT NOT NULL,
    -- JSON array of {field, condition, value}; all must match (AND).
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

CREATE UNIQUE INDEX ifc_partner_connections_name
    ON ifc_partner_connections (name COLLATE NOCASE);

-- One row per revision: the settings as saved, who, when and why.
CREATE TABLE ifc_partner_connection_changes (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    revision INTEGER NOT NULL,
    change TEXT NOT NULL CHECK (change IN ('created', 'updated', 'started', 'stopped', 'verified')),
    settings TEXT NOT NULL,
    reason TEXT,
    changed_by TEXT NOT NULL,
    changed_at TEXT NOT NULL,
    PRIMARY KEY (connection_id, revision)
) STRICT;

-- A destination check covers the folder it names; choosing another folder
-- needs a new check before patient data goes there (REQ-RPT-012).
CREATE TABLE ifc_partner_verifications (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    folder TEXT NOT NULL,
    method TEXT NOT NULL,
    verified_by TEXT NOT NULL,
    verified_at TEXT NOT NULL,
    self_approval INTEGER NOT NULL CHECK (self_approval IN (0, 1))
) STRICT;

CREATE INDEX ifc_partner_verifications_connection
    ON ifc_partner_verifications (connection_id, folder);

-- One outgoing message. A source record gives at most one message, so a
-- replay has no second effect (REQ-INT-003).
CREATE TABLE ifc_partner_messages (
    id TEXT PRIMARY KEY,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json')),
    source_kind TEXT NOT NULL CHECK (source_kind IN ('billing_handoff', 'test')),
    source_id TEXT,
    source_number INTEGER,
    artifact_sha256 TEXT NOT NULL,
    -- JSON: the client, practitioner, location and test values rules match.
    routing_facts TEXT NOT NULL,
    -- How many connections of the type the rules were checked against.
    checked_against INTEGER NOT NULL,
    unrouted INTEGER NOT NULL CHECK (unrouted IN (0, 1)),
    created_at TEXT NOT NULL,
    CHECK ((source_kind = 'test') = (source_id IS NULL)),
    UNIQUE (source_kind, source_id)
) STRICT;

CREATE INDEX ifc_partner_messages_unrouted
    ON ifc_partner_messages (unrouted, created_at);

-- A message's delivery to one connection: the queue row the delivery poller
-- works from, with the latest attempt.
CREATE TABLE ifc_partner_deliveries (
    id TEXT PRIMARY KEY,
    message_id TEXT NOT NULL REFERENCES ifc_partner_messages (id),
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    reason TEXT NOT NULL CHECK (reason IN ('routed', 'sent_once', 'test')),
    created_at TEXT NOT NULL,
    state TEXT NOT NULL CHECK (state IN ('waiting', 'written', 'failed')),
    -- Failed attempts since the delivery was created or sent again.
    attempts INTEGER NOT NULL CHECK (attempts >= 0),
    next_attempt_at TEXT,
    last_attempt_at TEXT,
    last_error_code TEXT,
    written_path TEXT,
    written_at TEXT,
    CHECK ((state = 'written') = (written_path IS NOT NULL AND written_at IS NOT NULL)),
    UNIQUE (message_id, connection_id)
) STRICT;

CREATE INDEX ifc_partner_deliveries_state
    ON ifc_partner_deliveries (state, next_attempt_at);
CREATE INDEX ifc_partner_deliveries_connection
    ON ifc_partner_deliveries (connection_id, last_attempt_at);

-- The decision on a message that matched no connection.
CREATE TABLE ifc_partner_unrouted_resolutions (
    message_id TEXT PRIMARY KEY REFERENCES ifc_partner_messages (id),
    resolution TEXT NOT NULL CHECK (resolution IN ('sent_once', 'routed_again', 'delivered_another_way', 'dismissed')),
    connection_id TEXT REFERENCES ifc_partner_connections (id),
    note TEXT,
    resolved_by TEXT NOT NULL,
    resolved_at TEXT NOT NULL,
    CHECK ((resolution = 'sent_once') = (connection_id IS NOT NULL)),
    CHECK ((resolution IN ('delivered_another_way', 'dismissed')) = (note IS NOT NULL))
) STRICT;

CREATE TRIGGER ifc_partner_connection_changes_no_update
BEFORE UPDATE ON ifc_partner_connection_changes
BEGIN
    SELECT RAISE(ABORT, 'connection changes are append-only');
END;

CREATE TRIGGER ifc_partner_connection_changes_no_delete
BEFORE DELETE ON ifc_partner_connection_changes
BEGIN
    SELECT RAISE(ABORT, 'connection changes are append-only');
END;

CREATE TRIGGER ifc_partner_verifications_no_update
BEFORE UPDATE ON ifc_partner_verifications
BEGIN
    SELECT RAISE(ABORT, 'destination checks are append-only');
END;

CREATE TRIGGER ifc_partner_verifications_no_delete
BEFORE DELETE ON ifc_partner_verifications
BEGIN
    SELECT RAISE(ABORT, 'destination checks are append-only');
END;

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

CREATE TRIGGER ifc_partner_unrouted_resolutions_no_update
BEFORE UPDATE ON ifc_partner_unrouted_resolutions
BEGIN
    SELECT RAISE(ABORT, 'unrouted decisions are append-only');
END;

CREATE TRIGGER ifc_partner_unrouted_resolutions_no_delete
BEFORE DELETE ON ifc_partner_unrouted_resolutions
BEGIN
    SELECT RAISE(ABORT, 'unrouted decisions are append-only');
END;
