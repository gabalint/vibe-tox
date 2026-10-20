-- A credential reset ends the target's open sessions (security review 2026-10-13,
-- REQ-IDN-004), recorded with its own end reason. SQLite cannot change a CHECK
-- constraint in place, so the table is rebuilt with the same rows, index and triggers.

CREATE TABLE auth_sessions_next (
    id             TEXT NOT NULL PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES auth_users (id),
    workstation_id TEXT NOT NULL REFERENCES auth_workstations (id),
    -- Web engine name and version, for the session's audit context (ADR 0012).
    webview        TEXT,
    started_at     TEXT NOT NULL,
    ended_at       TEXT,
    end_reason     TEXT CHECK (end_reason IN ('signed_out', 'switched', 'app_exit', 'abandoned',
                                              'credentials_reset')),
    CHECK ((ended_at IS NULL) = (end_reason IS NULL))
) STRICT;

INSERT INTO auth_sessions_next (id, user_id, workstation_id, webview, started_at, ended_at,
                                end_reason)
SELECT id, user_id, workstation_id, webview, started_at, ended_at, end_reason
FROM auth_sessions;

DROP TABLE auth_sessions;
ALTER TABLE auth_sessions_next RENAME TO auth_sessions;

CREATE INDEX auth_sessions_open ON auth_sessions (started_at) WHERE ended_at IS NULL;

CREATE TRIGGER auth_sessions_end_only BEFORE UPDATE ON auth_sessions
WHEN OLD.ended_at IS NOT NULL
    OR NEW.id IS NOT OLD.id OR NEW.user_id IS NOT OLD.user_id
    OR NEW.workstation_id IS NOT OLD.workstation_id OR NEW.webview IS NOT OLD.webview
    OR NEW.started_at IS NOT OLD.started_at
BEGIN
    SELECT RAISE(ABORT, 'auth_sessions rows are only ended, never rewritten');
END;

CREATE TRIGGER auth_sessions_no_delete BEFORE DELETE ON auth_sessions
BEGIN
    SELECT RAISE(ABORT, 'auth_sessions rows are never deleted');
END;
