-- A message identical to an earlier one from the same instrument is listed as
-- an import attempt that imports nothing (REQ-ANA-003, REQ-INT-003, NFR-02):
-- outcome `duplicate`, naming the earlier receipt whose attempts hold its
-- rows. It has no rows, failure code or source registration.
--
-- SQLite cannot widen a CHECK constraint in place, so the import attempts are
-- rebuilt with the documented steps of 0005: create `x_next`, copy, drop `x`,
-- rename, then recreate the triggers of `x`. The store runs this with
-- foreign-key enforcement off and a full foreign-key check before commit. The
-- table has no index of its own besides its keys, and no view or other
-- table's trigger reads it.

CREATE TABLE res_import_attempts_next (
    id                      TEXT    NOT NULL PRIMARY KEY,
    receipt_id              TEXT    NOT NULL REFERENCES ifc_receipts (id),
    attempt_no              INTEGER NOT NULL CHECK (attempt_no >= 1),
    instrument_id           TEXT    NOT NULL REFERENCES ifc_instruments (id),
    registration_id         TEXT    REFERENCES ana_source_registrations (id),
    outcome                 TEXT    NOT NULL CHECK (outcome IN
                                   ('processed', 'processed_with_quarantine', 'quarantined',
                                    'duplicate')),
    failure_code            TEXT    CHECK (failure_code IN
                                   ('malformed_message', 'unregistered_source', 'sender_mismatch',
                                    'undecodable_text', 'layout_mismatch', 'limits_exceeded')),
    rows_total              INTEGER NOT NULL CHECK (rows_total >= 0),
    rows_new                INTEGER NOT NULL CHECK (rows_new >= 0),
    rows_duplicate          INTEGER NOT NULL CHECK (rows_duplicate >= 0),
    rows_reprocessed        INTEGER NOT NULL CHECK (rows_reprocessed >= 0),
    rows_qc                 INTEGER NOT NULL CHECK (rows_qc >= 0),
    rows_quarantined        INTEGER NOT NULL CHECK (rows_quarantined >= 0),
    rows_not_result         INTEGER NOT NULL CHECK (rows_not_result >= 0),
    rows_set_aside          INTEGER NOT NULL CHECK (rows_set_aside >= 0),
    rows_retained           INTEGER NOT NULL CHECK (rows_retained >= 0),
    parser_version          TEXT    NOT NULL,
    processed_at            TEXT    NOT NULL,
    duplicate_of_receipt_id TEXT    REFERENCES ifc_receipts (id),
    UNIQUE (receipt_id, attempt_no),
    CHECK (failure_code IS NULL OR outcome = 'quarantined'),
    CHECK (rows_new + rows_duplicate + rows_reprocessed + rows_qc + rows_quarantined
           + rows_not_result + rows_set_aside + rows_retained <= rows_total),
    CHECK ((outcome = 'duplicate') = (duplicate_of_receipt_id IS NOT NULL)),
    CHECK (outcome <> 'duplicate'
           OR (rows_total = 0 AND registration_id IS NULL
               AND duplicate_of_receipt_id <> receipt_id))
) STRICT;

INSERT INTO res_import_attempts_next
SELECT id, receipt_id, attempt_no, instrument_id, registration_id, outcome, failure_code,
       rows_total, rows_new, rows_duplicate, rows_reprocessed, rows_qc, rows_quarantined,
       rows_not_result, rows_set_aside, rows_retained, parser_version, processed_at, NULL
FROM res_import_attempts;

DROP TABLE res_import_attempts;
ALTER TABLE res_import_attempts_next RENAME TO res_import_attempts;

CREATE TRIGGER res_import_attempts_no_update BEFORE UPDATE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;
CREATE TRIGGER res_import_attempts_no_delete BEFORE DELETE ON res_import_attempts
BEGIN SELECT RAISE(ABORT, 'import attempts are append-only'); END;
