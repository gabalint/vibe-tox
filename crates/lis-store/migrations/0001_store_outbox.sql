-- Transactional outbox (NFR-02, NFR-06, ADR 0012). A row is written in the
-- same transaction as the change that needs follow-up work; the row, not any
-- in-memory queue, is the authority after a crash.
CREATE TABLE store_outbox (
    id               TEXT    NOT NULL PRIMARY KEY,
    topic            TEXT    NOT NULL CHECK (length(topic) > 0),
    dedup_key        TEXT    NOT NULL CHECK (length(dedup_key) > 0),
    payload          TEXT    NOT NULL,
    state            TEXT    NOT NULL CHECK (state IN ('pending', 'leased', 'done', 'dead')),
    attempts         INTEGER NOT NULL CHECK (attempts >= 0),
    max_attempts     INTEGER NOT NULL CHECK (max_attempts >= 1),
    available_at     TEXT    NOT NULL,
    lease_expires_at TEXT,
    last_error_code  TEXT,
    created_at       TEXT    NOT NULL,
    updated_at       TEXT    NOT NULL,
    UNIQUE (topic, dedup_key),
    CHECK ((state = 'leased') = (lease_expires_at IS NOT NULL))
) STRICT;

CREATE INDEX store_outbox_ready ON store_outbox (topic, state, available_at);
