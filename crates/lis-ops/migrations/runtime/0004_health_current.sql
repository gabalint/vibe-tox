-- A causal pointer for newly observed health state. Legacy observation timestamps
-- cannot establish the last actual evaluation when clocks tied or moved backward.
-- Intentionally no backfill: the next real evaluation records a fresh observation.
CREATE TABLE ops_health_current (
    check_code TEXT NOT NULL PRIMARY KEY,
    event_id TEXT NOT NULL REFERENCES ops_health_events(id)
) STRICT;

CREATE TRIGGER ops_health_current_valid_insert BEFORE INSERT ON ops_health_current
WHEN NOT EXISTS (SELECT 1 FROM ops_health_events e
                 WHERE e.id = NEW.event_id AND e.check_code = NEW.check_code)
BEGIN
    SELECT RAISE(ABORT, 'health pointer must reference the same check');
END;

CREATE TRIGGER ops_health_current_valid_update BEFORE UPDATE ON ops_health_current
WHEN NEW.check_code != OLD.check_code OR NOT EXISTS (
    SELECT 1 FROM ops_health_events e WHERE e.id = NEW.event_id AND e.check_code = NEW.check_code)
BEGIN
    SELECT RAISE(ABORT, 'health pointer must reference the same check');
END;
