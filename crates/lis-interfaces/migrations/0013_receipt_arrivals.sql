-- When each receipt began to arrive and, for a result file, its name, for the
-- results module's import list: its received-day filter and its search by file
-- name (REQ-ANA-003). Read only in queries; file names are not listed.
CREATE VIEW ifc_v_receipt_arrivals AS
SELECT r.id AS receipt_id, r.started_at AS received_at, f.file_name
FROM ifc_receipts r
LEFT JOIN ifc_receipt_files f ON f.receipt_id = r.id;
