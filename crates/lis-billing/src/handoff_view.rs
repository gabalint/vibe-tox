//! The handoff read model: items with their reconciliation status from
//! `bil_v_handoff_status` and any recorded outcome (REQ-BIL-002).

use lis_core::{UserId, UtcTimestamp};
use rusqlite::{Connection, OptionalExtension};
use serde::Serialize;
use ts_rs::TS;

use crate::error::BillingError;
use crate::model::{
    BillingDerivationId, BillingHandoffId, BillingHandoffItemId, HandoffItemKind, HandoffOutcome,
    HandoffStatus,
};

/// One handoff with its items.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HandoffView {
    pub handoff_id: BillingHandoffId,
    /// Sequential number used to recognize this handoff outside the LIS.
    #[ts(type = "number")]
    pub handoff_no: i64,
    pub format: String,
    pub artifact_sha256: String,
    #[ts(type = "number")]
    pub item_count: i64,
    pub created_by: UserId,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub created_by_display: Option<String>,
    pub created_at: UtcTimestamp,
    pub items: Vec<HandoffItemView>,
    pub allowed_actions: Vec<String>,
}

/// One item and where its reconciliation stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HandoffItemView {
    pub item_id: BillingHandoffItemId,
    pub derivation_id: BillingDerivationId,
    #[ts(type = "number")]
    pub derivation_no: i64,
    pub kind: HandoffItemKind,
    pub adjusts_item_id: Option<BillingHandoffItemId>,
    pub status: HandoffStatus,
    pub outcome: Option<HandoffOutcomeView>,
    #[ts(optional)]
    #[ts(type = "number")]
    pub outcome_revision: Option<i64>,
    #[ts(optional)]
    pub outcome_history: Option<Vec<HandoffOutcomeView>>,
    pub allowed_actions: Vec<String>,
}

/// The billing owner's recorded outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HandoffOutcomeView {
    pub outcome: HandoffOutcome,
    pub external_reference: Option<String>,
    pub reason: Option<String>,
    pub recorded_by: UserId,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub recorded_by_display: Option<String>,
    pub recorded_at: UtcTimestamp,
}

/// The handoff `id` and its items, oldest derivation first.
pub fn get_billing_handoff(
    conn: &Connection,
    id: BillingHandoffId,
) -> Result<HandoffView, BillingError> {
    let (handoff_no, format, artifact_sha256, item_count, created_by, created_at) = conn
        .prepare_cached(
            "SELECT handoff_no, format, artifact_sha256, item_count, created_by, created_at \
             FROM bil_handoffs WHERE id = ?1",
        )?
        .query_row([id], |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        })
        .optional()?
        .ok_or(BillingError::NotFound("billing handoff"))?;
    let mut items: Vec<HandoffItemView> = conn
        .prepare_cached(
            "SELECT item.id, item.derivation_id, d.derivation_no, item.kind, item.adjusts_item_id, \
                    status.status, outcome.outcome, outcome.external_reference, outcome.reason, \
                    outcome.recorded_by, outcome.recorded_at \
             FROM bil_handoff_items item \
             JOIN bil_derivations d ON d.id = item.derivation_id \
             JOIN bil_v_handoff_status status ON status.item_id = item.id \
             LEFT JOIN bil_v_latest_handoff_outcomes outcome ON outcome.item_id = item.id \
             WHERE item.handoff_id = ?1 ORDER BY d.derived_at, item.id",
        )?
        .query_map([id], item_row)?
        .collect::<Result<_, _>>()?;
    for item in &mut items {
        let history = outcome_history(conn, item.item_id)?;
        item.outcome_revision = Some(
            i64::try_from(history.len())
                .map_err(|_| BillingError::Inconsistent("outcome history"))?,
        );
        item.outcome_history = Some(history);
    }
    Ok(HandoffView {
        handoff_id: id,
        handoff_no,
        format,
        artifact_sha256,
        item_count,
        created_by,
        created_by_display: None,
        created_at,
        items,
        allowed_actions: vec!["export_billing_handoff".to_owned()],
    })
}

/// Current human label for a billing audit record. The number is resolved
/// from the owner table and never copied into the audit trail (REQ-OPS-001).
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, BillingError> {
    match record_type {
        "billing_derivation" => number_label(
            conn,
            "SELECT derivation_no FROM bil_derivations WHERE id = ?1",
            record_id,
            "Derivation",
        ),
        "billing_handoff" => number_label(
            conn,
            "SELECT handoff_no FROM bil_handoffs WHERE id = ?1",
            record_id,
            "Handoff",
        ),
        "billing_handoff_item" => Ok(conn
            .prepare_cached(
                "SELECT handoff.handoff_no FROM bil_handoff_items item \
                 JOIN bil_handoffs handoff ON handoff.id = item.handoff_id WHERE item.id = ?1",
            )?
            .query_row([record_id], |row| row.get::<_, i64>(0))
            .optional()?
            .map(|number| format!("Handoff {number}"))),
        _ => Ok(None),
    }
}

fn number_label(
    conn: &Connection,
    sql: &str,
    record_id: &str,
    kind: &str,
) -> Result<Option<String>, BillingError> {
    Ok(conn
        .prepare_cached(sql)?
        .query_row([record_id], |row| row.get::<_, i64>(0))
        .optional()?
        .map(|number| format!("{kind} {number}")))
}

fn item_row(row: &rusqlite::Row<'_>) -> rusqlite::Result<HandoffItemView> {
    let status: HandoffStatus = row.get(5)?;
    let outcome = match row.get::<_, Option<HandoffOutcome>>(6)? {
        Some(outcome) => Some(HandoffOutcomeView {
            outcome,
            external_reference: row.get(7)?,
            reason: row.get(8)?,
            recorded_by: row.get(9)?,
            recorded_by_display: None,
            recorded_at: row.get(10)?,
        }),
        None => None,
    };
    let allowed_actions = match status {
        HandoffStatus::Outstanding => vec!["record_handoff_outcome".to_owned()],
        HandoffStatus::Rejected => vec!["record_handoff_followup".to_owned()],
        HandoffStatus::Acknowledged => Vec::new(),
    };
    Ok(HandoffItemView {
        item_id: row.get(0)?,
        derivation_id: row.get(1)?,
        derivation_no: row.get(2)?,
        kind: row.get(3)?,
        adjusts_item_id: row.get(4)?,
        status,
        outcome,
        outcome_revision: None,
        outcome_history: None,
        allowed_actions,
    })
}

fn outcome_history(
    conn: &Connection,
    item: BillingHandoffItemId,
) -> Result<Vec<HandoffOutcomeView>, BillingError> {
    Ok(conn.prepare_cached("SELECT outcome, external_reference, reason, recorded_by, recorded_at FROM bil_v_handoff_outcome_history WHERE item_id = ?1 ORDER BY revision")?
        .query_map([item], |row| Ok(HandoffOutcomeView {
            outcome: row.get(0)?, external_reference: row.get(1)?, reason: row.get(2)?,
            recorded_by: row.get(3)?, recorded_at: row.get(4)?, recorded_by_display: None,
        }))?.collect::<Result<_, _>>()?)
}
