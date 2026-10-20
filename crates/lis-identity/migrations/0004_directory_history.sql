-- DM-05: current directory projections plus immutable correction history.
-- Legacy baseline records preserve available facts without inventing their author.
CREATE TABLE idn_directory_history (
    record_kind TEXT NOT NULL CHECK(record_kind IN ('client','location','practitioner')),
    record_id TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK(revision >= 1),
    code TEXT NOT NULL,
    name TEXT NOT NULL,
    given_names TEXT,
    active INTEGER NOT NULL CHECK(active IN (0,1)),
    recorded_at TEXT NOT NULL,
    recorded_by TEXT,
    reason TEXT,
    origin TEXT NOT NULL CHECK(origin IN ('legacy_baseline','registered','corrected')),
    PRIMARY KEY(record_kind, record_id, revision),
    CHECK((origin = 'legacy_baseline') = (recorded_by IS NULL)),
    CHECK((origin = 'corrected') = (reason IS NOT NULL))
) STRICT;
CREATE TRIGGER idn_directory_history_no_update BEFORE UPDATE ON idn_directory_history
BEGIN SELECT RAISE(ABORT, 'directory history is immutable'); END;
CREATE TRIGGER idn_directory_history_no_delete BEFORE DELETE ON idn_directory_history
BEGIN SELECT RAISE(ABORT, 'directory history is retained'); END;

ALTER TABLE idn_client_organizations ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1));
ALTER TABLE idn_client_organizations ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1);
INSERT INTO idn_directory_history(record_kind,record_id,revision,code,name,given_names,active,recorded_at,origin)
SELECT 'client',id,1,code,name,NULL,1,created_at,'legacy_baseline' FROM idn_client_organizations;
CREATE TRIGGER idn_client_organizations_retain BEFORE DELETE ON idn_client_organizations
BEGIN SELECT RAISE(ABORT, 'directory records are retained'); END;
CREATE TRIGGER idn_client_organizations_versioned BEFORE UPDATE ON idn_client_organizations
WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at
 OR NEW.revision <> OLD.revision + 1 OR NOT EXISTS (
    SELECT 1 FROM idn_directory_history h WHERE h.record_kind='client'
      AND h.record_id=NEW.id AND h.revision=NEW.revision AND h.code=NEW.code
      AND h.name=NEW.name AND h.active=NEW.active
      AND h.given_names IS NULL
 )
BEGIN SELECT RAISE(ABORT, 'directory correction requires matching immutable history'); END;

ALTER TABLE idn_client_locations ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1));
ALTER TABLE idn_client_locations ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1);
INSERT INTO idn_directory_history(record_kind,record_id,revision,code,name,given_names,active,recorded_at,origin)
SELECT 'location',id,1,code,name,NULL,1,created_at,'legacy_baseline' FROM idn_client_locations;
CREATE TRIGGER idn_client_locations_retain BEFORE DELETE ON idn_client_locations
BEGIN SELECT RAISE(ABORT, 'directory records are retained'); END;
CREATE TRIGGER idn_client_locations_versioned BEFORE UPDATE ON idn_client_locations
WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at
 OR NEW.client_id IS NOT OLD.client_id
 OR NEW.revision <> OLD.revision + 1 OR NOT EXISTS (
    SELECT 1 FROM idn_directory_history h WHERE h.record_kind='location'
      AND h.record_id=NEW.id AND h.revision=NEW.revision AND h.code=NEW.code
      AND h.name=NEW.name AND h.active=NEW.active
      AND h.given_names IS NULL
 )
BEGIN SELECT RAISE(ABORT, 'directory correction requires matching immutable history'); END;

ALTER TABLE idn_practitioners ADD COLUMN active INTEGER NOT NULL DEFAULT 1 CHECK(active IN (0,1));
ALTER TABLE idn_practitioners ADD COLUMN revision INTEGER NOT NULL DEFAULT 1 CHECK(revision >= 1);
INSERT INTO idn_directory_history(record_kind,record_id,revision,code,name,given_names,active,recorded_at,origin)
SELECT 'practitioner',id,1,npi,family_name,given_names,1,created_at,'legacy_baseline' FROM idn_practitioners;
CREATE TRIGGER idn_practitioners_retain BEFORE DELETE ON idn_practitioners
BEGIN SELECT RAISE(ABORT, 'directory records are retained'); END;
CREATE TRIGGER idn_practitioners_versioned BEFORE UPDATE ON idn_practitioners
WHEN NEW.id IS NOT OLD.id OR NEW.created_at IS NOT OLD.created_at
 OR NEW.revision <> OLD.revision + 1 OR NOT EXISTS (
    SELECT 1 FROM idn_directory_history h WHERE h.record_kind='practitioner'
      AND h.record_id=NEW.id AND h.revision=NEW.revision AND h.code=NEW.npi
      AND h.name=NEW.family_name AND h.active=NEW.active
      AND h.given_names IS NEW.given_names
 )
BEGIN SELECT RAISE(ABORT, 'directory correction requires matching immutable history'); END;
