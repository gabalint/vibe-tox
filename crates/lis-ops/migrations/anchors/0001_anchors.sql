-- The anchors area of the operations module (M4 design brief §2.2, §4; NFR-18, ADR 0008,
-- REQ-OPS-008): which audit seals were written to which target, as one new anchor file
-- per target and pass. The seals themselves live in `audit_seals`; the files on the
-- targets are written once and never changed, and so are these rows.

-- One anchor file written to a target. `kind` is `scheduled` for the job runner and
-- `manual` for a user's export, who is then named.
CREATE TABLE ops_anchor_exports (
    id          TEXT NOT NULL PRIMARY KEY,
    target_id   TEXT NOT NULL REFERENCES ops_targets (id),
    -- The file's name inside `VibeToxLIS/anchors/<YYYY-MM-DD>/`; never a path.
    file_name   TEXT NOT NULL CHECK (length(file_name) > 0 AND instr(file_name, '/') = 0
        AND instr(file_name, '\') = 0),
    exported_at TEXT NOT NULL,
    kind        TEXT NOT NULL CHECK (kind IN ('scheduled', 'manual')),
    -- The acting user's ID text, for a manual export.
    exported_by TEXT CHECK (length(exported_by) > 0),
    CHECK ((kind = 'manual') = (exported_by IS NOT NULL))
) STRICT;

CREATE INDEX ops_anchor_exports_target ON ops_anchor_exports (target_id);
CREATE INDEX ops_anchor_exports_time ON ops_anchor_exports (exported_at, id);

-- The seals of one stream an anchor file holds, as a range of seal numbers.
CREATE TABLE ops_anchor_export_ranges (
    export_id     TEXT    NOT NULL REFERENCES ops_anchor_exports (id),
    stream        TEXT    NOT NULL CHECK (stream IN ('clinical', 'security', 'access')),
    first_seal_no INTEGER NOT NULL CHECK (first_seal_no >= 1),
    last_seal_no  INTEGER NOT NULL CHECK (last_seal_no >= first_seal_no),
    PRIMARY KEY (export_id, stream)
) STRICT;

-- Each target receives each stream's seals in order, once: a range starts right after
-- the last seal already exported to the same target (ADR 0008 item 3).
CREATE TRIGGER ops_anchor_export_ranges_in_order BEFORE INSERT ON ops_anchor_export_ranges
WHEN NEW.first_seal_no <> 1 + coalesce((
    SELECT max(r.last_seal_no)
    FROM ops_anchor_export_ranges r
    JOIN ops_anchor_exports e ON e.id = r.export_id
    WHERE r.stream = NEW.stream
      AND e.target_id = (SELECT target_id FROM ops_anchor_exports WHERE id = NEW.export_id)
), 0)
BEGIN
    SELECT RAISE(ABORT, 'anchor ranges continue the target''s previous range of the stream');
END;

CREATE TRIGGER ops_anchor_exports_no_update BEFORE UPDATE ON ops_anchor_exports
BEGIN
    SELECT RAISE(ABORT, 'anchor exports are append-only');
END;

CREATE TRIGGER ops_anchor_exports_no_delete BEFORE DELETE ON ops_anchor_exports
BEGIN
    SELECT RAISE(ABORT, 'anchor exports are append-only');
END;

CREATE TRIGGER ops_anchor_export_ranges_no_update BEFORE UPDATE ON ops_anchor_export_ranges
BEGIN
    SELECT RAISE(ABORT, 'anchor export ranges are append-only');
END;

CREATE TRIGGER ops_anchor_export_ranges_no_delete BEFORE DELETE ON ops_anchor_export_ranges
BEGIN
    SELECT RAISE(ABORT, 'anchor export ranges are append-only');
END;
