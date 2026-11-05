-- MLLP connections (Interfaces, ticket #4555 step 4): a connection can send HL7
-- messages over MLLP to a partner's interface on the lab network (the LIS
-- connects), or receive them on a lab-network address the LIS listens on. Its
-- MLLP settings are kept as JSON. A delivery sent over MLLP keeps the
-- partner's application acknowledgement code (AA, AE or AR) and its text.
-- Rebuild the CHECK-constrained table with the foreign-key checked migration
-- procedure of 0002 and 0004; existing rows keep their values.

CREATE TABLE ifc_partner_connections_next (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    partner TEXT NOT NULL,
    message_type TEXT NOT NULL
        CHECK (message_type IN ('billing_json', 'report_pdf', 'hl7_oru', 'hl7_orders')),
    protocol TEXT NOT NULL
        CHECK (protocol IN ('folder', 'sftp', 'mllp_connect', 'mllp_listen')),
    -- The local folder, or the remote folder on the SFTP server: where files are
    -- written, or for incoming orders where they are read. Empty for MLLP.
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
    -- What a destination check covers: the folder; the server, port, user,
    -- remote folder and fingerprint; or the MLLP address and port.
    destination TEXT NOT NULL,
    -- For incoming orders: how often the input folder is read, the sending
    -- facility (MSH-4) accepted and what happens for an unknown patient, as JSON.
    intake TEXT,
    -- For incoming orders: where read files move to, in year and month
    -- subfolders; local folders, or folders on the SFTP server.
    processed_folder TEXT,
    failed_folder TEXT,
    -- For MLLP: the partner's or the listener's address and port, and the
    -- acknowledgement settings, as JSON.
    mllp TEXT,
    CHECK ((schedule = 'every_minutes') = (schedule_minutes IS NOT NULL)),
    CHECK ((schedule = 'daily_at') = (schedule_time IS NOT NULL)),
    CHECK ((protocol = 'sftp') = (sftp_host IS NOT NULL AND sftp_port IS NOT NULL
        AND sftp_username IS NOT NULL AND sftp_auth IS NOT NULL AND sftp_fingerprint IS NOT NULL)),
    CHECK (sftp_password IS NULL OR sftp_auth = 'password'),
    CHECK ((protocol IN ('mllp_connect', 'mllp_listen')) = (mllp IS NOT NULL)),
    CHECK ((message_type = 'hl7_orders') = (intake IS NOT NULL)),
    -- Orders read from files move to Processed and Failed; orders received over
    -- MLLP are answered instead.
    CHECK ((processed_folder IS NOT NULL) = (failed_folder IS NOT NULL)),
    CHECK ((processed_folder IS NOT NULL)
        = (message_type = 'hl7_orders' AND protocol IN ('folder', 'sftp')))
) STRICT;

INSERT INTO ifc_partner_connections_next (
    id, name, partner, message_type, protocol, folder, folder_set_by, rules, schedule,
    schedule_minutes, schedule_time, file_name_pattern, active, revision, sftp_host, sftp_port,
    sftp_username, sftp_auth, sftp_fingerprint, sftp_password, destination, intake,
    processed_folder, failed_folder
)
SELECT id, name, partner, message_type, protocol, folder, folder_set_by, rules, schedule,
       schedule_minutes, schedule_time, file_name_pattern, active, revision, sftp_host, sftp_port,
       sftp_username, sftp_auth, sftp_fingerprint, sftp_password, destination, intake,
       processed_folder, failed_folder
FROM ifc_partner_connections;

DROP TABLE ifc_partner_connections;
ALTER TABLE ifc_partner_connections_next RENAME TO ifc_partner_connections;

CREATE UNIQUE INDEX ifc_partner_connections_name
    ON ifc_partner_connections (name COLLATE NOCASE);

-- The partner's application acknowledgement of a message sent over MLLP, and the
-- reason it gave (MSA-3, or ERR-8 when present).
ALTER TABLE ifc_partner_deliveries ADD COLUMN ack_code TEXT
    CHECK (ack_code IN ('AA', 'AE', 'AR'));
ALTER TABLE ifc_partner_deliveries ADD COLUMN ack_text TEXT;
