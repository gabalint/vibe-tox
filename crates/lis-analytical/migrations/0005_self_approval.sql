-- Self-approval of source registrations under the laboratory's separation-of-duties
-- policy (ADR 0015; REQ-ANA-003). Until now the author of a registration could never
-- approve it, so a one-person laboratory could never activate a result source. The
-- rule now follows the policy in force (`auth_v_separation_policy`): while it needs a
-- second person, the guard trigger below refuses an author's approval exactly as the
-- dropped CHECK did.
--
-- SQLite cannot drop a CHECK constraint, so the registrations are rebuilt with the
-- documented steps: create `x_next` without the approver <> author CHECK, copy every
-- column, drop `x`, rename, then recreate the index and triggers of `x`. The store
-- runs this with foreign-key enforcement off and a full foreign-key check before
-- commit. Foreign keys of the test-code, file-layout and results tables name this
-- table and need no change. SQLite refuses a rename while any trigger reads a
-- missing table, so the test-code and file-layout draft-only triggers, which read
-- this table, are dropped first and recreated unchanged after the rename. The
-- separation guard comes last.

DROP TRIGGER ana_source_test_codes_draft_only;
DROP TRIGGER ana_source_file_layouts_draft_only;

CREATE TABLE ana_source_registrations_next (
    id                          TEXT    NOT NULL PRIMARY KEY,
    instrument_id               TEXT    NOT NULL REFERENCES ifc_instruments (id),
    sender_name                 TEXT    NOT NULL,
    sender_version              TEXT    NOT NULL,
    sample_id_field             TEXT    NOT NULL
        CHECK (sample_id_field IN ('specimen_id', 'instrument_specimen_id')),
    revision_indicator          TEXT    NOT NULL
        CHECK (revision_indicator IN ('none', 'result_status_correction')),
    reports_final_concentration INTEGER NOT NULL CHECK (reports_final_concentration IN (0, 1)),
    state                       TEXT    NOT NULL CHECK (state IN ('draft', 'active', 'retired')),
    authored_by                 TEXT    NOT NULL,
    authored_at                 TEXT    NOT NULL,
    approved_by                 TEXT,
    approved_at                 TEXT,
    retired_at                  TEXT,
    revision                    INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'draft') = (approved_by IS NULL)),
    CHECK ((approved_by IS NULL) = (approved_at IS NULL)),
    CHECK ((state = 'retired') = (retired_at IS NOT NULL))
) STRICT;

INSERT INTO ana_source_registrations_next (id, instrument_id, sender_name, sender_version,
    sample_id_field, revision_indicator, reports_final_concentration, state, authored_by,
    authored_at, approved_by, approved_at, retired_at, revision)
SELECT id, instrument_id, sender_name, sender_version, sample_id_field, revision_indicator,
       reports_final_concentration, state, authored_by, authored_at, approved_by,
       approved_at, retired_at, revision
FROM ana_source_registrations;

DROP TABLE ana_source_registrations;
ALTER TABLE ana_source_registrations_next RENAME TO ana_source_registrations;

CREATE UNIQUE INDEX ana_source_registrations_one_active
    ON ana_source_registrations (instrument_id) WHERE state = 'active';

CREATE TRIGGER ana_source_registrations_start_draft BEFORE INSERT ON ana_source_registrations
WHEN NEW.state <> 'draft' OR NEW.revision <> 1
BEGIN SELECT RAISE(ABORT, 'a source registration starts as a draft'); END;

CREATE TRIGGER ana_source_registrations_lifecycle BEFORE UPDATE ON ana_source_registrations
WHEN NEW.id IS NOT OLD.id OR NEW.instrument_id IS NOT OLD.instrument_id
  OR NEW.sender_name IS NOT OLD.sender_name OR NEW.sender_version IS NOT OLD.sender_version
  OR NEW.sample_id_field IS NOT OLD.sample_id_field
  OR NEW.revision_indicator IS NOT OLD.revision_indicator
  OR NEW.reports_final_concentration IS NOT OLD.reports_final_concentration
  OR NEW.authored_by IS NOT OLD.authored_by OR NEW.authored_at IS NOT OLD.authored_at
  OR NEW.revision IS NOT OLD.revision + 1
  OR NOT ((OLD.state = 'draft' AND NEW.state = 'active')
       OR (OLD.state = 'active' AND NEW.state = 'retired'
           AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at))
BEGIN SELECT RAISE(ABORT, 'source registrations only move draft to active to retired'); END;

CREATE TRIGGER ana_source_registrations_no_delete BEFORE DELETE ON ana_source_registrations
BEGIN SELECT RAISE(ABORT, 'source registrations are never deleted'); END;

-- The draft-only triggers, unchanged.
CREATE TRIGGER ana_source_test_codes_draft_only BEFORE INSERT ON ana_source_test_codes
WHEN (SELECT state FROM ana_source_registrations WHERE id = NEW.registration_id) IS NOT 'draft'
BEGIN SELECT RAISE(ABORT, 'test codes are fixed once a registration is approved'); END;
CREATE TRIGGER ana_source_file_layouts_draft_only BEFORE INSERT ON ana_source_file_layouts
WHEN (SELECT state FROM ana_source_registrations WHERE id = NEW.registration_id) IS NOT 'draft'
BEGIN SELECT RAISE(ABORT, 'a file layout is fixed once its registration is approved'); END;

-- Separation of duties (ADR 0015): the author approves only when the policy in force
-- allows it. No row in the view means the default: a second person.
CREATE TRIGGER ana_source_registrations_self_approval
BEFORE UPDATE OF approved_by ON ana_source_registrations
WHEN NEW.approved_by = NEW.authored_by
    AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'separation of duties: the laboratory policy needs a second person to approve configuration');
END;
