-- Files already in a watched folder when it was chosen (result-files brief §6,
-- "Present at selection"; REQ-INT-009). The poller never picks them up, so
-- pointing an instrument at a vendor's existing export folder, or moving a
-- folder to another instrument, cannot import old files. They can still be
-- loaded by hand. A file whose size or modification time changes is no longer
-- the recorded file and is picked up. Choosing the same folder again records
-- only files not already listed.
CREATE TABLE ifc_folder_baseline_files (
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments(id),
    folder TEXT NOT NULL CHECK (length(folder) > 0),
    file_name TEXT NOT NULL CHECK (length(file_name) > 0),
    byte_len INTEGER NOT NULL CHECK (byte_len >= 0),
    modified_at TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY (instrument_id, folder, file_name, byte_len, modified_at)
) STRICT;

CREATE TRIGGER ifc_folder_baseline_file_no_update
BEFORE UPDATE ON ifc_folder_baseline_files
BEGIN
    SELECT RAISE(ABORT, 'folder baseline update is forbidden');
END;

CREATE TRIGGER ifc_folder_baseline_file_no_delete
BEFORE DELETE ON ifc_folder_baseline_files
BEGIN
    SELECT RAISE(ABORT, 'folder baseline deletion is forbidden');
END;

-- How each result-file receipt arrived, for the import attempt list of the
-- results module (manual load or folder pickup). No file names.
CREATE VIEW ifc_v_receipt_origins AS
SELECT receipt_id, origin FROM ifc_receipt_files;
