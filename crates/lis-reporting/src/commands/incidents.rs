//! Preservation incidents (NFR-07, REQ-OPS-004): an issued file or stored
//! report record that failed its digest is recorded in its own transaction,
//! once while its incident is open, and quality staff close it with a
//! resolution and a reason. Closure rows are append-only.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::actions;
use crate::authority::user_id;
use crate::error::{ReportingError, required};
use crate::model::{
    IncidentResolution, PreservationIncidentId, PreservationIncidentKind, StoredRecord,
};
use crate::queries::incidents::incident_view;
use crate::repo::quality::{self, ClosureRow, IncidentRow, IncidentSubject};
use crate::stored::ContentIncident;
use crate::views::PreservationIncidentView;

/// Input of `close_preservation_incident`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ClosePreservationIncidentInput {
    /// The open incident.
    pub incident_id: PreservationIncidentId,
    /// The incident revision shown: 1 while open.
    #[serde(default)]
    #[ts(optional, type = "number")]
    pub expected_revision: Option<i64>,
    /// How it was resolved.
    pub resolution: IncidentResolution,
    /// Why, as entered.
    pub reason: String,
}

/// Records that an issued file could not be read back intact (NFR-07), in
/// its own transaction, by a caller holding `capability`.
pub fn record_preservation_incident(
    ctx: &mut CommandContext<'_>,
    capability: Capability,
    subject: IncidentSubject,
    artifact_sha256: &str,
    kind: PreservationIncidentKind,
) -> Result<PreservationIncidentId, ReportingError> {
    record_incident(
        ctx,
        capability,
        subject,
        artifact_sha256,
        kind,
        StoredRecord::Pdf,
    )
}

/// Records that stored report content or data failed its digest (NFR-07),
/// in its own transaction, by a caller holding `capability`.
pub fn record_content_incident(
    ctx: &mut CommandContext<'_>,
    capability: Capability,
    incident: &ContentIncident,
) -> Result<PreservationIncidentId, ReportingError> {
    record_incident(
        ctx,
        capability,
        incident.subject,
        &incident.sha256,
        PreservationIncidentKind::Corrupt,
        incident.record,
    )
}

/// Records a detection, or returns the incident that already covers it
/// ([`quality::known_incident`]) without a second row or event.
fn record_incident(
    ctx: &mut CommandContext<'_>,
    capability: Capability,
    subject: IncidentSubject,
    sha256: &str,
    kind: PreservationIncidentKind,
    record: StoredRecord,
) -> Result<PreservationIncidentId, ReportingError> {
    ctx.require(capability)?;
    if let Some(known) = quality::known_incident(ctx.tx(), subject, record, sha256)? {
        return Ok(known);
    }
    let incident = IncidentRow {
        id: PreservationIncidentId::generate(),
        artifact_sha256: sha256.to_owned(),
        subject,
        kind,
        record,
        detected_by: ctx.principal().actor().id_text(),
        detected_at: ctx.now(),
    };
    quality::insert_incident(ctx.tx(), &incident)?;
    let (subject_key, subject_id) = subject.audit_detail();
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::PRESERVATION_INCIDENT,
            Outcome::Succeeded,
        )
        .record("preservation_incident", incident.id)
        .detail(subject_key, subject_id)
        .detail("kind", AuditDetail::code(kind.as_str()))
        .detail("stored_record", AuditDetail::code(record.as_str()))
        .detail("artifact_sha256", AuditDetail::sha256(sha256)?),
    )?;
    Ok(incident.id)
}

/// Closes an open incident with a resolution and a required reason
/// (`manage_quality`). The incident and its closure stay readable.
pub fn close_preservation_incident(
    ctx: &mut CommandContext<'_>,
    input: ClosePreservationIncidentInput,
) -> Result<PreservationIncidentView, ReportingError> {
    ctx.require(Capability::ManageQuality)?;
    let closed_by = user_id(ctx)?;
    let reason = required("reason", &input.reason, 1000)?;
    let stored = quality::incident(ctx.tx(), input.incident_id)?
        .ok_or(ReportingError::NotFound("preservation incident"))?;
    if input
        .expected_revision
        .is_some_and(|expected| expected != stored.revision())
    {
        return Err(ReportingError::RevisionConflict);
    }
    if stored.closure.is_some() {
        return Err(ReportingError::WrongState("preservation incident"));
    }
    let closure = ClosureRow {
        incident_id: input.incident_id,
        resolution: input.resolution,
        reason: reason.clone(),
        closed_by,
        closed_at: ctx.now(),
    };
    quality::insert_closure(ctx.tx(), &closure)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            actions::CLOSE_PRESERVATION_INCIDENT,
            Outcome::Succeeded,
        )
        .record("preservation_incident", input.incident_id)
        .old_revision(1)
        .new_revision(2)
        .detail("resolution", AuditDetail::code(closure.resolution.as_str()))
        .reason(reason),
    )?;
    let closed = quality::incident(ctx.tx(), input.incident_id)?
        .ok_or(ReportingError::Inconsistent("closed incident"))?;
    incident_view(ctx.tx(), closed, ctx.principal())
}
