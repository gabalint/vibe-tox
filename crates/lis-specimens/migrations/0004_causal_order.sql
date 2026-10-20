-- DM-01: causal aggregate revisions, never wall-clock/UUID ordering.
-- Legacy links are accepted only when both directions of the audit match
-- are unique. Missing links remain explicit and block clinical use.
CREATE TABLE spc_receipt_order (
 decision_id TEXT PRIMARY KEY REFERENCES spc_receipt_decisions(id),
 specimen_id TEXT NOT NULL REFERENCES spc_specimens(id),
 sequence INTEGER NOT NULL CHECK(sequence > 0),
 source TEXT NOT NULL CHECK(source IN ('audit', 'command', 'review')),
 UNIQUE(specimen_id, sequence)
) STRICT;
CREATE TABLE spc_location_order (
 event_id TEXT PRIMARY KEY REFERENCES spc_location_events(id),
 container_id TEXT NOT NULL REFERENCES spc_containers(id),
 sequence INTEGER NOT NULL CHECK(sequence > 0),
 source TEXT NOT NULL CHECK(source IN ('audit', 'command')),
 UNIQUE(container_id, sequence)
) STRICT;
CREATE TEMP TABLE receipt_matches AS
 SELECT d.id, d.specimen_id, a.new_revision AS sequence
 FROM spc_receipt_decisions d JOIN audit_events a
 ON a.stream = 'clinical' AND a.action = 'specimen.record_receipt_decision'
 AND a.outcome = 'succeeded' AND a.record_id = d.specimen_id
 AND a.principal_id = d.decided_by AND a.occurred_at = d.decided_at
 AND json_extract(a.details, '$.decision') = d.decision
 AND a.reason IS d.reason AND a.new_revision IS NOT NULL;
INSERT INTO spc_receipt_order
 SELECT m.id, m.specimen_id, m.sequence, 'audit' FROM receipt_matches m
 WHERE (SELECT count(*) FROM receipt_matches x WHERE x.id = m.id) = 1
 AND (SELECT count(*) FROM receipt_matches x WHERE x.specimen_id = m.specimen_id
      AND x.sequence = m.sequence) = 1;
DROP TABLE receipt_matches;
CREATE TEMP TABLE location_matches AS
 SELECT e.id, e.container_id, a.new_revision AS sequence
 FROM spc_location_events e JOIN audit_events a
 ON a.stream = 'clinical' AND a.action = 'specimen.move_container'
 AND a.outcome = 'succeeded' AND a.record_id = e.container_id
 AND a.principal_id = e.moved_by AND a.occurred_at = e.moved_at
 AND json_extract(a.details, '$.location') = e.location_id
 AND a.reason IS e.reason AND a.new_revision IS NOT NULL;
INSERT INTO spc_location_order
 SELECT m.id, m.container_id, m.sequence, 'audit' FROM location_matches m
 WHERE (SELECT count(*) FROM location_matches x WHERE x.id = m.id) = 1
 AND (SELECT count(*) FROM location_matches x WHERE x.container_id = m.container_id
      AND x.sequence = m.sequence) = 1;
DROP TABLE location_matches;
CREATE TRIGGER spc_receipt_order_new AFTER INSERT ON spc_receipt_decisions
BEGIN INSERT INTO spc_receipt_order VALUES(new.id, new.specimen_id,
 (SELECT revision FROM spc_specimens WHERE id = new.specimen_id), 'command'); END;
CREATE TRIGGER spc_location_order_new AFTER INSERT ON spc_location_events
BEGIN INSERT INTO spc_location_order VALUES(new.id, new.container_id,
 (SELECT revision FROM spc_containers WHERE id = new.container_id), 'command'); END;
CREATE TRIGGER spc_receipt_order_no_update BEFORE UPDATE ON spc_receipt_order
BEGIN SELECT RAISE(ABORT, 'spc_receipt_order is append-only'); END;
CREATE TRIGGER spc_receipt_order_no_delete BEFORE DELETE ON spc_receipt_order
BEGIN SELECT RAISE(ABORT, 'spc_receipt_order is append-only'); END;
CREATE TRIGGER spc_location_order_no_update BEFORE UPDATE ON spc_location_order
BEGIN SELECT RAISE(ABORT, 'spc_location_order is append-only'); END;
CREATE TRIGGER spc_location_order_no_delete BEFORE DELETE ON spc_location_order
BEGIN SELECT RAISE(ABORT, 'spc_location_order is append-only'); END;
