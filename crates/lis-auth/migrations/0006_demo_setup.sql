-- First-run consent to prepare synthetic demo data after the account signs in.
-- The request and its owner survive an interrupted sign-in or failed data load.
CREATE TABLE auth_demo_setup (
    singleton    INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
    user_id      TEXT NOT NULL REFERENCES auth_users(id),
    requested_at TEXT NOT NULL,
    completed_at TEXT,
    revision     INTEGER NOT NULL DEFAULT 1 CHECK (revision >= 1)
) STRICT;

CREATE TRIGGER auth_demo_setup_keep_request BEFORE UPDATE ON auth_demo_setup
WHEN NEW.user_id <> OLD.user_id OR NEW.requested_at <> OLD.requested_at
    OR OLD.completed_at IS NOT NULL OR NEW.completed_at IS NULL
    OR NEW.revision <> OLD.revision + 1
BEGIN
    SELECT RAISE(ABORT, 'demo setup requests may only be completed once');
END;

CREATE TRIGGER auth_demo_setup_no_delete BEFORE DELETE ON auth_demo_setup
BEGIN
    SELECT RAISE(ABORT, 'demo setup requests are retained');
END;
