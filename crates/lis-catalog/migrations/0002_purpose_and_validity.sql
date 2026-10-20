-- M2 catalog additions (REQ-SPC-004, REQ-SPC-008, REQ-ANA-002, REQ-OPS-001).
--
-- 1. Component purpose: an analytical component measures drugs; a validity
--    component measures specimen validity (creatinine, pH, oxidants and
--    similar). Orders copy it when they freeze a component. Adding a column
--    does not fire the immutability triggers; components approved before this
--    migration are analytical.
-- 2. Validity policy versions per specimen type: lab-approved categories, each
--    mapped to one action. They follow the method and orderable lifecycle
--    (draft -> approved -> retired, approver <> author, transitions bound to
--    their audit events, approved content immutable). The product ships no
--    categories.

ALTER TABLE cat_orderable_components ADD COLUMN purpose TEXT NOT NULL DEFAULT 'analytical'
    CHECK (purpose IN ('analytical', 'validity'));

CREATE TABLE cat_validity_policy_versions (
    id               TEXT    NOT NULL PRIMARY KEY,
    specimen_type_id TEXT    NOT NULL REFERENCES cat_specimen_types (id),
    version          INTEGER NOT NULL CHECK (version >= 1),
    status           TEXT    NOT NULL CHECK (status IN ('draft', 'approved', 'retired')),
    revision         INTEGER NOT NULL CHECK (revision >= 1),
    created_by       TEXT    NOT NULL,
    created_at       TEXT    NOT NULL,
    approved_by      TEXT,
    effective_from   TEXT,
    effective_to     TEXT,
    replaced_by_id   TEXT    REFERENCES cat_validity_policy_versions (id),
    retired_by       TEXT,
    retired_at       TEXT,
    retire_reason    TEXT,
    UNIQUE (specimen_type_id, version),
    CHECK ((status = 'draft') = (approved_by IS NULL)),
    CHECK ((approved_by IS NULL) = (effective_from IS NULL)),
    -- Separation of duties: the approver is never the author.
    CHECK (approved_by IS NULL OR approved_by <> created_by),
    CHECK (status <> 'draft' OR effective_to IS NULL),
    CHECK (replaced_by_id IS NULL OR effective_to IS NOT NULL),
    CHECK ((status = 'retired') = (retired_at IS NOT NULL)),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

CREATE UNIQUE INDEX cat_validity_policy_versions_one_draft
    ON cat_validity_policy_versions (specimen_type_id) WHERE status = 'draft';
CREATE UNIQUE INDEX cat_validity_policy_versions_one_current
    ON cat_validity_policy_versions (specimen_type_id)
    WHERE status = 'approved' AND effective_to IS NULL;

CREATE TRIGGER cat_validity_policy_versions_initial_draft
BEFORE INSERT ON cat_validity_policy_versions
WHEN NEW.status <> 'draft'
    OR NEW.approved_by IS NOT NULL OR NEW.effective_from IS NOT NULL
    OR NEW.effective_to IS NOT NULL OR NEW.replaced_by_id IS NOT NULL
    OR NEW.retired_by IS NOT NULL OR NEW.retired_at IS NOT NULL
    OR NEW.retire_reason IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'validity policy versions must be inserted as unapproved drafts');
END;

-- Status changes are permitted only after the command context appends the
-- matching event, as for method and orderable versions.
CREATE TRIGGER cat_validity_policy_versions_status_transition
BEFORE UPDATE OF status ON cat_validity_policy_versions
WHEN OLD.status <> NEW.status AND NOT (
    (
        OLD.status = 'draft' AND NEW.status = 'approved'
        AND NEW.revision = OLD.revision + 1
        AND NEW.id IS OLD.id AND NEW.specimen_type_id IS OLD.specimen_type_id
        AND NEW.version IS OLD.version
        AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
        AND NEW.approved_by IS NOT NULL AND NEW.approved_by <> NEW.created_by
        AND NEW.effective_from IS NOT NULL
        AND NEW.effective_to IS NULL AND NEW.replaced_by_id IS NULL
        AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
        AND EXISTS (
            SELECT 1 FROM audit_events e
            WHERE e.stream = 'clinical' AND e.action = 'catalog.approve_validity_policy_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'validity_policy_version'
              AND e.record_id = NEW.id AND e.old_revision = OLD.revision
              AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
              AND e.principal_id = NEW.approved_by AND e.occurred_at = NEW.effective_from
              AND e.authority = 'approve_configuration'
        )
    ) OR (
        OLD.status = 'approved' AND NEW.status = 'retired'
        AND NEW.revision = OLD.revision + 1
        AND NEW.retired_by IS NOT NULL AND NEW.retired_at IS NOT NULL
        AND NEW.retire_reason IS NOT NULL
        AND NEW.approved_by IS OLD.approved_by AND NEW.effective_from IS OLD.effective_from
        AND NEW.replaced_by_id IS OLD.replaced_by_id
        AND NEW.effective_to IS COALESCE(OLD.effective_to, NEW.retired_at)
        AND EXISTS (
            SELECT 1 FROM audit_events e
            WHERE e.stream = 'clinical' AND e.action = 'catalog.retire_validity_policy_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'validity_policy_version'
              AND e.record_id = NEW.id AND e.old_revision = OLD.revision
              AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
              AND e.principal_id = NEW.retired_by AND e.occurred_at = NEW.retired_at
              AND e.authority = 'approve_configuration' AND e.reason = NEW.retire_reason
        )
    )
)
BEGIN
    SELECT RAISE(ABORT, 'validity policy version transition requires its authorized audit event');
END;

-- After approval only the effective end, the replacement and the retirement
-- may be set, each once; status moves forward only.
CREATE TRIGGER cat_validity_policy_versions_approved_immutable
BEFORE UPDATE ON cat_validity_policy_versions
WHEN OLD.status <> 'draft' AND (
    NEW.status = 'draft' OR (OLD.status = 'retired' AND NEW.status <> 'retired')
    OR NEW.id IS NOT OLD.id OR NEW.specimen_type_id IS NOT OLD.specimen_type_id
    OR NEW.version IS NOT OLD.version
    OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
    OR NEW.approved_by IS NOT OLD.approved_by OR NEW.effective_from IS NOT OLD.effective_from
    OR (OLD.effective_to IS NOT NULL AND NEW.effective_to IS NOT OLD.effective_to)
    OR (OLD.replaced_by_id IS NOT NULL AND NEW.replaced_by_id IS NOT OLD.replaced_by_id)
    OR (OLD.retired_at IS NOT NULL AND (NEW.retired_at IS NOT OLD.retired_at
        OR NEW.retired_by IS NOT OLD.retired_by OR NEW.retire_reason IS NOT OLD.retire_reason))
)
BEGIN
    SELECT RAISE(ABORT, 'approved validity policy versions are immutable');
END;

CREATE TRIGGER cat_validity_policy_versions_no_delete
BEFORE DELETE ON cat_validity_policy_versions
BEGIN
    SELECT RAISE(ABORT, 'validity policy versions are never deleted');
END;

-- The categories of a policy version, each mapped to one action. A qualifier
-- action carries the qualifier text the report shows; no other action does.
CREATE TABLE cat_validity_categories (
    policy_version_id TEXT NOT NULL REFERENCES cat_validity_policy_versions (id),
    code              TEXT NOT NULL,
    label             TEXT NOT NULL,
    action            TEXT NOT NULL
        CHECK (action IN ('none', 'qualifier', 'hold', 'recollect', 'comment')),
    qualifier_text    TEXT,
    PRIMARY KEY (policy_version_id, code),
    CHECK ((action = 'qualifier') = (qualifier_text IS NOT NULL))
) STRICT;

CREATE TRIGGER cat_validity_categories_frozen_insert BEFORE INSERT ON cat_validity_categories
WHEN (SELECT status FROM cat_validity_policy_versions WHERE id = NEW.policy_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved validity policy versions are immutable');
END;

CREATE TRIGGER cat_validity_categories_frozen_update BEFORE UPDATE ON cat_validity_categories
WHEN (SELECT status FROM cat_validity_policy_versions WHERE id = OLD.policy_version_id) <> 'draft'
    OR (SELECT status FROM cat_validity_policy_versions WHERE id = NEW.policy_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved validity policy versions are immutable');
END;

CREATE TRIGGER cat_validity_categories_frozen_delete BEFORE DELETE ON cat_validity_categories
WHEN (SELECT status FROM cat_validity_policy_versions WHERE id = OLD.policy_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved validity policy versions are immutable');
END;
