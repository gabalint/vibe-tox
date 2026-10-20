-- REQ-INT-001/004: AU wire settings and numeric units are declared and approved
-- with the source registration, never inferred from absent message fields.
CREATE TABLE ana_source_au_profiles (
    registration_id TEXT NOT NULL PRIMARY KEY REFERENCES ana_source_registrations (id),
    profile_json TEXT NOT NULL CHECK (json_valid(profile_json)),
    profile_version INTEGER NOT NULL CHECK (profile_version = 1)
) STRICT;

CREATE TRIGGER ana_source_au_profiles_draft_only BEFORE INSERT ON ana_source_au_profiles
WHEN (SELECT state FROM ana_source_registrations WHERE id = NEW.registration_id) IS NOT 'draft'
BEGIN SELECT RAISE(ABORT, 'an AU profile is fixed once its registration is approved'); END;
CREATE TRIGGER ana_source_au_profiles_no_update BEFORE UPDATE ON ana_source_au_profiles
BEGIN SELECT RAISE(ABORT, 'source AU profiles are append-only'); END;
CREATE TRIGGER ana_source_au_profiles_no_delete BEFORE DELETE ON ana_source_au_profiles
BEGIN SELECT RAISE(ABORT, 'source AU profiles are append-only'); END;
CREATE TRIGGER ana_source_au_profiles_no_file BEFORE INSERT ON ana_source_au_profiles
WHEN EXISTS (SELECT 1 FROM ana_source_file_layouts WHERE registration_id = NEW.registration_id)
BEGIN SELECT RAISE(ABORT, 'a source has one wire format'); END;
CREATE TRIGGER ana_source_file_layouts_no_au BEFORE INSERT ON ana_source_file_layouts
WHEN EXISTS (SELECT 1 FROM ana_source_au_profiles WHERE registration_id = NEW.registration_id)
BEGIN SELECT RAISE(ABORT, 'a source has one wire format'); END;
