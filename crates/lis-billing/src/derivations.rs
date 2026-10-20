//! Derivation persistence and approval (REQ-BIL-001, REQ-BIL-005, brief
//! §2.2, §5.2). Every command rebuilds the inputs inside its transaction.
//!
//! - **Idempotent.** When the newest derivation of the subject and date of
//!   service has the same inputs digest, it is returned: no row, no event.
//! - **Never edited.** A new digest gives a new derivation number. Earlier
//!   derivations not handed off become `superseded`; the new one is an
//!   adjustment of the newest derivation that was actually handed off.
//! - **States.** `held` while a blocking flag exists, `candidate` otherwise;
//!   `approved` only from `candidate` with a current digest; `handed_off`
//!   through a handoff. Staleness is computed, never stored.

use jiff::civil::Date;
use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, GateReason};
use lis_identity::SubjectId;
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::derivation::{DerivationOutcome, derive};
use crate::derivation_repo::{self as repo, DerivationRow, NewDerivation};
use crate::error::{BillingError, invalid};
use crate::inputs::gather;
use crate::mapping_support::acting_user;
use crate::model::{BillingDerivationId, BillingDerivationState};

pub(crate) const RECORD_TYPE: &str = "billing_derivation";

/// Input of `derive_services`: one subject and date of service; `null` is
/// the group of specimens whose collection date is unknown.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DeriveServices {
    pub subject_id: SubjectId,
    pub date_of_service: Option<String>,
}

/// Input of `approve_service_derivation`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApproveServiceDerivation {
    pub derivation_id: BillingDerivationId,
    #[ts(type = "number")]
    pub expected_revision: i64,
}

/// Derives the subject's performed services for one date of service, or
/// returns the newest derivation when its inputs are unchanged.
pub fn derive_services(
    ctx: &mut CommandContext<'_>,
    input: DeriveServices,
) -> Result<BillingDerivationId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let date_of_service = parse_date_of_service(input.date_of_service)?;
    let dos = date_of_service.as_deref();
    let inputs = gather(ctx.tx(), input.subject_id, dos)?;
    let latest = repo::latest(ctx.tx(), input.subject_id, dos)?;
    if inputs.specimens.is_empty()
        && repo::latest_handed_off(ctx.tx(), input.subject_id, dos)?.is_none()
    {
        let prior = latest.ok_or(BillingError::NotFound("specimen on that date of service"))?;
        supersede_open(ctx, input.subject_id, dos)?;
        return Ok(prior.id);
    }
    let outcome = derive(&inputs);
    if let Some(latest) = &latest
        && latest.inputs_sha256 == outcome.inputs_sha256
    {
        return Ok(latest.id);
    }
    supersede_open(ctx, input.subject_id, dos)?;
    let new = NewDerivation {
        id: BillingDerivationId::generate(),
        derivation_no: latest.map_or(1, |latest| latest.derivation_no + 1),
        adjustment_of_id: repo::latest_handed_off(ctx.tx(), input.subject_id, dos)?,
        derived_by: actor,
        derived_at: ctx.now(),
    };
    repo::insert(ctx.tx(), &new, &outcome)?;
    ctx.audit(derive_event(&new, &outcome)?)?;
    Ok(new.id)
}

/// Approves a `candidate` whose inputs are still current.
pub fn approve_service_derivation(
    ctx: &mut CommandContext<'_>,
    input: ApproveServiceDerivation,
) -> Result<BillingDerivationId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let row = repo::load(ctx.tx(), input.derivation_id)?;
    if !row.facts_sealed {
        return Err(BillingError::Inconsistent("unsealed billing derivation"));
    }
    if row.revision != input.expected_revision {
        return Err(BillingError::RevisionConflict);
    }
    match row.state {
        BillingDerivationState::Candidate => {}
        BillingDerivationState::Held => {
            return Err(BillingError::DerivationHeld(held_reasons(
                ctx.tx(),
                row.id,
            )?));
        }
        BillingDerivationState::Approved
        | BillingDerivationState::HandedOff
        | BillingDerivationState::Superseded => return Err(BillingError::InvalidDerivationState),
    }
    if is_stale(ctx.tx(), &row)? {
        return Err(BillingError::DerivationStale);
    }
    let next_revision = row.revision + 1;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.approve_service_derivation",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, row.id)
        .old_revision(row.revision)
        .new_revision(next_revision)
        .detail("derivation_no", AuditDetail::integer(row.derivation_no))
        .detail("inputs_sha256", AuditDetail::sha256(&row.inputs_sha256)?),
    )?;
    repo::approve(ctx.tx(), row.id, next_revision, actor, ctx.now())?;
    Ok(row.id)
}

/// Whether the inputs rebuilt now differ from those the derivation froze.
pub(crate) fn is_stale(conn: &Connection, row: &DerivationRow) -> Result<bool, BillingError> {
    Ok(current_outcome(conn, row)?.inputs_sha256 != row.inputs_sha256)
}

fn current_outcome(
    conn: &Connection,
    row: &DerivationRow,
) -> Result<DerivationOutcome, BillingError> {
    Ok(derive(&gather(
        conn,
        row.subject_id,
        row.date_of_service.as_deref(),
    )?))
}

/// Supersedes every derivation of the group not yet handed off.
fn supersede_open(
    ctx: &CommandContext<'_>,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<(), BillingError> {
    for (id, revision) in repo::open_in_group(ctx.tx(), subject_id, date_of_service)? {
        ctx.audit(
            AuditEvent::new(
                Stream::Clinical,
                "billing.supersede_derivation",
                Outcome::Succeeded,
            )
            .record(RECORD_TYPE, id)
            .old_revision(revision)
            .new_revision(revision + 1),
        )?;
        repo::set_state(
            ctx.tx(),
            id,
            BillingDerivationState::Superseded,
            revision + 1,
        )?;
    }
    Ok(())
}

fn derive_event(
    new: &NewDerivation,
    outcome: &DerivationOutcome,
) -> Result<AuditEvent, BillingError> {
    let blocking = outcome.facts.flags.iter().filter(|f| f.blocking).count();
    let mut event = AuditEvent::new(Stream::Clinical, "billing.derive", Outcome::Succeeded)
        .record(RECORD_TYPE, new.id)
        .new_revision(1)
        .detail("derivation_no", AuditDetail::integer(new.derivation_no))
        .detail(
            "inputs_sha256",
            AuditDetail::sha256(&outcome.inputs_sha256)?,
        )
        .detail("state", AuditDetail::code(outcome.facts.state.as_str()))
        .detail(
            "specimen_count",
            AuditDetail::count(outcome.facts.specimens.len()),
        )
        .detail("blocking_flag_count", AuditDetail::count(blocking));
    if let Some(original) = new.adjustment_of_id {
        event = event.detail("adjustment_of", AuditDetail::id(original));
    }
    Ok(event)
}

/// The blocking flags of a held derivation as gate reasons.
fn held_reasons(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<Vec<GateReason>, BillingError> {
    Ok(repo::flags(conn, id)?
        .into_iter()
        .filter(|flag| flag.blocking)
        .map(|flag| GateReason {
            message: blocking_message(&flag.code).to_owned(),
            code: flag.code,
        })
        .collect())
}

fn blocking_message(code: &str) -> &'static str {
    match code {
        "mapping_missing" => "An analyte has no confirmed billing class.",
        "date_of_service_unknown" => "The collection date is unknown.",
        "report_not_final" => "A specimen has no current final report.",
        "laboratory_billing_identity_missing" => {
            "The laboratory profile has no billing NPI or TIN."
        }
        _ => "A blocking billing flag is open.",
    }
}

/// A `YYYY-MM-DD` date, exactly as the work list shows it.
fn parse_date_of_service(text: Option<String>) -> Result<Option<String>, BillingError> {
    let Some(text) = text else {
        return Ok(None);
    };
    match text.parse::<Date>() {
        Ok(date) if date.to_string() == text => Ok(Some(text)),
        _ => Err(invalid(FieldError {
            field: "dateOfService".to_owned(),
            code: "invalid_date".to_owned(),
            message: "Enter the date of service as YYYY-MM-DD.".to_owned(),
        })),
    }
}
