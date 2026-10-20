//! Immutable patient financial profile commands and public read contract.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Principal;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use crate::billing_model::{
    GetPatientBillingProfileInput, PatientBillingProfile, PatientBillingProfileView,
    SetPatientBillingProfileInput,
};
use crate::billing_validation::{require_financial_access, validate_content};
use crate::error::IdentityError;
use crate::model::{ActorRef, required_text};
use crate::{SubjectId, subject_summary};

/// Records an immutable patient financial version; does not alter clinical identity.
pub fn set_patient_billing_profile(
    ctx: &mut CommandContext<'_>,
    input: SetPatientBillingProfileInput,
) -> Result<PatientBillingProfile, IdentityError> {
    ctx.require(require_financial_access(ctx.principal())?)?;
    let actor = ActorRef::user(ctx.principal())?;
    subject_summary(ctx.tx(), input.subject_id)?;
    let previous = get_patient_billing_profile(ctx.tx(), input.subject_id, None)?
        .map(|profile| profile.version);
    if previous != input.expected_version {
        return Err(IdentityError::RevisionConflict);
    }
    required_text("source", input.source.expose(), 1000)?;
    required_text("reason", input.reason.expose(), 1000)?;
    validate_content(&input.content, ctx.now())?;
    let profile = PatientBillingProfile {
        subject_id: input.subject_id,
        version: previous.unwrap_or(0) + 1,
        content: input.content,
        source: input.source,
        reason: input.reason,
        recorded_by: actor.id,
        recorded_at: ctx.now(),
    };
    let json = serde_json::to_string(&profile.content)
        .map_err(|_| IdentityError::InvalidFinancialRecord)?;
    if json.len() > 262_144 {
        return Err(crate::error::invalid(
            "content",
            "too_large",
            "Financial profile exceeds its size limit.",
        ));
    }
    ctx.tx().execute("INSERT INTO idn_patient_billing_profiles (subject_id, version, content_json, source, reason, recorded_by, recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![profile.subject_id, profile.version, json, profile.source.expose(), profile.reason.expose(), profile.recorded_by, profile.recorded_at])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.set_patient_billing_profile",
            Outcome::Succeeded,
        )
        .record("subject", profile.subject_id)
        .old_revision(previous.unwrap_or(0))
        .new_revision(profile.version)
        .reason(profile.reason.expose().clone()),
    )?;
    Ok(profile)
}

/// Immutable profile lookup for owner-checked engine reads and order snapshot selection.
/// The caller must authorize and audit sensitive UI reads.
pub fn get_patient_billing_profile(
    conn: &Connection,
    subject_id: SubjectId,
    version: Option<i64>,
) -> Result<Option<PatientBillingProfile>, IdentityError> {
    let row = conn.query_row("SELECT version, content_json, source, reason, recorded_by, recorded_at FROM idn_patient_billing_profiles WHERE subject_id = ?1 AND (?2 IS NULL OR version = ?2) ORDER BY version DESC LIMIT 1", params![subject_id, version], |row| {
        Ok((row.get::<_, i64>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?, row.get::<_, String>(3)?, row.get::<_, String>(4)?, row.get(5)?))
    }).optional()?;
    row.map(
        |(version, json, source, reason, recorded_by, recorded_at)| {
            Ok(PatientBillingProfile {
                subject_id,
                version,
                content: serde_json::from_str(&json)
                    .map_err(|_| IdentityError::InvalidFinancialRecord)?,
                source: lis_core::Phi::new(source),
                reason: lis_core::Phi::new(reason),
                recorded_by,
                recorded_at,
            })
        },
    )
    .transpose()
}

/// Authorized financial read model; engine records the sensitive access before return.
pub fn patient_billing_view(
    conn: &Connection,
    principal: &Principal,
    input: GetPatientBillingProfileInput,
) -> Result<PatientBillingProfileView, IdentityError> {
    require_financial_access(principal)?;
    let subject = subject_summary(conn, input.subject_id)?;
    let profile = get_patient_billing_profile(conn, input.subject_id, input.version)?;
    if input.version.is_some() && profile.is_none() {
        return Err(IdentityError::NotFound("patient_billing_profile"));
    }
    Ok(PatientBillingProfileView {
        subject_id: input.subject_id,
        subject,
        profile,
        allowed_actions: vec!["set_patient_billing_profile".to_owned()],
    })
}
