-- Causal dependency checks resolve exact audit records, not a whole stream.
-- This covering index changes no event, chain input, sequence or retention rule.
CREATE INDEX audit_record_events ON audit_events
    (stream, action, record_type, record_id, outcome, seq, new_revision);
