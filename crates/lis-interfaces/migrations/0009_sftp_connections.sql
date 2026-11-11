-- SFTP connections (Interfaces, ticket #4555 step 3): a connection can write to
-- a partner's SFTP server instead of a folder. The server's key is pinned by its
-- SHA-256 fingerprint; the LIS signs in with its own SSH key or a password,
-- sealed with the installation key. A destination check covers the destination
-- as a whole (server, port, user, fingerprint and remote folder), so changing
-- any of them needs a new check (REQ-RPT-012). Rebuild the CHECK-constrained
-- table with the foreign-key checked migration procedure of 0002 and 0004;
-- existing rows keep their values.

CREATE TABLE ifc_partner_connections_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    partner TEXT NOT NULL,
    message_type TEXT NOT NULL CHECK (message_type IN ('billing_json', 'report_pdf', 'hl7_oru')),
    protocol TEXT NOT NULL CHECK (protocol IN ('folder', 'sftp')),
    -- The local folder, or the remote folder on the SFTP server.
    folder TEXT NOT NULL,
    -- Who chose the current destination: verifying it is the second step of that duty.
    folder_set_by TEXT NOT NULL,
    rules TEXT NOT NULL,
    schedule TEXT NOT NULL CHECK (schedule IN ('immediately', 'every_minutes', 'daily_at')),
    schedule_minutes INTEGER CHECK (schedule_minutes BETWEEN 1 AND 1440),
    schedule_time TEXT,
    file_name_pattern TEXT NOT NULL,
    active INTEGER NOT NULL CHECK (active IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    sftp_host TEXT,
    sftp_port INTEGER CHECK (sftp_port BETWEEN 1 AND 65535),
    sftp_username TEXT,
    sftp_auth TEXT CHECK (sftp_auth IN ('key', 'password')),
    sftp_fingerprint TEXT,
    -- The password, sealed with the installation key; only for password sign-in.
    sftp_password BLOB,
    -- What a destination check covers: the folder, or the server, port, user,
    -- remote folder and fingerprint, as the LIS writes it.
    destination TEXT NOT NULL,
    CHECK ((schedule = 'every_minutes') = (schedule_minutes IS NOT NULL)),
    CHECK ((schedule = 'daily_at') = (schedule_time IS NOT NULL)),
    CHECK ((protocol = 'sftp') = (sftp_host IS NOT NULL AND sftp_port IS NOT NULL
        AND sftp_username IS NOT NULL AND sftp_auth IS NOT NULL AND sftp_fingerprint IS NOT NULL)),
    CHECK (sftp_password IS NULL OR sftp_auth = 'password')
) STRICT;

INSERT INTO ifc_partner_connections_next (
    id, name, partner, message_type, protocol, folder, folder_set_by, rules, schedule,
    schedule_minutes, schedule_time, file_name_pattern, active, revision, destination
)
SELECT id, name, partner, message_type, protocol, folder, folder_set_by, rules, schedule,
       schedule_minutes, schedule_time, file_name_pattern, active, revision, folder
FROM ifc_partner_connections;

DROP TABLE ifc_partner_connections;
ALTER TABLE ifc_partner_connections_next RENAME TO ifc_partner_connections;

CREATE UNIQUE INDEX ifc_partner_connections_name
    ON ifc_partner_connections (name COLLATE NOCASE);

-- The LIS's own SSH key for SFTP sign-in, one per installation: its public key
-- goes to the partner's IT; the private key is sealed with the installation key.
CREATE TABLE ifc_sftp_identity (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    public_key TEXT NOT NULL,
    private_key BLOB NOT NULL,
    created_by TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TRIGGER ifc_sftp_identity_no_update
BEFORE UPDATE ON ifc_sftp_identity
BEGIN
    SELECT RAISE(ABORT, 'the SFTP key is kept as created');
END;

CREATE TRIGGER ifc_sftp_identity_no_delete
BEFORE DELETE ON ifc_sftp_identity
BEGIN
    SELECT RAISE(ABORT, 'the SFTP key is kept as created');
END;
