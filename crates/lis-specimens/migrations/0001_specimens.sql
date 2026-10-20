-- Specimens module (REQ-SPC-001–005, REQ-SPC-008, REQ-OPS-001).
-- Accession, collection, specimen, container and aliquot are separate
-- identities (REQ-SPC-001). Evidence tables are append-only; mutable
-- aggregates carry a revision. Decimal volumes are lexical TEXT and are never
-- computed in SQL. `*_by` columns hold `Actor::id_text()`.

CREATE TABLE spc_accessions (
    id               TEXT NOT NULL PRIMARY KEY,
    accession_number TEXT NOT NULL UNIQUE,
    order_id         TEXT NOT NULL REFERENCES ord_orders (id),
    subject_id       TEXT NOT NULL REFERENCES idn_subjects (id),
    created_by       TEXT NOT NULL,
    created_at       TEXT NOT NULL
) STRICT;

-- The order items an accession fulfils. An item is accessioned once; a
-- recollection flow is not part of M2.
CREATE TABLE spc_accession_items (
    accession_id  TEXT NOT NULL REFERENCES spc_accessions (id),
    order_item_id TEXT NOT NULL UNIQUE REFERENCES ord_order_items (id),
    PRIMARY KEY (accession_id, order_item_id)
) STRICT;

-- One collection event per accession. Unknown and date-only collection
-- times stay explicit (REQ-SPC-002); the source offset is kept separately.
CREATE TABLE spc_collections (
    id                       TEXT    NOT NULL PRIMARY KEY,
    accession_id             TEXT    NOT NULL UNIQUE REFERENCES spc_accessions (id),
    precision                TEXT    NOT NULL CHECK (precision IN ('datetime', 'date', 'unknown')),
    collected_at             TEXT,
    collected_offset_minutes INTEGER,
    collected_date           TEXT,
    collector_text           TEXT,
    recorded_by              TEXT    NOT NULL,
    recorded_at              TEXT    NOT NULL,
    CHECK (
        (precision = 'datetime' AND collected_at IS NOT NULL
            AND collected_offset_minutes IS NOT NULL AND collected_date IS NULL)
        OR (precision = 'date' AND collected_at IS NULL
            AND collected_offset_minutes IS NULL AND collected_date IS NOT NULL)
        OR (precision = 'unknown' AND collected_at IS NULL
            AND collected_offset_minutes IS NULL AND collected_date IS NULL)
    )
) STRICT;

-- Collection-site observations as reported, attributed to the observer (REQ-SPC-008).
CREATE TABLE spc_collection_observations (
    id               TEXT NOT NULL PRIMARY KEY,
    collection_id    TEXT NOT NULL REFERENCES spc_collections (id),
    code             TEXT NOT NULL,
    value_text       TEXT NOT NULL,
    observed_by_text TEXT NOT NULL,
    recorded_by      TEXT NOT NULL,
    recorded_at      TEXT NOT NULL
) STRICT;

CREATE INDEX spc_collection_observations_by_collection
    ON spc_collection_observations (collection_id);

CREATE TABLE spc_specimens (
    id                 TEXT    NOT NULL PRIMARY KEY,
    collection_id      TEXT    NOT NULL REFERENCES spc_collections (id),
    specimen_type_code TEXT    NOT NULL REFERENCES cat_specimen_types (code),
    acceptance         TEXT    NOT NULL CHECK (acceptance IN
        ('received', 'accepted', 'accepted_conditionally', 'held', 'rejected')),
    received_at        TEXT    NOT NULL,
    received_by        TEXT    NOT NULL,
    revision           INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

CREATE INDEX spc_specimens_by_collection ON spc_specimens (collection_id);

-- Receipt decisions (REQ-SPC-002). Conditional acceptance needs a policy
-- reference and scope; hold and rejection need a reason.
CREATE TABLE spc_receipt_decisions (
    id               TEXT NOT NULL PRIMARY KEY,
    specimen_id      TEXT NOT NULL REFERENCES spc_specimens (id),
    decision         TEXT NOT NULL CHECK (decision IN
        ('accepted', 'accepted_conditionally', 'held', 'rejected')),
    policy_reference TEXT,
    scope_text       TEXT,
    reason           TEXT,
    decided_by       TEXT NOT NULL,
    decided_at       TEXT NOT NULL,
    CHECK (decision <> 'accepted_conditionally'
        OR (policy_reference IS NOT NULL AND scope_text IS NOT NULL)),
    CHECK (decision NOT IN ('held', 'rejected') OR reason IS NOT NULL)
) STRICT;

CREATE INDEX spc_receipt_decisions_by_specimen ON spc_receipt_decisions (specimen_id);

CREATE TABLE spc_receipt_discrepancies (
    decision_id TEXT NOT NULL REFERENCES spc_receipt_decisions (id),
    code        TEXT NOT NULL CHECK (code IN ('unlabeled', 'label_mismatch', 'leaking',
        'insufficient_volume', 'temperature_or_stability', 'container_damaged', 'other')),
    note        TEXT,
    CHECK (code <> 'other' OR note IS NOT NULL),
    PRIMARY KEY (decision_id, code)
) STRICT;

-- Primary containers and aliquots. An aliquot's volume is the amount taken
-- from its parent; volumes are never edited, so the remaining volume of a
-- parent is computed (REQ-SPC-003).
CREATE TABLE spc_containers (
    id                  TEXT    NOT NULL PRIMARY KEY,
    specimen_id         TEXT    NOT NULL REFERENCES spc_specimens (id),
    label_code          TEXT    NOT NULL UNIQUE,
    kind                TEXT    NOT NULL CHECK (kind IN ('primary', 'aliquot')),
    parent_container_id TEXT    REFERENCES spc_containers (id),
    volume              TEXT,
    volume_unit         TEXT,
    volume_kind         TEXT    NOT NULL CHECK (volume_kind IN ('estimated', 'measured', 'unknown')),
    state               TEXT    NOT NULL CHECK (state IN ('available', 'depleted', 'disposed')),
    created_by          TEXT    NOT NULL,
    created_at          TEXT    NOT NULL,
    revision            INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((kind = 'aliquot') = (parent_container_id IS NOT NULL)),
    CHECK ((volume_kind = 'unknown') = (volume IS NULL)),
    CHECK ((volume IS NULL) = (volume_unit IS NULL))
) STRICT;

CREATE INDEX spc_containers_by_specimen ON spc_containers (specimen_id);
CREATE INDEX spc_containers_by_parent ON spc_containers (parent_container_id);

-- REQ-SPC-001: a container keeps its identity, label and lineage for good;
-- only its state and revision change, and a disposed container is final.
CREATE TRIGGER spc_containers_identity_immutable
BEFORE UPDATE OF id, specimen_id, label_code, kind, parent_container_id, volume, volume_unit,
    volume_kind, created_by, created_at ON spc_containers
BEGIN
    SELECT RAISE(ABORT, 'container identity and volume are immutable');
END;

CREATE TRIGGER spc_containers_disposed_final
BEFORE UPDATE ON spc_containers
WHEN OLD.state = 'disposed'
BEGIN
    SELECT RAISE(ABORT, 'a disposed container is final');
END;

-- Storage locations (configuration).
CREATE TABLE spc_locations (
    id                TEXT    NOT NULL PRIMARY KEY,
    code              TEXT    NOT NULL UNIQUE,
    name              TEXT    NOT NULL,
    storage_condition TEXT,
    active            INTEGER NOT NULL CHECK (active IN (0, 1)),
    created_at        TEXT    NOT NULL,
    revision          INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;

-- Moves with actor and time evidence (REQ-SPC-003); the latest is current.
CREATE TABLE spc_location_events (
    id           TEXT NOT NULL PRIMARY KEY,
    container_id TEXT NOT NULL REFERENCES spc_containers (id),
    location_id  TEXT NOT NULL REFERENCES spc_locations (id),
    moved_by     TEXT NOT NULL,
    moved_at     TEXT NOT NULL,
    reason       TEXT
) STRICT;

CREATE INDEX spc_location_events_by_container ON spc_location_events (container_id, moved_at);

-- Temperature or storage excursions at a location, each with an assessment (REQ-SPC-005).
CREATE TABLE spc_excursions (
    id                TEXT    NOT NULL PRIMARY KEY,
    location_id       TEXT    NOT NULL REFERENCES spc_locations (id),
    observed_from     TEXT    NOT NULL,
    observed_to       TEXT,
    description       TEXT    NOT NULL,
    recorded_by       TEXT    NOT NULL,
    recorded_at       TEXT    NOT NULL,
    state             TEXT    NOT NULL CHECK (state IN ('open', 'assessed')),
    outcome           TEXT    CHECK (outcome IN ('no_impact', 'holds_placed')),
    assessed_by       TEXT,
    assessed_at       TEXT,
    assessment_reason TEXT,
    revision          INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (observed_to IS NULL OR observed_to >= observed_from),
    CHECK ((state = 'assessed') = (outcome IS NOT NULL)),
    CHECK ((state = 'assessed') = (assessed_by IS NOT NULL AND assessed_at IS NOT NULL
        AND assessment_reason IS NOT NULL))
) STRICT;

-- Holds on containers (REQ-SPC-005). An open hold blocks disposal and aliquoting.
CREATE TABLE spc_holds (
    id             TEXT    NOT NULL PRIMARY KEY,
    container_id   TEXT    NOT NULL REFERENCES spc_containers (id),
    kind           TEXT    NOT NULL CHECK (kind IN ('retention', 'legal', 'quality', 'excursion')),
    excursion_id   TEXT    REFERENCES spc_excursions (id),
    reason         TEXT    NOT NULL,
    state          TEXT    NOT NULL CHECK (state IN ('open', 'released')),
    placed_by      TEXT    NOT NULL,
    placed_at      TEXT    NOT NULL,
    released_by    TEXT,
    released_at    TEXT,
    release_reason TEXT,
    revision       INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((kind = 'excursion') = (excursion_id IS NOT NULL)),
    CHECK ((state = 'released') = (released_by IS NOT NULL AND released_at IS NOT NULL
        AND release_reason IS NOT NULL))
) STRICT;

CREATE INDEX spc_holds_by_container ON spc_holds (container_id, state);

-- Durable disposal evidence; the container row stays (REQ-SPC-005).
CREATE TABLE spc_disposals (
    container_id TEXT NOT NULL PRIMARY KEY REFERENCES spc_containers (id),
    disposed_by  TEXT NOT NULL,
    disposed_at  TEXT NOT NULL,
    reason       TEXT NOT NULL
) STRICT;

-- Label printers (configuration). TCP printers are addressed by IP, never by host name.
CREATE TABLE spc_printers (
    id         TEXT    NOT NULL PRIMARY KEY,
    code       TEXT    NOT NULL UNIQUE,
    name       TEXT    NOT NULL,
    transport  TEXT    NOT NULL CHECK (transport IN ('tcp', 'cups')),
    ip_address TEXT,
    port       INTEGER CHECK (port BETWEEN 1 AND 65535),
    queue      TEXT,
    active     INTEGER NOT NULL CHECK (active IN (0, 1)),
    created_at TEXT    NOT NULL,
    revision   INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (
        (transport = 'tcp' AND ip_address IS NOT NULL AND port IS NOT NULL AND queue IS NULL)
        OR (transport = 'cups' AND queue IS NOT NULL AND ip_address IS NULL AND port IS NULL)
    )
) STRICT;

-- Label print requests. A reprint keeps the container's label code and
-- records actor and reason (REQ-SPC-001).
CREATE TABLE spc_label_jobs (
    id               TEXT    NOT NULL PRIMARY KEY,
    container_id     TEXT    NOT NULL REFERENCES spc_containers (id),
    printer_id       TEXT    NOT NULL REFERENCES spc_printers (id),
    template_id      TEXT    NOT NULL,
    template_version INTEGER NOT NULL CHECK (template_version >= 1),
    kind             TEXT    NOT NULL CHECK (kind IN ('print', 'reprint')),
    reason           TEXT,
    requested_by     TEXT    NOT NULL,
    requested_at     TEXT    NOT NULL,
    CHECK (kind = 'print' OR reason IS NOT NULL)
) STRICT;

CREATE INDEX spc_label_jobs_by_container ON spc_label_jobs (container_id);

-- One outcome per job. `handed_off` means the bytes reached the printer or
-- spooler, not that a label printed.
CREATE TABLE spc_label_job_outcomes (
    job_id       TEXT    NOT NULL PRIMARY KEY REFERENCES spc_label_jobs (id),
    outcome      TEXT    NOT NULL CHECK (outcome IN ('handed_off', 'failed')),
    error_code   TEXT,
    label_sha256 TEXT,
    truncated    INTEGER CHECK (truncated IN (0, 1)),
    recorded_at  TEXT    NOT NULL,
    CHECK ((outcome = 'failed') = (error_code IS NOT NULL)),
    CHECK (outcome = 'failed' OR (label_sha256 IS NOT NULL AND truncated IS NOT NULL))
) STRICT;

-- Append-only evidence.
CREATE TRIGGER spc_accessions_no_update BEFORE UPDATE ON spc_accessions
BEGIN SELECT RAISE(ABORT, 'spc_accessions is append-only'); END;
CREATE TRIGGER spc_accessions_no_delete BEFORE DELETE ON spc_accessions
BEGIN SELECT RAISE(ABORT, 'spc_accessions is append-only'); END;
CREATE TRIGGER spc_accession_items_no_update BEFORE UPDATE ON spc_accession_items
BEGIN SELECT RAISE(ABORT, 'spc_accession_items is append-only'); END;
CREATE TRIGGER spc_accession_items_no_delete BEFORE DELETE ON spc_accession_items
BEGIN SELECT RAISE(ABORT, 'spc_accession_items is append-only'); END;
CREATE TRIGGER spc_collections_no_update BEFORE UPDATE ON spc_collections
BEGIN SELECT RAISE(ABORT, 'spc_collections is append-only'); END;
CREATE TRIGGER spc_collections_no_delete BEFORE DELETE ON spc_collections
BEGIN SELECT RAISE(ABORT, 'spc_collections is append-only'); END;
CREATE TRIGGER spc_collection_observations_no_update BEFORE UPDATE ON spc_collection_observations
BEGIN SELECT RAISE(ABORT, 'spc_collection_observations is append-only'); END;
CREATE TRIGGER spc_collection_observations_no_delete BEFORE DELETE ON spc_collection_observations
BEGIN SELECT RAISE(ABORT, 'spc_collection_observations is append-only'); END;
CREATE TRIGGER spc_specimens_no_delete BEFORE DELETE ON spc_specimens
BEGIN SELECT RAISE(ABORT, 'specimens are never deleted'); END;
CREATE TRIGGER spc_receipt_decisions_no_update BEFORE UPDATE ON spc_receipt_decisions
BEGIN SELECT RAISE(ABORT, 'spc_receipt_decisions is append-only'); END;
CREATE TRIGGER spc_receipt_decisions_no_delete BEFORE DELETE ON spc_receipt_decisions
BEGIN SELECT RAISE(ABORT, 'spc_receipt_decisions is append-only'); END;
CREATE TRIGGER spc_receipt_discrepancies_no_update BEFORE UPDATE ON spc_receipt_discrepancies
BEGIN SELECT RAISE(ABORT, 'spc_receipt_discrepancies is append-only'); END;
CREATE TRIGGER spc_receipt_discrepancies_no_delete BEFORE DELETE ON spc_receipt_discrepancies
BEGIN SELECT RAISE(ABORT, 'spc_receipt_discrepancies is append-only'); END;
CREATE TRIGGER spc_containers_no_delete BEFORE DELETE ON spc_containers
BEGIN SELECT RAISE(ABORT, 'containers are never deleted'); END;
CREATE TRIGGER spc_location_events_no_update BEFORE UPDATE ON spc_location_events
BEGIN SELECT RAISE(ABORT, 'spc_location_events is append-only'); END;
CREATE TRIGGER spc_location_events_no_delete BEFORE DELETE ON spc_location_events
BEGIN SELECT RAISE(ABORT, 'spc_location_events is append-only'); END;
CREATE TRIGGER spc_holds_no_delete BEFORE DELETE ON spc_holds
BEGIN SELECT RAISE(ABORT, 'holds are never deleted'); END;
CREATE TRIGGER spc_excursions_no_delete BEFORE DELETE ON spc_excursions
BEGIN SELECT RAISE(ABORT, 'excursions are never deleted'); END;
CREATE TRIGGER spc_disposals_no_update BEFORE UPDATE ON spc_disposals
BEGIN SELECT RAISE(ABORT, 'spc_disposals is append-only'); END;
CREATE TRIGGER spc_disposals_no_delete BEFORE DELETE ON spc_disposals
BEGIN SELECT RAISE(ABORT, 'spc_disposals is append-only'); END;
CREATE TRIGGER spc_label_jobs_no_update BEFORE UPDATE ON spc_label_jobs
BEGIN SELECT RAISE(ABORT, 'spc_label_jobs is append-only'); END;
CREATE TRIGGER spc_label_jobs_no_delete BEFORE DELETE ON spc_label_jobs
BEGIN SELECT RAISE(ABORT, 'spc_label_jobs is append-only'); END;
CREATE TRIGGER spc_label_job_outcomes_no_update BEFORE UPDATE ON spc_label_job_outcomes
BEGIN SELECT RAISE(ABORT, 'spc_label_job_outcomes is append-only'); END;
CREATE TRIGGER spc_label_job_outcomes_no_delete BEFORE DELETE ON spc_label_job_outcomes
BEGIN SELECT RAISE(ABORT, 'spc_label_job_outcomes is append-only'); END;

-- Containers with their specimen, acceptance, open holds and accession, for
-- other modules (analytical positions, result review). No demographics.
CREATE VIEW spc_v_containers AS
SELECT
    c.id AS container_id,
    c.label_code,
    c.state,
    s.id AS specimen_id,
    s.specimen_type_code,
    s.acceptance,
    (SELECT count(*) FROM spc_holds h WHERE h.container_id = c.id AND h.state = 'open')
        AS open_hold_count,
    a.id AS accession_id,
    a.accession_number,
    a.subject_id
FROM spc_containers c
JOIN spc_specimens s ON s.id = c.specimen_id
JOIN spc_collections col ON col.id = s.collection_id
JOIN spc_accessions a ON a.id = col.accession_id;
