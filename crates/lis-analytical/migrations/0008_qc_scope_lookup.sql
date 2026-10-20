-- UI-12: exact QC scope lookup must not scan every run assessment whose
-- prep_batch_id is NULL. Match the shared current/history scope predicate.
CREATE INDEX ana_qc_assessments_scope_lookup
    ON ana_qc_assessments (scope, run_id, analyte_id, prep_batch_id);
