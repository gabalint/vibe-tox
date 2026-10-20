-- A destination's creator may verify it where the laboratory's separation-of-duties
-- policy allows (ADR 0015; REQ-RPT-012, L10). The policy is read through
-- auth_v_separation_policy, which has no row while the default applies, so a second
-- person stays required until the laboratory sets a version that allows it. The
-- command also asks for the creator's confirmation and marks its audit event as a
-- self-approval; this guard keeps a direct insert from bypassing the policy. It
-- replaces the trigger of 0001, which always required a second person.

DROP TRIGGER rpt_destination_verifications_insert;

CREATE TRIGGER rpt_destination_verifications_insert BEFORE INSERT ON rpt_destination_verifications
WHEN NOT EXISTS (
    SELECT 1 FROM rpt_destinations d
    WHERE d.id = NEW.destination_id AND d.retired_at IS NULL
      AND (d.created_by <> NEW.verified_by
           OR coalesce((SELECT self_verify_destinations FROM auth_v_separation_policy), 0) = 1)
)
BEGIN
    SELECT RAISE(ABORT, 'a destination is verified while active, by a second person unless the separation-of-duties policy allows its creator');
END;
