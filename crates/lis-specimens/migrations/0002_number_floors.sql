-- Number floors (REQ-OPS-002; M4 design brief §2.2, §3.8). Restoring an older backup
-- would issue again accession numbers and container codes that are already on physical
-- labels, so a restore (and a quality manager, by hand) sets a floor: numbering then
-- continues above the larger of the highest issued number and the highest floor of the
-- same prefix. Append-only; a floor only goes up within its prefix. Floors are full
-- numbers in the format `numbering.rs` defines (`A` + yy + 6 digits, `C` + 8 digits).

CREATE TABLE spc_number_floors (
    id           TEXT NOT NULL PRIMARY KEY,
    kind         TEXT NOT NULL CHECK (kind IN ('accession', 'container')),
    floor_number TEXT NOT NULL CHECK (
        (kind = 'accession' AND length(floor_number) = 9 AND substr(floor_number, 1, 1) = 'A')
        OR (kind = 'container' AND length(floor_number) = 9 AND substr(floor_number, 1, 1) = 'C')
    ),
    source       TEXT NOT NULL CHECK (source IN ('restore', 'manual')),
    set_by       TEXT NOT NULL,
    set_at       TEXT NOT NULL
) STRICT;

CREATE INDEX spc_number_floors_kind ON spc_number_floors (kind, floor_number);

-- The prefix is `Ayy` for accession numbers and `C` for container codes.
CREATE TRIGGER spc_number_floors_only_up BEFORE INSERT ON spc_number_floors
WHEN EXISTS (
    SELECT 1 FROM spc_number_floors f
    WHERE f.kind = NEW.kind
      AND substr(f.floor_number, 1, CASE NEW.kind WHEN 'accession' THEN 3 ELSE 1 END)
        = substr(NEW.floor_number, 1, CASE NEW.kind WHEN 'accession' THEN 3 ELSE 1 END)
      AND f.floor_number >= NEW.floor_number
)
BEGIN
    SELECT RAISE(ABORT, 'a number floor only goes up');
END;

CREATE TRIGGER spc_number_floors_no_update BEFORE UPDATE ON spc_number_floors
BEGIN
    SELECT RAISE(ABORT, 'number floors are append-only');
END;

CREATE TRIGGER spc_number_floors_no_delete BEFORE DELETE ON spc_number_floors
BEGIN
    SELECT RAISE(ABORT, 'number floors are append-only');
END;
