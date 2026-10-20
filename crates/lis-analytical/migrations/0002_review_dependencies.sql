-- Forward-only corrections from the M2b review (REQ-ANA-001, REQ-ANA-003,
-- REQ-ANA-004, REQ-ANA-005, REQ-ANA-007): orderings that never depend on the
-- host clock, the preparation-batch lifecycle, and QC re-export conflicts.

-- Instrument status: a per-instrument sequence assigned in the writing
-- transaction decides the latest event, so a backward clock change cannot
-- reorder events. Existing events are numbered in insertion (rowid) order.
ALTER TABLE ana_instrument_status_events ADD COLUMN seq INTEGER CHECK (seq >= 1);

DROP TRIGGER ana_instrument_status_events_no_update;
UPDATE ana_instrument_status_events SET seq = (
    SELECT count(*) FROM ana_instrument_status_events o
    WHERE o.instrument_id = ana_instrument_status_events.instrument_id
      AND o.rowid <= ana_instrument_status_events.rowid);
CREATE TRIGGER ana_instrument_status_events_no_update BEFORE UPDATE ON ana_instrument_status_events
BEGIN SELECT RAISE(ABORT, 'instrument status events are append-only'); END;

CREATE UNIQUE INDEX ana_instrument_status_events_seq
    ON ana_instrument_status_events (instrument_id, seq);

CREATE TRIGGER ana_instrument_status_events_next_seq BEFORE INSERT ON ana_instrument_status_events
WHEN NEW.seq IS NOT (SELECT coalesce(max(seq), 0) + 1 FROM ana_instrument_status_events
                     WHERE instrument_id = NEW.instrument_id)
BEGIN SELECT RAISE(ABORT, 'a status event takes the next sequence number of its instrument'); END;

-- Deviations: one database-wide sequence. A review binds the highest
-- sequence of the deviations applicable to it; any later applicable
-- deviation, even once closed, keeps that review stale.
ALTER TABLE ana_deviations ADD COLUMN seq INTEGER CHECK (seq >= 1);

DROP TRIGGER ana_deviations_close_only;
UPDATE ana_deviations SET seq = (
    SELECT count(*) FROM ana_deviations o WHERE o.rowid <= ana_deviations.rowid);
CREATE TRIGGER ana_deviations_close_only BEFORE UPDATE ON ana_deviations
WHEN NEW.id IS NOT OLD.id OR NEW.scope IS NOT OLD.scope OR NEW.scope_id IS NOT OLD.scope_id
  OR NEW.analyte_id IS NOT OLD.analyte_id OR NEW.description IS NOT OLD.description
  OR NEW.reason IS NOT OLD.reason OR NEW.opened_by IS NOT OLD.opened_by
  OR NEW.opened_at IS NOT OLD.opened_at OR NEW.seq IS NOT OLD.seq
  OR NEW.revision IS NOT OLD.revision + 1 OR OLD.state <> 'open' OR NEW.state <> 'closed'
BEGIN SELECT RAISE(ABORT, 'a deviation only moves from open to closed'); END;

CREATE UNIQUE INDEX ana_deviations_seq ON ana_deviations (seq);

CREATE TRIGGER ana_deviations_next_seq BEFORE INSERT ON ana_deviations
WHEN NEW.seq IS NOT (SELECT coalesce(max(seq), 0) + 1 FROM ana_deviations)
BEGIN SELECT RAISE(ABORT, 'a deviation takes the next sequence number'); END;

-- Preparation batches: open -> completed. Each recorded step raises the
-- revision; a completed batch never changes. Existing batches stay open, so
-- they must be completed and assessed again before results can depend on them.
ALTER TABLE ana_prep_batches ADD COLUMN state TEXT NOT NULL DEFAULT 'open'
    CHECK (state IN ('open', 'completed'));
ALTER TABLE ana_prep_batches ADD COLUMN completed_by TEXT;
ALTER TABLE ana_prep_batches ADD COLUMN completed_at TEXT;
ALTER TABLE ana_prep_batches ADD COLUMN revision INTEGER NOT NULL DEFAULT 1
    CHECK (revision >= 1);

DROP TRIGGER ana_prep_batches_no_update;

CREATE TRIGGER ana_prep_batches_start_open BEFORE INSERT ON ana_prep_batches
WHEN NEW.state <> 'open' OR NEW.revision <> 1
  OR NEW.completed_by IS NOT NULL OR NEW.completed_at IS NOT NULL
BEGIN SELECT RAISE(ABORT, 'a preparation batch starts open'); END;

CREATE TRIGGER ana_prep_batches_lifecycle BEFORE UPDATE ON ana_prep_batches
WHEN NEW.id IS NOT OLD.id OR NEW.method_version_id IS NOT OLD.method_version_id
  OR NEW.created_by IS NOT OLD.created_by OR NEW.created_at IS NOT OLD.created_at
  OR NEW.revision IS NOT OLD.revision + 1 OR OLD.state <> 'open'
  OR NOT ((NEW.state = 'open' AND NEW.completed_by IS NULL AND NEW.completed_at IS NULL)
       OR (NEW.state = 'completed' AND NEW.completed_by IS NOT NULL
           AND NEW.completed_at IS NOT NULL))
BEGIN SELECT RAISE(ABORT, 'a preparation batch changes only while open'); END;

CREATE TRIGGER ana_prep_steps_open_batch BEFORE INSERT ON ana_prep_steps
WHEN (SELECT state FROM ana_prep_batches WHERE id = NEW.batch_id) IS NOT 'open'
BEGIN SELECT RAISE(ABORT, 'steps are recorded only on open preparation batches'); END;

-- A preparation-batch assessment records the batch revision it assessed;
-- only a completed batch can be accepted.
ALTER TABLE ana_qc_assessments ADD COLUMN prep_batch_revision INTEGER;

CREATE TRIGGER ana_qc_assessments_prep_revision BEFORE INSERT ON ana_qc_assessments
WHEN NEW.prep_batch_revision IS NOT (SELECT revision FROM ana_prep_batches
                                     WHERE id = NEW.prep_batch_id)
BEGIN SELECT RAISE(ABORT, 'a batch assessment records the batch revision it assessed'); END;

CREATE TRIGGER ana_qc_assessments_prep_completed BEFORE INSERT ON ana_qc_assessments
WHEN NEW.scope = 'prep_batch' AND NEW.outcome = 'accepted'
 AND (SELECT state FROM ana_prep_batches WHERE id = NEW.prep_batch_id) IS NOT 'completed'
BEGIN SELECT RAISE(ABORT, 'only a completed preparation batch can be accepted'); END;

-- A changed QC row the source registration cannot classify (R5): kept
-- against the exact observation it contradicts, with the analyte the
-- registration in force mapped it to. While open it blocks every result of
-- the run that depends on either analyte; a person accepts it as a
-- correction (appending a superseding observation) or rejects it.
CREATE TABLE ana_qc_conflicts (
    id                TEXT    NOT NULL PRIMARY KEY,
    position_id       TEXT    NOT NULL REFERENCES ana_run_positions (id),
    conflicts_with_id TEXT    NOT NULL REFERENCES ana_qc_observations (id),
    receipt_id        TEXT    NOT NULL REFERENCES ifc_receipts (id),
    record_path       TEXT    NOT NULL,
    source_test_code  TEXT    NOT NULL,
    analyte_id        TEXT    REFERENCES cat_analytes (id),
    value_text        TEXT,
    units_text        TEXT,
    flags_text        TEXT,
    status_text       TEXT,
    state             TEXT    NOT NULL CHECK (state IN ('open', 'accepted', 'rejected')),
    observation_id    TEXT    UNIQUE REFERENCES ana_qc_observations (id),
    reason            TEXT,
    resolved_by       TEXT,
    resolved_at       TEXT,
    recorded_at       TEXT    NOT NULL,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((state = 'open') = (resolved_at IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolved_by IS NULL)),
    CHECK ((resolved_at IS NULL) = (reason IS NULL)),
    CHECK ((state = 'accepted') = (observation_id IS NOT NULL))
) STRICT;

CREATE INDEX ana_qc_conflicts_position ON ana_qc_conflicts (position_id, state);

CREATE TRIGGER ana_qc_conflicts_start_open BEFORE INSERT ON ana_qc_conflicts
WHEN NEW.state <> 'open' OR NEW.revision <> 1
  OR coalesce((SELECT role FROM ana_run_positions WHERE id = NEW.position_id), 'patient')
     NOT IN ('control', 'calibrator', 'blank')
BEGIN SELECT RAISE(ABORT, 'a QC conflict starts open on a QC position'); END;

CREATE TRIGGER ana_qc_conflicts_resolve_once BEFORE UPDATE ON ana_qc_conflicts
WHEN NEW.id IS NOT OLD.id OR NEW.position_id IS NOT OLD.position_id
  OR NEW.conflicts_with_id IS NOT OLD.conflicts_with_id OR NEW.receipt_id IS NOT OLD.receipt_id
  OR NEW.record_path IS NOT OLD.record_path OR NEW.source_test_code IS NOT OLD.source_test_code
  OR NEW.analyte_id IS NOT OLD.analyte_id OR NEW.value_text IS NOT OLD.value_text
  OR NEW.units_text IS NOT OLD.units_text OR NEW.flags_text IS NOT OLD.flags_text
  OR NEW.status_text IS NOT OLD.status_text OR NEW.recorded_at IS NOT OLD.recorded_at
  OR NEW.revision IS NOT OLD.revision + 1 OR OLD.state <> 'open' OR NEW.state = 'open'
BEGIN SELECT RAISE(ABORT, 'a QC conflict is resolved once'); END;

CREATE TRIGGER ana_qc_conflicts_no_delete BEFORE DELETE ON ana_qc_conflicts
BEGIN SELECT RAISE(ABORT, 'QC conflicts are never deleted'); END;
