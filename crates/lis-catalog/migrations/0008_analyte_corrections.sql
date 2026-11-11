-- Analyte corrections and retirement (REQ-ANA-002, DM-05 pattern). The row keeps
-- its ID, so methods, maps and results that refer to it follow the correction;
-- issued reports keep the text they froze. Every revision is kept in an
-- immutable history; legacy baselines preserve the facts without inventing an author.
CREATE TABLE cat_analyte_history (
    analyte_id      TEXT    NOT NULL REFERENCES cat_analytes (id),
    revision        INTEGER NOT NULL CHECK (revision >= 1),
    code            TEXT    NOT NULL,
    name            TEXT    NOT NULL,
    loinc_code      TEXT,
    drug_class_code TEXT,
    active          INTEGER NOT NULL CHECK (active IN (0, 1)),
    recorded_at     TEXT    NOT NULL,
    recorded_by     TEXT,
    reason          TEXT,
    origin          TEXT    NOT NULL CHECK (origin IN ('legacy_baseline', 'registered', 'corrected')),
    PRIMARY KEY (analyte_id, revision),
    CHECK ((origin = 'legacy_baseline') = (recorded_by IS NULL)),
    CHECK ((origin = 'corrected') = (reason IS NOT NULL))
) STRICT;

CREATE TRIGGER cat_analyte_history_no_update BEFORE UPDATE ON cat_analyte_history
BEGIN
    SELECT RAISE(ABORT, 'analyte history is immutable');
END;

CREATE TRIGGER cat_analyte_history_no_delete BEFORE DELETE ON cat_analyte_history
BEGIN
    SELECT RAISE(ABORT, 'analyte history is retained');
END;

ALTER TABLE cat_analytes ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1));
ALTER TABLE cat_analytes ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1);

INSERT INTO cat_analyte_history (analyte_id, revision, code, name, loinc_code, drug_class_code,
    active, recorded_at, origin)
SELECT id, 1, code, name, loinc_code, drug_class_code, 1, created_at, 'legacy_baseline'
FROM cat_analytes;

-- An update must be the next revision, match its history row, and keep the code
-- while a method version uses the analyte. The command also refuses a code
-- change while anything else refers to the analyte.
DROP TRIGGER cat_analytes_no_update;

CREATE TRIGGER cat_analytes_versioned BEFORE UPDATE ON cat_analytes
WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at
 OR NEW.revision <> OLD.revision + 1
 OR (NEW.code IS NOT OLD.code AND EXISTS (
    SELECT 1 FROM cat_method_version_analytes x WHERE x.analyte_id = OLD.id
 ))
 OR NOT EXISTS (
    SELECT 1 FROM cat_analyte_history h WHERE h.analyte_id = NEW.id
      AND h.revision = NEW.revision AND h.code = NEW.code AND h.name = NEW.name
      AND h.loinc_code IS NEW.loinc_code AND h.drug_class_code IS NEW.drug_class_code
      AND h.active = NEW.active
 )
BEGIN
    SELECT RAISE(ABORT, 'analyte correction requires matching immutable history');
END;
