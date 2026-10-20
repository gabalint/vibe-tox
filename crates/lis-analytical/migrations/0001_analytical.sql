-- Analytical module (REQ-ANA-001, REQ-ANA-003 registration part, REQ-ANA-004–007).
-- Design: docs/planning/m2-design-brief.md §2.4. Evidence tables are
-- append-only; mutable aggregates change only along their state machine.

-- Instrument readiness: an event log. No event means not ready (R14).
CREATE TABLE ana_instrument_status_events (
    id            TEXT NOT NULL PRIMARY KEY,
    instrument_id TEXT NOT NULL REFERENCES ifc_instruments (id),
    status        TEXT NOT NULL CHECK (status IN ('in_service', 'out_of_service')),
    reason        TEXT NOT NULL,
    recorded_by   TEXT NOT NULL,
    recorded_at   TEXT NOT NULL
) STRICT;

CREATE INDEX ana_instrument_status_events_instrument
    ON ana_instrument_status_events (instrument_id, recorded_at);

CREATE TRIGGER ana_instrument_status_events_no_update BEFORE UPDATE ON ana_instrument_status_events
BEGIN SELECT RAISE(ABORT, 'instrument status events are append-only'); END;
CREATE TRIGGER ana_instrument_status_events_no_delete BEFORE DELETE ON ana_instrument_status_events
BEGIN SELECT RAISE(ABORT, 'instrument status events are append-only'); END;

-- ASTM source registrations: draft -> active -> retired; one active per
-- instrument; the approver is never the author; content is immutable.
CREATE TABLE ana_source_registrations (
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
    CHECK ((state = 'retired') = (retired_at IS NOT NULL)),
    -- Separation of duties: the approver is never the author.
    CHECK (approved_by IS NULL OR approved_by <> authored_by)
) STRICT;

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

-- The registration's test-code map to catalog analytes, fixed at approval.
CREATE TABLE ana_source_test_codes (
    registration_id  TEXT NOT NULL REFERENCES ana_source_registrations (id),
    source_test_code TEXT NOT NULL,
    analyte_id       TEXT NOT NULL REFERENCES cat_analytes (id),
    PRIMARY KEY (registration_id, source_test_code)
) STRICT;

CREATE TRIGGER ana_source_test_codes_draft_only BEFORE INSERT ON ana_source_test_codes
WHEN (SELECT state FROM ana_source_registrations WHERE id = NEW.registration_id) IS NOT 'draft'
BEGIN SELECT RAISE(ABORT, 'test codes are fixed once a registration is approved'); END;
CREATE TRIGGER ana_source_test_codes_no_update BEFORE UPDATE ON ana_source_test_codes
BEGIN SELECT RAISE(ABORT, 'source test codes are append-only'); END;
CREATE TRIGGER ana_source_test_codes_no_delete BEFORE DELETE ON ana_source_test_codes
BEGIN SELECT RAISE(ABORT, 'source test codes are append-only'); END;

-- Material lots: active -> withdrawn; expiry is derived from expires_on.
-- Controls and calibrators carry the lab-defined sample label the analyzer reports.
CREATE TABLE ana_lots (
    id              TEXT    NOT NULL PRIMARY KEY,
    material_kind   TEXT    NOT NULL
        CHECK (material_kind IN ('reagent', 'calibrator', 'control', 'internal_standard')),
    product_name    TEXT    NOT NULL,
    lot_number      TEXT    NOT NULL,
    level_text      TEXT,
    sample_label    TEXT,
    expires_on      TEXT    NOT NULL
        CHECK (expires_on GLOB '[0-9][0-9][0-9][0-9]-[0-9][0-9]-[0-9][0-9]'),
    state           TEXT    NOT NULL CHECK (state IN ('active', 'withdrawn')),
    recorded_by     TEXT    NOT NULL,
    recorded_at     TEXT    NOT NULL,
    withdrawn_by    TEXT,
    withdrawn_at    TEXT,
    withdraw_reason TEXT,
    revision        INTEGER NOT NULL CHECK (revision >= 1),
    UNIQUE (material_kind, product_name, lot_number),
    CHECK ((material_kind IN ('control', 'calibrator')) = (sample_label IS NOT NULL)),
    CHECK ((state = 'withdrawn') = (withdrawn_at IS NOT NULL)),
    CHECK ((withdrawn_at IS NULL) = (withdrawn_by IS NULL)),
    CHECK ((withdrawn_at IS NULL) = (withdraw_reason IS NULL))
) STRICT;

CREATE TRIGGER ana_lots_withdraw_only BEFORE UPDATE ON ana_lots
WHEN NEW.id IS NOT OLD.id OR NEW.material_kind IS NOT OLD.material_kind
  OR NEW.product_name IS NOT OLD.product_name OR NEW.lot_number IS NOT OLD.lot_number
  OR NEW.level_text IS NOT OLD.level_text OR NEW.sample_label IS NOT OLD.sample_label
  OR NEW.expires_on IS NOT OLD.expires_on OR NEW.recorded_by IS NOT OLD.recorded_by
  OR NEW.recorded_at IS NOT OLD.recorded_at OR NEW.revision IS NOT OLD.revision + 1
  OR OLD.state <> 'active' OR NEW.state <> 'withdrawn'
BEGIN SELECT RAISE(ABORT, 'a lot only moves from active to withdrawn'); END;
CREATE TRIGGER ana_lots_no_delete BEFORE DELETE ON ana_lots
BEGIN SELECT RAISE(ABORT, 'lots are never deleted'); END;

-- Preparation batches and their ordered steps (REQ-ANA-001).
CREATE TABLE ana_prep_batches (
    id                TEXT NOT NULL PRIMARY KEY,
    method_version_id TEXT NOT NULL REFERENCES cat_method_versions (id),
    created_by        TEXT NOT NULL,
    created_at        TEXT NOT NULL
) STRICT;

CREATE TRIGGER ana_prep_batches_no_update BEFORE UPDATE ON ana_prep_batches
BEGIN SELECT RAISE(ABORT, 'preparation batches are append-only'); END;
CREATE TRIGGER ana_prep_batches_no_delete BEFORE DELETE ON ana_prep_batches
BEGIN SELECT RAISE(ABORT, 'preparation batches are append-only'); END;

CREATE TABLE ana_prep_steps (
    id           TEXT    NOT NULL PRIMARY KEY,
    batch_id     TEXT    NOT NULL REFERENCES ana_prep_batches (id),
    seq          INTEGER NOT NULL CHECK (seq >= 1),
    step_code    TEXT    NOT NULL,
    lot_id       TEXT    REFERENCES ana_lots (id),
    note         TEXT,
    performed_by TEXT    NOT NULL,
    performed_at TEXT    NOT NULL,
    UNIQUE (batch_id, seq)
) STRICT;

CREATE TRIGGER ana_prep_steps_no_update BEFORE UPDATE ON ana_prep_steps
BEGIN SELECT RAISE(ABORT, 'preparation steps are append-only'); END;
CREATE TRIGGER ana_prep_steps_no_delete BEFORE DELETE ON ana_prep_steps
BEGIN SELECT RAISE(ABORT, 'preparation steps are append-only'); END;

-- Instrument runs: open -> closed. Positions and imports only while open.
CREATE TABLE ana_runs (
    id                TEXT    NOT NULL PRIMARY KEY,
    instrument_id     TEXT    NOT NULL REFERENCES ifc_instruments (id),
    method_version_id TEXT    NOT NULL REFERENCES cat_method_versions (id),
    state             TEXT    NOT NULL CHECK (state IN ('open', 'closed')),
    created_by        TEXT    NOT NULL,
    created_at        TEXT    NOT NULL,
    closed_by         TEXT,
    closed_at         TEXT,
    close_reason      TEXT,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'closed') = (closed_at IS NOT NULL)),
    CHECK ((closed_at IS NULL) = (closed_by IS NULL)),
    CHECK (closed_at IS NOT NULL OR close_reason IS NULL)
) STRICT;

CREATE INDEX ana_runs_instrument_state ON ana_runs (instrument_id, state);

CREATE TRIGGER ana_runs_close_only BEFORE UPDATE ON ana_runs
WHEN NEW.id IS NOT OLD.id OR NEW.instrument_id IS NOT OLD.instrument_id
  OR NEW.method_version_id IS NOT OLD.method_version_id
  OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
  OR NEW.revision IS NOT OLD.revision + 1 OR OLD.state <> 'open' OR NEW.state <> 'closed'
BEGIN SELECT RAISE(ABORT, 'a run only moves from open to closed'); END;
CREATE TRIGGER ana_runs_no_delete BEFORE DELETE ON ana_runs
BEGIN SELECT RAISE(ABORT, 'runs are never deleted'); END;

CREATE TABLE ana_run_lots (
    run_id TEXT NOT NULL REFERENCES ana_runs (id),
    lot_id TEXT NOT NULL REFERENCES ana_lots (id),
    PRIMARY KEY (run_id, lot_id)
) STRICT;

CREATE TRIGGER ana_run_lots_no_update BEFORE UPDATE ON ana_run_lots
BEGIN SELECT RAISE(ABORT, 'run lots are append-only'); END;
CREATE TRIGGER ana_run_lots_no_delete BEFORE DELETE ON ana_run_lots
BEGIN SELECT RAISE(ABORT, 'run lots are append-only'); END;

-- Test executions of patient material: active -> failed, with lineage (REQ-ANA-006).
CREATE TABLE ana_executions (
    id                   TEXT    NOT NULL PRIMARY KEY,
    container_id         TEXT    NOT NULL REFERENCES spc_containers (id),
    method_version_id    TEXT    NOT NULL REFERENCES cat_method_versions (id),
    attempt_kind         TEXT    NOT NULL
        CHECK (attempt_kind IN ('initial', 'rerun', 'reextraction', 'dilution')),
    related_execution_id TEXT    REFERENCES ana_executions (id),
    dilution_factor      TEXT,
    prep_batch_id        TEXT    REFERENCES ana_prep_batches (id),
    reason               TEXT,
    state                TEXT    NOT NULL CHECK (state IN ('active', 'failed')),
    state_reason         TEXT,
    created_by           TEXT    NOT NULL,
    created_at           TEXT    NOT NULL,
    revision             INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((attempt_kind = 'initial') = (related_execution_id IS NULL)),
    CHECK (attempt_kind = 'initial' OR reason IS NOT NULL),
    CHECK ((attempt_kind = 'dilution') = (dilution_factor IS NOT NULL)),
    CHECK ((state = 'failed') = (state_reason IS NOT NULL))
) STRICT;

CREATE TRIGGER ana_executions_fail_only BEFORE UPDATE ON ana_executions
WHEN NEW.id IS NOT OLD.id OR NEW.container_id IS NOT OLD.container_id
  OR NEW.method_version_id IS NOT OLD.method_version_id
  OR NEW.attempt_kind IS NOT OLD.attempt_kind
  OR NEW.related_execution_id IS NOT OLD.related_execution_id
  OR NEW.dilution_factor IS NOT OLD.dilution_factor OR NEW.prep_batch_id IS NOT OLD.prep_batch_id
  OR NEW.reason IS NOT OLD.reason OR NEW.created_by IS NOT OLD.created_by
  OR NEW.created_at IS NOT OLD.created_at OR NEW.revision IS NOT OLD.revision + 1
  OR OLD.state <> 'active' OR NEW.state <> 'failed'
BEGIN SELECT RAISE(ABORT, 'an execution only moves from active to failed'); END;
CREATE TRIGGER ana_executions_no_delete BEFORE DELETE ON ana_executions
BEGIN SELECT RAISE(ABORT, 'executions are never deleted'); END;

CREATE TABLE ana_execution_components (
    execution_id       TEXT NOT NULL REFERENCES ana_executions (id),
    order_component_id TEXT NOT NULL REFERENCES ord_order_components (id),
    PRIMARY KEY (execution_id, order_component_id)
) STRICT;

CREATE INDEX ana_execution_components_component
    ON ana_execution_components (order_component_id);

CREATE TRIGGER ana_execution_components_no_update BEFORE UPDATE ON ana_execution_components
BEGIN SELECT RAISE(ABORT, 'execution components are append-only'); END;
CREATE TRIGGER ana_execution_components_no_delete BEFORE DELETE ON ana_execution_components
BEGIN SELECT RAISE(ABORT, 'execution components are append-only'); END;

-- Run positions. Only patient positions carry an execution, so controls,
-- calibrators and blanks can never become patient observations (REQ-ANA-001).
CREATE TABLE ana_run_positions (
    id             TEXT NOT NULL PRIMARY KEY,
    run_id         TEXT NOT NULL REFERENCES ana_runs (id),
    position_label TEXT NOT NULL,
    role           TEXT NOT NULL CHECK (role IN ('patient', 'control', 'calibrator', 'blank')),
    execution_id   TEXT UNIQUE REFERENCES ana_executions (id),
    lot_id         TEXT REFERENCES ana_lots (id),
    sample_label   TEXT NOT NULL,
    created_by     TEXT NOT NULL,
    created_at     TEXT NOT NULL,
    UNIQUE (run_id, position_label),
    UNIQUE (run_id, sample_label),
    CHECK ((role = 'patient') = (execution_id IS NOT NULL)),
    CHECK ((role IN ('control', 'calibrator')) = (lot_id IS NOT NULL))
) STRICT;

CREATE INDEX ana_run_positions_sample_label ON ana_run_positions (sample_label);

CREATE TRIGGER ana_run_positions_open_run BEFORE INSERT ON ana_run_positions
WHEN (SELECT state FROM ana_runs WHERE id = NEW.run_id) IS NOT 'open'
BEGIN SELECT RAISE(ABORT, 'positions are added only to open runs'); END;
CREATE TRIGGER ana_run_positions_no_update BEFORE UPDATE ON ana_run_positions
BEGIN SELECT RAISE(ABORT, 'run positions are append-only'); END;
CREATE TRIGGER ana_run_positions_no_delete BEFORE DELETE ON ana_run_positions
BEGIN SELECT RAISE(ABORT, 'run positions are append-only'); END;

-- QC observations as received; a changed re-export appends a superseding row.
CREATE TABLE ana_qc_observations (
    id               TEXT NOT NULL PRIMARY KEY,
    position_id      TEXT NOT NULL REFERENCES ana_run_positions (id),
    receipt_id       TEXT NOT NULL REFERENCES ifc_receipts (id),
    record_path      TEXT NOT NULL,
    source_test_code TEXT NOT NULL,
    analyte_id       TEXT REFERENCES cat_analytes (id),
    value_text       TEXT,
    units_text       TEXT,
    flags_text       TEXT,
    status_text      TEXT,
    supersedes_id    TEXT UNIQUE REFERENCES ana_qc_observations (id),
    recorded_at      TEXT NOT NULL
) STRICT;

CREATE INDEX ana_qc_observations_position ON ana_qc_observations (position_id, source_test_code);

-- Structural separation (brief §3.4): QC rows never attach to patient positions.
CREATE TRIGGER ana_qc_observations_not_patient BEFORE INSERT ON ana_qc_observations
WHEN coalesce((SELECT role FROM ana_run_positions WHERE id = NEW.position_id), 'patient')
     NOT IN ('control', 'calibrator', 'blank')
BEGIN SELECT RAISE(ABORT, 'QC observations attach only to control, calibrator or blank positions'); END;
CREATE TRIGGER ana_qc_observations_no_update BEFORE UPDATE ON ana_qc_observations
BEGIN SELECT RAISE(ABORT, 'QC observations are append-only'); END;
CREATE TRIGGER ana_qc_observations_no_delete BEFORE DELETE ON ana_qc_observations
BEGIN SELECT RAISE(ABORT, 'QC observations are append-only'); END;

-- QC assessments: no row means pending. Supersession appends a row with a
-- reason; a rejection is never relabeled as accepted (REQ-ANA-005).
CREATE TABLE ana_qc_assessments (
    id            TEXT NOT NULL PRIMARY KEY,
    scope         TEXT NOT NULL CHECK (scope IN ('run_analyte', 'run', 'prep_batch')),
    run_id        TEXT REFERENCES ana_runs (id),
    analyte_id    TEXT REFERENCES cat_analytes (id),
    prep_batch_id TEXT REFERENCES ana_prep_batches (id),
    outcome       TEXT NOT NULL CHECK (outcome IN ('accepted', 'rejected')),
    supersedes_id TEXT UNIQUE REFERENCES ana_qc_assessments (id),
    reason        TEXT,
    assessed_by   TEXT NOT NULL,
    assessed_at   TEXT NOT NULL,
    CHECK (supersedes_id IS NULL OR reason IS NOT NULL),
    CHECK ((scope = 'run_analyte' AND run_id IS NOT NULL AND analyte_id IS NOT NULL
                AND prep_batch_id IS NULL)
        OR (scope = 'run' AND run_id IS NOT NULL AND analyte_id IS NULL AND prep_batch_id IS NULL)
        OR (scope = 'prep_batch' AND prep_batch_id IS NOT NULL AND run_id IS NULL
                AND analyte_id IS NULL))
) STRICT;

CREATE INDEX ana_qc_assessments_run ON ana_qc_assessments (run_id, analyte_id);
CREATE INDEX ana_qc_assessments_prep_batch ON ana_qc_assessments (prep_batch_id);

CREATE TRIGGER ana_qc_assessments_supersede_same_scope BEFORE INSERT ON ana_qc_assessments
WHEN NEW.supersedes_id IS NOT NULL AND NOT EXISTS (
    SELECT 1 FROM ana_qc_assessments o WHERE o.id = NEW.supersedes_id AND o.scope = NEW.scope
      AND o.run_id IS NEW.run_id AND o.analyte_id IS NEW.analyte_id
      AND o.prep_batch_id IS NEW.prep_batch_id)
BEGIN SELECT RAISE(ABORT, 'an assessment supersedes only one of the same scope'); END;
CREATE TRIGGER ana_qc_assessments_no_waiver BEFORE INSERT ON ana_qc_assessments
WHEN NEW.outcome = 'accepted'
 AND (SELECT outcome FROM ana_qc_assessments WHERE id = NEW.supersedes_id) = 'rejected'
BEGIN SELECT RAISE(ABORT, 'a rejected QC assessment is never superseded by an acceptance'); END;
CREATE TRIGGER ana_qc_assessments_no_update BEFORE UPDATE ON ana_qc_assessments
BEGIN SELECT RAISE(ABORT, 'QC assessments are append-only'); END;
CREATE TRIGGER ana_qc_assessments_no_delete BEFORE DELETE ON ana_qc_assessments
BEGIN SELECT RAISE(ABORT, 'QC assessments are append-only'); END;

CREATE TABLE ana_qc_assessment_evidence (
    assessment_id  TEXT NOT NULL REFERENCES ana_qc_assessments (id),
    observation_id TEXT NOT NULL REFERENCES ana_qc_observations (id),
    PRIMARY KEY (assessment_id, observation_id)
) STRICT;

CREATE TRIGGER ana_qc_assessment_evidence_no_update BEFORE UPDATE ON ana_qc_assessment_evidence
BEGIN SELECT RAISE(ABORT, 'QC assessment evidence is append-only'); END;
CREATE TRIGGER ana_qc_assessment_evidence_no_delete BEFORE DELETE ON ana_qc_assessment_evidence
BEGIN SELECT RAISE(ABORT, 'QC assessment evidence is append-only'); END;

-- Deviations scoped to an instrument, lot, preparation batch, run or
-- position, optionally one analyte: open -> closed with a disposition.
CREATE TABLE ana_deviations (
    id           TEXT    NOT NULL PRIMARY KEY,
    scope        TEXT    NOT NULL
        CHECK (scope IN ('instrument', 'lot', 'prep_batch', 'run', 'position')),
    scope_id     TEXT    NOT NULL,
    analyte_id   TEXT    REFERENCES cat_analytes (id),
    description  TEXT    NOT NULL,
    reason       TEXT    NOT NULL,
    state        TEXT    NOT NULL CHECK (state IN ('open', 'closed')),
    disposition  TEXT,
    close_reason TEXT,
    opened_by    TEXT    NOT NULL,
    opened_at    TEXT    NOT NULL,
    closed_by    TEXT,
    closed_at    TEXT,
    revision     INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'closed') = (closed_at IS NOT NULL)),
    CHECK ((closed_at IS NULL) = (closed_by IS NULL)),
    CHECK ((closed_at IS NULL) = (disposition IS NULL)),
    CHECK ((closed_at IS NULL) = (close_reason IS NULL))
) STRICT;

CREATE INDEX ana_deviations_scope ON ana_deviations (scope, scope_id);

CREATE TRIGGER ana_deviations_close_only BEFORE UPDATE ON ana_deviations
WHEN NEW.id IS NOT OLD.id OR NEW.scope IS NOT OLD.scope OR NEW.scope_id IS NOT OLD.scope_id
  OR NEW.analyte_id IS NOT OLD.analyte_id OR NEW.description IS NOT OLD.description
  OR NEW.reason IS NOT OLD.reason OR NEW.opened_by IS NOT OLD.opened_by
  OR NEW.opened_at IS NOT OLD.opened_at OR NEW.revision IS NOT OLD.revision + 1
  OR OLD.state <> 'open' OR NEW.state <> 'closed'
BEGIN SELECT RAISE(ABORT, 'a deviation only moves from open to closed'); END;
CREATE TRIGGER ana_deviations_no_delete BEFORE DELETE ON ana_deviations
BEGIN SELECT RAISE(ABORT, 'deviations are never deleted'); END;

-- Published to lis-results (brief §3.1): the measurement trigger refuses
-- positions whose role is not 'patient'.
CREATE VIEW ana_v_positions AS
SELECT p.id AS position_id, p.run_id, r.state AS run_state, r.instrument_id,
       r.method_version_id, p.role, p.execution_id, e.container_id, p.sample_label,
       e.prep_batch_id, p.lot_id, e.attempt_kind, e.state AS execution_state
FROM ana_run_positions p
JOIN ana_runs r ON r.id = p.run_id
LEFT JOIN ana_executions e ON e.id = p.execution_id;
