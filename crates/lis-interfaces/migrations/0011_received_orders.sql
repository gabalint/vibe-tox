-- Incoming order messages (Interfaces, ticket #4555 step 3c): every file read
-- from an HL7 Orders connection, its original bytes in the artifact store,
-- what became of it and where the file moved (REQ-INT-002, REQ-INT-003). A
-- message waiting for its patient later becomes an order or is rejected; nothing
-- else about a row changes, and no row is deleted.

-- An incoming order code maps to a LIS test with the sequence role the
-- partner's contract gives it; HL7 carries none, and the LIS never infers one.
ALTER TABLE ifc_partner_mappings ADD COLUMN sequence_role TEXT
    CHECK (sequence_role IN ('first_line', 'confirmation'));
ALTER TABLE ifc_partner_mapping_drafts ADD COLUMN sequence_role TEXT
    CHECK (sequence_role IN ('first_line', 'confirmation'));

CREATE TABLE ifc_partner_received (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    -- The file's name as read, without its folder.
    file_name TEXT NOT NULL,
    artifact_sha256 TEXT NOT NULL,
    received_at TEXT NOT NULL,
    -- MSH-10 and the requisition's placer number, when the message could be read.
    control_id TEXT,
    placer_order TEXT,
    state TEXT NOT NULL CHECK (state IN
        ('order_created', 'waiting_patient', 'held', 'duplicate', 'rejected')),
    -- Why it waits, is held or was rejected: a PHI-free code.
    reason_code TEXT,
    order_id TEXT,
    duplicate_of TEXT REFERENCES ifc_partner_received (id),
    -- Where the file moved: Processed or Failed, by year and month.
    moved_to TEXT,
    -- Who decided on a waiting message, and when; the system when its patient appeared.
    decided_by TEXT,
    decided_at TEXT,
    CHECK ((state = 'order_created') = (order_id IS NOT NULL)),
    CHECK ((state = 'duplicate') = (duplicate_of IS NOT NULL)),
    CHECK (state NOT IN ('waiting_patient', 'held', 'rejected') OR reason_code IS NOT NULL)
) STRICT;

CREATE INDEX ifc_partner_received_content
    ON ifc_partner_received (connection_id, artifact_sha256);
CREATE INDEX ifc_partner_received_control
    ON ifc_partner_received (connection_id, control_id) WHERE control_id IS NOT NULL;
CREATE INDEX ifc_partner_received_time ON ifc_partner_received (received_at);
CREATE INDEX ifc_partner_received_waiting
    ON ifc_partner_received (state) WHERE state = 'waiting_patient';

CREATE TRIGGER ifc_partner_received_no_delete
BEFORE DELETE ON ifc_partner_received
BEGIN
    SELECT RAISE(ABORT, 'received messages are kept');
END;

-- What was read stays as read; only a waiting message's outcome and the file's
-- new place are recorded later.
CREATE TRIGGER ifc_partner_received_kept
BEFORE UPDATE ON ifc_partner_received
WHEN NEW.id IS NOT OLD.id OR NEW.connection_id IS NOT OLD.connection_id
    OR NEW.file_name IS NOT OLD.file_name OR NEW.artifact_sha256 IS NOT OLD.artifact_sha256
    OR NEW.received_at IS NOT OLD.received_at OR NEW.control_id IS NOT OLD.control_id
    OR NEW.placer_order IS NOT OLD.placer_order OR NEW.duplicate_of IS NOT OLD.duplicate_of
    OR (OLD.moved_to IS NOT NULL AND NEW.moved_to IS NOT OLD.moved_to)
    OR (OLD.state <> 'waiting_patient' AND NEW.state IS NOT OLD.state)
BEGIN
    SELECT RAISE(ABORT, 'a received message is kept as read');
END;
