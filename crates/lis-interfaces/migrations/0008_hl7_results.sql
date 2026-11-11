-- HL7 Results connections (Interfaces, ticket #4555 step 2): a released report
-- revision becomes an HL7 v2.5.1 ORU^R01 message, written to a folder with the
-- partner's codes. Each connection has versioned code mappings a second person
-- approves; a result without an approved mapping is held, never sent with a
-- guessed code (REQ-INT-004). Rebuild the CHECK-constrained tables with the
-- foreign-key checked migration procedure of 0002, 0004 and 0007; existing rows
-- keep their values.

CREATE TABLE ifc_partner_connections_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    partner TEXT NOT NULL,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json', 'report_pdf', 'hl7_oru')),
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

-- `mappings_approved`: a new version of the connection's code mappings.
CREATE TABLE ifc_partner_connection_changes_next (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    revision INTEGER NOT NULL,
    change TEXT NOT NULL CHECK (change IN (
        'created', 'updated', 'started', 'stopped', 'verified', 'mappings_approved'
    )),
    settings TEXT NOT NULL,
    reason TEXT,
    changed_by TEXT NOT NULL,
    changed_at TEXT NOT NULL,
    PRIMARY KEY (connection_id, revision)
) STRICT;

INSERT INTO ifc_partner_connection_changes_next
SELECT connection_id, revision, change, settings, reason, changed_by, changed_at
FROM ifc_partner_connection_changes;

DROP TABLE ifc_partner_connection_changes;
ALTER TABLE ifc_partner_connection_changes_next RENAME TO ifc_partner_connection_changes;

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

CREATE TABLE ifc_partner_messages_next (
    id TEXT PRIMARY KEY,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json', 'report_pdf', 'hl7_oru')),
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

INSERT INTO ifc_partner_messages_next
SELECT id, message_type, source_kind, source_id, source_number, source_group, source_label,
       artifact_sha256, routing_facts, checked_against, unrouted, created_at
FROM ifc_partner_messages;

DROP VIEW ifc_v_partner_report_revisions;
DROP TABLE ifc_partner_messages;
ALTER TABLE ifc_partner_messages_next RENAME TO ifc_partner_messages;

CREATE INDEX ifc_partner_messages_unrouted
    ON ifc_partner_messages (unrouted, created_at);
CREATE INDEX ifc_partner_messages_group
    ON ifc_partner_messages (source_group) WHERE source_group IS NOT NULL;

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

CREATE VIEW ifc_v_partner_report_revisions AS
SELECT DISTINCT m.source_id AS revision_id
FROM ifc_partner_messages m
JOIN ifc_partner_deliveries d ON d.message_id = m.id
WHERE m.source_kind = 'report_revision' AND d.state <> 'superseded';

-- A delivery that makes its own file (HL7, with the connection's codes) keeps
-- the exact bytes it writes and the code mapping version they used; every
-- retry writes the same bytes.
ALTER TABLE ifc_partner_deliveries ADD COLUMN payload_sha256 TEXT;
ALTER TABLE ifc_partner_deliveries ADD COLUMN mapping_version INTEGER;

-- One approved version of a connection's code mappings, the whole set.
CREATE TABLE ifc_partner_mapping_versions (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    version INTEGER NOT NULL CHECK (version >= 1),
    reason TEXT NOT NULL,
    approved_by TEXT NOT NULL,
    approved_at TEXT NOT NULL,
    self_approval INTEGER NOT NULL CHECK (self_approval IN (0, 1)),
    PRIMARY KEY (connection_id, version)
) STRICT;

-- The mappings of each approved version: a LIS test (orderable) or analyte to
-- the partner's code, with its LOINC (none: the catalog's) and UCUM unit.
CREATE TABLE ifc_partner_mappings (
    connection_id TEXT NOT NULL,
    version INTEGER NOT NULL,
    kind TEXT NOT NULL CHECK (kind IN ('test', 'analyte')),
    lis_id TEXT NOT NULL,
    lis_code TEXT NOT NULL,
    lis_name TEXT NOT NULL,
    partner_code TEXT NOT NULL,
    loinc TEXT,
    ucum_unit TEXT,
    PRIMARY KEY (connection_id, version, kind, lis_id),
    FOREIGN KEY (connection_id, version)
        REFERENCES ifc_partner_mapping_versions (connection_id, version)
) STRICT;

-- Proposed changes, applied by the next approval. No partner code: remove the mapping.
CREATE TABLE ifc_partner_mapping_drafts (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    kind TEXT NOT NULL CHECK (kind IN ('test', 'analyte')),
    lis_id TEXT NOT NULL,
    lis_code TEXT NOT NULL,
    lis_name TEXT NOT NULL,
    partner_code TEXT,
    loinc TEXT,
    ucum_unit TEXT,
    drafted_by TEXT NOT NULL,
    drafted_at TEXT NOT NULL,
    PRIMARY KEY (connection_id, kind, lis_id)
) STRICT;

CREATE TRIGGER ifc_partner_mapping_versions_no_update
BEFORE UPDATE ON ifc_partner_mapping_versions
BEGIN
    SELECT RAISE(ABORT, 'code mapping versions are append-only');
END;

CREATE TRIGGER ifc_partner_mapping_versions_no_delete
BEFORE DELETE ON ifc_partner_mapping_versions
BEGIN
    SELECT RAISE(ABORT, 'code mapping versions are append-only');
END;

CREATE TRIGGER ifc_partner_mappings_no_update
BEFORE UPDATE ON ifc_partner_mappings
BEGIN
    SELECT RAISE(ABORT, 'code mappings are append-only');
END;

CREATE TRIGGER ifc_partner_mappings_no_delete
BEFORE DELETE ON ifc_partner_mappings
BEGIN
    SELECT RAISE(ABORT, 'code mappings are append-only');
END;
