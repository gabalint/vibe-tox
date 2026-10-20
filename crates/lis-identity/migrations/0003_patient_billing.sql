-- Immutable, bounded patient financial profile versions (BILL-G01/G07).
CREATE TABLE idn_patient_billing_profiles (
    subject_id TEXT NOT NULL REFERENCES idn_subjects(id),
    version INTEGER NOT NULL CHECK(version >= 1),
    content_json TEXT NOT NULL CHECK(json_valid(content_json) AND length(content_json) <= 262144),
    source TEXT NOT NULL,
    reason TEXT NOT NULL,
    recorded_by TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    PRIMARY KEY(subject_id, version)
) STRICT;

CREATE TRIGGER idn_patient_billing_version BEFORE INSERT ON idn_patient_billing_profiles
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM idn_patient_billing_profiles WHERE subject_id = NEW.subject_id)
BEGIN SELECT RAISE(ABORT, 'financial profile version must be consecutive'); END;
CREATE TRIGGER idn_patient_billing_no_update BEFORE UPDATE ON idn_patient_billing_profiles
BEGIN SELECT RAISE(ABORT, 'financial profile versions are immutable'); END;
CREATE TRIGGER idn_patient_billing_no_delete BEFORE DELETE ON idn_patient_billing_profiles
BEGIN SELECT RAISE(ABORT, 'financial profile versions are retained'); END;
