-- Financial versions are separate from clinical holds (BILL-G02/G03).
CREATE TABLE ord_billing_contexts (
    order_id TEXT NOT NULL REFERENCES ord_orders(id),
    version INTEGER NOT NULL CHECK(version >= 1),
    order_version INTEGER NOT NULL CHECK(order_version >= 1),
    subject_id TEXT NOT NULL REFERENCES idn_subjects(id),
    profile_version INTEGER,
    client_id TEXT REFERENCES idn_client_organizations(id),
    responsibility TEXT NOT NULL CHECK(responsibility IN ('third_party','patient_self_pay','client','not_supplied')),
    context_json TEXT NOT NULL CHECK(json_valid(context_json) AND length(context_json) <= 262144),
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY(order_id, version),
    FOREIGN KEY(subject_id, profile_version) REFERENCES idn_patient_billing_profiles(subject_id, version)
) STRICT;

CREATE TRIGGER ord_billing_context_version BEFORE INSERT ON ord_billing_contexts
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM ord_billing_contexts WHERE order_id = NEW.order_id)
  OR NEW.subject_id <> (SELECT subject_id FROM ord_orders WHERE id = NEW.order_id)
BEGIN SELECT RAISE(ABORT, 'billing context identity or version mismatch'); END;
CREATE TRIGGER ord_billing_context_no_update BEFORE UPDATE ON ord_billing_contexts
BEGIN SELECT RAISE(ABORT, 'billing contexts are immutable'); END;
CREATE TRIGGER ord_billing_context_no_delete BEFORE DELETE ON ord_billing_contexts
BEGIN SELECT RAISE(ABORT, 'billing contexts are retained'); END;

CREATE TABLE ord_billing_review_actions (
    item_id TEXT NOT NULL REFERENCES ord_billing_review_items(id),
    version INTEGER NOT NULL CHECK(version >= 2),
    status TEXT NOT NULL CHECK(status IN ('open','resolved')),
    owner TEXT,
    source_ref TEXT,
    reason TEXT NOT NULL,
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY(item_id, version),
    CHECK(status <> 'resolved' OR (owner IS NOT NULL AND source_ref IS NOT NULL))
) STRICT;
CREATE TRIGGER ord_billing_review_action_version BEFORE INSERT ON ord_billing_review_actions
WHEN NEW.version <> (SELECT coalesce(max(version), 1) + 1 FROM ord_billing_review_actions WHERE item_id = NEW.item_id)
BEGIN SELECT RAISE(ABORT, 'billing review version must be consecutive'); END;
CREATE TRIGGER ord_billing_review_action_no_update BEFORE UPDATE ON ord_billing_review_actions
BEGIN SELECT RAISE(ABORT, 'billing review actions are immutable'); END;
CREATE TRIGGER ord_billing_review_action_no_delete BEFORE DELETE ON ord_billing_review_actions
BEGIN SELECT RAISE(ABORT, 'billing review actions are retained'); END;
CREATE INDEX ord_billing_review_order ON ord_billing_review_items(order_id, opened_at);
