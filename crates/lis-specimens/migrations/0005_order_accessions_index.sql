-- The order worklist reads each listed order's accessions and sample barcodes, and
-- finds orders by accession number or sample barcode (REQ-ORD-001, NFR-05).

CREATE INDEX spc_accessions_by_order ON spc_accessions (order_id, created_at);
