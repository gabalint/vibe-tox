-- The one row that names this installation (M4 design brief §2.2). Audit anchors, backup
-- sets and exit exports carry its ID. Written once, at the first start.

CREATE TABLE ops_installation (
    singleton       INTEGER NOT NULL PRIMARY KEY CHECK (singleton = 1),
    installation_id TEXT    NOT NULL UNIQUE,
    created_at      TEXT    NOT NULL
) STRICT;

CREATE TRIGGER ops_installation_no_update BEFORE UPDATE ON ops_installation
BEGIN
    SELECT RAISE(ABORT, 'the installation row is append-only');
END;

CREATE TRIGGER ops_installation_no_delete BEFORE DELETE ON ops_installation
BEGIN
    SELECT RAISE(ABORT, 'the installation row is append-only');
END;
