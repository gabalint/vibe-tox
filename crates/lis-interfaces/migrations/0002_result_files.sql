-- Result files from LC-MS/MS software (REQ-INT-005, REQ-INT-009; result-files
-- design brief F1, F13, F14 and §13). A file becomes an ordinary receipt with
-- the transport `result_file`; a side table holds the file facts, and a ledger
-- of files already received keeps the watched-folder poller from reading a
-- file twice.
--
-- SQLite cannot widen a CHECK constraint in place, so `ifc_instruments` and
-- `ifc_receipts` are rebuilt with the documented steps: create `x_next`, copy,
-- drop `x`, rename, then recreate the indexes and triggers of `x`. The store
-- runs this with foreign-key enforcement off and a full foreign-key check
-- before commit. The frame trigger reads `ifc_receipts`, so it is dropped
-- first and recreated unchanged after the rename.

DROP TRIGGER ifc_receipt_frame_receiving_only;

-- Instruments gain the `result_file` kind and its optional watched folder.
CREATE TABLE ifc_instruments_next (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    connection_kind TEXT NOT NULL CHECK (connection_kind IN ('astm_tcp', 'astm_serial', 'result_file')),
    tcp_bind_address TEXT,
    tcp_port INTEGER,
    tcp_allowed_peers TEXT,
    serial_port_path TEXT,
    serial_baud_rate INTEGER,
    serial_data_bits INTEGER,
    serial_parity TEXT,
    serial_stop_bits INTEGER,
    result_folder TEXT CHECK (result_folder IS NULL OR length(result_folder) > 0),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (
        (connection_kind = 'astm_tcp'
            AND tcp_bind_address IS NOT NULL
            AND tcp_port BETWEEN 1 AND 65535
            AND tcp_allowed_peers IS NOT NULL
            AND length(tcp_allowed_peers) > 2
            AND serial_port_path IS NULL
            AND serial_baud_rate IS NULL
            AND serial_data_bits IS NULL
            AND serial_parity IS NULL
            AND serial_stop_bits IS NULL
            AND result_folder IS NULL)
        OR
        (connection_kind = 'astm_serial'
            AND tcp_bind_address IS NULL
            AND tcp_port IS NULL
            AND tcp_allowed_peers IS NULL
            AND serial_port_path IS NOT NULL
            AND serial_baud_rate BETWEEN 1 AND 4000000
            AND serial_data_bits IN (5, 6, 7, 8)
            AND serial_parity IN ('none', 'odd', 'even')
            AND serial_stop_bits IN (1, 2)
            AND result_folder IS NULL)
        OR
        (connection_kind = 'result_file'
            AND tcp_bind_address IS NULL
            AND tcp_port IS NULL
            AND tcp_allowed_peers IS NULL
            AND serial_port_path IS NULL
            AND serial_baud_rate IS NULL
            AND serial_data_bits IS NULL
            AND serial_parity IS NULL
            AND serial_stop_bits IS NULL)
    )
) STRICT;

INSERT INTO ifc_instruments_next (
    id, code, name, connection_kind, tcp_bind_address, tcp_port, tcp_allowed_peers,
    serial_port_path, serial_baud_rate, serial_data_bits, serial_parity, serial_stop_bits,
    result_folder, enabled, revision
)
SELECT id, code, name, connection_kind, tcp_bind_address, tcp_port, tcp_allowed_peers,
       serial_port_path, serial_baud_rate, serial_data_bits, serial_parity, serial_stop_bits,
       NULL, enabled, revision
FROM ifc_instruments;

DROP TABLE ifc_instruments;
ALTER TABLE ifc_instruments_next RENAME TO ifc_instruments;

CREATE UNIQUE INDEX ifc_instruments_tcp_endpoint
ON ifc_instruments(tcp_bind_address, tcp_port)
WHERE connection_kind = 'astm_tcp';

CREATE UNIQUE INDEX ifc_instruments_serial_endpoint
ON ifc_instruments(serial_port_path)
WHERE connection_kind = 'astm_serial';

-- F13: a folder belongs to one instrument only.
CREATE UNIQUE INDEX ifc_instruments_result_folder
ON ifc_instruments(result_folder)
WHERE result_folder IS NOT NULL;

-- Receipts gain the `result_file` transport (no peer, no frames) and the
-- abort reason `file_too_large` (F14). A file is received whole, so its
-- receipt is complete, or aborted only because it is too large.
CREATE TABLE ifc_receipts_next (
    id TEXT PRIMARY KEY,
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    transport TEXT NOT NULL CHECK (transport IN ('astm_tcp', 'astm_serial', 'result_file')),
    peer TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    state TEXT NOT NULL CHECK (state IN ('receiving', 'complete', 'aborted', 'interrupted')),
    abort_reason TEXT CHECK (abort_reason IN (
        'retry_limit_exceeded', 'receive_timeout', 'incomplete_record', 'empty_transfer',
        'message_too_large', 'input_buffer_exceeded', 'empty_frame',
        'frame_limit_exceeded', 'wire_bytes_exceeded', 'connection_lost',
        'transport_error', 'listener_stopped', 'protocol_error', 'startup_interrupted',
        'file_too_large'
    )),
    frame_count INTEGER NOT NULL CHECK (frame_count >= 0),
    message_sha256 TEXT,
    artifact_digest TEXT,
    duplicate_of TEXT REFERENCES ifc_receipts(id),
    CHECK ((transport = 'astm_tcp' AND peer IS NOT NULL)
        OR (transport IN ('astm_serial', 'result_file') AND peer IS NULL)),
    CHECK (transport <> 'result_file'
        OR (frame_count = 0
            AND (state = 'complete' OR (state = 'aborted' AND abort_reason = 'file_too_large')))),
    CHECK (abort_reason IS NOT 'file_too_large' OR transport = 'result_file'),
    CHECK (
        (state = 'receiving'
            AND completed_at IS NULL AND abort_reason IS NULL
            AND message_sha256 IS NULL AND artifact_digest IS NULL AND duplicate_of IS NULL)
        OR
        (state = 'complete'
            AND completed_at IS NOT NULL AND abort_reason IS NULL
            AND message_sha256 IS NOT NULL AND length(message_sha256) = 64
            AND artifact_digest IS NOT NULL AND length(artifact_digest) = 64)
        OR
        (state IN ('aborted', 'interrupted')
            AND completed_at IS NOT NULL AND abort_reason IS NOT NULL
            AND message_sha256 IS NULL AND artifact_digest IS NULL AND duplicate_of IS NULL)
    )
) STRICT;

INSERT INTO ifc_receipts_next (
    id, instrument_id, transport, peer, started_at, completed_at, state, abort_reason,
    frame_count, message_sha256, artifact_digest, duplicate_of
)
SELECT id, instrument_id, transport, peer, started_at, completed_at, state, abort_reason,
       frame_count, message_sha256, artifact_digest, duplicate_of
FROM ifc_receipts;

DROP TABLE ifc_receipts;
ALTER TABLE ifc_receipts_next RENAME TO ifc_receipts;

CREATE INDEX ifc_receipts_instrument_message
ON ifc_receipts(instrument_id, message_sha256)
WHERE state = 'complete';

CREATE TRIGGER ifc_terminal_receipt_immutable
BEFORE UPDATE ON ifc_receipts
WHEN OLD.state <> 'receiving'
BEGIN
    SELECT RAISE(ABORT, 'terminal receipt is immutable');
END;

CREATE TRIGGER ifc_receipt_no_delete
BEFORE DELETE ON ifc_receipts
BEGIN
    SELECT RAISE(ABORT, 'receipt deletion is forbidden');
END;

CREATE TRIGGER ifc_receipt_frame_receiving_only
BEFORE INSERT ON ifc_receipt_frames
WHEN (SELECT state FROM ifc_receipts WHERE id = NEW.receipt_id) <> 'receiving'
BEGIN
    SELECT RAISE(ABORT, 'frames require a receiving receipt');
END;

-- F1: the facts of a received file. `loaded_by` is the user of a manual load;
-- a folder pickup is attributed to the instrument (F16).
CREATE TABLE ifc_receipt_files (
    receipt_id TEXT PRIMARY KEY REFERENCES ifc_receipts(id),
    origin TEXT NOT NULL CHECK (origin IN ('upload', 'folder')),
    file_name TEXT NOT NULL CHECK (length(file_name) > 0),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    modified_at TEXT NOT NULL,
    loaded_by TEXT,
    CHECK ((origin = 'upload') = (loaded_by IS NOT NULL))
) STRICT;

CREATE TRIGGER ifc_receipt_file_result_file_only
BEFORE INSERT ON ifc_receipt_files
WHEN (SELECT transport FROM ifc_receipts WHERE id = NEW.receipt_id) IS NOT 'result_file'
BEGIN
    SELECT RAISE(ABORT, 'file facts require a result-file receipt');
END;

CREATE TRIGGER ifc_receipt_file_no_update
BEFORE UPDATE ON ifc_receipt_files
BEGIN
    SELECT RAISE(ABORT, 'receipt file update is forbidden');
END;

CREATE TRIGGER ifc_receipt_file_no_delete
BEFORE DELETE ON ifc_receipt_files
BEGIN
    SELECT RAISE(ABORT, 'receipt file deletion is forbidden');
END;

-- F13: files already received, by instrument, name, size and modification
-- time. The same name with new content is a new key, so a re-export is read.
CREATE TABLE ifc_seen_files (
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    file_name TEXT NOT NULL CHECK (length(file_name) > 0),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    modified_at TEXT NOT NULL,
    receipt_id TEXT NOT NULL UNIQUE REFERENCES ifc_receipts(id),
    PRIMARY KEY (instrument_id, file_name, byte_len, modified_at)
) STRICT;

CREATE TRIGGER ifc_seen_file_no_update
BEFORE UPDATE ON ifc_seen_files
BEGIN
    SELECT RAISE(ABORT, 'seen file update is forbidden');
END;

CREATE TRIGGER ifc_seen_file_no_delete
BEFORE DELETE ON ifc_seen_files
BEGIN
    SELECT RAISE(ABORT, 'seen file deletion is forbidden');
END;
