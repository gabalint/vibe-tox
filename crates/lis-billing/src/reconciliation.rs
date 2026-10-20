//! Attributable financial reconciliation, without changing performed work (REQ-BIL-002).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Id, UserId, UtcTimestamp, optional_text, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::derivation_repo as repo;
use crate::mapping_support::acting_user;
use crate::{
    BillingDerivationId, BillingError, BillingHandoffId, BillingHandoffItemId, DeriveServices,
    HandoffOutcome,
};

/// A new response to a previously rejected item; earlier responses remain evidence.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordHandoffFollowupInput {
    pub item_id: BillingHandoffItemId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub outcome: HandoffOutcome,
    #[serde(default)]
    #[ts(optional)]
    pub external_reference: Option<String>,
    pub reason: String,
}

/// Records the billing owner's response after administrative rework or retransmission.
pub fn record_handoff_followup(
    ctx: &mut CommandContext<'_>,
    input: RecordHandoffFollowupInput,
) -> Result<BillingHandoffId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let reason = required_text("reason", &input.reason, 1000).map_err(BillingError::Invalid)?;
    let reference = optional_text(
        "externalReference",
        input.external_reference.as_deref(),
        200,
    )
    .map_err(BillingError::Invalid)?;
    let (handoff, revision, outcome): (BillingHandoffId, i64, HandoffOutcome) = ctx
        .tx()
        .query_row(
            "SELECT i.handoff_id, o.revision, o.outcome FROM bil_handoff_items i \
         JOIN bil_v_latest_handoff_outcomes o ON o.item_id = i.id WHERE i.id = ?1",
            [input.item_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(BillingError::NotFound("rejected handoff item"))?;
    if revision != input.expected_revision {
        return Err(BillingError::RevisionConflict);
    }
    if outcome != HandoffOutcome::Rejected {
        return Err(BillingError::OutcomeRecorded);
    }
    ctx.tx().execute("INSERT INTO bil_handoff_followups (item_id, revision, outcome, external_reference, reason, recorded_by, recorded_at) \
        VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)", params![input.item_id, revision + 1, input.outcome, reference, reason, actor, ctx.now()])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.record_handoff_followup",
            Outcome::Succeeded,
        )
        .record("billing_handoff_item", input.item_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail("handoff_id", AuditDetail::id(handoff))
        .detail("outcome", AuditDetail::code(input.outcome.as_str()))
        .reason(reason),
    )?;
    Ok(handoff)
}

/// Confirms that differing same-day billing arrangements have been reconciled with the billing owner.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ResolveBillingContextConflictInput {
    pub derivation_id: BillingDerivationId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
}

/// A retained explanation of same-day billing allocation; never a new performed service.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct BillingContextResolution {
    pub reason: String,
    pub recorded_by: UserId,
    pub recorded_at: UtcTimestamp,
}

/// Binds reconciliation to the exact current clinical and financial inputs and rederives.
pub fn resolve_billing_context_conflict(
    ctx: &mut CommandContext<'_>,
    input: ResolveBillingContextConflictInput,
) -> Result<BillingDerivationId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let reason = required_text("reason", &input.reason, 1000).map_err(BillingError::Invalid)?;
    let row = repo::load(ctx.tx(), input.derivation_id)?;
    if row.revision != input.expected_revision {
        return Err(BillingError::RevisionConflict);
    }
    if crate::derivations::is_stale(ctx.tx(), &row)? {
        return Err(BillingError::DerivationStale);
    }
    if !repo::flags(ctx.tx(), row.id)?
        .iter()
        .any(|flag| flag.code == "billing_context_conflict" && flag.blocking)
    {
        return Err(BillingError::InvalidDerivationState);
    }
    let id = Id::<Resolution>::generate();
    ctx.tx().execute("INSERT INTO bil_context_resolutions (id, derivation_id, inputs_sha256, reason, recorded_by, recorded_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![id, row.id, row.inputs_sha256, reason, actor, ctx.now()])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.resolve_context_conflict",
            Outcome::Succeeded,
        )
        .record("billing_derivation", row.id)
        .detail("resolution_id", AuditDetail::id(id))
        .reason(reason),
    )?;
    crate::derive_services(
        ctx,
        DeriveServices {
            subject_id: row.subject_id,
            date_of_service: row.date_of_service,
        },
    )
}

enum Resolution {}

pub(crate) fn resolution_for(
    conn: &Connection,
    derivation: BillingDerivationId,
) -> Result<Option<BillingContextResolution>, BillingError> {
    Ok(conn.query_row("SELECT r.reason, r.recorded_by, r.recorded_at FROM bil_context_resolutions r \
        JOIN bil_derivation_inputs i ON i.kind = 'conflict_resolution' AND i.ref_id = r.id WHERE i.derivation_id = ?1",
        [derivation], |row| Ok(BillingContextResolution { reason: row.get(0)?, recorded_by: row.get(1)?, recorded_at: row.get(2)? })).optional()?)
}
