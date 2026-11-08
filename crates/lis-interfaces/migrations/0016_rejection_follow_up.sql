-- A person's follow-up of rejected messages (ticket #4555 step 5c; REQ-INT-002). A message
-- the LIS refused, or one the partner rejected with AE or AR, stays under Rejected until a
-- person resolves it with a note; a message waiting for its patient or held for review that
-- a person rejects is resolved by that decision, its reason the note. Append-only: a
-- rejection is resolved once.
CREATE TABLE ifc_partner_rejection_resolutions (
    received_id TEXT UNIQUE REFERENCES ifc_partner_received (id),
    delivery_id TEXT UNIQUE REFERENCES ifc_partner_deliveries (id),
    note TEXT NOT NULL CHECK (length(note) BETWEEN 1 AND 500),
    resolved_by TEXT NOT NULL,
    resolved_at TEXT NOT NULL,
    CHECK ((received_id IS NULL) <> (delivery_id IS NULL))
) STRICT;

CREATE TRIGGER ifc_partner_rejection_resolutions_no_update
BEFORE UPDATE ON ifc_partner_rejection_resolutions
BEGIN
    SELECT RAISE(ABORT, 'rejection resolutions are append-only');
END;

CREATE TRIGGER ifc_partner_rejection_resolutions_no_delete
BEFORE DELETE ON ifc_partner_rejection_resolutions
BEGIN
    SELECT RAISE(ABORT, 'rejection resolutions are append-only');
END;
