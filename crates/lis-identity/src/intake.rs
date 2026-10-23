//! Retained, incomplete identity intake with explicit source-backed disposition (DM-06).
//! These records cannot be used to order or test. No date is inferred from age or partial text.
use crate::error::{ensure_revision, invalid};
use crate::model::{ActorRef, optional_text, required_text};
use crate::{IdentityError, SubjectId};
use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, Phi, UtcTimestamp};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub enum IntakeDeferral {}
pub type IntakeDeferralId = Id<IntakeDeferral>;
lis_core::text_enum!(/// The exact kind of incomplete evidence supplied.
    BirthEvidenceKind { Unknown = "unknown", PartialDate = "partial_date", SuppliedAge = "supplied_age", });
lis_core::text_enum!(/// Terminal disposition of an incomplete intake.
    IntakeDisposition { Declined = "declined", Resolved = "resolved", });
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordIntakeDeferralInput {
    pub supplied_name: Phi<String>,
    pub birth_evidence_kind: BirthEvidenceKind,
    pub birth_evidence_text: Option<Phi<String>>,
    pub source_reference: String,
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveIntakeDeferralInput {
    pub intake_id: IntakeDeferralId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub disposition: IntakeDisposition,
    pub subject_id: Option<SubjectId>,
    pub source_reference: String,
    pub reason: String,
}
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListIntakeDeferralsInput {
    #[serde(default)]
    pub include_closed: bool,
    #[serde(default)]
    pub after_id: Option<IntakeDeferralId>,
}
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IntakeDeferralView {
    pub intake_id: IntakeDeferralId,
    #[ts(type = "number")]
    pub revision: i64,
    pub supplied_name: Phi<String>,
    pub birth_evidence_kind: BirthEvidenceKind,
    pub birth_evidence_text: Option<Phi<String>>,
    pub source_reference: String,
    pub reason: String,
    pub recorded_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub recorded_by_display: Option<String>,
    pub recorded_at: UtcTimestamp,
    pub disposition: Option<IntakeDisposition>,
    pub subject_id: Option<SubjectId>,
    pub disposition_source: Option<String>,
    pub disposition_reason: Option<String>,
    pub disposed_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub disposed_by_display: Option<String>,
    pub disposed_at: Option<UtcTimestamp>,
}
const COLUMNS: &str = "SELECT d.id, d.supplied_name, d.birth_evidence_kind, d.birth_evidence_text, d.source_reference, d.reason, d.recorded_by, d.recorded_at, x.disposition, x.subject_id, x.source_reference, x.reason, x.recorded_by, x.recorded_at FROM idn_intake_deferrals d LEFT JOIN idn_intake_dispositions x ON x.id=d.id";
fn row(r: &rusqlite::Row<'_>) -> rusqlite::Result<IntakeDeferralView> {
    let disposition: Option<IntakeDisposition> = r.get(8)?;
    Ok(IntakeDeferralView {
        intake_id: r.get(0)?,
        revision: if disposition.is_some() { 2 } else { 1 },
        supplied_name: Phi::new(r.get(1)?),
        birth_evidence_kind: r.get(2)?,
        birth_evidence_text: r.get::<_, Option<String>>(3)?.map(Phi::new),
        source_reference: r.get(4)?,
        reason: r.get(5)?,
        recorded_by: r.get(6)?,
        recorded_by_display: None,
        recorded_at: r.get(7)?,
        disposition,
        subject_id: r.get(9)?,
        disposition_source: r.get(10)?,
        disposition_reason: r.get(11)?,
        disposed_by: r.get(12)?,
        disposed_by_display: None,
        disposed_at: r.get(13)?,
    })
}
fn get(conn: &Connection, id: IntakeDeferralId) -> Result<IntakeDeferralView, IdentityError> {
    conn.query_row(&format!("{COLUMNS} WHERE d.id=?1"), [id], row)
        .optional()?
        .ok_or(IdentityError::NotFound("intake deferral"))
}
pub fn list_intake_deferrals(
    conn: &Connection,
    input: ListIntakeDeferralsInput,
) -> Result<Vec<IntakeDeferralView>, IdentityError> {
    Ok(conn.prepare(&format!("{COLUMNS} WHERE (?1 OR x.id IS NULL) AND (?2 IS NULL OR d.id > ?2) ORDER BY d.id LIMIT 50"))?.query_map(params![input.include_closed,input.after_id],row)?.collect::<Result<_,_>>()?)
}
pub fn record_intake_deferral(
    ctx: &mut CommandContext<'_>,
    input: RecordIntakeDeferralInput,
) -> Result<IntakeDeferralView, IdentityError> {
    ctx.require(Capability::EnterOrders)?;
    let actor = ActorRef::user(ctx.principal())?.id;
    let name = required_text("suppliedName", input.supplied_name.expose(), 400)?;
    let evidence = optional_text(
        "birthEvidenceText",
        input
            .birth_evidence_text
            .as_ref()
            .map(|v| v.expose().as_str()),
        200,
    )?;
    if input.birth_evidence_kind != BirthEvidenceKind::Unknown && evidence.is_none() {
        return Err(invalid(
            "birthEvidenceText",
            "required",
            "Preserve the supplied partial date or age as text.",
        ));
    }
    let source = required_text("sourceReference", &input.source_reference, 1000)?;
    let reason = required_text("reason", &input.reason, 1000)?;
    let id = IntakeDeferralId::generate();
    ctx.tx().execute(
        "INSERT INTO idn_intake_deferrals VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            id,
            name,
            input.birth_evidence_kind.as_str(),
            evidence,
            source,
            reason,
            actor,
            ctx.now()
        ],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.record_intake_deferral",
            Outcome::Succeeded,
        )
        .record("intake_deferral", id)
        .new_revision(1),
    )?;
    get(ctx.tx(), id)
}
pub fn resolve_intake_deferral(
    ctx: &mut CommandContext<'_>,
    input: ResolveIntakeDeferralInput,
) -> Result<IntakeDeferralView, IdentityError> {
    ctx.require(Capability::EnterOrders)?;
    let actor = ActorRef::user(ctx.principal())?.id;
    let current = get(ctx.tx(), input.intake_id)?;
    ensure_revision(current.revision, input.expected_revision)?;
    if current.disposition.is_some() {
        return Err(IdentityError::RevisionConflict);
    }
    let source = required_text("sourceReference", &input.source_reference, 1000)?;
    let reason = required_text("reason", &input.reason, 1000)?;
    match (input.disposition, input.subject_id) {
        (IntakeDisposition::Resolved, Some(subject)) => {
            crate::get_subject(ctx.tx(), subject)?;
        }
        (IntakeDisposition::Declined, None) => {}
        _ => {
            return Err(invalid(
                "subjectId",
                "disposition_mismatch",
                "Choose a verified patient only when resolving intake; declined intake has no patient link.",
            ));
        }
    }
    ctx.tx().execute(
        "INSERT INTO idn_intake_dispositions VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            input.intake_id,
            input.disposition.as_str(),
            input.subject_id,
            source,
            reason,
            actor,
            ctx.now()
        ],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.resolve_intake_deferral",
            Outcome::Succeeded,
        )
        .record("intake_deferral", input.intake_id)
        .old_revision(1)
        .new_revision(2),
    )?;
    get(ctx.tx(), input.intake_id)
}
