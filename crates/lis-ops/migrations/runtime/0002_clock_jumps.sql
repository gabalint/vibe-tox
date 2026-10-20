-- Wall-clock jumps the light loop detects (NFR-26, REQ-OPS-013; M4 review M9). A backward
-- jump keeps the clock alert for 24 hours, also across a restart. A forward jump while
-- analyzer listeners were active is a listening gap, acknowledged with the run's gaps by
-- someone who performs analysis; the note is the audit reason.
CREATE TABLE ops_clock_jumps (
    id              TEXT    NOT NULL PRIMARY KEY,
    run_id          TEXT    NOT NULL REFERENCES ops_app_runs (id),
    detected_at     TEXT    NOT NULL,
    direction       TEXT    NOT NULL CHECK (direction IN ('backward', 'forward')),
    seconds         INTEGER NOT NULL CHECK (seconds > 0),
    listening       INTEGER NOT NULL CHECK (listening IN (0, 1)),
    acknowledged_by TEXT,
    acknowledged_at TEXT,
    CHECK ((acknowledged_by IS NULL) = (acknowledged_at IS NULL)),
    CHECK (acknowledged_at IS NULL OR (direction = 'forward' AND listening = 1))
) STRICT;

CREATE INDEX ops_clock_jumps_run ON ops_clock_jumps (run_id);
CREATE INDEX ops_clock_jumps_direction ON ops_clock_jumps (direction, detected_at);

CREATE TRIGGER ops_clock_jumps_insert BEFORE INSERT ON ops_clock_jumps
WHEN NEW.acknowledged_at IS NOT NULL
BEGIN
    SELECT RAISE(ABORT, 'a clock jump is recorded unacknowledged');
END;

CREATE TRIGGER ops_clock_jumps_acknowledge_once BEFORE UPDATE ON ops_clock_jumps
WHEN OLD.acknowledged_at IS NOT NULL OR NEW.acknowledged_at IS NULL
    OR NEW.id IS NOT OLD.id OR NEW.run_id IS NOT OLD.run_id
    OR NEW.detected_at IS NOT OLD.detected_at OR NEW.direction IS NOT OLD.direction
    OR NEW.seconds IS NOT OLD.seconds OR NEW.listening IS NOT OLD.listening
BEGIN
    SELECT RAISE(ABORT, 'a clock jump is only acknowledged, once');
END;

CREATE TRIGGER ops_clock_jumps_no_delete BEFORE DELETE ON ops_clock_jumps
BEGIN
    SELECT RAISE(ABORT, 'clock jumps are never deleted');
END;
