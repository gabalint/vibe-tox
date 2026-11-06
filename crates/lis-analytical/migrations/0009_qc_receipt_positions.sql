-- The run positions each receipt brought QC observations for, so the results
-- module's import search finds the receipt by a control or calibrator label
-- (REQ-ANA-003). QC rows have no import-attempt row of their own.
CREATE VIEW ana_v_qc_receipt_positions AS
SELECT DISTINCT receipt_id, position_id FROM ana_qc_observations;
