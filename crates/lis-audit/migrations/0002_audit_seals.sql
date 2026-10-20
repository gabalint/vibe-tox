-- Seals of the audit chains (NFR-18, ADR 0008, M4 design brief §4).
-- A seal records a stream's head at a point in time. Seals form their own
-- chain per stream: each continues the previous one exactly, and its head
-- hash is the stored hash of the event at last_seq, which commits to the whole
-- prefix of the stream. Anchors are these rows written off the host.
CREATE TABLE audit_seals (
    stream    TEXT    NOT NULL CHECK (stream IN ('clinical', 'security', 'access')),
    seal_no   INTEGER NOT NULL CHECK (seal_no >= 1),
    first_seq INTEGER NOT NULL CHECK (first_seq >= 1),
    last_seq  INTEGER NOT NULL CHECK (last_seq >= first_seq),
    prev_hash TEXT    NOT NULL CHECK (length(prev_hash) = 64),
    head_hash TEXT    NOT NULL CHECK (length(head_hash) = 64),
    sealed_at TEXT    NOT NULL,
    hash_alg  TEXT    NOT NULL CHECK (hash_alg IN ('sha256-v1')),
    PRIMARY KEY (stream, seal_no)
) STRICT;

-- The chain of seals: seal_no is the previous one plus 1 (or 1); first_seq is
-- the previous last_seq plus 1 (or 1); prev_hash is the previous head_hash (or
-- the genesis value); head_hash is the stored hash of the event at last_seq.
CREATE TRIGGER audit_seals_continue_chain BEFORE INSERT ON audit_seals
BEGIN
    SELECT RAISE(ABORT, 'audit seal does not continue the chain of seals')
    WHERE NEW.seal_no IS NOT coalesce(
            (SELECT seal_no + 1 FROM audit_seals WHERE stream = NEW.stream
             ORDER BY seal_no DESC LIMIT 1), 1)
       OR NEW.first_seq IS NOT coalesce(
            (SELECT last_seq + 1 FROM audit_seals WHERE stream = NEW.stream
             ORDER BY seal_no DESC LIMIT 1), 1)
       OR NEW.prev_hash IS NOT coalesce(
            (SELECT head_hash FROM audit_seals WHERE stream = NEW.stream
             ORDER BY seal_no DESC LIMIT 1),
            '0000000000000000000000000000000000000000000000000000000000000000')
       OR NEW.head_hash IS NOT
            (SELECT hash FROM audit_events WHERE stream = NEW.stream AND seq = NEW.last_seq);
END;

-- Stops application bugs, not the OS administrator; verification compares
-- seals with the chain, and off-host anchors cover rewritten seals.
CREATE TRIGGER audit_seals_no_update BEFORE UPDATE ON audit_seals
BEGIN
    SELECT RAISE(ABORT, 'audit_seals is append-only');
END;

CREATE TRIGGER audit_seals_no_delete BEFORE DELETE ON audit_seals
BEGIN
    SELECT RAISE(ABORT, 'audit_seals is append-only');
END;
