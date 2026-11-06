-- Interface contracts and acceptance tests per connection (ticket #4555 step 5a;
-- REQ-INT-001). A contract is drafted, then approved by a second person as the
-- connection's next version. Acceptance tests run on synthetic data against what the
-- connection is now: its contract version, code mapping version and message settings,
-- which `acceptance_key` names. A connection writes and reads patient data only while
-- the latest run for its current key passed.

ALTER TABLE ifc_partner_connections ADD COLUMN acceptance_key TEXT NOT NULL DEFAULT '';

-- `contract_approved`: a new version of the connection's interface contract.
CREATE TABLE ifc_partner_connection_changes_next (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    revision INTEGER NOT NULL,
    change TEXT NOT NULL CHECK (change IN (
        'created', 'updated', 'started', 'stopped', 'verified', 'mappings_approved',
        'contract_approved'
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

CREATE TABLE ifc_partner_contract_drafts (
    connection_id TEXT PRIMARY KEY REFERENCES ifc_partner_connections (id),
    contract TEXT NOT NULL,
    drafted_by TEXT NOT NULL,
    drafted_at TEXT NOT NULL
) STRICT;

CREATE TABLE ifc_partner_contracts (
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    version INTEGER NOT NULL CHECK (version >= 1),
    contract TEXT NOT NULL,
    drafted_by TEXT NOT NULL,
    drafted_at TEXT NOT NULL,
    reason TEXT NOT NULL,
    approved_by TEXT NOT NULL,
    approved_at TEXT NOT NULL,
    self_approval INTEGER NOT NULL CHECK (self_approval IN (0, 1)),
    PRIMARY KEY (connection_id, version)
) STRICT;

CREATE TABLE ifc_partner_acceptance_runs (
    id TEXT PRIMARY KEY,
    connection_id TEXT NOT NULL REFERENCES ifc_partner_connections (id),
    -- The connection's acceptance key when the tests ran.
    covers TEXT NOT NULL,
    passed INTEGER NOT NULL CHECK (passed IN (0, 1)),
    -- Each case's code, whether it passed and why not, as JSON; no patient data.
    results TEXT NOT NULL,
    ran_by TEXT NOT NULL,
    ran_at TEXT NOT NULL
) STRICT;

CREATE INDEX ifc_partner_acceptance_runs_connection
    ON ifc_partner_acceptance_runs (connection_id, covers, ran_at);

CREATE TRIGGER ifc_partner_contracts_no_update
BEFORE UPDATE ON ifc_partner_contracts
BEGIN
    SELECT RAISE(ABORT, 'interface contract versions are append-only');
END;

CREATE TRIGGER ifc_partner_contracts_no_delete
BEFORE DELETE ON ifc_partner_contracts
BEGIN
    SELECT RAISE(ABORT, 'interface contract versions are append-only');
END;

CREATE TRIGGER ifc_partner_acceptance_runs_no_update
BEFORE UPDATE ON ifc_partner_acceptance_runs
BEGIN
    SELECT RAISE(ABORT, 'acceptance test runs are append-only');
END;

CREATE TRIGGER ifc_partner_acceptance_runs_no_delete
BEFORE DELETE ON ifc_partner_acceptance_runs
BEGIN
    SELECT RAISE(ABORT, 'acceptance test runs are append-only');
END;
