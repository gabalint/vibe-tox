-- Which stored record of an incident failed its digest: the issued PDF, or a
-- candidate's canonical content or template data (NFR-07). The digest in
-- `artifact_sha256` is the one that record should have.
ALTER TABLE rpt_preservation_incidents ADD COLUMN stored_record TEXT NOT NULL DEFAULT 'pdf'
    CHECK (stored_record IN ('pdf', 'content_json', 'data_json'));
