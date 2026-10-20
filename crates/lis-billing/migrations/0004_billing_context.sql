-- Billing context evidence, v2 exports and append-only reconciliation (BILL-G04/G06/G09).
DROP VIEW bil_v_handoff_status;


CREATE TABLE bil_derivation_flags_next (
    derivation_id TEXT    NOT NULL REFERENCES bil_derivations (id),
    code          TEXT    NOT NULL CHECK (code IN ('mapping_missing', 'date_of_service_unknown',
        'report_not_final', 'laboratory_billing_identity_missing', 'ordered_not_performed',
        'presumptive_and_definitive_same_specimen', 'multiple_presumptive_same_dos',
        'performed_not_ordered', 'failed_attempt_only', 'performed_not_reported', 'billing_context_missing', 'billing_context_incomplete', 'billing_context_conflict', 'billing_review_open', 'collection_review_pending')),
    ref_id        TEXT    NOT NULL DEFAULT '',
    blocking      INTEGER NOT NULL CHECK (blocking IN (0, 1)),
    PRIMARY KEY (derivation_id, code, ref_id)
) STRICT;

INSERT INTO bil_derivation_flags_next SELECT * FROM bil_derivation_flags;
DROP TABLE bil_derivation_flags;
ALTER TABLE bil_derivation_flags_next RENAME TO bil_derivation_flags;

CREATE TRIGGER bil_derivation_flags_insert_unsealed BEFORE INSERT ON bil_derivation_flags
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;

CREATE TRIGGER bil_derivation_flags_no_update BEFORE UPDATE ON bil_derivation_flags
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;

CREATE TRIGGER bil_derivation_flags_no_delete BEFORE DELETE ON bil_derivation_flags
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;

CREATE TABLE bil_derivation_inputs_next (
    derivation_id TEXT NOT NULL REFERENCES bil_derivations (id),
    kind          TEXT NOT NULL CHECK (kind IN ('order_version', 'component_state',
        'result_revision', 'review', 'report_revision', 'execution', 'quarantine_item', 'billing_context', 'billing_review', 'collection_revision', 'conflict_resolution')),
    ref_id        TEXT NOT NULL,
    PRIMARY KEY (derivation_id, kind, ref_id)
) STRICT;

INSERT INTO bil_derivation_inputs_next SELECT * FROM bil_derivation_inputs;
DROP TABLE bil_derivation_inputs;
ALTER TABLE bil_derivation_inputs_next RENAME TO bil_derivation_inputs;

CREATE TRIGGER bil_derivation_inputs_insert_unsealed BEFORE INSERT ON bil_derivation_inputs
WHEN COALESCE((SELECT facts_sealed FROM bil_derivations WHERE id = NEW.derivation_id), 1) <> 0
BEGIN SELECT RAISE(ABORT, 'billing derivation facts are sealed'); END;

CREATE TRIGGER bil_derivation_inputs_no_update BEFORE UPDATE ON bil_derivation_inputs
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;

CREATE TRIGGER bil_derivation_inputs_no_delete BEFORE DELETE ON bil_derivation_inputs
BEGIN SELECT RAISE(ABORT, 'billing derivation evidence is append-only'); END;

CREATE TABLE bil_handoffs_next (
    id              TEXT    NOT NULL PRIMARY KEY,
    format          TEXT    NOT NULL CHECK (format IN ('bil-handoff-v1', 'bil-handoff-v2')),
    artifact_sha256 TEXT    NOT NULL CHECK (length(artifact_sha256) = 64),
    item_count      INTEGER NOT NULL CHECK (item_count >= 1),
    created_by      TEXT    NOT NULL REFERENCES auth_users (id),
    created_at      TEXT    NOT NULL,
    handoff_no INTEGER NOT NULL UNIQUE
) STRICT;

INSERT INTO bil_handoffs_next SELECT id, format, artifact_sha256, item_count, created_by, created_at, handoff_no FROM bil_handoffs;
DROP TABLE bil_handoffs;
ALTER TABLE bil_handoffs_next RENAME TO bil_handoffs;

CREATE TRIGGER bil_handoffs_no_update BEFORE UPDATE ON bil_handoffs
BEGIN SELECT RAISE(ABORT, 'billing handoffs are append-only'); END;

CREATE TRIGGER bil_handoffs_no_delete BEFORE DELETE ON bil_handoffs
BEGIN SELECT RAISE(ABORT, 'billing handoffs are append-only'); END;

CREATE TRIGGER bil_handoffs_number_required BEFORE INSERT ON bil_handoffs
WHEN NEW.handoff_no <> (SELECT COALESCE(max(handoff_no), 0) + 1 FROM bil_handoffs)
BEGIN SELECT RAISE(ABORT, 'billing handoffs need the next sequential number'); END;
CREATE TABLE bil_handoff_followups (
 item_id TEXT NOT NULL REFERENCES bil_handoff_items(id),
 revision INTEGER NOT NULL CHECK (revision >= 2),
 outcome TEXT NOT NULL CHECK (outcome IN ('acknowledged', 'rejected')),
 external_reference TEXT,
 reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
 recorded_by TEXT NOT NULL REFERENCES auth_users(id),
 recorded_at TEXT NOT NULL,
 PRIMARY KEY (item_id, revision)
) STRICT;
CREATE VIEW bil_v_handoff_outcome_history AS
 SELECT item_id, 1 AS revision, outcome, external_reference, reason, recorded_by, recorded_at FROM bil_handoff_outcomes
 UNION ALL
 SELECT item_id, revision, outcome, external_reference, reason, recorded_by, recorded_at FROM bil_handoff_followups;
CREATE VIEW bil_v_latest_handoff_outcomes AS
 SELECT h.* FROM bil_v_handoff_outcome_history h WHERE h.revision =
 (SELECT max(revision) FROM bil_v_handoff_outcome_history newer WHERE newer.item_id = h.item_id);
CREATE VIEW bil_v_handoff_status AS
 SELECT item.id AS item_id, item.handoff_id, item.derivation_id,
 COALESCE(outcome.outcome, 'outstanding') AS status, handoff.created_at
 FROM bil_handoff_items item JOIN bil_handoffs handoff ON handoff.id = item.handoff_id
 LEFT JOIN bil_v_latest_handoff_outcomes outcome ON outcome.item_id = item.id;
CREATE TRIGGER bil_handoff_followups_sequence BEFORE INSERT ON bil_handoff_followups
WHEN NOT EXISTS (SELECT 1 FROM bil_v_latest_handoff_outcomes prior
 WHERE prior.item_id = NEW.item_id AND prior.revision + 1 = NEW.revision AND prior.outcome = 'rejected')
BEGIN SELECT RAISE(ABORT, 'follow-up requires current rejected outcome'); END;
CREATE TRIGGER bil_handoff_followups_no_update BEFORE UPDATE ON bil_handoff_followups
BEGIN SELECT RAISE(ABORT, 'handoff reconciliation is append-only'); END;
CREATE TRIGGER bil_handoff_followups_no_delete BEFORE DELETE ON bil_handoff_followups
BEGIN SELECT RAISE(ABORT, 'handoff reconciliation is append-only'); END;
CREATE TABLE bil_context_resolutions (
 id TEXT NOT NULL PRIMARY KEY,
 derivation_id TEXT NOT NULL REFERENCES bil_derivations(id),
 inputs_sha256 TEXT NOT NULL UNIQUE CHECK (length(inputs_sha256) = 64),
 reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),
 recorded_by TEXT NOT NULL REFERENCES auth_users(id),
 recorded_at TEXT NOT NULL
) STRICT;
CREATE TRIGGER bil_context_resolutions_no_update BEFORE UPDATE ON bil_context_resolutions
BEGIN SELECT RAISE(ABORT, 'billing context resolutions are append-only'); END;
CREATE TRIGGER bil_context_resolutions_no_delete BEFORE DELETE ON bil_context_resolutions
BEGIN SELECT RAISE(ABORT, 'billing context resolutions are append-only'); END;
