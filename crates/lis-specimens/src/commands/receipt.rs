//! Receipt decisions and discrepancies (REQ-SPC-002).

use lis_audit::AuditDetail;
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{actor, clinical, require_user};
use crate::actions;
use crate::error::{SpecimenError, ensure_revision, invalid};
use crate::input::{self, NAME_MAX, TEXT_MAX};
use crate::model::{ReceiptDecision, ReceiptDecisionId, ReceiptDiscrepancyCode, SpecimenId};
use crate::queries;
use crate::repo::accessions::{self as repo, NewDecision};
use crate::repo::receipt_history;
use crate::views::AccessionDetail;

/// Input of [`record_receipt_decision`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RecordReceiptDecisionInput {
    /// The specimen.
    pub specimen_id: SpecimenId,
    /// The specimen revision the decision was made on.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The decision.
    pub decision: ReceiptDecision,
    /// Documented discrepancies, each code once.
    #[serde(default)]
    pub discrepancies: Vec<DiscrepancyInput>,
    /// The lab policy permitting a conditional acceptance.
    #[serde(default)]
    #[ts(optional)]
    pub policy_reference: Option<String>,
    /// The scope of a conditional acceptance.
    #[serde(default)]
    #[ts(optional)]
    pub scope: Option<String>,
    /// Required to hold or reject.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// One documented discrepancy.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DiscrepancyInput {
    /// The discrepancy.
    pub code: ReceiptDiscrepancyCode,
    /// Required for `other`.
    #[serde(default)]
    #[ts(optional)]
    pub note: Option<String>,
}

struct ParsedDecision {
    discrepancies: Vec<(ReceiptDiscrepancyCode, Option<String>)>,
    policy_reference: Option<String>,
    scope: Option<String>,
    reason: Option<String>,
}

/// Records a receipt decision with its discrepancies (REQ-SPC-002).
/// Conditional acceptance needs a policy reference and scope; hold and
/// rejection need a reason. Accepted, conditionally accepted and rejected
/// specimens take no further decision in M2.
pub fn record_receipt_decision(
    ctx: &mut CommandContext<'_>,
    input: RecordReceiptDecisionInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let parsed = parse_decision(&input)?;
    let (acceptance, revision) = repo::specimen_state(ctx.tx(), input.specimen_id)?
        .ok_or(SpecimenError::NotFound("specimen"))?;
    ensure_revision(revision, input.expected_revision)?;
    let next = acceptance.decide(input.decision)?;
    repo::update_acceptance(ctx.tx(), input.specimen_id, next, revision + 1)?;
    let decision_id = ReceiptDecisionId::generate();
    let by = actor(ctx);
    repo::insert_decision(
        ctx.tx(),
        &NewDecision {
            id: decision_id,
            specimen_id: input.specimen_id,
            decision: input.decision,
            policy_reference: parsed.policy_reference.as_deref(),
            scope: parsed.scope.as_deref(),
            reason: parsed.reason.as_deref(),
            by: &by,
            at: ctx.now(),
        },
    )?;
    for (code, note) in &parsed.discrepancies {
        repo::insert_discrepancy(ctx.tx(), decision_id, *code, note.as_deref())?;
    }
    let codes = parsed.discrepancies.iter().map(|(code, _)| code.as_str());
    let mut event = clinical(actions::RECORD_RECEIPT_DECISION)
        .record("specimen", input.specimen_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail("decision", AuditDetail::code(input.decision.as_str()))
        .detail("discrepancies", AuditDetail::codes(codes));
    if let Some(reason) = parsed.reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    let accession_id = queries::specimen_accession(ctx.tx(), input.specimen_id)?;
    queries::get_accession(ctx.tx(), accession_id)
}

fn parse_decision(input: &RecordReceiptDecisionInput) -> Result<ParsedDecision, SpecimenError> {
    let mut discrepancies = Vec::new();
    for discrepancy in &input.discrepancies {
        if discrepancies
            .iter()
            .any(|(code, _)| *code == discrepancy.code)
        {
            return Err(invalid(
                "discrepancies",
                "duplicate",
                "List each discrepancy once.",
            ));
        }
        let note =
            input::optional_text("discrepancies.note", discrepancy.note.as_deref(), TEXT_MAX)?;
        if discrepancy.code == ReceiptDiscrepancyCode::Other && note.is_none() {
            return Err(invalid(
                "discrepancies.note",
                "required",
                "Describe the other discrepancy.",
            ));
        }
        discrepancies.push((discrepancy.code, note));
    }
    let policy_reference = input::optional_text(
        "policyReference",
        input.policy_reference.as_deref(),
        NAME_MAX,
    )?;
    let scope = input::optional_text("scope", input.scope.as_deref(), TEXT_MAX)?;
    let reason = input::optional_text("reason", input.reason.as_deref(), TEXT_MAX)?;
    check_decision_fields(input.decision, &policy_reference, &scope, &reason)?;
    Ok(ParsedDecision {
        discrepancies,
        policy_reference,
        scope,
        reason,
    })
}

/// Conditional acceptance needs a policy reference and scope, which no other
/// decision takes; hold and rejection need a reason (REQ-SPC-002).
fn check_decision_fields(
    decision: ReceiptDecision,
    policy_reference: &Option<String>,
    scope: &Option<String>,
    reason: &Option<String>,
) -> Result<(), SpecimenError> {
    let conditional = decision == ReceiptDecision::AcceptedConditionally;
    for (field, value) in [("policyReference", policy_reference), ("scope", scope)] {
        match (conditional, value.is_some()) {
            (true, false) => {
                return Err(invalid(
                    field,
                    "required",
                    "Conditional acceptance needs the lab policy reference and its scope.",
                ));
            }
            (false, true) => {
                return Err(invalid(
                    field,
                    "not_applicable",
                    "Only a conditional acceptance takes a policy reference and scope.",
                ));
            }
            (true, true) | (false, false) => {}
        }
    }
    let needs_reason = matches!(decision, ReceiptDecision::Held | ReceiptDecision::Rejected);
    if needs_reason && reason.is_none() {
        return Err(invalid(
            "reason",
            "required",
            "Enter the reason for holding or rejecting the specimen.",
        ));
    }
    Ok(())
}

/// Explicit legacy chronology resolution; original decisions remain immutable.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ResolveReceiptChronologyInput {
    pub specimen_id: SpecimenId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Every decision in evidenced causal order, oldest first.
    pub decision_ids: Vec<ReceiptDecisionId>,
    /// Source evidence supporting the ordering; never infer it from timestamps.
    pub reason: String,
}

/// Resolves ambiguous legacy chronology using reviewed source evidence.
pub fn resolve_receipt_chronology(
    ctx: &mut CommandContext<'_>,
    input: ResolveReceiptChronologyInput,
) -> Result<AccessionDetail, SpecimenError> {
    require_user(ctx, Capability::ManageQuality)?;
    let reason = input::reason(Some(&input.reason))?;
    let (acceptance, revision) = repo::specimen_state(ctx.tx(), input.specimen_id)?
        .ok_or(SpecimenError::NotFound("specimen"))?;
    ensure_revision(revision, input.expected_revision)?;
    if !receipt_history::chronology_uncertain(ctx.tx(), input.specimen_id)? {
        return Err(SpecimenError::AlreadyClosed("receipt chronology"));
    }
    let decisions = receipt_history::decisions(ctx.tx(), input.specimen_id)?;
    let additions = reviewed_order(&decisions, &input.decision_ids, acceptance, revision)?;
    for (id, sequence) in additions {
        ctx.tx().execute(
            "INSERT INTO spc_receipt_order VALUES (?1, ?2, ?3, 'review')",
            rusqlite::params![id, input.specimen_id, sequence],
        )?;
    }
    repo::update_acceptance(ctx.tx(), input.specimen_id, acceptance, revision + 1)?;
    ctx.audit(
        clinical("specimen.resolve_receipt_chronology")
            .record("specimen", input.specimen_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(reason),
    )?;
    queries::get_accession(
        ctx.tx(),
        queries::specimen_accession(ctx.tx(), input.specimen_id)?,
    )
}

fn reviewed_order(
    decisions: &[crate::ReceiptDecisionView],
    decision_ids: &[ReceiptDecisionId],
    acceptance: crate::SpecimenAcceptance,
    revision: i64,
) -> Result<Vec<(ReceiptDecisionId, i64)>, SpecimenError> {
    if decision_ids.len() != decisions.len() {
        return Err(invalid(
            "decisionIds",
            "incomplete",
            "Include every receipt decision exactly once.",
        ));
    }
    let mut additions = Vec::new();
    let mut state = crate::SpecimenAcceptance::Received;
    let mut seen = std::collections::HashSet::new();
    let mut previous_sequence = 0;
    for id in decision_ids {
        let decision = decisions
            .iter()
            .find(|d| d.decision_id == *id)
            .ok_or(SpecimenError::NotFound("receipt decision"))?;
        if !seen.insert(*id) {
            return Err(invalid(
                "decisionIds",
                "duplicate",
                "Include every receipt decision exactly once.",
            ));
        }
        state = state.decide(decision.decision)?;
        let sequence = decision.causal_sequence.unwrap_or(previous_sequence + 1);
        if sequence <= previous_sequence || sequence > revision {
            return Err(invalid(
                "decisionIds",
                "contradiction",
                "The proposed order contradicts committed causal evidence.",
            ));
        }
        previous_sequence = sequence;
        match decision.causal_sequence {
            Some(_) => {}
            None => {
                additions.push((*id, sequence));
            }
        }
    }
    if state != acceptance {
        return Err(invalid(
            "decisionIds",
            "state",
            "The proposed order contradicts the specimen's current acceptance.",
        ));
    }
    Ok(additions)
}
