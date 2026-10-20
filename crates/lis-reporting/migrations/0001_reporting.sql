-- Reports, candidates, released revisions, withdrawal, impact, preservation
-- incidents and staff-operated delivery (REQ-RPT-001/002/003/005/011/012/013,
-- NFR-01, NFR-07). Design: docs/planning/m3-design-brief.md §2.1. Released
-- records are append-only; the only updates are the candidate state machine
-- and a destination's single retirement.

CREATE TABLE rpt_reports (
    id            TEXT NOT NULL PRIMARY KEY,
    specimen_id   TEXT NOT NULL UNIQUE REFERENCES spc_specimens (id),
    -- Accession number, '-', specimen type code: PHI-free (L2).
    report_number TEXT NOT NULL UNIQUE,
    created_by    TEXT NOT NULL,
    created_at    TEXT NOT NULL
) STRICT;

CREATE TABLE rpt_candidates (
    id                     TEXT    NOT NULL PRIMARY KEY,
    report_id              TEXT    NOT NULL REFERENCES rpt_reports (id),
    proposed_revision_no   INTEGER NOT NULL CHECK (proposed_revision_no >= 1),
    relationship           TEXT    NOT NULL
        CHECK (relationship IN ('initial', 'supplement', 'correction')),
    supersedes_revision_id TEXT REFERENCES rpt_revisions (id),
    amendment_reason       TEXT,
    completeness           TEXT    NOT NULL CHECK (completeness IN ('partial', 'final')),
    signatory_user_id      TEXT    NOT NULL REFERENCES auth_users (id),
    -- The printed issue time.
    prepared_at            TEXT    NOT NULL,
    -- Canonical report snapshot and its digest.
    content_json           TEXT    NOT NULL,
    content_sha256         TEXT    NOT NULL CHECK (length(content_sha256) = 64),
    -- The exact template input and its digest.
    data_json              TEXT    NOT NULL,
    data_sha256            TEXT    NOT NULL CHECK (length(data_sha256) = 64),
    template_id            TEXT    NOT NULL,
    template_version       INTEGER NOT NULL CHECK (template_version >= 1),
    state                  TEXT    NOT NULL
        CHECK (state IN ('rendering', 'ready', 'released', 'superseded', 'failed')),
    artifact_sha256        TEXT CHECK (artifact_sha256 IS NULL OR length(artifact_sha256) = 64),
    artifact_size          INTEGER CHECK (artifact_size IS NULL OR artifact_size > 0),
    template_sha256        TEXT,
    renderer_version       TEXT,
    fonts_json             TEXT,
    rendered_at            TEXT,
    failure_code           TEXT,
    revision               INTEGER NOT NULL CHECK (revision >= 1),
    CHECK ((relationship = 'initial') = (supersedes_revision_id IS NULL)),
    CHECK (relationship <> 'correction' OR amendment_reason IS NOT NULL),
    CHECK (state NOT IN ('ready', 'released') OR artifact_sha256 IS NOT NULL),
    CHECK ((state = 'failed') = (failure_code IS NOT NULL)),
    CHECK ((artifact_sha256 IS NULL) = (artifact_size IS NULL)
        AND (artifact_sha256 IS NULL) = (template_sha256 IS NULL)
        AND (artifact_sha256 IS NULL) = (renderer_version IS NULL)
        AND (artifact_sha256 IS NULL) = (fonts_json IS NULL)
        AND (artifact_sha256 IS NULL) = (rendered_at IS NULL))
) STRICT;

-- One open candidate per report.
CREATE UNIQUE INDEX rpt_candidates_one_open ON rpt_candidates (report_id)
    WHERE state IN ('rendering', 'ready');

CREATE TABLE rpt_candidate_lines (
    candidate_id       TEXT    NOT NULL REFERENCES rpt_candidates (id),
    line_no            INTEGER NOT NULL CHECK (line_no >= 1),
    kind               TEXT    NOT NULL CHECK (kind IN ('result', 'trigger')),
    order_component_id TEXT REFERENCES ord_order_components (id),
    analyte_id         TEXT REFERENCES cat_analytes (id),
    parent_item_id     TEXT REFERENCES ord_order_items (id),
    orderable_id       TEXT REFERENCES cat_orderables (id),
    disposition        TEXT    NOT NULL CHECK (disposition IN
        ('reported', 'pending', 'held', 'cancelled', 'not_activated', 'trigger_pending')),
    result_id          TEXT REFERENCES res_results (id),
    result_revision_id TEXT REFERENCES res_result_revisions (id),
    review_id          TEXT REFERENCES res_technical_reviews (id),
    reviewed_by        TEXT,
    PRIMARY KEY (candidate_id, line_no),
    CHECK ((kind = 'result') = (order_component_id IS NOT NULL)),
    CHECK ((kind = 'result') = (analyte_id IS NOT NULL)),
    CHECK ((kind = 'trigger') = (parent_item_id IS NOT NULL AND orderable_id IS NOT NULL)),
    CHECK (kind = 'result' OR disposition IN ('not_activated', 'trigger_pending')),
    CHECK ((disposition = 'reported') = (result_revision_id IS NOT NULL AND review_id IS NOT NULL)),
    CHECK ((review_id IS NULL) = (reviewed_by IS NULL)),
    CHECK ((result_id IS NULL) = (result_revision_id IS NULL))
) STRICT;

CREATE TABLE rpt_revisions (
    id                     TEXT    NOT NULL PRIMARY KEY,
    report_id              TEXT    NOT NULL REFERENCES rpt_reports (id),
    revision_no            INTEGER NOT NULL CHECK (revision_no >= 1),
    candidate_id           TEXT    NOT NULL UNIQUE REFERENCES rpt_candidates (id),
    relationship           TEXT    NOT NULL
        CHECK (relationship IN ('initial', 'supplement', 'correction')),
    completeness           TEXT    NOT NULL CHECK (completeness IN ('partial', 'final')),
    supersedes_revision_id TEXT REFERENCES rpt_revisions (id),
    amendment_reason       TEXT,
    content_sha256         TEXT    NOT NULL CHECK (length(content_sha256) = 64),
    artifact_sha256        TEXT    NOT NULL CHECK (length(artifact_sha256) = 64),
    released_by            TEXT    NOT NULL REFERENCES auth_users (id),
    released_at            TEXT    NOT NULL,
    UNIQUE (report_id, revision_no),
    CHECK ((relationship = 'initial') = (supersedes_revision_id IS NULL)),
    CHECK (relationship <> 'correction' OR amendment_reason IS NOT NULL)
) STRICT;

CREATE TABLE rpt_withdrawals (
    report_id               TEXT    NOT NULL PRIMARY KEY REFERENCES rpt_reports (id),
    -- The current revision at withdrawal.
    revision_id             TEXT    NOT NULL REFERENCES rpt_revisions (id),
    reason                  TEXT    NOT NULL,
    notice_artifact_sha256  TEXT    NOT NULL CHECK (length(notice_artifact_sha256) = 64),
    notice_template_version INTEGER NOT NULL CHECK (notice_template_version >= 1),
    withdrawn_by            TEXT    NOT NULL REFERENCES auth_users (id),
    withdrawn_at            TEXT    NOT NULL
) STRICT;

CREATE TABLE rpt_impact_assessments (
    id          TEXT NOT NULL PRIMARY KEY,
    revision_id TEXT NOT NULL REFERENCES rpt_revisions (id),
    decision    TEXT NOT NULL CHECK (decision IN ('no_impact', 'correction_required')),
    reason      TEXT NOT NULL,
    assessed_by TEXT NOT NULL REFERENCES auth_users (id),
    assessed_at TEXT NOT NULL
) STRICT;

CREATE TABLE rpt_impact_assessment_keys (
    assessment_id TEXT NOT NULL REFERENCES rpt_impact_assessments (id),
    reason_key    TEXT NOT NULL,
    PRIMARY KEY (assessment_id, reason_key)
) STRICT;

-- An issued file that could not be read back intact (NFR-07). Exactly one
-- of a revision, a withdrawal notice or a not-yet-released candidate.
CREATE TABLE rpt_preservation_incidents (
    id                   TEXT NOT NULL PRIMARY KEY,
    artifact_sha256      TEXT NOT NULL CHECK (length(artifact_sha256) = 64),
    revision_id          TEXT REFERENCES rpt_revisions (id),
    withdrawal_report_id TEXT REFERENCES rpt_withdrawals (report_id),
    candidate_id         TEXT REFERENCES rpt_candidates (id),
    kind                 TEXT NOT NULL CHECK (kind IN ('missing', 'corrupt')),
    detected_by          TEXT NOT NULL,
    detected_at          TEXT NOT NULL,
    CHECK ((revision_id IS NOT NULL) + (withdrawal_report_id IS NOT NULL)
        + (candidate_id IS NOT NULL) = 1)
) STRICT;

CREATE TABLE rpt_destinations (
    id                 TEXT    NOT NULL PRIMARY KEY,
    client_id          TEXT    NOT NULL REFERENCES idn_client_organizations (id),
    client_location_id TEXT REFERENCES idn_client_locations (id),
    channel            TEXT    NOT NULL
        CHECK (channel IN ('fax', 'email', 'mail', 'courier', 'pickup')),
    address            TEXT,
    label              TEXT    NOT NULL,
    replaces_id        TEXT UNIQUE REFERENCES rpt_destinations (id),
    created_by         TEXT    NOT NULL REFERENCES auth_users (id),
    created_at         TEXT    NOT NULL,
    retired_by         TEXT,
    retired_at         TEXT,
    retire_reason      TEXT,
    revision           INTEGER NOT NULL CHECK (revision >= 1),
    CHECK (channel IN ('courier', 'pickup') OR address IS NOT NULL),
    CHECK (channel <> 'pickup' OR address IS NULL),
    CHECK ((retired_at IS NULL) = (retired_by IS NULL)),
    CHECK ((retired_at IS NULL) = (retire_reason IS NULL))
) STRICT;

CREATE TABLE rpt_destination_verifications (
    destination_id TEXT NOT NULL PRIMARY KEY REFERENCES rpt_destinations (id),
    method         TEXT NOT NULL,
    verified_by    TEXT NOT NULL REFERENCES auth_users (id),
    verified_at    TEXT NOT NULL
) STRICT;

CREATE TABLE rpt_delivery_obligations (
    id                 TEXT NOT NULL PRIMARY KEY,
    revision_id        TEXT NOT NULL REFERENCES rpt_revisions (id),
    destination_id     TEXT NOT NULL REFERENCES rpt_destinations (id),
    kind               TEXT NOT NULL CHECK (kind IN ('report', 'withdrawal_notice')),
    origin             TEXT NOT NULL CHECK (origin IN
        ('release', 'correction_follow_up', 'withdrawal', 'added', 'redirected')),
    origin_reason      TEXT,
    redirected_from_id TEXT REFERENCES rpt_delivery_obligations (id),
    created_by         TEXT NOT NULL,
    created_at         TEXT NOT NULL,
    UNIQUE (revision_id, destination_id, kind),
    CHECK (origin NOT IN ('added', 'redirected') OR origin_reason IS NOT NULL),
    CHECK ((origin = 'redirected') = (redirected_from_id IS NOT NULL)),
    CHECK (kind = 'report' OR origin IN ('withdrawal', 'redirected')),
    CHECK (origin <> 'withdrawal' OR kind = 'withdrawal_notice')
) STRICT;

CREATE TABLE rpt_obligation_closures (
    obligation_id    TEXT NOT NULL PRIMARY KEY REFERENCES rpt_delivery_obligations (id),
    cause            TEXT NOT NULL CHECK (cause IN ('newer_revision', 'redirected', 'report_withdrawn')),
    by_obligation_id TEXT REFERENCES rpt_delivery_obligations (id),
    closed_by        TEXT NOT NULL,
    closed_at        TEXT NOT NULL,
    CHECK ((cause = 'redirected') = (by_obligation_id IS NOT NULL))
) STRICT;

CREATE TABLE rpt_delivery_attempts (
    id            TEXT NOT NULL PRIMARY KEY,
    obligation_id TEXT NOT NULL REFERENCES rpt_delivery_obligations (id),
    evidence      TEXT NOT NULL CHECK (evidence IN ('handed_off', 'failed', 'outcome_unknown')),
    signal_source TEXT NOT NULL CHECK (signal_source IN ('staff_attestation')),
    observed_at   TEXT NOT NULL,
    failure_code  TEXT CHECK (failure_code IS NULL OR failure_code IN
        ('no_answer_or_busy', 'wrong_destination', 'returned', 'refused', 'other')),
    note          TEXT,
    recorded_by   TEXT NOT NULL REFERENCES auth_users (id),
    recorded_at   TEXT NOT NULL,
    CHECK ((evidence = 'failed') = (failure_code IS NOT NULL)),
    CHECK (observed_at <= recorded_at)
) STRICT;

CREATE TABLE rpt_exports (
    id                   TEXT NOT NULL PRIMARY KEY,
    revision_id          TEXT REFERENCES rpt_revisions (id),
    withdrawal_report_id TEXT REFERENCES rpt_withdrawals (report_id),
    purpose              TEXT NOT NULL CHECK (purpose IN ('delivery', 'records', 'internal')),
    obligation_id        TEXT REFERENCES rpt_delivery_obligations (id),
    artifact_sha256      TEXT NOT NULL CHECK (length(artifact_sha256) = 64),
    -- Built by the engine from the report number: PHI-free.
    file_name            TEXT NOT NULL,
    exported_by          TEXT NOT NULL REFERENCES auth_users (id),
    exported_at          TEXT NOT NULL,
    CHECK ((revision_id IS NULL) <> (withdrawal_report_id IS NULL))
) STRICT;

-- Candidate state machine (brief §2.1): content and identity columns never
-- change; render columns go from NULL to a value once, on rendering → ready;
-- state moves only rendering → ready | failed | superseded and
-- ready → released | superseded.
CREATE TRIGGER rpt_candidates_transition BEFORE UPDATE ON rpt_candidates
WHEN NOT (
    NEW.id IS OLD.id AND NEW.report_id IS OLD.report_id
    AND NEW.proposed_revision_no IS OLD.proposed_revision_no
    AND NEW.relationship IS OLD.relationship
    AND NEW.supersedes_revision_id IS OLD.supersedes_revision_id
    AND NEW.amendment_reason IS OLD.amendment_reason
    AND NEW.completeness IS OLD.completeness
    AND NEW.signatory_user_id IS OLD.signatory_user_id
    AND NEW.prepared_at IS OLD.prepared_at
    AND NEW.content_json IS OLD.content_json AND NEW.content_sha256 IS OLD.content_sha256
    AND NEW.data_json IS OLD.data_json AND NEW.data_sha256 IS OLD.data_sha256
    AND NEW.template_id IS OLD.template_id AND NEW.template_version IS OLD.template_version
    AND NEW.revision = OLD.revision + 1
    AND (
        (OLD.state = 'rendering' AND NEW.state = 'ready' AND OLD.artifact_sha256 IS NULL
            AND NEW.failure_code IS NULL)
        OR (OLD.state = 'rendering' AND NEW.state = 'failed'
            AND NEW.artifact_sha256 IS NULL AND NEW.rendered_at IS NULL)
        OR (OLD.state IN ('rendering', 'ready') AND NEW.state = 'superseded'
            AND NEW.artifact_sha256 IS OLD.artifact_sha256 AND NEW.rendered_at IS OLD.rendered_at
            AND NEW.failure_code IS OLD.failure_code)
        OR (OLD.state = 'ready' AND NEW.state = 'released'
            AND NEW.artifact_sha256 IS OLD.artifact_sha256 AND NEW.artifact_size IS OLD.artifact_size
            AND NEW.template_sha256 IS OLD.template_sha256
            AND NEW.renderer_version IS OLD.renderer_version
            AND NEW.fonts_json IS OLD.fonts_json AND NEW.rendered_at IS OLD.rendered_at
            AND NEW.failure_code IS NULL)
    )
)
BEGIN
    SELECT RAISE(ABORT, 'report candidate transition not allowed');
END;

CREATE TRIGGER rpt_candidates_no_delete BEFORE DELETE ON rpt_candidates
BEGIN
    SELECT RAISE(ABORT, 'report candidates are never deleted');
END;

-- Lines are written with their candidate, while it is still rendering.
CREATE TRIGGER rpt_candidate_lines_insert BEFORE INSERT ON rpt_candidate_lines
WHEN NOT EXISTS (
    SELECT 1 FROM rpt_candidates c WHERE c.id = NEW.candidate_id AND c.state = 'rendering'
)
BEGIN
    SELECT RAISE(ABORT, 'candidate lines are written only with their candidate');
END;

-- A revision exists only for a released candidate with the same report,
-- number, digests, signatory and relationship; it follows the latest
-- revision of a report that is not withdrawn; and its succeeded clinical
-- `report.release` event, by the releaser under `release_reports`, is already
-- in the audit chain (catalog lifecycle pattern; NFR-01, REQ-RPT-001).
CREATE TRIGGER rpt_revisions_insert BEFORE INSERT ON rpt_revisions
WHEN NOT EXISTS (
    SELECT 1 FROM rpt_candidates c
    WHERE c.id = NEW.candidate_id AND c.state = 'released' AND c.report_id = NEW.report_id
      AND c.proposed_revision_no = NEW.revision_no
      AND c.content_sha256 = NEW.content_sha256 AND c.artifact_sha256 = NEW.artifact_sha256
      AND c.signatory_user_id = NEW.released_by
      AND c.relationship = NEW.relationship AND c.completeness = NEW.completeness
      AND c.supersedes_revision_id IS NEW.supersedes_revision_id
      AND c.amendment_reason IS NEW.amendment_reason
) OR NEW.supersedes_revision_id IS NOT (
    SELECT r.id FROM rpt_revisions r WHERE r.report_id = NEW.report_id
    ORDER BY r.revision_no DESC LIMIT 1
) OR NEW.revision_no <> 1 + (
    SELECT count(*) FROM rpt_revisions r WHERE r.report_id = NEW.report_id
) OR EXISTS (
    SELECT 1 FROM rpt_withdrawals w WHERE w.report_id = NEW.report_id
) OR NOT EXISTS (
    SELECT 1 FROM audit_events e
    WHERE e.stream = 'clinical' AND e.action = 'report.release' AND e.outcome = 'succeeded'
      AND e.record_type = 'report_revision' AND e.record_id = NEW.id
      AND e.new_revision = NEW.revision_no
      AND e.principal_kind = 'user' AND e.principal_id = NEW.released_by
      AND e.occurred_at = NEW.released_at AND e.authority = 'release_reports'
)
BEGIN
    SELECT RAISE(ABORT, 'a report revision needs its released candidate and release event');
END;

-- A withdrawal names the current revision.
CREATE TRIGGER rpt_withdrawals_insert BEFORE INSERT ON rpt_withdrawals
WHEN NEW.revision_id IS NOT (
    SELECT r.id FROM rpt_revisions r WHERE r.report_id = NEW.report_id
    ORDER BY r.revision_no DESC LIMIT 1
)
BEGIN
    SELECT RAISE(ABORT, 'a withdrawal names the current revision');
END;

-- Verification needs a second person and an active destination (RPT-012).
CREATE TRIGGER rpt_destination_verifications_insert BEFORE INSERT ON rpt_destination_verifications
WHEN NOT EXISTS (
    SELECT 1 FROM rpt_destinations d
    WHERE d.id = NEW.destination_id AND d.retired_at IS NULL AND d.created_by <> NEW.verified_by
)
BEGIN
    SELECT RAISE(ABORT, 'a destination is verified by a second person while active');
END;

-- The only update of a destination: its retirement, once.
CREATE TRIGGER rpt_destinations_retire_only BEFORE UPDATE ON rpt_destinations
WHEN NOT (
    OLD.retired_at IS NULL AND NEW.retired_at IS NOT NULL
    AND NEW.id IS OLD.id AND NEW.client_id IS OLD.client_id
    AND NEW.client_location_id IS OLD.client_location_id AND NEW.channel IS OLD.channel
    AND NEW.address IS OLD.address AND NEW.label IS OLD.label
    AND NEW.replaces_id IS OLD.replaces_id AND NEW.created_by IS OLD.created_by
    AND NEW.created_at IS OLD.created_at AND NEW.revision = OLD.revision + 1
)
BEGIN
    SELECT RAISE(ABORT, 'a destination changes only by retirement, once');
END;

CREATE TRIGGER rpt_destinations_no_delete BEFORE DELETE ON rpt_destinations
BEGIN
    SELECT RAISE(ABORT, 'destinations are never deleted');
END;

CREATE TRIGGER rpt_reports_no_update BEFORE UPDATE ON rpt_reports
BEGIN SELECT RAISE(ABORT, 'rpt_reports is append-only'); END;
CREATE TRIGGER rpt_reports_no_delete BEFORE DELETE ON rpt_reports
BEGIN SELECT RAISE(ABORT, 'rpt_reports is append-only'); END;
CREATE TRIGGER rpt_candidate_lines_no_update BEFORE UPDATE ON rpt_candidate_lines
BEGIN SELECT RAISE(ABORT, 'rpt_candidate_lines is append-only'); END;
CREATE TRIGGER rpt_candidate_lines_no_delete BEFORE DELETE ON rpt_candidate_lines
BEGIN SELECT RAISE(ABORT, 'rpt_candidate_lines is append-only'); END;
CREATE TRIGGER rpt_revisions_no_update BEFORE UPDATE ON rpt_revisions
BEGIN SELECT RAISE(ABORT, 'rpt_revisions is append-only'); END;
CREATE TRIGGER rpt_revisions_no_delete BEFORE DELETE ON rpt_revisions
BEGIN SELECT RAISE(ABORT, 'rpt_revisions is append-only'); END;
CREATE TRIGGER rpt_withdrawals_no_update BEFORE UPDATE ON rpt_withdrawals
BEGIN SELECT RAISE(ABORT, 'rpt_withdrawals is append-only'); END;
CREATE TRIGGER rpt_withdrawals_no_delete BEFORE DELETE ON rpt_withdrawals
BEGIN SELECT RAISE(ABORT, 'rpt_withdrawals is append-only'); END;
CREATE TRIGGER rpt_impact_assessments_no_update BEFORE UPDATE ON rpt_impact_assessments
BEGIN SELECT RAISE(ABORT, 'rpt_impact_assessments is append-only'); END;
CREATE TRIGGER rpt_impact_assessments_no_delete BEFORE DELETE ON rpt_impact_assessments
BEGIN SELECT RAISE(ABORT, 'rpt_impact_assessments is append-only'); END;
CREATE TRIGGER rpt_impact_assessment_keys_no_update BEFORE UPDATE ON rpt_impact_assessment_keys
BEGIN SELECT RAISE(ABORT, 'rpt_impact_assessment_keys is append-only'); END;
CREATE TRIGGER rpt_impact_assessment_keys_no_delete BEFORE DELETE ON rpt_impact_assessment_keys
BEGIN SELECT RAISE(ABORT, 'rpt_impact_assessment_keys is append-only'); END;
CREATE TRIGGER rpt_preservation_incidents_no_update BEFORE UPDATE ON rpt_preservation_incidents
BEGIN SELECT RAISE(ABORT, 'rpt_preservation_incidents is append-only'); END;
CREATE TRIGGER rpt_preservation_incidents_no_delete BEFORE DELETE ON rpt_preservation_incidents
BEGIN SELECT RAISE(ABORT, 'rpt_preservation_incidents is append-only'); END;
CREATE TRIGGER rpt_destination_verifications_no_update BEFORE UPDATE ON rpt_destination_verifications
BEGIN SELECT RAISE(ABORT, 'rpt_destination_verifications is append-only'); END;
CREATE TRIGGER rpt_destination_verifications_no_delete BEFORE DELETE ON rpt_destination_verifications
BEGIN SELECT RAISE(ABORT, 'rpt_destination_verifications is append-only'); END;
CREATE TRIGGER rpt_delivery_obligations_no_update BEFORE UPDATE ON rpt_delivery_obligations
BEGIN SELECT RAISE(ABORT, 'rpt_delivery_obligations is append-only'); END;
CREATE TRIGGER rpt_delivery_obligations_no_delete BEFORE DELETE ON rpt_delivery_obligations
BEGIN SELECT RAISE(ABORT, 'rpt_delivery_obligations is append-only'); END;
CREATE TRIGGER rpt_obligation_closures_no_update BEFORE UPDATE ON rpt_obligation_closures
BEGIN SELECT RAISE(ABORT, 'rpt_obligation_closures is append-only'); END;
CREATE TRIGGER rpt_obligation_closures_no_delete BEFORE DELETE ON rpt_obligation_closures
BEGIN SELECT RAISE(ABORT, 'rpt_obligation_closures is append-only'); END;
CREATE TRIGGER rpt_delivery_attempts_no_update BEFORE UPDATE ON rpt_delivery_attempts
BEGIN SELECT RAISE(ABORT, 'rpt_delivery_attempts is append-only'); END;
CREATE TRIGGER rpt_delivery_attempts_no_delete BEFORE DELETE ON rpt_delivery_attempts
BEGIN SELECT RAISE(ABORT, 'rpt_delivery_attempts is append-only'); END;
CREATE TRIGGER rpt_exports_no_update BEFORE UPDATE ON rpt_exports
BEGIN SELECT RAISE(ABORT, 'rpt_exports is append-only'); END;
CREATE TRIGGER rpt_exports_no_delete BEFORE DELETE ON rpt_exports
BEGIN SELECT RAISE(ABORT, 'rpt_exports is append-only'); END;

-- Every released revision with its computed availability: withdrawn when
-- the report is withdrawn, superseded once a later revision exists,
-- otherwise current.
CREATE VIEW rpt_v_revisions AS
SELECT r.id AS revision_id,
       r.report_id,
       rep.specimen_id,
       rep.report_number,
       r.revision_no,
       r.relationship,
       r.completeness,
       r.released_at,
       CASE
           WHEN EXISTS (SELECT 1 FROM rpt_withdrawals w WHERE w.report_id = r.report_id)
               THEN 'withdrawn'
           WHEN EXISTS (SELECT 1 FROM rpt_revisions later
                        WHERE later.report_id = r.report_id AND later.revision_no > r.revision_no)
               THEN 'superseded'
           ELSE 'current'
       END AS availability
FROM rpt_revisions r
JOIN rpt_reports rep ON rep.id = r.report_id;

-- Per-analyte lines of every released revision, published to billing.
CREATE VIEW rpt_v_released_lines AS
SELECT v.revision_id,
       v.report_id,
       v.specimen_id,
       v.availability,
       v.completeness,
       v.released_at,
       l.order_component_id,
       l.analyte_id,
       l.disposition,
       l.result_revision_id
FROM rpt_v_revisions v
JOIN rpt_revisions r ON r.id = v.revision_id
JOIN rpt_candidate_lines l ON l.candidate_id = r.candidate_id AND l.kind = 'result';
