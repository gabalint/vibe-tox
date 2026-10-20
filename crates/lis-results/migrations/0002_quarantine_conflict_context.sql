-- Bind a conflicting imported row to the exact current measurement reviewed
-- during quarantine resolution (REQ-ANA-003, REQ-RES-004; INV-04).

ALTER TABLE res_quarantine_items ADD COLUMN conflicting_measurement_id TEXT
    REFERENCES res_measurements (id);

CREATE TRIGGER res_quarantine_conflicting_measurement_immutable
BEFORE UPDATE OF conflicting_measurement_id ON res_quarantine_items
BEGIN SELECT RAISE(ABORT, 'quarantine conflict context is immutable'); END;
