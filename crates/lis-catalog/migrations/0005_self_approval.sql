-- Self-approval under the laboratory's separation-of-duties policy (ADR 0015;
-- REQ-ANA-002, REQ-ANA-009, REQ-OPS-015). Until now the author of a version could
-- never approve it. CLIA lets one qualified person hold every role, so a one-person
-- laboratory could never activate its configuration. The rule now follows the policy
-- in force (`auth_v_separation_policy`): while it needs a second person, the guard
-- triggers below refuse an author's approval exactly as the dropped CHECK did.
--
-- SQLite cannot drop a CHECK constraint, so the method, orderable and validity policy
-- versions are rebuilt with the documented steps: create `x_next` without the
-- approver <> author CHECK, copy every column, drop `x`, rename, then recreate the
-- indexes and triggers of `x`. The status transition triggers lose their own
-- approver <> author term; the audit-event binding is unchanged. The store runs this
-- with foreign-key enforcement off and a full foreign-key check before commit.
-- Foreign keys of other tables (catalog, analytical, orders, results) name these
-- tables and need no change. SQLite refuses a rename while any trigger reads a
-- missing table, so the frozen-content triggers of the analytes, components and
-- validity categories, which read these tables, are dropped first and recreated
-- unchanged after the renames. The separation guards come last.

DROP TRIGGER cat_method_version_analytes_frozen_insert;
DROP TRIGGER cat_method_version_analytes_frozen_update;
DROP TRIGGER cat_method_version_analytes_frozen_delete;
DROP TRIGGER cat_orderable_components_frozen_insert;
DROP TRIGGER cat_orderable_components_frozen_update;
DROP TRIGGER cat_orderable_components_frozen_delete;
DROP TRIGGER cat_orderable_component_analytes_frozen_insert;
DROP TRIGGER cat_orderable_component_analytes_frozen_update;
DROP TRIGGER cat_orderable_component_analytes_frozen_delete;
DROP TRIGGER cat_validity_categories_frozen_insert;
DROP TRIGGER cat_validity_categories_frozen_update;
DROP TRIGGER cat_validity_categories_frozen_delete;

-- Method versions.
CREATE TABLE cat_method_versions_next (
    id                 TEXT    NOT NULL PRIMARY KEY,
    method_id          TEXT    NOT NULL REFERENCES cat_methods (id),
    version            INTEGER NOT NULL CHECK (version >= 1),
    status             TEXT    NOT NULL CHECK (status IN ('draft', 'approved', 'retired')),
    revision           INTEGER NOT NULL CHECK (revision >= 1),
    specimen_type_id   TEXT    NOT NULL REFERENCES cat_specimen_types (id),
    certainty          TEXT    NOT NULL CHECK (certainty IN ('presumptive', 'definitive')),
    output_scale       TEXT    NOT NULL CHECK (output_scale IN ('qualitative', 'quantitative')),
    threshold_rule     TEXT    NOT NULL CHECK (threshold_rule IN ('at_or_above', 'above')),
    threshold_rounding TEXT    NOT NULL CHECK (threshold_rounding IN ('before_rounding', 'after_rounding')),
    created_by         TEXT    NOT NULL,
    created_at         TEXT    NOT NULL,
    approved_by        TEXT,
    effective_from     TEXT,
    effective_to       TEXT,
    replaced_by_id     TEXT    REFERENCES cat_method_versions (id),
    retired_by         TEXT,
    retired_at         TEXT,
    retire_reason      TEXT,
    UNIQUE (method_id, version),
    CHECK ((status = 'draft') = (approved_by IS NULL)),
    CHECK ((approved_by IS NULL) = (effective_from IS NULL)),
    CHECK (status <> 'draft' OR effective_to IS NULL),
    CHECK (replaced_by_id IS NULL OR effective_to IS NOT NULL),
    CHECK ((status = 'retired') = (retired_at IS NOT NULL)),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

INSERT INTO cat_method_versions_next (id, method_id, version, status, revision,
    specimen_type_id, certainty, output_scale, threshold_rule, threshold_rounding,
    created_by, created_at, approved_by, effective_from, effective_to, replaced_by_id,
    retired_by, retired_at, retire_reason)
SELECT id, method_id, version, status, revision, specimen_type_id, certainty, output_scale,
       threshold_rule, threshold_rounding, created_by, created_at, approved_by,
       effective_from, effective_to, replaced_by_id, retired_by, retired_at, retire_reason
FROM cat_method_versions;

DROP TABLE cat_method_versions;
ALTER TABLE cat_method_versions_next RENAME TO cat_method_versions;

CREATE UNIQUE INDEX cat_method_versions_one_draft
    ON cat_method_versions (method_id) WHERE status = 'draft';
CREATE UNIQUE INDEX cat_method_versions_one_current
    ON cat_method_versions (method_id) WHERE status = 'approved' AND effective_to IS NULL;

CREATE TRIGGER cat_method_versions_initial_draft BEFORE INSERT ON cat_method_versions
WHEN NEW.status <> 'draft'
    OR NEW.approved_by IS NOT NULL OR NEW.effective_from IS NOT NULL
    OR NEW.effective_to IS NOT NULL OR NEW.replaced_by_id IS NOT NULL
    OR NEW.retired_by IS NOT NULL OR NEW.retired_at IS NOT NULL
    OR NEW.retire_reason IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'method versions must be inserted as unapproved drafts');
END;

-- Status changes are permitted only after the command context appends the
-- matching event. The event binds the stored actor and time to the authorized
-- signed-in principal; rollback removes it if the transition fails.
CREATE TRIGGER cat_method_versions_status_transition BEFORE UPDATE OF status ON cat_method_versions
WHEN OLD.status <> NEW.status AND NOT (
    (
        OLD.status = 'draft' AND NEW.status = 'approved'
        AND NEW.revision = OLD.revision + 1
        AND NEW.id IS OLD.id AND NEW.method_id IS OLD.method_id
        AND NEW.version IS OLD.version
        AND NEW.specimen_type_id IS OLD.specimen_type_id
        AND NEW.certainty IS OLD.certainty AND NEW.output_scale IS OLD.output_scale
        AND NEW.threshold_rule IS OLD.threshold_rule
        AND NEW.threshold_rounding IS OLD.threshold_rounding
        AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
        AND NEW.approved_by IS NOT NULL
        AND NEW.effective_from IS NOT NULL
        AND NEW.effective_to IS NULL AND NEW.replaced_by_id IS NULL
        AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
        AND EXISTS (
            SELECT 1 FROM audit_events e
            WHERE e.stream = 'clinical' AND e.action = 'catalog.approve_method_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'method_version'
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
            WHERE e.stream = 'clinical' AND e.action = 'catalog.retire_method_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'method_version'
              AND e.record_id = NEW.id AND e.old_revision = OLD.revision
              AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
              AND e.principal_id = NEW.retired_by AND e.occurred_at = NEW.retired_at
              AND e.authority = 'approve_configuration' AND e.reason = NEW.retire_reason
        )
    )
)
BEGIN
    SELECT RAISE(ABORT, 'method version transition requires its authorized audit event');
END;

-- After approval only the effective end, the replacement and the retirement
-- may be set, each once; status moves forward only.
CREATE TRIGGER cat_method_versions_approved_immutable BEFORE UPDATE ON cat_method_versions
WHEN OLD.status <> 'draft' AND (
    NEW.status = 'draft' OR (OLD.status = 'retired' AND NEW.status <> 'retired')
    OR NEW.id IS NOT OLD.id OR NEW.method_id IS NOT OLD.method_id
    OR NEW.version IS NOT OLD.version
    OR NEW.specimen_type_id IS NOT OLD.specimen_type_id
    OR NEW.certainty IS NOT OLD.certainty OR NEW.output_scale IS NOT OLD.output_scale
    OR NEW.threshold_rule IS NOT OLD.threshold_rule
    OR NEW.threshold_rounding IS NOT OLD.threshold_rounding
    OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
    OR NEW.approved_by IS NOT OLD.approved_by OR NEW.effective_from IS NOT OLD.effective_from
    OR (OLD.effective_to IS NOT NULL AND NEW.effective_to IS NOT OLD.effective_to)
    OR (OLD.replaced_by_id IS NOT NULL AND NEW.replaced_by_id IS NOT OLD.replaced_by_id)
    OR (OLD.retired_at IS NOT NULL AND (NEW.retired_at IS NOT OLD.retired_at
        OR NEW.retired_by IS NOT OLD.retired_by OR NEW.retire_reason IS NOT OLD.retire_reason))
)
BEGIN
    SELECT RAISE(ABORT, 'approved method versions are immutable');
END;

CREATE TRIGGER cat_method_versions_no_delete BEFORE DELETE ON cat_method_versions
BEGIN
    SELECT RAISE(ABORT, 'method versions are never deleted');
END;

-- Orderable versions.
CREATE TABLE cat_orderable_versions_next (
    id             TEXT    NOT NULL PRIMARY KEY,
    orderable_id   TEXT    NOT NULL REFERENCES cat_orderables (id),
    version        INTEGER NOT NULL CHECK (version >= 1),
    status         TEXT    NOT NULL CHECK (status IN ('draft', 'approved', 'retired')),
    revision       INTEGER NOT NULL CHECK (revision >= 1),
    created_by     TEXT    NOT NULL,
    created_at     TEXT    NOT NULL,
    approved_by    TEXT,
    effective_from TEXT,
    effective_to   TEXT,
    replaced_by_id TEXT    REFERENCES cat_orderable_versions (id),
    retired_by     TEXT,
    retired_at     TEXT,
    retire_reason  TEXT,
    UNIQUE (orderable_id, version),
    CHECK ((status = 'draft') = (approved_by IS NULL)),
    CHECK ((approved_by IS NULL) = (effective_from IS NULL)),
    CHECK (status <> 'draft' OR effective_to IS NULL),
    CHECK (replaced_by_id IS NULL OR effective_to IS NOT NULL),
    CHECK ((status = 'retired') = (retired_at IS NOT NULL)),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

INSERT INTO cat_orderable_versions_next (id, orderable_id, version, status, revision,
    created_by, created_at, approved_by, effective_from, effective_to, replaced_by_id,
    retired_by, retired_at, retire_reason)
SELECT id, orderable_id, version, status, revision, created_by, created_at, approved_by,
       effective_from, effective_to, replaced_by_id, retired_by, retired_at, retire_reason
FROM cat_orderable_versions;

DROP TABLE cat_orderable_versions;
ALTER TABLE cat_orderable_versions_next RENAME TO cat_orderable_versions;

CREATE UNIQUE INDEX cat_orderable_versions_one_draft
    ON cat_orderable_versions (orderable_id) WHERE status = 'draft';
CREATE UNIQUE INDEX cat_orderable_versions_one_current
    ON cat_orderable_versions (orderable_id) WHERE status = 'approved' AND effective_to IS NULL;

CREATE TRIGGER cat_orderable_versions_initial_draft BEFORE INSERT ON cat_orderable_versions
WHEN NEW.status <> 'draft'
    OR NEW.approved_by IS NOT NULL OR NEW.effective_from IS NOT NULL
    OR NEW.effective_to IS NOT NULL OR NEW.replaced_by_id IS NOT NULL
    OR NEW.retired_by IS NOT NULL OR NEW.retired_at IS NOT NULL
    OR NEW.retire_reason IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'orderable versions must be inserted as unapproved drafts');
END;

CREATE TRIGGER cat_orderable_versions_status_transition BEFORE UPDATE OF status ON cat_orderable_versions
WHEN OLD.status <> NEW.status AND NOT (
    (
        OLD.status = 'draft' AND NEW.status = 'approved'
        AND NEW.revision = OLD.revision + 1
        AND NEW.id IS OLD.id AND NEW.orderable_id IS OLD.orderable_id
        AND NEW.version IS OLD.version
        AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
        AND NEW.approved_by IS NOT NULL
        AND NEW.effective_from IS NOT NULL
        AND NEW.effective_to IS NULL AND NEW.replaced_by_id IS NULL
        AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
        AND EXISTS (
            SELECT 1 FROM audit_events e
            WHERE e.stream = 'clinical' AND e.action = 'catalog.approve_orderable_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'orderable_version'
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
            WHERE e.stream = 'clinical' AND e.action = 'catalog.retire_orderable_version'
              AND e.outcome = 'succeeded' AND e.record_type = 'orderable_version'
              AND e.record_id = NEW.id AND e.old_revision = OLD.revision
              AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
              AND e.principal_id = NEW.retired_by AND e.occurred_at = NEW.retired_at
              AND e.authority = 'approve_configuration' AND e.reason = NEW.retire_reason
        )
    )
)
BEGIN
    SELECT RAISE(ABORT, 'orderable version transition requires its authorized audit event');
END;

CREATE TRIGGER cat_orderable_versions_approved_immutable BEFORE UPDATE ON cat_orderable_versions
WHEN OLD.status <> 'draft' AND (
    NEW.status = 'draft' OR (OLD.status = 'retired' AND NEW.status <> 'retired')
    OR NEW.id IS NOT OLD.id OR NEW.orderable_id IS NOT OLD.orderable_id
    OR NEW.version IS NOT OLD.version
    OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
    OR NEW.approved_by IS NOT OLD.approved_by OR NEW.effective_from IS NOT OLD.effective_from
    OR (OLD.effective_to IS NOT NULL AND NEW.effective_to IS NOT OLD.effective_to)
    OR (OLD.replaced_by_id IS NOT NULL AND NEW.replaced_by_id IS NOT OLD.replaced_by_id)
    OR (OLD.retired_at IS NOT NULL AND (NEW.retired_at IS NOT OLD.retired_at
        OR NEW.retired_by IS NOT OLD.retired_by OR NEW.retire_reason IS NOT OLD.retire_reason))
)
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_versions_no_delete BEFORE DELETE ON cat_orderable_versions
BEGIN
    SELECT RAISE(ABORT, 'orderable versions are never deleted');
END;

-- Validity policy versions.
CREATE TABLE cat_validity_policy_versions_next (
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
    CHECK (status <> 'draft' OR effective_to IS NULL),
    CHECK (replaced_by_id IS NULL OR effective_to IS NOT NULL),
    CHECK ((status = 'retired') = (retired_at IS NOT NULL)),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

INSERT INTO cat_validity_policy_versions_next (id, specimen_type_id, version, status,
    revision, created_by, created_at, approved_by, effective_from, effective_to,
    replaced_by_id, retired_by, retired_at, retire_reason)
SELECT id, specimen_type_id, version, status, revision, created_by, created_at, approved_by,
       effective_from, effective_to, replaced_by_id, retired_by, retired_at, retire_reason
FROM cat_validity_policy_versions;

DROP TABLE cat_validity_policy_versions;
ALTER TABLE cat_validity_policy_versions_next RENAME TO cat_validity_policy_versions;

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
        AND NEW.approved_by IS NOT NULL
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

-- The frozen-content triggers, unchanged.
CREATE TRIGGER cat_method_version_analytes_frozen_insert BEFORE INSERT ON cat_method_version_analytes
WHEN (SELECT status FROM cat_method_versions WHERE id = NEW.method_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved method versions are immutable');
END;

CREATE TRIGGER cat_method_version_analytes_frozen_update BEFORE UPDATE ON cat_method_version_analytes
WHEN (SELECT status FROM cat_method_versions WHERE id = OLD.method_version_id) <> 'draft'
    OR (SELECT status FROM cat_method_versions WHERE id = NEW.method_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved method versions are immutable');
END;

CREATE TRIGGER cat_method_version_analytes_frozen_delete BEFORE DELETE ON cat_method_version_analytes
WHEN (SELECT status FROM cat_method_versions WHERE id = OLD.method_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved method versions are immutable');
END;

CREATE TRIGGER cat_orderable_components_frozen_insert BEFORE INSERT ON cat_orderable_components
WHEN (SELECT status FROM cat_orderable_versions WHERE id = NEW.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_components_frozen_update BEFORE UPDATE ON cat_orderable_components
WHEN (SELECT status FROM cat_orderable_versions WHERE id = OLD.orderable_version_id) <> 'draft'
    OR (SELECT status FROM cat_orderable_versions WHERE id = NEW.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_components_frozen_delete BEFORE DELETE ON cat_orderable_components
WHEN (SELECT status FROM cat_orderable_versions WHERE id = OLD.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_component_analytes_frozen_insert BEFORE INSERT ON cat_orderable_component_analytes
WHEN (SELECT status FROM cat_orderable_versions WHERE id = NEW.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_component_analytes_frozen_update BEFORE UPDATE ON cat_orderable_component_analytes
WHEN (SELECT status FROM cat_orderable_versions WHERE id = OLD.orderable_version_id) <> 'draft'
    OR (SELECT status FROM cat_orderable_versions WHERE id = NEW.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

CREATE TRIGGER cat_orderable_component_analytes_frozen_delete BEFORE DELETE ON cat_orderable_component_analytes
WHEN (SELECT status FROM cat_orderable_versions WHERE id = OLD.orderable_version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved orderable versions are immutable');
END;

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

-- Separation of duties (ADR 0015): the author approves only when the policy in force
-- allows it. No row in the view means the default: a second person.
CREATE TRIGGER cat_method_versions_self_approval BEFORE UPDATE OF approved_by ON cat_method_versions
WHEN NEW.approved_by = NEW.created_by
    AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'separation of duties: the laboratory policy needs a second person to approve configuration');
END;

CREATE TRIGGER cat_orderable_versions_self_approval BEFORE UPDATE OF approved_by ON cat_orderable_versions
WHEN NEW.approved_by = NEW.created_by
    AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'separation of duties: the laboratory policy needs a second person to approve configuration');
END;

CREATE TRIGGER cat_validity_policy_versions_self_approval
BEFORE UPDATE OF approved_by ON cat_validity_policy_versions
WHEN NEW.approved_by = NEW.created_by
    AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'separation of duties: the laboratory policy needs a second person to approve configuration');
END;
