-- Identity and organizations (REQ-IDN-001, REQ-OPS-001; INV-02).
-- Names, birth dates and identifier values are PHI: they live here and never
-- in audit details or logs.

-- A person the laboratory holds records for. The row carries only the
-- revision; demographics are assertions and identifiers are separate rows.
CREATE TABLE idn_subjects (
    id         TEXT    NOT NULL PRIMARY KEY,
    revision   INTEGER NOT NULL CHECK (revision >= 1),
    created_at TEXT    NOT NULL
) STRICT;

-- Demographics as provided by a source. A change adds a new assertion with a
-- reason; earlier assertions are never changed (REQ-IDN-002 builds on this).
CREATE TABLE idn_identity_assertions (
    id                 TEXT    NOT NULL PRIMARY KEY,
    subject_id         TEXT    NOT NULL REFERENCES idn_subjects (id),
    sequence           INTEGER NOT NULL CHECK (sequence >= 1),
    family_name        TEXT    NOT NULL,
    given_names        TEXT,
    birth_date         TEXT    NOT NULL,
    administrative_sex TEXT,
    source             TEXT    NOT NULL,
    reason             TEXT,
    -- NFC-normalized, lowercased family name, given names and birth date for candidate listing.
    match_key          TEXT    NOT NULL,
    asserted_at        TEXT    NOT NULL,
    asserted_by_kind   TEXT    NOT NULL CHECK (asserted_by_kind IN ('user', 'instrument', 'system')),
    asserted_by_id     TEXT    NOT NULL,
    UNIQUE (subject_id, sequence),
    -- The first assertion registers the subject; every later one needs a reason.
    CHECK ((sequence = 1) = (reason IS NULL))
) STRICT;

CREATE INDEX idn_identity_assertions_match ON idn_identity_assertions (match_key);

CREATE TRIGGER idn_identity_assertions_no_update BEFORE UPDATE ON idn_identity_assertions
BEGIN
    SELECT RAISE(ABORT, 'identity assertions are append-only');
END;

CREATE TRIGGER idn_identity_assertions_no_delete BEFORE DELETE ON idn_identity_assertions
BEGIN
    SELECT RAISE(ABORT, 'identity assertions are append-only');
END;

-- An identifier is meaningful only within its assigning authority and type:
-- the same value from two authorities never links subjects (INV-02).
CREATE TABLE idn_subject_identifiers (
    id                  TEXT NOT NULL PRIMARY KEY,
    subject_id          TEXT NOT NULL REFERENCES idn_subjects (id),
    assigning_authority TEXT NOT NULL,
    identifier_type     TEXT NOT NULL,
    value               TEXT NOT NULL,
    status              TEXT NOT NULL CHECK (status IN ('active', 'inactive')),
    added_at            TEXT NOT NULL,
    deactivated_at      TEXT,
    deactivation_reason TEXT,
    CHECK ((status = 'active') = (deactivated_at IS NULL)),
    CHECK ((deactivated_at IS NULL) = (deactivation_reason IS NULL))
) STRICT;

CREATE UNIQUE INDEX idn_subject_identifiers_active
    ON idn_subject_identifiers (assigning_authority, identifier_type, value)
    WHERE status = 'active';

-- Historical ownership prevents a delayed artifact from routing an old
-- identifier to a different subject. Reuse by the same subject is permitted.
CREATE TRIGGER idn_subject_identifiers_no_transfer BEFORE INSERT ON idn_subject_identifiers
WHEN EXISTS (
    SELECT 1 FROM idn_subject_identifiers existing
    WHERE existing.assigning_authority = NEW.assigning_authority
      AND existing.identifier_type = NEW.identifier_type
      AND existing.value = NEW.value
      AND existing.subject_id <> NEW.subject_id
)
BEGIN
    SELECT RAISE(ABORT, 'subject identifiers cannot be transferred');
END;

CREATE INDEX idn_subject_identifiers_subject ON idn_subject_identifiers (subject_id);

-- The only permitted change is deactivation; the identifier itself never changes.
CREATE TRIGGER idn_subject_identifiers_deactivate_only BEFORE UPDATE ON idn_subject_identifiers
WHEN OLD.status <> 'active' OR NEW.status <> 'inactive'
    OR NEW.id IS NOT OLD.id OR NEW.subject_id IS NOT OLD.subject_id
    OR NEW.assigning_authority IS NOT OLD.assigning_authority
    OR NEW.identifier_type IS NOT OLD.identifier_type
    OR NEW.value IS NOT OLD.value OR NEW.added_at IS NOT OLD.added_at
BEGIN
    SELECT RAISE(ABORT, 'subject identifiers can only be deactivated');
END;

CREATE TRIGGER idn_subject_identifiers_no_delete BEFORE DELETE ON idn_subject_identifiers
BEGIN
    SELECT RAISE(ABORT, 'subject identifiers are never deleted');
END;

-- A review task for an unresolved identity question; while open it blocks
-- release for the subject and every candidate subject (REQ-IDN-001).
CREATE TABLE idn_identity_holds (
    id                TEXT    NOT NULL PRIMARY KEY,
    subject_id        TEXT    NOT NULL REFERENCES idn_subjects (id),
    subject_revision  INTEGER NOT NULL CHECK (subject_revision >= 1),
    kind              TEXT    NOT NULL CHECK (kind IN ('possible_duplicate')),
    status            TEXT    NOT NULL CHECK (status IN ('open', 'resolved')),
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    opened_at         TEXT    NOT NULL,
    resolution        TEXT    CHECK (resolution IN ('confirmed_distinct')),
    resolution_reason TEXT,
    resolved_at       TEXT,
    resolved_by_kind  TEXT    CHECK (resolved_by_kind = 'user'),
    resolved_by_id    TEXT,
    CHECK ((status = 'open') = (resolution IS NULL)),
    CHECK ((resolution IS NULL) = (resolution_reason IS NULL)),
    CHECK ((resolution IS NULL) = (resolved_at IS NULL)),
    CHECK ((resolution IS NULL) = (resolved_by_kind IS NULL)),
    CHECK ((resolution IS NULL) = (resolved_by_id IS NULL))
) STRICT;

CREATE INDEX idn_identity_holds_subject ON idn_identity_holds (subject_id) WHERE status = 'open';

CREATE TRIGGER idn_identity_holds_resolved_immutable BEFORE UPDATE ON idn_identity_holds
WHEN OLD.status <> 'open'
    OR NEW.id IS NOT OLD.id OR NEW.subject_id IS NOT OLD.subject_id
    OR NEW.subject_revision IS NOT OLD.subject_revision
    OR NEW.kind IS NOT OLD.kind OR NEW.opened_at IS NOT OLD.opened_at
BEGIN
    SELECT RAISE(ABORT, 'resolved identity holds are immutable');
END;

CREATE TRIGGER idn_identity_holds_no_delete BEFORE DELETE ON idn_identity_holds
BEGIN
    SELECT RAISE(ABORT, 'identity holds are never deleted');
END;

-- The existing subjects a hold's subject may duplicate.
CREATE TABLE idn_identity_hold_candidates (
    hold_id    TEXT NOT NULL REFERENCES idn_identity_holds (id),
    subject_id TEXT NOT NULL REFERENCES idn_subjects (id),
    subject_revision INTEGER NOT NULL CHECK (subject_revision >= 1),
    PRIMARY KEY (hold_id, subject_id)
) STRICT;

CREATE INDEX idn_identity_hold_candidates_subject ON idn_identity_hold_candidates (subject_id);

CREATE TRIGGER idn_identity_hold_candidates_no_update BEFORE UPDATE ON idn_identity_hold_candidates
BEGIN
    SELECT RAISE(ABORT, 'identity hold candidates are append-only');
END;

CREATE TRIGGER idn_identity_hold_candidates_no_delete BEFORE DELETE ON idn_identity_hold_candidates
BEGIN
    SELECT RAISE(ABORT, 'identity hold candidates are append-only');
END;

-- Ordering clients and their locations.
CREATE TABLE idn_client_organizations (
    id         TEXT NOT NULL PRIMARY KEY,
    code       TEXT NOT NULL UNIQUE,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL
) STRICT;

CREATE TABLE idn_client_locations (
    id         TEXT NOT NULL PRIMARY KEY,
    client_id  TEXT NOT NULL REFERENCES idn_client_organizations (id),
    code       TEXT NOT NULL,
    name       TEXT NOT NULL,
    created_at TEXT NOT NULL,
    UNIQUE (client_id, code)
) STRICT;

-- Ordering practitioners, identified by NPI (assigning authority "NPI").
CREATE TABLE idn_practitioners (
    id          TEXT NOT NULL PRIMARY KEY,
    npi         TEXT NOT NULL UNIQUE CHECK (length(npi) = 10 AND npi NOT GLOB '*[^0-9]*'),
    family_name TEXT NOT NULL,
    given_names TEXT,
    created_at  TEXT NOT NULL
) STRICT;
