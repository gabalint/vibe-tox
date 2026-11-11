-- The audit viewer reads every stream together, newest first, by append time.
-- Sequence numbers are per stream, so (recorded_at, stream, seq) is the order
-- and the page cursor. This index changes no event, chain input or sequence.
CREATE INDEX audit_events_by_time ON audit_events (recorded_at, stream, seq);
