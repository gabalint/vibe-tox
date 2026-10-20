-- Preserve all history; the first version may now carry the mandatory reason
-- for opting into touch-only sign-in. Existing installations remain Required.
DROP TRIGGER auth_security_policies_next_version;
DROP TRIGGER auth_security_policies_no_update;
DROP TRIGGER auth_security_policies_no_delete;
ALTER TABLE auth_security_policies RENAME TO auth_security_policies_previous;

CREATE TABLE auth_security_policies (
    version              INTEGER NOT NULL PRIMARY KEY CHECK (version >= 1),
    idle_timeout_minutes INTEGER NOT NULL CHECK (idle_timeout_minutes BETWEEN 2 AND 60),
    lockout_failures     INTEGER NOT NULL CHECK (lockout_failures BETWEEN 3 AND 10),
    lockout_minutes      INTEGER NOT NULL CHECK (lockout_minutes BETWEEN 5 AND 60),
    reason               TEXT CHECK (length(reason) > 0),
    set_by               TEXT NOT NULL CHECK (length(set_by) > 0),
    set_at               TEXT NOT NULL,
    allow_security_key_touch_only INTEGER NOT NULL DEFAULT 0
        CHECK (allow_security_key_touch_only IN (0, 1)),
    CHECK (version = 1 OR reason IS NOT NULL),
    CHECK (allow_security_key_touch_only = 0 OR reason IS NOT NULL)
) STRICT;

INSERT INTO auth_security_policies
    (version, idle_timeout_minutes, lockout_failures, lockout_minutes, reason, set_by, set_at)
SELECT version, idle_timeout_minutes, lockout_failures, lockout_minutes, reason, set_by, set_at
FROM auth_security_policies_previous;
DROP TABLE auth_security_policies_previous;

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
