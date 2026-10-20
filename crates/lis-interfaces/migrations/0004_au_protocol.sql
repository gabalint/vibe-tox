-- AU400/AU640 interfaces. Rebuild CHECK-constrained tables using the same
-- foreign-key checked migration procedure as 0002; preserve existing data.

DROP TRIGGER ifc_receipt_frame_receiving_only;
DROP TRIGGER ifc_receipt_file_result_file_only;

-- Preserve all existing configuration while adding the explicit AU transports.
CREATE TABLE ifc_instruments_next (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    connection_kind TEXT NOT NULL CHECK (connection_kind IN ('astm_tcp', 'astm_serial', 'au_tcp', 'au_serial', 'result_file')),
    tcp_bind_address TEXT,
    tcp_port INTEGER,
    tcp_allowed_peers TEXT,
    serial_port_path TEXT,
    serial_baud_rate INTEGER,
    serial_data_bits INTEGER,
    serial_parity TEXT,
    serial_stop_bits INTEGER,
    result_folder TEXT CHECK (result_folder IS NULL OR length(result_folder) > 0),
    au_settings TEXT CHECK (au_settings IS NULL OR json_valid(au_settings)),
    enabled INTEGER NOT NULL CHECK (enabled IN (0, 1)),
    revision INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((connection_kind IN ('au_tcp', 'au_serial')) = (au_settings IS NOT NULL)),
    CHECK (
        (connection_kind IN ('astm_tcp', 'au_tcp')
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
        (connection_kind IN ('astm_serial', 'au_serial')
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
       result_folder, enabled, revision
FROM ifc_instruments;

DROP TABLE ifc_instruments;
ALTER TABLE ifc_instruments_next RENAME TO ifc_instruments;

CREATE UNIQUE INDEX ifc_instruments_tcp_endpoint
ON ifc_instruments(tcp_bind_address, tcp_port)
WHERE connection_kind IN ('astm_tcp', 'au_tcp');

CREATE UNIQUE INDEX ifc_instruments_serial_endpoint
ON ifc_instruments(serial_port_path)
WHERE connection_kind IN ('astm_serial', 'au_serial');

-- F13: a folder belongs to one instrument only.
CREATE UNIQUE INDEX ifc_instruments_result_folder
ON ifc_instruments(result_folder)
WHERE result_folder IS NOT NULL;

-- Existing receipt semantics are unchanged; AU peers follow the same TCP/serial rules.
CREATE TABLE ifc_receipts_next (
    id TEXT PRIMARY KEY,
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    transport TEXT NOT NULL CHECK (transport IN ('astm_tcp', 'astm_serial', 'au_tcp', 'au_serial', 'result_file')),
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
    CHECK ((transport IN ('astm_tcp', 'au_tcp') AND peer IS NOT NULL)
        OR (transport IN ('astm_serial', 'au_serial', 'result_file') AND peer IS NULL)),
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

CREATE TRIGGER ifc_receipt_file_result_file_only
BEFORE INSERT ON ifc_receipt_files
WHEN (SELECT transport FROM ifc_receipts WHERE id = NEW.receipt_id) IS NOT 'result_file'
BEGIN
    SELECT RAISE(ABORT, 'file facts require a result-file receipt');
END;

-- Freeze the wire settings with every AU receipt. Delayed imports and recovery
-- must never parse old bytes with today's instrument settings (REQ-INT-005).
CREATE TABLE ifc_receipt_au_profiles (
    receipt_id TEXT PRIMARY KEY REFERENCES ifc_receipts(id),
    profile_json TEXT NOT NULL CHECK (json_valid(profile_json)),
    profile_version INTEGER NOT NULL CHECK (profile_version = 1)
) STRICT;

CREATE TRIGGER ifc_receipt_au_profile_receiving_only
BEFORE INSERT ON ifc_receipt_au_profiles
WHEN NOT EXISTS (SELECT 1 FROM ifc_receipts WHERE id = NEW.receipt_id
    AND transport IN ('au_tcp', 'au_serial') AND state = 'receiving')
BEGIN
    SELECT RAISE(ABORT, 'AU profile requires a receiving AU receipt');
END;

CREATE TRIGGER ifc_receipt_au_profile_no_update
BEFORE UPDATE ON ifc_receipt_au_profiles
BEGIN
    SELECT RAISE(ABORT, 'AU receipt profile is immutable');
END;

CREATE TRIGGER ifc_receipt_au_profile_no_delete
BEFORE DELETE ON ifc_receipt_au_profiles
BEGIN
    SELECT RAISE(ABORT, 'AU receipt profile deletion is forbidden');
END;

-- A final E is unnumbered. Once an identity has used intermediate blocks,
-- accept it on a new connection only with a fresh block 0, even after a crash
-- or a completed receipt whose final ACK was lost. Exact frames remain the
-- source evidence; this digest-only index cannot be cleared by configuration.
CREATE TABLE ifc_au_multiblock_guards (
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    identity_sha256 TEXT NOT NULL CHECK (length(identity_sha256) = 64),
    first_receipt_id TEXT NOT NULL REFERENCES ifc_receipts(id),
    PRIMARY KEY (instrument_id, identity_sha256)
) STRICT;

CREATE TRIGGER ifc_au_multiblock_guard_receiving_only
BEFORE INSERT ON ifc_au_multiblock_guards
WHEN NOT EXISTS (SELECT 1 FROM ifc_receipts WHERE id = NEW.first_receipt_id
    AND instrument_id = NEW.instrument_id AND transport IN ('au_tcp', 'au_serial')
    AND state = 'receiving')
BEGIN
    SELECT RAISE(ABORT, 'AU guard requires a receiving AU receipt');
END;

CREATE TRIGGER ifc_au_multiblock_guard_no_update
BEFORE UPDATE ON ifc_au_multiblock_guards
BEGIN
    SELECT RAISE(ABORT, 'AU multiblock identity is immutable');
END;

CREATE TRIGGER ifc_au_multiblock_guard_no_delete
BEFORE DELETE ON ifc_au_multiblock_guards
BEGIN
    SELECT RAISE(ABORT, 'AU multiblock identity deletion is forbidden');
END;
