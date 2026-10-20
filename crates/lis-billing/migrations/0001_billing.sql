-- Billing class mappings and performed-service handoff (REQ-BIL-001/002/005).
-- The product ships no mappings, payer rules or procedure-code content.

CREATE TABLE bil_class_mapping_versions (
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
    CHECK (approved_by IS NULL OR approved_by <> created_by),
    CHECK ((confirmed_by IS NULL) = (confirmed_at IS NULL)),
    CHECK ((confirmed_at IS NULL) = (effective_from IS NULL)),
    CHECK (effective_to IS NULL OR effective_from IS NOT NULL),
    CHECK ((status = 'retired') = (retired_by IS NOT NULL)),
    CHECK ((retired_by IS NULL) = (retired_at IS NULL)),
    CHECK ((retired_by IS NULL) = (retire_reason IS NULL))
) STRICT;

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

CREATE TABLE bil_class_mapping_entries (
    version_id  TEXT NOT NULL REFERENCES bil_class_mapping_versions (id),
    analyte_id  TEXT NOT NULL REFERENCES cat_analytes (id),
    class_code  TEXT NOT NULL,
    class_label TEXT NOT NULL,
    PRIMARY KEY (version_id, analyte_id)
) STRICT;

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
      AND NEW.approved_by IS NOT NULL AND NEW.approved_by <> NEW.created_by
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

CREATE TABLE bil_derivations (
    id                 TEXT    NOT NULL PRIMARY KEY,
    subject_id         TEXT    NOT NULL REFERENCES idn_subjects (id),
    date_of_service    TEXT,
    dos_rule           TEXT    NOT NULL,
    derivation_no      INTEGER NOT NULL CHECK (derivation_no >= 1),
    mapping_version_id TEXT REFERENCES bil_class_mapping_versions (id),
    inputs_sha256      TEXT    NOT NULL CHECK (length(inputs_sha256) = 64),
    completeness       TEXT    NOT NULL CHECK (completeness IN ('complete', 'awaiting_final')),
    state              TEXT    NOT NULL CHECK (state IN ('held', 'candidate', 'approved', 'handed_off', 'superseded')),
    adjustment_of_id   TEXT REFERENCES bil_derivations (id),
    derived_by         TEXT    NOT NULL REFERENCES auth_users (id),
    derived_at         TEXT    NOT NULL,
    approved_by        TEXT REFERENCES auth_users (id),
    approved_at        TEXT,
    facts_sealed       INTEGER NOT NULL CHECK (facts_sealed IN (0, 1)),
    revision           INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((approved_by IS NULL) = (approved_at IS NULL)),
    CHECK (state NOT IN ('approved', 'handed_off') OR approved_by IS NOT NULL),
    CHECK (state NOT IN ('held', 'candidate') OR approved_by IS NULL)
) STRICT;
CREATE UNIQUE INDEX bil_derivations_number
    ON bil_derivations (subject_id, COALESCE(date_of_service, ''), derivation_no);
-- Idempotency compares a new digest with the group's newest derivation
-- only, so an older digest may recur after an intervening change.

CREATE TRIGGER bil_derivations_initial_state BEFORE INSERT ON bil_derivations
WHEN NOT (
    NEW.revision = 1 AND NEW.state IN ('held', 'candidate') AND NEW.facts_sealed = 0
    AND NEW.approved_by IS NULL AND NEW.approved_at IS NULL
)
BEGIN SELECT RAISE(ABORT, 'billing derivations start held or candidate'); END;

CREATE TABLE bil_derivation_specimens (
    derivation_id     TEXT    NOT NULL REFERENCES bil_derivations (id),
    specimen_id       TEXT    NOT NULL REFERENCES spc_specimens (id),
    order_id          TEXT    NOT NULL REFERENCES ord_orders (id),
    order_version     INTEGER NOT NULL CHECK (order_version >= 1),
    report_revision_id TEXT REFERENCES rpt_revisions (id),
    PRIMARY KEY (derivation_id, specimen_id)
) STRICT;
CREATE TABLE bil_derivation_components (
    derivation_id     TEXT NOT NULL,
    order_component_id TEXT NOT NULL REFERENCES ord_order_components (id),
    specimen_id       TEXT NOT NULL,
    disposition       TEXT NOT NULL CHECK (disposition IN ('performed', 'cancelled', 'not_performed', 'failed_only', 'validity_excluded')),
    PRIMARY KEY (derivation_id, order_component_id),
    FOREIGN KEY (derivation_id, specimen_id)
        REFERENCES bil_derivation_specimens (derivation_id, specimen_id)
) STRICT;
CREATE TABLE bil_derivation_classes (
    derivation_id TEXT    NOT NULL,
    specimen_id   TEXT    NOT NULL,
    class_code    TEXT    NOT NULL,
    class_label   TEXT    NOT NULL,
    certainty     TEXT    NOT NULL CHECK (certainty IN ('presumptive', 'definitive')),
    ordered       INTEGER NOT NULL CHECK (ordered IN (0, 1)),
    performed     INTEGER NOT NULL CHECK (performed IN (0, 1)),
    reported      INTEGER NOT NULL CHECK (reported IN (0, 1)),
    CHECK (performed <= ordered),
    CHECK (reported <= ordered),
    PRIMARY KEY (derivation_id, specimen_id, class_code, certainty),
    FOREIGN KEY (derivation_id, specimen_id)
        REFERENCES bil_derivation_specimens (derivation_id, specimen_id)
) STRICT;
CREATE TABLE bil_derivation_flags (
    derivation_id TEXT    NOT NULL REFERENCES bil_derivations (id),
    code          TEXT    NOT NULL CHECK (code IN ('mapping_missing', 'date_of_service_unknown',
        'report_not_final', 'laboratory_billing_identity_missing', 'ordered_not_performed',
        'presumptive_and_definitive_same_specimen', 'multiple_presumptive_same_dos',
        'performed_not_ordered', 'failed_attempt_only', 'performed_not_reported')),
    ref_id        TEXT    NOT NULL DEFAULT '',
    blocking      INTEGER NOT NULL CHECK (blocking IN (0, 1)),
    PRIMARY KEY (derivation_id, code, ref_id)
) STRICT;
CREATE TABLE bil_derivation_inputs (
    derivation_id TEXT NOT NULL REFERENCES bil_derivations (id),
    kind          TEXT NOT NULL CHECK (kind IN ('order_version', 'component_state',
        'result_revision', 'review', 'report_revision', 'execution', 'quarantine_item')),
    ref_id        TEXT NOT NULL,
    PRIMARY KEY (derivation_id, kind, ref_id)
) STRICT;

CREATE TABLE bil_handoffs (
    id              TEXT    NOT NULL PRIMARY KEY,
    format          TEXT    NOT NULL CHECK (format = 'bil-handoff-v1'),
    artifact_sha256 TEXT    NOT NULL CHECK (length(artifact_sha256) = 64),
    item_count      INTEGER NOT NULL CHECK (item_count >= 1),
    created_by      TEXT    NOT NULL REFERENCES auth_users (id),
    created_at      TEXT    NOT NULL
) STRICT;
CREATE TABLE bil_handoff_items (
    id             TEXT NOT NULL PRIMARY KEY,
    handoff_id     TEXT NOT NULL REFERENCES bil_handoffs (id),
    derivation_id  TEXT NOT NULL UNIQUE REFERENCES bil_derivations (id),
    kind           TEXT NOT NULL CHECK (kind IN ('original', 'adjustment')),
    adjusts_item_id TEXT REFERENCES bil_handoff_items (id),
    CHECK ((kind = 'adjustment') = (adjusts_item_id IS NOT NULL))
) STRICT;
CREATE TABLE bil_handoff_outcomes (
    item_id            TEXT NOT NULL PRIMARY KEY REFERENCES bil_handoff_items (id),
    outcome            TEXT NOT NULL CHECK (outcome IN ('acknowledged', 'rejected')),
    external_reference TEXT,
    reason             TEXT,
    recorded_by        TEXT NOT NULL REFERENCES auth_users (id),
    recorded_at        TEXT NOT NULL,
    CHECK (outcome = 'acknowledged' OR reason IS NOT NULL)
) STRICT;

CREATE VIEW bil_v_handoff_status AS
SELECT item.id AS item_id, item.handoff_id, item.derivation_id,
       CASE WHEN outcome.outcome IS NULL THEN 'outstanding' ELSE outcome.outcome END AS status,
       handoff.created_at
FROM bil_handoff_items item
JOIN bil_handoffs handoff ON handoff.id = item.handoff_id
LEFT JOIN bil_handoff_outcomes outcome ON outcome.item_id = item.id;

-- Derivation evidence and handoff ledgers are append-only. Evidence may be
-- inserted only while its parent is under construction; sealing is one-way.
CREATE TRIGGER bil_derivation_specimens_insert_unsealed BEFORE INSERT ON bil_derivation_specimens
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;
CREATE TRIGGER bil_derivation_components_insert_unsealed BEFORE INSERT ON bil_derivation_components
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;
CREATE TRIGGER bil_derivation_classes_insert_unsealed BEFORE INSERT ON bil_derivation_classes
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;
CREATE TRIGGER bil_derivation_flags_insert_unsealed BEFORE INSERT ON bil_derivation_flags
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;
CREATE TRIGGER bil_derivation_inputs_insert_unsealed BEFORE INSERT ON bil_derivation_inputs
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;
CREATE TRIGGER bil_derivation_specimens_no_update BEFORE UPDATE ON bil_derivation_specimens
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_specimens_no_delete BEFORE DELETE ON bil_derivation_specimens
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_components_no_update BEFORE UPDATE ON bil_derivation_components
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_components_no_delete BEFORE DELETE ON bil_derivation_components
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_classes_no_update BEFORE UPDATE ON bil_derivation_classes
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_classes_no_delete BEFORE DELETE ON bil_derivation_classes
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_flags_no_update BEFORE UPDATE ON bil_derivation_flags
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_flags_no_delete BEFORE DELETE ON bil_derivation_flags
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_inputs_no_update BEFORE UPDATE ON bil_derivation_inputs
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivation_inputs_no_delete BEFORE DELETE ON bil_derivation_inputs
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;
CREATE TRIGGER bil_derivations_no_delete BEFORE DELETE ON bil_derivations
BEGIN SELECT RAISE(ABORT, 'billing derivations are never deleted'); END;
CREATE TRIGGER bil_derivations_immutable_fields BEFORE UPDATE ON bil_derivations
WHEN NEW.id IS NOT OLD.id OR NEW.subject_id IS NOT OLD.subject_id
  OR NEW.date_of_service IS NOT OLD.date_of_service OR NEW.dos_rule IS NOT OLD.dos_rule
  OR NEW.derivation_no IS NOT OLD.derivation_no
  OR NEW.mapping_version_id IS NOT OLD.mapping_version_id
  OR NEW.inputs_sha256 IS NOT OLD.inputs_sha256
  OR NEW.completeness IS NOT OLD.completeness
  OR NEW.adjustment_of_id IS NOT OLD.adjustment_of_id
  OR NEW.derived_by IS NOT OLD.derived_by OR NEW.derived_at IS NOT OLD.derived_at
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are immutable'); END;
CREATE TRIGGER bil_derivations_transition BEFORE UPDATE ON bil_derivations
WHEN NOT (
    (OLD.facts_sealed = 0 AND NEW.facts_sealed = 1
      AND NEW.state IS OLD.state AND NEW.revision = OLD.revision
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at)
 OR (OLD.facts_sealed = 1 AND NEW.facts_sealed = 1
      AND OLD.state = 'candidate' AND NEW.state = 'approved'
      AND NEW.revision = OLD.revision + 1
      AND NEW.approved_by IS NOT NULL AND NEW.approved_at IS NOT NULL
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.approve_service_derivation'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_derivation'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.principal_id = NEW.approved_by AND e.occurred_at = NEW.approved_at
            AND e.authority = 'billing'))
 OR (OLD.facts_sealed = 1 AND NEW.facts_sealed = 1
      AND OLD.state = 'approved' AND NEW.state = 'handed_off'
      AND NEW.revision = OLD.revision + 1
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.create_handoff'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_derivation'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.principal_kind = 'user'
            AND e.authority = 'billing'))
 OR (OLD.facts_sealed = 1 AND NEW.facts_sealed = 1
      AND OLD.state IN ('held', 'candidate', 'approved') AND NEW.state = 'superseded'
      AND NEW.revision = OLD.revision + 1
      AND NEW.approved_by IS OLD.approved_by AND NEW.approved_at IS OLD.approved_at
      AND EXISTS (SELECT 1 FROM audit_events e
          WHERE e.stream = 'clinical' AND e.action = 'billing.supersede_derivation'
            AND e.outcome = 'succeeded' AND e.record_type = 'billing_derivation'
            AND e.record_id = NEW.id AND e.old_revision = OLD.revision
            AND e.new_revision = NEW.revision AND e.authority = 'billing'))
)
BEGIN SELECT RAISE(ABORT, 'billing derivation transition requires its authorized audit event'); END;
CREATE TRIGGER bil_handoffs_no_update BEFORE UPDATE ON bil_handoffs
BEGIN SELECT RAISE(ABORT, 'billing handoffs are append-only'); END;
CREATE TRIGGER bil_handoffs_no_delete BEFORE DELETE ON bil_handoffs
BEGIN SELECT RAISE(ABORT, 'billing handoffs are append-only'); END;
CREATE TRIGGER bil_handoff_items_no_update BEFORE UPDATE ON bil_handoff_items
BEGIN SELECT RAISE(ABORT, 'billing handoff items are append-only'); END;
CREATE TRIGGER bil_handoff_items_no_delete BEFORE DELETE ON bil_handoff_items
BEGIN SELECT RAISE(ABORT, 'billing handoff items are append-only'); END;
CREATE TRIGGER bil_handoff_outcomes_no_update BEFORE UPDATE ON bil_handoff_outcomes
BEGIN SELECT RAISE(ABORT, 'billing handoff outcomes are append-only'); END;
CREATE TRIGGER bil_handoff_outcomes_no_delete BEFORE DELETE ON bil_handoff_outcomes
BEGIN SELECT RAISE(ABORT, 'billing handoff outcomes are append-only'); END;
