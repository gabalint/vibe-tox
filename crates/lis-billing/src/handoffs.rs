//! Billing handoffs (REQ-BIL-002, brief §5.3). The engine plans a handoff,
//! builds its file on a read connection, stores the file as an artifact,
//! and then [`create_billing_handoff`] rebuilds the file inside its own
//! transaction: every derivation must still be approved, current and not
//! handed off, and the rebuilt bytes must match the stored digest. Outcomes
//! retain their initial response and append-only follow-ups to rejections.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, UtcTimestamp, optional_text};
use lis_identity::SubjectId;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use sha2::{Digest, Sha256};
use ts_rs::TS;

mod export;
pub use export::{HandoffExportSource, handoff_export_source, record_handoff_export};

use crate::derivation_repo::{self as repo, DerivationRow};
use crate::derivation_view::specimen_views;
use crate::derivations::{RECORD_TYPE, is_stale};
use crate::error::{BillingError, invalid};
use crate::handoff_document::{
    HANDOFF_FORMAT, HandoffDocument, ItemPart, laboratory_part, specimen_part, subject_part,
    to_bytes,
};
use crate::mapping_support::{acting_user, load_summary};
use crate::model::{
    BillingDerivationId, BillingDerivationState, BillingHandoffId, BillingHandoffItemId,
    HandoffItemKind, HandoffOutcome,
};

const MAX_ITEMS: usize = 200;
const REFERENCE_MAX: usize = 200;
const REASON_MAX: usize = 1000;

/// Input of `create_billing_handoff`: approved derivations to hand off together.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct CreateBillingHandoff {
    pub derivation_ids: Vec<BillingDerivationId>,
}

/// Input of `record_handoff_outcome`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RecordHandoffOutcome {
    pub item_id: BillingHandoffItemId,
    pub outcome: HandoffOutcome,
    /// The billing owner's reference, as given.
    #[serde(default)]
    #[ts(optional)]
    pub external_reference: Option<String>,
    /// Required for a rejection.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// A handoff's identifiers, fixed before its file is built.
#[derive(Debug, Clone)]
pub struct HandoffPlan {
    /// The handoff.
    pub handoff_id: BillingHandoffId,
    /// When the file was built.
    pub generated_at: UtcTimestamp,
    items: Vec<(BillingDerivationId, BillingHandoffItemId)>,
}

/// Plans a handoff of 1 to 200 distinct derivations.
pub fn plan_handoff(
    input: CreateBillingHandoff,
    generated_at: UtcTimestamp,
) -> Result<HandoffPlan, BillingError> {
    let distinct: BTreeSet<_> = input.derivation_ids.iter().collect();
    if input.derivation_ids.is_empty()
        || input.derivation_ids.len() > MAX_ITEMS
        || distinct.len() != input.derivation_ids.len()
    {
        return Err(invalid(FieldError {
            field: "derivationIds".to_owned(),
            code: "out_of_range".to_owned(),
            message: "Choose between 1 and 200 different derivations.".to_owned(),
        }));
    }
    Ok(HandoffPlan {
        handoff_id: BillingHandoffId::generate(),
        generated_at,
        items: input
            .derivation_ids
            .into_iter()
            .map(|id| (id, BillingHandoffItemId::generate()))
            .collect(),
    })
}

/// The `bil-handoff-v2` bytes of `plan`, after checking that every
/// derivation is approved, current and not handed off. Usable inside a
/// transaction.
pub fn handoff_file(conn: &Connection, plan: &HandoffPlan) -> Result<Vec<u8>, BillingError> {
    let profile = lis_identity::current_laboratory_profile(conn)?
        .ok_or(BillingError::LaboratoryBillingIdentityMissing)?;
    let mut items = Vec::new();
    for (derivation_id, item_id) in &plan.items {
        let row = repo::load(conn, *derivation_id)?;
        ensure_handoff_ready(conn, &row)?;
        items.push(item_part(conn, &row, *item_id)?);
    }
    to_bytes(&HandoffDocument {
        format: HANDOFF_FORMAT,
        handoff_id: plan.handoff_id,
        generated_at: plan.generated_at,
        laboratory: laboratory_part(&profile)?,
        items,
    })
}

/// Records the handoff whose file is stored as `artifact_sha256`, and hands
/// off its derivations.
pub fn create_billing_handoff(
    ctx: &mut CommandContext<'_>,
    plan: &HandoffPlan,
    artifact_sha256: &str,
) -> Result<BillingHandoffId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let rebuilt = handoff_file(ctx.tx(), plan)?;
    if hex::encode(Sha256::digest(&rebuilt)) != artifact_sha256 {
        return Err(BillingError::HandoffChanged);
    }
    ctx.tx().execute(
        "INSERT INTO bil_handoffs \
         (id, format, artifact_sha256, item_count, created_by, created_at, handoff_no) \
         SELECT ?1, ?2, ?3, ?4, ?5, ?6, COALESCE(max(handoff_no), 0) + 1 FROM bil_handoffs",
        params![
            plan.handoff_id,
            HANDOFF_FORMAT,
            artifact_sha256,
            i64::try_from(plan.items.len()).map_err(|_| BillingError::Inconsistent("items"))?,
            actor,
            ctx.now()
        ],
    )?;
    for (derivation_id, item_id) in &plan.items {
        hand_off(ctx, plan.handoff_id, *derivation_id, *item_id)?;
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.create_handoff",
            Outcome::Succeeded,
        )
        .record("billing_handoff", plan.handoff_id)
        .detail(
            "handoff_no",
            AuditDetail::integer(handoff_number(ctx.tx(), plan.handoff_id)?),
        )
        .detail("item_count", AuditDetail::count(plan.items.len()))
        .detail("artifact_sha256", AuditDetail::sha256(artifact_sha256)?),
    )?;
    Ok(plan.handoff_id)
}

fn handoff_number(conn: &Connection, handoff_id: BillingHandoffId) -> Result<i64, BillingError> {
    Ok(conn.query_row(
        "SELECT handoff_no FROM bil_handoffs WHERE id = ?1",
        [handoff_id],
        |row| row.get(0),
    )?)
}

fn hand_off(
    ctx: &CommandContext<'_>,
    handoff_id: BillingHandoffId,
    derivation_id: BillingDerivationId,
    item_id: BillingHandoffItemId,
) -> Result<(), BillingError> {
    let row = repo::load(ctx.tx(), derivation_id)?;
    let adjusts = latest_handed_off_item(ctx.tx(), row.subject_id, row.date_of_service.as_deref())?;
    ctx.tx().execute(
        "INSERT INTO bil_handoff_items (id, handoff_id, derivation_id, kind, adjusts_item_id) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            item_id,
            handoff_id,
            derivation_id,
            item_kind(adjusts),
            adjusts
        ],
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.create_handoff",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, derivation_id)
        .old_revision(row.revision)
        .new_revision(row.revision + 1)
        .detail("handoff_id", AuditDetail::id(handoff_id))
        .detail("item_id", AuditDetail::id(item_id)),
    )?;
    repo::set_state(
        ctx.tx(),
        derivation_id,
        BillingDerivationState::HandedOff,
        row.revision + 1,
    )
}

/// Records the billing owner's outcome for one item, once.
pub fn record_handoff_outcome(
    ctx: &mut CommandContext<'_>,
    input: RecordHandoffOutcome,
) -> Result<BillingHandoffId, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let reference = optional_text(
        "externalReference",
        input.external_reference.as_deref(),
        REFERENCE_MAX,
    )
    .map_err(invalid)?;
    let reason = optional_text("reason", input.reason.as_deref(), REASON_MAX).map_err(invalid)?;
    if input.outcome == HandoffOutcome::Rejected && reason.is_none() {
        return Err(invalid(FieldError {
            field: "reason".to_owned(),
            code: "required".to_owned(),
            message: "Enter why the billing owner rejected the item.".to_owned(),
        }));
    }
    let handoff_id: BillingHandoffId = ctx
        .tx()
        .query_row(
            "SELECT handoff_id FROM bil_handoff_items WHERE id = ?1",
            [input.item_id],
            |row| row.get(0),
        )
        .optional()?
        .ok_or(BillingError::NotFound("billing handoff item"))?;
    let recorded: bool = ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM bil_handoff_outcomes WHERE item_id = ?1)",
        [input.item_id],
        |row| row.get(0),
    )?;
    if recorded {
        return Err(BillingError::OutcomeRecorded);
    }
    ctx.tx().execute(
        "INSERT INTO bil_handoff_outcomes \
         (item_id, outcome, external_reference, reason, recorded_by, recorded_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            input.item_id,
            input.outcome,
            reference,
            reason,
            actor,
            ctx.now()
        ],
    )?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        "billing.record_handoff_outcome",
        Outcome::Succeeded,
    )
    .record("billing_handoff_item", input.item_id)
    .detail("handoff_id", AuditDetail::id(handoff_id))
    .detail("outcome", AuditDetail::code(input.outcome.as_str()))
    .detail(
        "external_reference",
        AuditDetail::boolean(reference.is_some()),
    );
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    Ok(handoff_id)
}

fn ensure_handoff_ready(conn: &Connection, row: &DerivationRow) -> Result<(), BillingError> {
    if !row.facts_sealed {
        return Err(BillingError::Inconsistent("unsealed billing derivation"));
    }
    match row.state {
        BillingDerivationState::Approved => {}
        BillingDerivationState::HandedOff => return Err(BillingError::AlreadyHandedOff),
        BillingDerivationState::Held
        | BillingDerivationState::Candidate
        | BillingDerivationState::Superseded => return Err(BillingError::InvalidDerivationState),
    }
    if is_stale(conn, row)? {
        return Err(BillingError::DerivationStale);
    }
    Ok(())
}

fn item_part(
    conn: &Connection,
    row: &DerivationRow,
    item_id: BillingHandoffItemId,
) -> Result<ItemPart, BillingError> {
    let adjusts = latest_handed_off_item(conn, row.subject_id, row.date_of_service.as_deref())?;
    let mut specimens = Vec::new();
    for specimen in specimen_views(conn, row)? {
        let order = lis_orders::get_order(conn, specimen.order_id)?;
        let practitioner = lis_identity::get_practitioner(conn, order.practitioner_id)?;
        specimens.push(specimen_part(conn, specimen, &order, practitioner)?);
    }
    let mapping_version = match row.mapping_version_id {
        Some(id) => Some(load_summary(conn, id)?.version),
        None => None,
    };
    Ok(ItemPart {
        item_id,
        kind: item_kind(adjusts),
        adjusts_item_id: adjusts,
        derivation_id: row.id,
        derivation_no: row.derivation_no,
        subject: subject_part(&lis_identity::subject_summary(conn, row.subject_id)?),
        date_of_service: row.date_of_service.clone(),
        date_of_service_rule: row.dos_rule.clone(),
        mapping_version,
        flags: repo::flags(conn, row.id)?,
        voids_prior_services: specimens.is_empty() && adjusts.is_some(),
        context_resolution: crate::reconciliation::resolution_for(conn, row.id)?,
        specimens,
    })
}

/// An item adjusts the latest item already handed off for the same subject
/// and date of service.
fn item_kind(adjusts: Option<BillingHandoffItemId>) -> HandoffItemKind {
    if adjusts.is_some() {
        HandoffItemKind::Adjustment
    } else {
        HandoffItemKind::Original
    }
}

fn latest_handed_off_item(
    conn: &Connection,
    subject_id: SubjectId,
    date_of_service: Option<&str>,
) -> Result<Option<BillingHandoffItemId>, BillingError> {
    Ok(conn
        .prepare_cached(
            "SELECT item.id FROM bil_handoff_items item \
             JOIN bil_derivations d ON d.id = item.derivation_id \
             WHERE d.subject_id = ?1 AND d.date_of_service IS ?2 \
             ORDER BY d.derivation_no DESC LIMIT 1",
        )?
        .query_row(params![subject_id, date_of_service], |row| row.get(0))
        .optional()?)
}
