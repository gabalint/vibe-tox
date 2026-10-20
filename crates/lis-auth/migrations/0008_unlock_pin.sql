-- Optional personal PIN unlocks only an already authenticated session.
-- Only the installation-key-encrypted, salted Argon2id verifier is persisted.
CREATE TABLE auth_unlock_pins (
    user_id TEXT NOT NULL PRIMARY KEY REFERENCES auth_users (id),
    verifier BLOB NOT NULL CHECK (length(verifier) > 28),
    updated_at TEXT NOT NULL,
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

-- This counter survives commands and locks. Only PIN success or an explicitly
-- authenticated PIN configuration change resets it; a new session starts empty.
CREATE TABLE auth_unlock_pin_attempts (
    session_id TEXT NOT NULL PRIMARY KEY REFERENCES auth_sessions (id),
    failures INTEGER NOT NULL CHECK (failures BETWEEN 1 AND 5)
) STRICT;
