-- The laboratory's security policy: idle timeout and sign-in lockout (NFR-19, M4 design
-- brief §2.2 and §8, laboratory question L11). Versioned and append-only: each change is
-- a new row; the newest version applies. Until the laboratory sets values, the defaults
-- in code apply (15 minutes; 5 failures lock for 15 minutes). Every version after the
-- first carries the reason for the change.

CREATE TABLE auth_security_policies (
    version              INTEGER NOT NULL PRIMARY KEY CHECK (version >= 1),
    idle_timeout_minutes INTEGER NOT NULL CHECK (idle_timeout_minutes BETWEEN 2 AND 60),
    lockout_failures     INTEGER NOT NULL CHECK (lockout_failures BETWEEN 3 AND 10),
    lockout_minutes      INTEGER NOT NULL CHECK (lockout_minutes BETWEEN 5 AND 60),
    reason               TEXT CHECK (length(reason) > 0),
    -- The acting principal's ID text.
    set_by               TEXT    NOT NULL CHECK (length(set_by) > 0),
    set_at               TEXT    NOT NULL,
    CHECK ((version = 1) = (reason IS NULL))
) STRICT;

CREATE TRIGGER auth_security_policies_next_version BEFORE INSERT ON auth_security_policies
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM auth_security_policies)
BEGIN
    SELECT RAISE(ABORT, 'security policy versions count up from 1 without gaps');
END;

CREATE TRIGGER auth_security_policies_no_update BEFORE UPDATE ON auth_security_policies
BEGIN
    SELECT RAISE(ABORT, 'security policy versions are append-only');
END;

CREATE TRIGGER auth_security_policies_no_delete BEFORE DELETE ON auth_security_policies
BEGIN
    SELECT RAISE(ABORT, 'security policy versions are append-only');
END;
