-- Delivery queue/detail lookup support and durable reconciliation of a
-- withdrawal notice stored between preflight and the final withdrawal.
CREATE INDEX rpt_delivery_attempts_by_obligation
    ON rpt_delivery_attempts (obligation_id);
CREATE INDEX rpt_delivery_obligations_by_destination
    ON rpt_delivery_obligations (destination_id);

CREATE TABLE rpt_withdrawal_notice_artifacts (
    artifact_sha256 TEXT NOT NULL PRIMARY KEY
        CHECK (length(artifact_sha256) = 64 AND artifact_sha256 NOT GLOB '*[^0-9a-f]*'),
    report_id        TEXT NOT NULL REFERENCES rpt_reports (id),
    revision_no      INTEGER NOT NULL CHECK (revision_no >= 1),
    template_version INTEGER NOT NULL CHECK (template_version >= 1),
    staged_by        TEXT NOT NULL REFERENCES auth_users (id),
    staged_at        TEXT NOT NULL
) STRICT;

CREATE TRIGGER rpt_withdrawal_notice_artifacts_no_update
BEFORE UPDATE ON rpt_withdrawal_notice_artifacts
BEGIN SELECT RAISE(ABORT, 'rpt_withdrawal_notice_artifacts is append-only'); END;
CREATE TRIGGER rpt_withdrawal_notice_artifacts_no_delete
BEFORE DELETE ON rpt_withdrawal_notice_artifacts
BEGIN SELECT RAISE(ABORT, 'rpt_withdrawal_notice_artifacts is append-only'); END;
