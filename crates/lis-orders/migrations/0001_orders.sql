-- Orders, frozen catalog manifests, supplied evidence, clarification holds,
-- standing orders and request/decision records (REQ-ORD-001–003, 005–007;
-- INV-13). Clinical content is never inferred or defaulted.

CREATE TABLE ord_orders (
    id                     TEXT    NOT NULL PRIMARY KEY,
    subject_id             TEXT    NOT NULL REFERENCES idn_subjects (id),
    client_id              TEXT    NOT NULL REFERENCES idn_client_organizations (id),
    client_location_id     TEXT    REFERENCES idn_client_locations (id),
    practitioner_id        TEXT    NOT NULL REFERENCES idn_practitioners (id),
    placer_order_id        TEXT,
    placer_authority       TEXT,
    source_kind            TEXT    NOT NULL CHECK (source_kind IN ('paper_requisition', 'fax')),
    source_document_ref    TEXT,
    billing_responsibility TEXT    NOT NULL CHECK (billing_responsibility IN
                                ('third_party', 'patient_self_pay', 'client', 'not_supplied')),
    facility_setting       TEXT,
    standing_order_id      TEXT REFERENCES ord_standing_orders (id),
    template_use           TEXT    NOT NULL CHECK (template_use IN ('none', 'clinic_template')),
    template_name          TEXT,
    template_confirmed     INTEGER CHECK (template_confirmed IN (0, 1)),
    status                 TEXT    NOT NULL CHECK (status IN ('received', 'accepted', 'rejected')),
    version                INTEGER NOT NULL CHECK (version >= 1),
    received_at            TEXT    NOT NULL,
    received_by_kind       TEXT    NOT NULL CHECK (received_by_kind IN ('user', 'instrument', 'system')),
    received_by_id         TEXT    NOT NULL,
    decided_at             TEXT,
    decided_by             TEXT,
    reject_reason          TEXT,
    revision               INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE (placer_authority, placer_order_id),
    CHECK ((placer_order_id IS NULL) = (placer_authority IS NULL)),
    CHECK ((template_use = 'none' AND template_name IS NULL AND template_confirmed IS NULL)
        OR (template_use = 'clinic_template' AND template_name IS NOT NULL
            AND template_confirmed IS NOT NULL)),
    CHECK ((status = 'received') = (decided_at IS NULL)),
    CHECK ((status = 'rejected') = (reject_reason IS NOT NULL))
) STRICT;

CREATE TABLE ord_order_items (
    id                       TEXT    NOT NULL PRIMARY KEY,
    order_id                 TEXT    NOT NULL REFERENCES ord_orders (id),
    orderable_version_id     TEXT    NOT NULL REFERENCES cat_orderable_versions (id),
    kind                     TEXT    NOT NULL CHECK (kind IN ('initial', 'add_on', 'reflex')),
    parent_item_id           TEXT    REFERENCES ord_order_items (id),
    reflex_authorization     TEXT    NOT NULL CHECK (reflex_authorization IN
                                  ('authorized', 'not_authorized', 'not_stated')),
    authority_reference      TEXT,
    state                    TEXT    NOT NULL CHECK (state IN
                                  ('requested', 'accepted', 'declined', 'cancelled')),
    requested_at             TEXT    NOT NULL,
    decided_at               TEXT,
    decided_by               TEXT,
    reason                   TEXT,
    revision                 INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (kind <> 'initial' OR parent_item_id IS NULL),
    CHECK (kind <> 'reflex' OR parent_item_id IS NOT NULL),
    CHECK (kind = 'initial' OR reason IS NOT NULL),
    CHECK ((state = 'requested') = (decided_at IS NULL))
) STRICT;

CREATE INDEX ord_order_items_order ON ord_order_items (order_id);

CREATE TABLE ord_reflex_scope (
    item_id      TEXT NOT NULL REFERENCES ord_order_items (id),
    orderable_id TEXT NOT NULL REFERENCES cat_orderables (id),
    PRIMARY KEY (item_id, orderable_id)
) STRICT;

CREATE TABLE ord_order_components (
    id                           TEXT    NOT NULL PRIMARY KEY,
    item_id                      TEXT    NOT NULL REFERENCES ord_order_items (id),
    catalog_orderable_version_id TEXT    NOT NULL,
    catalog_component_position   INTEGER NOT NULL CHECK (catalog_component_position >= 1),
    specimen_type_code           TEXT    NOT NULL,
    purpose                      TEXT    NOT NULL CHECK (purpose IN ('analytical', 'validity')),
    sequence_role                TEXT    NOT NULL CHECK (sequence_role IN ('first_line', 'confirmation')),
    state                        TEXT    NOT NULL CHECK (state IN ('active', 'cancelled')),
    revision                     INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE (item_id, catalog_component_position),
    FOREIGN KEY (catalog_orderable_version_id, catalog_component_position)
        REFERENCES cat_orderable_components (orderable_version_id, position)
) STRICT;

CREATE INDEX ord_order_components_item ON ord_order_components (item_id);

CREATE TABLE ord_authority_evidence (
    id                 TEXT    NOT NULL PRIMARY KEY,
    order_id           TEXT    NOT NULL REFERENCES ord_orders (id),
    kind               TEXT    NOT NULL CHECK (kind IN
                         ('wet_signature', 'electronic_signature', 'intent_attestation', 'not_supplied')),
    document_ref       TEXT,
    added_in_version   INTEGER NOT NULL CHECK (added_in_version >= 1),
    removed_in_version INTEGER CHECK (removed_in_version > added_in_version)
) STRICT;

CREATE TABLE ord_diagnoses (
    id                     TEXT    NOT NULL PRIMARY KEY,
    order_id               TEXT    NOT NULL REFERENCES ord_orders (id),
    code                   TEXT    NOT NULL,
    code_system            TEXT    NOT NULL,
    code_system_version    TEXT,
    document_ref           TEXT    NOT NULL,
    preprinted_on_lab_form INTEGER NOT NULL CHECK (preprinted_on_lab_form IN (0, 1)),
    added_in_version       INTEGER NOT NULL CHECK (added_in_version >= 1),
    removed_in_version     INTEGER CHECK (removed_in_version > added_in_version)
) STRICT;

CREATE TABLE ord_medication_statements (
    id                 TEXT    NOT NULL PRIMARY KEY,
    order_id           TEXT    NOT NULL REFERENCES ord_orders (id),
    received_text      TEXT    NOT NULL,
    code               TEXT,
    code_system        TEXT,
    source             TEXT    NOT NULL CHECK (source IN ('ordering_clinic', 'patient', 'other')),
    reporter           TEXT,
    provided_on        TEXT,
    as_of_date         TEXT,
    added_in_version   INTEGER NOT NULL CHECK (added_in_version >= 1),
    removed_in_version INTEGER CHECK (removed_in_version > added_in_version),
    CHECK ((code IS NULL) = (code_system IS NULL))
) STRICT;

CREATE TABLE ord_poc_results (
    id                 TEXT    NOT NULL PRIMARY KEY,
    order_id           TEXT    NOT NULL REFERENCES ord_orders (id),
    device_or_test     TEXT    NOT NULL,
    analyte_text       TEXT    NOT NULL,
    result_text        TEXT    NOT NULL,
    performed_at_text  TEXT,
    source             TEXT    NOT NULL,
    added_in_version   INTEGER NOT NULL CHECK (added_in_version >= 1),
    removed_in_version INTEGER CHECK (removed_in_version > added_in_version)
) STRICT;

CREATE TABLE ord_clarification_holds (
    id                TEXT    NOT NULL PRIMARY KEY,
    order_id          TEXT    NOT NULL REFERENCES ord_orders (id),
    deficiency        TEXT    NOT NULL CHECK (deficiency IN
                         ('requestor_missing', 'test_selection_missing', 'signature_or_intent_required',
                          'standing_order_expired', 'standing_order_exhausted',
                          'standing_order_out_of_scope', 'reflex_authority_missing', 'other_authority')),
    origin            TEXT    NOT NULL CHECK (origin IN ('user', 'standing_order_check')),
    note              TEXT,
    state             TEXT    NOT NULL CHECK (state IN ('open', 'resolved')),
    opened_by         TEXT    NOT NULL,
    opened_at         TEXT    NOT NULL,
    resolved_by       TEXT,
    resolved_at       TEXT,
    resolution_reason TEXT,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'open') = (resolved_at IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolved_by IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolution_reason IS NULL))
) STRICT;

CREATE INDEX ord_open_holds_order ON ord_clarification_holds (order_id) WHERE state = 'open';

CREATE TABLE ord_billing_review_items (
    id        TEXT NOT NULL PRIMARY KEY,
    order_id  TEXT NOT NULL REFERENCES ord_orders (id),
    element   TEXT NOT NULL CHECK (element IN
                ('diagnosis', 'lcd_rationale', 'payer_signature', 'enrollment_status', 'other')),
    note      TEXT,
    opened_by TEXT NOT NULL,
    opened_at TEXT NOT NULL
) STRICT;

CREATE TABLE ord_standing_orders (
    id              TEXT    NOT NULL PRIMARY KEY,
    subject_id      TEXT    NOT NULL REFERENCES idn_subjects (id),
    practitioner_id TEXT    NOT NULL REFERENCES idn_practitioners (id),
    valid_from      TEXT    NOT NULL,
    valid_until     TEXT    NOT NULL,
    visit_limit     INTEGER CHECK (visit_limit > 0),
    document_ref    TEXT    NOT NULL,
    recorded_by     TEXT    NOT NULL,
    recorded_at     TEXT    NOT NULL,
    CHECK (valid_from <= valid_until)
) STRICT;

CREATE TABLE ord_standing_order_scope (
    standing_order_id TEXT NOT NULL REFERENCES ord_standing_orders (id),
    orderable_id      TEXT NOT NULL REFERENCES cat_orderables (id),
    PRIMARY KEY (standing_order_id, orderable_id)
) STRICT;

CREATE TABLE ord_cancellation_requests (
    id                TEXT    NOT NULL PRIMARY KEY,
    item_id           TEXT    NOT NULL REFERENCES ord_order_items (id),
    requested_by_text TEXT    NOT NULL,
    reason            TEXT    NOT NULL,
    state             TEXT    NOT NULL CHECK (state IN ('requested', 'accepted', 'declined')),
    decided_by        TEXT,
    decided_at        TEXT,
    decision_reason   TEXT,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'requested') = (decided_at IS NULL)),
    CHECK ((decided_at IS NULL) = (decided_by IS NULL)),
    CHECK ((decided_at IS NULL) = (decision_reason IS NULL))
) STRICT;

CREATE TABLE ord_trigger_assessments (
    id             TEXT NOT NULL PRIMARY KEY,
    parent_item_id TEXT NOT NULL REFERENCES ord_order_items (id),
    orderable_id   TEXT NOT NULL REFERENCES cat_orderables (id),
    outcome        TEXT NOT NULL CHECK (outcome IN ('activated', 'condition_not_met')),
    trigger_source TEXT NOT NULL CHECK (trigger_source IN ('laboratory_result', 'client_point_of_care')),
    trigger_ref    TEXT NOT NULL,
    created_item_id TEXT REFERENCES ord_order_items (id),
    reason         TEXT NOT NULL,
    decided_by     TEXT NOT NULL,
    decided_at     TEXT NOT NULL,
    UNIQUE (parent_item_id, orderable_id),
    CHECK ((outcome = 'activated') = (created_item_id IS NOT NULL))
) STRICT;

-- Stable cross-module read contract. The catalog component is the immutable
-- pair (orderable version, position); purpose is copied from that approved
-- catalog component when the order item is accepted (INV-13).
CREATE VIEW ord_v_components AS
SELECT c.id AS component_id, i.order_id, o.subject_id, c.item_id,
       i.kind AS item_kind, i.state AS item_state, c.state AS component_state,
       c.catalog_orderable_version_id, c.catalog_component_position,
       c.specimen_type_code, c.purpose, c.sequence_role, o.status AS order_status,
       (SELECT count(*) FROM ord_clarification_holds h
        WHERE h.order_id = o.id AND h.state = 'open') AS open_hold_count
FROM ord_order_components c
JOIN ord_order_items i ON i.id = c.item_id
JOIN ord_orders o ON o.id = i.order_id;

-- Evidence is retained. The sole permitted update retires it in a later
-- evidence version; rows are never deleted or rewritten (REQ-ORD-005/007).
CREATE TRIGGER ord_authority_evidence_retire_only BEFORE UPDATE ON ord_authority_evidence
WHEN NEW.id IS NOT OLD.id OR NEW.order_id IS NOT OLD.order_id OR NEW.kind IS NOT OLD.kind
  OR NEW.document_ref IS NOT OLD.document_ref OR NEW.added_in_version IS NOT OLD.added_in_version
  OR OLD.removed_in_version IS NOT NULL OR NEW.removed_in_version IS NULL
BEGIN SELECT RAISE(ABORT, 'order evidence can only be retired'); END;
CREATE TRIGGER ord_authority_evidence_no_delete BEFORE DELETE ON ord_authority_evidence
BEGIN SELECT RAISE(ABORT, 'order evidence is never deleted'); END;

CREATE TRIGGER ord_diagnoses_retire_only BEFORE UPDATE ON ord_diagnoses
WHEN NEW.id IS NOT OLD.id OR NEW.order_id IS NOT OLD.order_id OR NEW.code IS NOT OLD.code
  OR NEW.code_system IS NOT OLD.code_system OR NEW.code_system_version IS NOT OLD.code_system_version
  OR NEW.document_ref IS NOT OLD.document_ref OR NEW.preprinted_on_lab_form IS NOT OLD.preprinted_on_lab_form
  OR NEW.added_in_version IS NOT OLD.added_in_version OR OLD.removed_in_version IS NOT NULL
  OR NEW.removed_in_version IS NULL
BEGIN SELECT RAISE(ABORT, 'order evidence can only be retired'); END;
CREATE TRIGGER ord_diagnoses_no_delete BEFORE DELETE ON ord_diagnoses
BEGIN SELECT RAISE(ABORT, 'order evidence is never deleted'); END;

CREATE TRIGGER ord_medication_statements_retire_only BEFORE UPDATE ON ord_medication_statements
WHEN NEW.id IS NOT OLD.id OR NEW.order_id IS NOT OLD.order_id OR NEW.received_text IS NOT OLD.received_text
  OR NEW.code IS NOT OLD.code OR NEW.code_system IS NOT OLD.code_system OR NEW.source IS NOT OLD.source
  OR NEW.reporter IS NOT OLD.reporter OR NEW.provided_on IS NOT OLD.provided_on
  OR NEW.as_of_date IS NOT OLD.as_of_date OR NEW.added_in_version IS NOT OLD.added_in_version
  OR OLD.removed_in_version IS NOT NULL OR NEW.removed_in_version IS NULL
BEGIN SELECT RAISE(ABORT, 'order evidence can only be retired'); END;
CREATE TRIGGER ord_medication_statements_no_delete BEFORE DELETE ON ord_medication_statements
BEGIN SELECT RAISE(ABORT, 'order evidence is never deleted'); END;

CREATE TRIGGER ord_poc_results_retire_only BEFORE UPDATE ON ord_poc_results
WHEN NEW.id IS NOT OLD.id OR NEW.order_id IS NOT OLD.order_id OR NEW.device_or_test IS NOT OLD.device_or_test
  OR NEW.analyte_text IS NOT OLD.analyte_text OR NEW.result_text IS NOT OLD.result_text
  OR NEW.performed_at_text IS NOT OLD.performed_at_text OR NEW.source IS NOT OLD.source
  OR NEW.added_in_version IS NOT OLD.added_in_version OR OLD.removed_in_version IS NOT NULL
  OR NEW.removed_in_version IS NULL
BEGIN SELECT RAISE(ABORT, 'order evidence can only be retired'); END;
CREATE TRIGGER ord_poc_results_no_delete BEFORE DELETE ON ord_poc_results
BEGIN SELECT RAISE(ABORT, 'order evidence is never deleted'); END;

CREATE TRIGGER ord_reflex_scope_no_update BEFORE UPDATE ON ord_reflex_scope
BEGIN SELECT RAISE(ABORT, 'reflex scope is append-only'); END;
CREATE TRIGGER ord_reflex_scope_no_delete BEFORE DELETE ON ord_reflex_scope
BEGIN SELECT RAISE(ABORT, 'reflex scope is append-only'); END;
CREATE TRIGGER ord_standing_order_scope_no_update BEFORE UPDATE ON ord_standing_order_scope
BEGIN SELECT RAISE(ABORT, 'standing-order scope is append-only'); END;
CREATE TRIGGER ord_standing_order_scope_no_delete BEFORE DELETE ON ord_standing_order_scope
BEGIN SELECT RAISE(ABORT, 'standing-order scope is append-only'); END;
CREATE TRIGGER ord_trigger_assessments_no_update BEFORE UPDATE ON ord_trigger_assessments
BEGIN SELECT RAISE(ABORT, 'trigger assessments are append-only'); END;
CREATE TRIGGER ord_trigger_assessments_no_delete BEFORE DELETE ON ord_trigger_assessments
BEGIN SELECT RAISE(ABORT, 'trigger assessments are append-only'); END;
