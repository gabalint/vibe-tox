-- Orders received from a clinic's EMR over an HL7 interface (Interfaces, ticket
-- #4555 step 3c): a third source kind, so an electronic order is never recorded
-- as paper or fax. Rebuild the CHECK-constrained table with the foreign-key
-- checked migration procedure; existing rows keep their values. The component
-- view and the billing context trigger read ord_orders, so they are dropped
-- first and made again after, unchanged, as a rename checks every view and
-- trigger.

DROP VIEW ord_v_components;
DROP TRIGGER ord_billing_context_version;

CREATE TABLE ord_orders_next (
    id                     TEXT    NOT NULL PRIMARY KEY,
    subject_id             TEXT    NOT NULL REFERENCES idn_subjects (id),
    client_id              TEXT    NOT NULL REFERENCES idn_client_organizations (id),
    client_location_id     TEXT    REFERENCES idn_client_locations (id),
    practitioner_id        TEXT    NOT NULL REFERENCES idn_practitioners (id),
    placer_order_id        TEXT,
    placer_authority       TEXT,
    source_kind            TEXT    NOT NULL
        CHECK (source_kind IN ('paper_requisition', 'fax', 'hl7_interface')),
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

INSERT INTO ord_orders_next (
    id, subject_id, client_id, client_location_id, practitioner_id, placer_order_id,
    placer_authority, source_kind, source_document_ref, billing_responsibility,
    facility_setting, standing_order_id, template_use, template_name, template_confirmed,
    status, version, received_at, received_by_kind, received_by_id, decided_at, decided_by,
    reject_reason, revision
)
SELECT id, subject_id, client_id, client_location_id, practitioner_id, placer_order_id,
       placer_authority, source_kind, source_document_ref, billing_responsibility,
       facility_setting, standing_order_id, template_use, template_name, template_confirmed,
       status, version, received_at, received_by_kind, received_by_id, decided_at, decided_by,
       reject_reason, revision
FROM ord_orders;

DROP TABLE ord_orders;
ALTER TABLE ord_orders_next RENAME TO ord_orders;

CREATE INDEX ord_orders_received_page
    ON ord_orders (received_at DESC, id DESC);
CREATE INDEX ord_orders_status_received_page
    ON ord_orders (status, received_at DESC, id DESC);
CREATE INDEX ord_orders_subject_received_page
    ON ord_orders (subject_id, received_at DESC, id DESC);

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

CREATE TRIGGER ord_billing_context_version BEFORE INSERT ON ord_billing_contexts
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM ord_billing_contexts WHERE order_id = NEW.order_id)
  OR NEW.subject_id <> (SELECT subject_id FROM ord_orders WHERE id = NEW.order_id)
BEGIN SELECT RAISE(ABORT, 'billing context identity or version mismatch'); END;
