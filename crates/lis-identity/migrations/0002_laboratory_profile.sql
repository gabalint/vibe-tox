-- The performing laboratory's identity and time zone (REQ-RPT-001,
-- REQ-RPT-008, REQ-BIL-005). Each change is a new version; earlier versions
-- stay, so a released report keeps naming the profile version it froze.
-- The time zone is an IANA name validated by the engine; the billing NPI and
-- tax ID are optional until billing needs them. `set_by` holds
-- `Actor::id_text()` of a signed-in user.
CREATE TABLE idn_laboratory_profiles (
    id             TEXT    NOT NULL PRIMARY KEY,
    version        INTEGER NOT NULL UNIQUE CHECK (version >= 1),
    name           TEXT    NOT NULL,
    address_line_1 TEXT    NOT NULL,
    address_line_2 TEXT,
    address_line_3 TEXT,
    clia_id        TEXT    NOT NULL CHECK (length(clia_id) = 10),
    director_name  TEXT    NOT NULL,
    phone          TEXT    NOT NULL,
    time_zone      TEXT    NOT NULL,
    billing_npi    TEXT    CHECK (billing_npi IS NULL
        OR (length(billing_npi) = 10 AND billing_npi NOT GLOB '*[^0-9]*')),
    tax_id         TEXT    CHECK (tax_id IS NULL
        OR (length(tax_id) = 9 AND tax_id NOT GLOB '*[^0-9]*')),
    reason         TEXT,
    set_by         TEXT    NOT NULL,
    set_at         TEXT    NOT NULL,
    CHECK ((version = 1) = (reason IS NULL)),
    CHECK (address_line_3 IS NULL OR address_line_2 IS NOT NULL)
) STRICT;

-- Versions are consecutive, so "current" is always the highest one.
CREATE TRIGGER idn_laboratory_profiles_next_version BEFORE INSERT ON idn_laboratory_profiles
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM idn_laboratory_profiles)
BEGIN
    SELECT RAISE(ABORT, 'laboratory profile versions are consecutive');
END;

CREATE TRIGGER idn_laboratory_profiles_no_update BEFORE UPDATE ON idn_laboratory_profiles
BEGIN
    SELECT RAISE(ABORT, 'idn_laboratory_profiles is append-only');
END;

CREATE TRIGGER idn_laboratory_profiles_no_delete BEFORE DELETE ON idn_laboratory_profiles
BEGIN
    SELECT RAISE(ABORT, 'idn_laboratory_profiles is append-only');
END;
