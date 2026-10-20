-- Self-approval of billing class mappings under the laboratory's separation-of-duties
-- policy (ADR 0015; REQ-BIL-005). Until now the author of a mapping version could
-- never approve it, so a one-person laboratory could never approve its mapping. The
-- rule now follows the policy in force (`auth_v_separation_policy`): while it needs a
-- second person, the guard trigger below refuses an author's approval exactly as the
-- dropped CHECK did. Laboratory confirmation keeps its own step, unchanged.
--
-- SQLite cannot drop a CHECK constraint, so the mapping versions are rebuilt with the
-- documented steps: create `x_next` without the approver <> author CHECK, copy every
-- column, drop `x`, rename, then recreate the indexes and triggers of `x`. The
-- transition trigger loses its own approver <> author term; the audit-event bindings
-- are unchanged. The store runs this with foreign-key enforcement off and a full
-- foreign-key check before commit. The derivations' foreign key names this table and
-- needs no change. SQLite refuses a rename while any trigger reads a missing table, so
-- the frozen-entry triggers, which read this table, are dropped first and recreated
-- unchanged after the rename. The separation guard comes last.

DROP TRIGGER bil_mapping_entries_frozen_insert;
DROP TRIGGER bil_mapping_entries_frozen_update;
DROP TRIGGER bil_mapping_entries_frozen_delete;

CREATE TABLE bil_class_mapping_versions_next (
    id             TEXT    NOT NULL PRIMARY KEY,
    version        INTEGER NOT NULL UNIQUE CHECK (version >= 1),
    status         TEXT    NOT NULL CHECK (status IN ('draft', 'approved', 'retired')),
    revision       INTEGER NOT NULL CHECK (revision >= 1),
    created_by     TEXT    NOT NULL REFERENCES auth_users (id),
    created_at     TEXT    NOT NULL,
    approved_by    TEXT REFERENCES auth_users (id),
    approved_at    TEXT,
    confirmed_by   TEXT REFERENCES auth_users (id),
    confirmed_at   TEXT,
    effective_from TEXT,
    effective_to   TEXT,
    retired_by     TEXT REFERENCES auth_users (id),
    retired_at     TEXT,
    retire_reason  TEXT,
    CHECK ((status = 'draft') = (approved_by IS NULL)),
    CHECK ((approved_by IS NULL) = (approved_at IS NULL)),
    CHECK ((confirmed_by IS NULL) = (confirmed_at IS NULL)),
    CHECK ((confirmed_at IS NULL) = (effective_from IS NULL)),
    CHECK (effective_to IS NULL OR effective_from IS NOT NULL),
    CHECK ((status = 'retired') = (retired_by IS NOT NULL)),
    CHECK ((retired_by IS NULL) = (retired_at IS NULL)),
    CHECK ((retired_by IS NULL) = (retire_reason IS NULL))
) STRICT;

INSERT INTO bil_class_mapping_versions_next (id, version, status, revision, created_by,
    created_at, approved_by, approved_at, confirmed_by, confirmed_at, effective_from,
    effective_to, retired_by, retired_at, retire_reason)
SELECT id, version, status, revision, created_by, created_at, approved_by, approved_at,
       confirmed_by, confirmed_at, effective_from, effective_to, retired_by, retired_at,
       retire_reason
FROM bil_class_mapping_versions;

DROP TABLE bil_class_mapping_versions;
ALTER TABLE bil_class_mapping_versions_next RENAME TO bil_class_mapping_versions;

CREATE UNIQUE INDEX bil_one_mapping_draft
    ON bil_class_mapping_versions ((status = 'draft')) WHERE status = 'draft';

CREATE UNIQUE INDEX bil_one_current_mapping
    ON bil_class_mapping_versions ((effective_to IS NULL))
    WHERE effective_from IS NOT NULL AND effective_to IS NULL AND status = 'approved';

CREATE TRIGGER bil_mapping_versions_initial_draft BEFORE INSERT ON bil_class_mapping_versions
WHEN NOT (
    NEW.status = 'draft' AND NEW.revision = 1
    AND NEW.approved_by IS NULL AND NEW.approved_at IS NULL
    AND NEW.confirmed_by IS NULL AND NEW.confirmed_at IS NULL
    AND NEW.effective_from IS NULL AND NEW.effective_to IS NULL
    AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
)
BEGIN
    SELECT RAISE(ABORT, 'billing class mappings start as clean drafts');
END;

CREATE TRIGGER bil_mapping_versions_transition BEFORE UPDATE ON bil_class_mapping_versions
WHEN NOT (
    (OLD.status = 'draft' AND NEW.status = 'draft'
      AND NEW.revision = OLD.revision + 1
      AND NEW.id IS OLD.id AND NEW.version IS OLD.version
      AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
      AND NEW.approved_by IS NULL AND NEW.approved_at IS NULL
      AND NEW.confirmed_by IS NULL AND NEW.confirmed_at IS NULL
      AND NEW.effective_from IS NULL AND NEW.effective_to IS NULL
      AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.update_class_mapping_draft'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_class_mapping_version'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.principal_id = NEW.created_by AND e.authority = 'billing'))
 OR (OLD.status = 'draft' AND NEW.status = 'approved'
      AND NEW.revision = OLD.revision + 1
      AND NEW.id IS OLD.id AND NEW.version IS OLD.version
      AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
      AND NEW.approved_by IS NOT NULL
      AND NEW.approved_at IS NOT NULL
      AND NEW.confirmed_by IS NULL AND NEW.confirmed_at IS NULL
      AND NEW.effective_from IS NULL AND NEW.effective_to IS NULL
      AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.approve_class_mapping'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_class_mapping_version'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.principal_id = NEW.approved_by AND e.occurred_at = NEW.approved_at
            AND e.authority = 'billing'))
 OR (OLD.status = 'approved' AND NEW.status = 'approved'
      AND OLD.confirmed_by IS NULL AND NEW.confirmed_by IS NOT NULL
      AND NEW.revision = OLD.revision + 1
      AND NEW.id IS OLD.id AND NEW.version IS OLD.version
      AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at
      AND NEW.confirmed_at IS NOT NULL AND NEW.effective_from IS NEW.confirmed_at
      AND NEW.effective_to IS NULL
      AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.confirm_class_mapping'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_class_mapping_version'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.principal_id = NEW.confirmed_by AND e.occurred_at = NEW.confirmed_at
            AND e.authority = 'approve_configuration'))
 OR (OLD.status = 'approved' AND NEW.status = 'approved'
      AND OLD.effective_from IS NOT NULL AND OLD.effective_to IS NULL
      AND NEW.revision = OLD.revision + 1 AND NEW.effective_to IS NOT NULL
      AND NEW.id IS OLD.id AND NEW.version IS OLD.version
      AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at
      AND NEW.confirmed_by IS OLD.confirmed_by AND NEW.confirmed_at IS OLD.confirmed_at
      AND NEW.effective_from IS OLD.effective_from
      AND NEW.retired_by IS NULL AND NEW.retired_at IS NULL AND NEW.retire_reason IS NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.supersede_class_mapping'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_class_mapping_version'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.occurred_at = NEW.effective_to AND e.authority = 'approve_configuration'))
 OR (OLD.status = 'approved' AND NEW.status = 'retired'
      AND NEW.revision = OLD.revision + 1
      AND NEW.id IS OLD.id AND NEW.version IS OLD.version
      AND NEW.created_by IS OLD.created_by AND NEW.created_at IS OLD.created_at
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at
      AND NEW.confirmed_by IS OLD.confirmed_by AND NEW.confirmed_at IS OLD.confirmed_at
      AND NEW.effective_from IS OLD.effective_from
      AND NEW.effective_to IS CASE WHEN OLD.effective_from IS NULL THEN NULL
                                   ELSE COALESCE(OLD.effective_to, NEW.retired_at) END
      AND NEW.retired_by IS NOT NULL AND NEW.retired_at IS NOT NULL
      AND NEW.retire_reason IS NOT NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.retire_class_mapping'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_class_mapping_version'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.principal_id = NEW.retired_by AND e.occurred_at = NEW.retired_at
            AND e.authority = 'billing' AND e.reason = NEW.retire_reason))
)
BEGIN
    SELECT RAISE(ABORT, 'billing class mapping transition requires its authorized audit event');
END;

CREATE TRIGGER bil_mapping_versions_no_delete BEFORE DELETE ON bil_class_mapping_versions
BEGIN SELECT RAISE(ABORT, 'billing class mapping versions are never deleted'); END;

-- The frozen-entry triggers, unchanged.
CREATE TRIGGER bil_mapping_entries_frozen_insert BEFORE INSERT ON bil_class_mapping_entries
WHEN (SELECT status FROM bil_class_mapping_versions WHERE id = NEW.version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved billing class mappings are immutable');
END;

CREATE TRIGGER bil_mapping_entries_frozen_update BEFORE UPDATE ON bil_class_mapping_entries
WHEN (SELECT status FROM bil_class_mapping_versions WHERE id = OLD.version_id) <> 'draft'
  OR (SELECT status FROM bil_class_mapping_versions WHERE id = NEW.version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved billing class mappings are immutable');
END;

CREATE TRIGGER bil_mapping_entries_frozen_delete BEFORE DELETE ON bil_class_mapping_entries
WHEN (SELECT status FROM bil_class_mapping_versions WHERE id = OLD.version_id) <> 'draft'
BEGIN
    SELECT RAISE(ABORT, 'approved billing class mappings are immutable');
END;

-- Separation of duties (ADR 0015): the author approves only when the policy in force
-- allows it. No row in the view means the default: a second person.
CREATE TRIGGER bil_mapping_versions_self_approval
BEFORE UPDATE OF approved_by ON bil_class_mapping_versions
WHEN NEW.approved_by = NEW.created_by
    AND coalesce((SELECT self_approve_configuration FROM auth_v_separation_policy), 0) = 0
BEGIN
    SELECT RAISE(ABORT, 'separation of duties: the laboratory policy needs a second person to approve configuration');
END;
