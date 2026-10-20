-- Personal security keys and offline credential replacement (REQ-IDN-004,
-- NFR-19). A key account has no password/TOTP fallback. Its existing non-null
-- password column contains an unreachable random hash for schema compatibility.
ALTER TABLE auth_users ADD COLUMN authentication_method TEXT NOT NULL
    DEFAULT 'password_totp' CHECK (authentication_method IN ('password_totp', 'security_key'));

CREATE TABLE auth_security_keys (
    id              TEXT NOT NULL PRIMARY KEY,
    user_id         TEXT NOT NULL REFERENCES auth_users (id),
    credential_id   TEXT NOT NULL UNIQUE,
    -- Trusted verifier's serialized public credential; never a private key or PIN.
    credential_json TEXT NOT NULL,
    label           TEXT NOT NULL CHECK (length(label) BETWEEN 1 AND 80),
    created_at      TEXT NOT NULL,
    last_used_at    TEXT,
    revoked_at      TEXT,
    revision        INTEGER NOT NULL CHECK (revision >= 1),
    assertion_revision INTEGER NOT NULL DEFAULT 1 CHECK (assertion_revision >= 1)
) STRICT;

CREATE INDEX auth_security_keys_user ON auth_security_keys (user_id, revoked_at);

CREATE TRIGGER auth_security_keys_preserve_identity BEFORE UPDATE ON auth_security_keys
WHEN OLD.revoked_at IS NOT NULL OR NEW.id IS NOT OLD.id
    OR NEW.user_id IS NOT OLD.user_id OR NEW.credential_id IS NOT OLD.credential_id
    OR NEW.label IS NOT OLD.label OR NEW.created_at IS NOT OLD.created_at
    OR NEW.revision < OLD.revision OR NEW.assertion_revision < OLD.assertion_revision
    OR (NEW.revision = OLD.revision AND NEW.assertion_revision = OLD.assertion_revision)
BEGIN
    SELECT RAISE(ABORT, 'security-key identity is immutable; revoked keys remain revoked');
END;

CREATE TRIGGER auth_security_keys_no_delete BEFORE DELETE ON auth_security_keys
BEGIN
    SELECT RAISE(ABORT, 'security-key history is never deleted');
END;

CREATE TABLE auth_key_recovery_codes (
    id          TEXT NOT NULL PRIMARY KEY,
    user_id     TEXT NOT NULL REFERENCES auth_users (id),
    code_hash   TEXT NOT NULL CHECK (code_hash LIKE '$argon2id$%'),
    created_at  TEXT NOT NULL,
    consumed_at TEXT
) STRICT;

CREATE UNIQUE INDEX auth_key_recovery_codes_active ON auth_key_recovery_codes (user_id)
    WHERE consumed_at IS NULL;

CREATE TRIGGER auth_key_recovery_codes_consume_only BEFORE UPDATE ON auth_key_recovery_codes
WHEN OLD.consumed_at IS NOT NULL OR NEW.consumed_at IS NULL
    OR NEW.id IS NOT OLD.id OR NEW.user_id IS NOT OLD.user_id
    OR NEW.code_hash IS NOT OLD.code_hash OR NEW.created_at IS NOT OLD.created_at
BEGIN
    SELECT RAISE(ABORT, 'recovery codes may only be consumed once');
END;

CREATE TRIGGER auth_key_recovery_codes_no_delete BEFORE DELETE ON auth_key_recovery_codes
BEGIN
    SELECT RAISE(ABORT, 'recovery-code history is never deleted');
END;

-- Short-lived, hashed receipts. They carry no action payload and cannot create
-- sessions. The engine dispatches only the exact frozen operation it verified.
CREATE TABLE auth_key_action_proofs (
    proof_digest     BLOB NOT NULL PRIMARY KEY CHECK (length(proof_digest) = 32),
    user_id          TEXT NOT NULL REFERENCES auth_users (id),
    session_id       TEXT NOT NULL REFERENCES auth_sessions (id),
    key_id           TEXT NOT NULL REFERENCES auth_security_keys (id),
    purpose          TEXT NOT NULL,
    account_revision INTEGER NOT NULL,
    key_revision     INTEGER NOT NULL,
    issued_at        TEXT NOT NULL,
    expires_at       TEXT NOT NULL,
    consumed_at      TEXT
) STRICT;
