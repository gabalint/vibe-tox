-- The laboratory's separation-of-duties policy (ADR 0015; Q-08, Q-37). Each switch says
-- whether the person who did the first step of a duty may also do the second: approve
-- configuration they wrote, release a report whose results they reviewed, or verify a
-- delivery destination they created. CLIA lets one qualified person hold every role
-- (42 CFR 493.1445(a)), so this is laboratory policy, not a fixed rule. Versioned and
-- append-only; until the laboratory sets a version, every duty needs a second person.
-- Every version carries the reason, because each permitted self-approval is a
-- laboratory-policy exception.

CREATE TABLE auth_separation_policies (
    version                    INTEGER NOT NULL PRIMARY KEY CHECK (version >= 1),
    self_approve_configuration INTEGER NOT NULL CHECK (self_approve_configuration IN (0, 1)),
    self_release_reports       INTEGER NOT NULL CHECK (self_release_reports IN (0, 1)),
    self_verify_destinations   INTEGER NOT NULL CHECK (self_verify_destinations IN (0, 1)),
    reason                     TEXT    NOT NULL CHECK (length(reason) > 0),
    -- The acting principal's ID text.
    set_by                     TEXT    NOT NULL CHECK (length(set_by) > 0),
    set_at                     TEXT    NOT NULL
) STRICT;

CREATE TRIGGER auth_separation_policies_next_version BEFORE INSERT ON auth_separation_policies
WHEN NEW.version <> (SELECT coalesce(max(version), 0) + 1 FROM auth_separation_policies)
BEGIN
    SELECT RAISE(ABORT, 'separation policy versions count up from 1 without gaps');
END;

CREATE TRIGGER auth_separation_policies_no_update BEFORE UPDATE ON auth_separation_policies
BEGIN
    SELECT RAISE(ABORT, 'separation policy versions are append-only');
END;

CREATE TRIGGER auth_separation_policies_no_delete BEFORE DELETE ON auth_separation_policies
BEGIN
    SELECT RAISE(ABORT, 'separation policy versions are append-only');
END;

-- The policy in force, for other modules' schema guards: no row while the defaults
-- apply, so a guard reads `coalesce(..., 0)` and needs a second person.
CREATE VIEW auth_v_separation_policy AS
SELECT version, self_approve_configuration, self_release_reports, self_verify_destinations
FROM auth_separation_policies
ORDER BY version DESC
LIMIT 1;
