-- Local accounts, role grants, sessions and sign-in throttling (REQ-IDN-004,
-- NFR-08, NFR-19). No secret is stored in clear: passwords are Argon2id PHC
-- strings and TOTP secrets are ChaCha20-Poly1305 ciphertexts under a key held
-- in the OS key store.

-- The computer this database runs on. Sessions record it (REQ-IDN-004).
CREATE TABLE auth_workstations (
    id            TEXT NOT NULL PRIMARY KEY,
    registered_at TEXT NOT NULL
) STRICT;

CREATE TABLE auth_users (
    id                   TEXT    NOT NULL PRIMARY KEY,
    -- Stored lowercase, so uniqueness is case-insensitive.
    username             TEXT    NOT NULL UNIQUE CHECK (username = lower(username)),
    display_name         TEXT    NOT NULL CHECK (length(display_name) > 0),
    status               TEXT    NOT NULL CHECK (status IN ('active', 'disabled')),
    -- Argon2id PHC string; the cost parameters are part of the string.
    password_hash        TEXT    NOT NULL CHECK (password_hash LIKE '$argon2id$%'),
    must_change_password INTEGER NOT NULL CHECK (must_change_password IN (0, 1)),
    -- 12-byte nonce, then ciphertext and tag. NULL until an enrolment is confirmed.
    totp_secret          BLOB,
    -- Last accepted RFC 6238 time step: a code for it or an earlier step is a replay.
    totp_last_step       INTEGER,
    created_at           TEXT    NOT NULL,
    updated_at           TEXT    NOT NULL,
    revision             INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((totp_secret IS NULL) = (totp_last_step IS NULL))
) STRICT;

-- A change of roles revokes grant rows and inserts new ones; rows are never
-- rewritten or deleted, so the grant history stays reconstructable.
CREATE TABLE auth_role_grants (
    id         TEXT NOT NULL PRIMARY KEY,
    user_id    TEXT NOT NULL REFERENCES auth_users (id),
    role       TEXT NOT NULL CHECK (role IN ('accessioner', 'analyst', 'technical_reviewer',
                   'releaser', 'quality_manager', 'billing', 'auditor', 'administrator')),
    -- NULL only for the first administrator, granted by first-run setup.
    granted_by TEXT REFERENCES auth_users (id),
    granted_at TEXT NOT NULL,
    revoked_by TEXT REFERENCES auth_users (id),
    revoked_at TEXT,
    CHECK ((revoked_by IS NULL) = (revoked_at IS NULL))
) STRICT;

CREATE UNIQUE INDEX auth_role_grants_current ON auth_role_grants (user_id, role)
    WHERE revoked_at IS NULL;

CREATE TRIGGER auth_role_grants_revoke_only BEFORE UPDATE ON auth_role_grants
WHEN OLD.revoked_at IS NOT NULL
    OR NEW.id IS NOT OLD.id OR NEW.user_id IS NOT OLD.user_id OR NEW.role IS NOT OLD.role
    OR NEW.granted_by IS NOT OLD.granted_by OR NEW.granted_at IS NOT OLD.granted_at
BEGIN
    SELECT RAISE(ABORT, 'auth_role_grants rows are only revoked, never rewritten');
END;

CREATE TRIGGER auth_role_grants_no_delete BEFORE DELETE ON auth_role_grants
BEGIN
    SELECT RAISE(ABORT, 'auth_role_grants rows are never deleted');
END;

CREATE TABLE auth_sessions (
    id             TEXT NOT NULL PRIMARY KEY,
    user_id        TEXT NOT NULL REFERENCES auth_users (id),
    workstation_id TEXT NOT NULL REFERENCES auth_workstations (id),
    -- Web engine name and version, for the session's audit context (ADR 0012).
    webview        TEXT,
    started_at     TEXT NOT NULL,
    ended_at       TEXT,
    end_reason     TEXT CHECK (end_reason IN ('signed_out', 'switched', 'app_exit', 'abandoned')),
    CHECK ((ended_at IS NULL) = (end_reason IS NULL))
) STRICT;

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

-- Consecutive failed attempts per normalized username, whether or not an
-- account has that name, so unknown and known usernames behave the same.
-- A successful sign-in deletes the row.
CREATE TABLE auth_sign_in_failures (
    username     TEXT    NOT NULL PRIMARY KEY CHECK (username = lower(username)),
    failures     INTEGER NOT NULL CHECK (failures >= 1),
    locked_until TEXT,
    updated_at   TEXT    NOT NULL
) STRICT;
