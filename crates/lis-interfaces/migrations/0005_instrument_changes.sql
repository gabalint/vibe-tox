-- Who registered each analyzer and who last changed its configuration, and when,
-- so its panel can show them without the audit trail. Analyzers registered
-- before this migration have no such record; their audit events remain.
ALTER TABLE ifc_instruments ADD COLUMN created_by TEXT;
ALTER TABLE ifc_instruments ADD COLUMN created_at TEXT;
ALTER TABLE ifc_instruments ADD COLUMN changed_by TEXT;
ALTER TABLE ifc_instruments ADD COLUMN changed_at TEXT;
