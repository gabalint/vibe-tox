CREATE TABLE ifc_instruments (
    id TEXT PRIMARY KEY,
    code TEXT NOT NULL UNIQUE,
    name TEXT NOT NULL,
    connection_kind TEXT NOT NULL CHECK (connection_kind IN ('astm_tcp', 'astm_serial')),
    tcp_bind_address TEXT,
    tcp_port INTEGER,
    tcp_allowed_peers TEXT,
    serial_port_path TEXT,
    serial_baud_rate INTEGER,
    serial_data_bits INTEGER,
    serial_parity TEXT,
    serial_stop_bits INTEGER,
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
            AND serial_stop_bits IS NULL)
        OR
        (connection_kind = 'astm_serial'
            AND tcp_bind_address IS NULL
            AND tcp_port IS NULL
            AND tcp_allowed_peers IS NULL
            AND serial_port_path IS NOT NULL
            AND serial_baud_rate BETWEEN 1 AND 4000000
            AND serial_data_bits IN (5, 6, 7, 8)
            AND serial_parity IN ('none', 'odd', 'even')
            AND serial_stop_bits IN (1, 2))
    )
) STRICT;

CREATE UNIQUE INDEX ifc_instruments_tcp_endpoint
ON ifc_instruments(tcp_bind_address, tcp_port)
WHERE connection_kind = 'astm_tcp';

CREATE UNIQUE INDEX ifc_instruments_serial_endpoint
ON ifc_instruments(serial_port_path)
WHERE connection_kind = 'astm_serial';

CREATE TABLE ifc_receipts (
    id TEXT PRIMARY KEY,
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    transport TEXT NOT NULL CHECK (transport IN ('astm_tcp', 'astm_serial')),
    peer TEXT,
    started_at TEXT NOT NULL,
    completed_at TEXT,
    state TEXT NOT NULL CHECK (state IN ('receiving', 'complete', 'aborted', 'interrupted')),
    abort_reason TEXT CHECK (abort_reason IN (
        'retry_limit_exceeded', 'receive_timeout', 'incomplete_record', 'empty_transfer',
        'message_too_large', 'input_buffer_exceeded', 'empty_frame',
        'frame_limit_exceeded', 'wire_bytes_exceeded', 'connection_lost',
        'transport_error', 'listener_stopped', 'protocol_error', 'startup_interrupted'
    )),
    frame_count INTEGER NOT NULL CHECK (frame_count >= 0),
    message_sha256 TEXT,
    artifact_digest TEXT,
    duplicate_of TEXT REFERENCES ifc_receipts(id),
    CHECK ((transport = 'astm_tcp' AND peer IS NOT NULL) OR (transport = 'astm_serial' AND peer IS NULL)),
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

CREATE INDEX ifc_receipts_instrument_message
ON ifc_receipts(instrument_id, message_sha256)
WHERE state = 'complete';

CREATE TABLE ifc_receipt_frames (
    receipt_id TEXT NOT NULL REFERENCES ifc_receipts(id),
    ordinal INTEGER NOT NULL CHECK (ordinal >= 0),
    bytes BLOB NOT NULL,
    PRIMARY KEY (receipt_id, ordinal)
) STRICT;

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

CREATE TRIGGER ifc_receipt_frame_no_update
BEFORE UPDATE ON ifc_receipt_frames
BEGIN
    SELECT RAISE(ABORT, 'receipt frame update is forbidden');
END;

CREATE TRIGGER ifc_receipt_frame_receiving_only
BEFORE INSERT ON ifc_receipt_frames
WHEN (SELECT state FROM ifc_receipts WHERE id = NEW.receipt_id) <> 'receiving'
BEGIN
    SELECT RAISE(ABORT, 'frames require a receiving receipt');
END;

CREATE TRIGGER ifc_receipt_frame_no_delete
BEFORE DELETE ON ifc_receipt_frames
BEGIN
    SELECT RAISE(ABORT, 'receipt frame deletion is forbidden');
END;
