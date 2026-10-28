-- Report layout choice (REQ-RPT-007). One laboratory-wide sequence of versions:
-- an administrator drafts a ready-made layout, and a quality manager approves or
-- rejects it with a reason. At most one draft and one active version exist. An
-- approved version replaces the active one, which becomes superseded. While no
-- version is active, candidates use the default layout. Candidates record the
-- template they render with, so issued reports never change with this table.

CREATE TABLE rpt_layout_versions (
    id               TEXT    NOT NULL PRIMARY KEY,
    version_no       INTEGER NOT NULL UNIQUE CHECK (version_no >= 1),
    layout           TEXT    NOT NULL
        CHECK (layout IN ('classic', 'standard', 'compact', 'results_first')),
    state            TEXT    NOT NULL
        CHECK (state IN ('draft', 'active', 'superseded', 'rejected')),
    drafted_by       TEXT    NOT NULL REFERENCES auth_users (id),
    drafted_at       TEXT    NOT NULL,
    decided_by       TEXT    REFERENCES auth_users (id),
    decided_at       TEXT,
    rejection_reason TEXT,
    superseded_at    TEXT,
    revision         INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'draft') = (decided_by IS NULL)),
    CHECK ((decided_by IS NULL) = (decided_at IS NULL)),
    CHECK ((state = 'rejected') = (rejection_reason IS NOT NULL)),
    CHECK ((state = 'superseded') = (superseded_at IS NOT NULL))
) STRICT;

CREATE UNIQUE INDEX rpt_layout_versions_one_draft
    ON rpt_layout_versions (state) WHERE state = 'draft';
CREATE UNIQUE INDEX rpt_layout_versions_one_active
    ON rpt_layout_versions (state) WHERE state = 'active';

-- A version is born a draft. Its identity never changes; its state moves only
-- draft → active | rejected and active → superseded, one revision at a time.
CREATE TRIGGER rpt_layout_versions_insert BEFORE INSERT ON rpt_layout_versions
WHEN NEW.state <> 'draft' OR NEW.revision <> 1
BEGIN
    SELECT RAISE(ABORT, 'a layout version is inserted as a draft at revision 1');
END;

CREATE TRIGGER rpt_layout_versions_update BEFORE UPDATE ON rpt_layout_versions
WHEN NEW.id <> OLD.id OR NEW.version_no <> OLD.version_no OR NEW.layout <> OLD.layout
  OR NEW.drafted_by <> OLD.drafted_by OR NEW.drafted_at <> OLD.drafted_at
  OR NEW.revision <> OLD.revision + 1
  OR NOT ((OLD.state = 'draft' AND NEW.state IN ('active', 'rejected'))
          OR (OLD.state = 'active' AND NEW.state = 'superseded'
              AND NEW.decided_by = OLD.decided_by AND NEW.decided_at = OLD.decided_at))
BEGIN
    SELECT RAISE(ABORT, 'a layout version moves only from draft to active or rejected, and from active to superseded');
END;

-- The author approves their own draft only where the laboratory's separation-of-duties
-- policy allows it (ADR 0015); the command also asks for their confirmation.
CREATE TRIGGER rpt_layout_versions_approve BEFORE UPDATE ON rpt_layout_versions
WHEN OLD.state = 'draft' AND NEW.state = 'active' AND NEW.decided_by = OLD.drafted_by
  AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'a layout is approved by a second person unless the separation-of-duties policy allows its author');
END;

CREATE TRIGGER rpt_layout_versions_no_delete BEFORE DELETE ON rpt_layout_versions
BEGIN
    SELECT RAISE(ABORT, 'layout versions are never deleted');
END;
