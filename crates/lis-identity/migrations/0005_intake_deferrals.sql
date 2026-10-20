-- Incomplete identity remains a source record, never a fabricated patient identity.
CREATE TABLE idn_intake_deferrals (
 id TEXT NOT NULL PRIMARY KEY,
 supplied_name TEXT NOT NULL,
 birth_evidence_kind TEXT NOT NULL CHECK (birth_evidence_kind IN ('unknown','partial_date','supplied_age')),
 birth_evidence_text TEXT,
 source_reference TEXT NOT NULL,
 reason TEXT NOT NULL,
 recorded_by TEXT NOT NULL,
 recorded_at TEXT NOT NULL
) STRICT;
CREATE TABLE idn_intake_dispositions (
 id TEXT NOT NULL PRIMARY KEY REFERENCES idn_intake_deferrals(id),
 disposition TEXT NOT NULL CHECK (disposition IN ('declined','resolved')),
 subject_id TEXT REFERENCES idn_subjects(id),
 source_reference TEXT NOT NULL,
 reason TEXT NOT NULL,
 recorded_by TEXT NOT NULL,
 recorded_at TEXT NOT NULL,
 CHECK ((disposition = 'resolved') = (subject_id IS NOT NULL))
) STRICT;
CREATE TRIGGER idn_intake_deferrals_no_update BEFORE UPDATE ON idn_intake_deferrals BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER idn_intake_deferrals_no_delete BEFORE DELETE ON idn_intake_deferrals BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER idn_intake_dispositions_no_update BEFORE UPDATE ON idn_intake_dispositions BEGIN SELECT RAISE(ABORT, 'append-only'); END;
CREATE TRIGGER idn_intake_dispositions_no_delete BEFORE DELETE ON idn_intake_dispositions BEGIN SELECT RAISE(ABORT, 'append-only'); END;
