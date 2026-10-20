-- Source-backed collection corrections preserve the original collection (REQ-SPC-002).
CREATE TABLE spc_collection_corrections (
    id TEXT NOT NULL PRIMARY KEY,
    collection_id TEXT NOT NULL REFERENCES spc_collections(id),
    revision INTEGER NOT NULL CHECK (revision >= 2),
    precision TEXT NOT NULL CHECK (precision IN ('datetime', 'date', 'unknown')),
    collected_at TEXT,
    collected_offset_minutes INTEGER,
    collected_date TEXT,
    source_document_ref TEXT NOT NULL CHECK (length(trim(source_document_ref)) > 0),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    UNIQUE (collection_id, revision),
    CHECK ((precision = 'datetime' AND collected_at IS NOT NULL AND collected_offset_minutes IS NOT NULL AND collected_date IS NULL)
        OR (precision = 'date' AND collected_at IS NULL AND collected_offset_minutes IS NULL AND collected_date IS NOT NULL)
        OR (precision = 'unknown' AND collected_at IS NULL AND collected_offset_minutes IS NULL AND collected_date IS NULL))
) STRICT;
CREATE TABLE spc_collection_correction_reviews (
    correction_id TEXT NOT NULL PRIMARY KEY REFERENCES spc_collection_corrections(id),
    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
    reviewed_by TEXT NOT NULL,
    reviewed_at TEXT NOT NULL
) STRICT;
CREATE TRIGGER spc_collection_corrections_sequence BEFORE INSERT ON spc_collection_corrections
WHEN NEW.revision <> (SELECT COALESCE(max(revision), 1) + 1 FROM spc_collection_corrections WHERE collection_id = NEW.collection_id)
BEGIN SELECT RAISE(ABORT, 'collection correction revision is not current'); END;
CREATE TRIGGER spc_collection_corrections_no_update BEFORE UPDATE ON spc_collection_corrections
BEGIN SELECT RAISE(ABORT, 'collection corrections are append-only'); END;
CREATE TRIGGER spc_collection_corrections_no_delete BEFORE DELETE ON spc_collection_corrections
BEGIN SELECT RAISE(ABORT, 'collection corrections are append-only'); END;
CREATE TRIGGER spc_collection_correction_reviews_no_update BEFORE UPDATE ON spc_collection_correction_reviews
BEGIN SELECT RAISE(ABORT, 'collection reviews are append-only'); END;
CREATE TRIGGER spc_collection_correction_reviews_no_delete BEFORE DELETE ON spc_collection_correction_reviews
BEGIN SELECT RAISE(ABORT, 'collection reviews are append-only'); END;

CREATE VIEW spc_v_collections AS
SELECT c.id, c.accession_id, COALESCE(r.revision, 1) AS revision,
       CASE WHEN r.id IS NULL THEN c.precision ELSE r.precision END AS precision,
       CASE WHEN r.id IS NULL THEN c.collected_at ELSE r.collected_at END AS collected_at,
       CASE WHEN r.id IS NULL THEN c.collected_offset_minutes ELSE r.collected_offset_minutes END AS collected_offset_minutes,
       CASE WHEN r.id IS NULL THEN c.collected_date ELSE r.collected_date END AS collected_date,
       c.collector_text, c.recorded_by, c.recorded_at
FROM spc_collections c LEFT JOIN spc_collection_corrections r ON r.collection_id = c.id
 AND r.revision = (SELECT max(newer.revision) FROM spc_collection_corrections newer WHERE newer.collection_id = c.id);
