//! Receipt evidence reads preserve unresolved causal links explicitly.
use crate::model::{ReceiptDecisionId, SpecimenId};
use crate::views::{ReceiptDecisionView, ReceiptDiscrepancyView};
use rusqlite::Connection;

pub(crate) fn decisions(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<Vec<ReceiptDecisionView>> {
    conn.prepare_cached(
        "SELECT d.id, decision, policy_reference, scope_text, reason, decided_by, decided_at, o.sequence \
         FROM spc_receipt_decisions d LEFT JOIN spc_receipt_order o ON o.decision_id = d.id \
         WHERE d.specimen_id = ?1 ORDER BY o.sequence, d.id",
    )?
    .query_map([specimen_id], |row| {
        Ok(ReceiptDecisionView {
            decision_id: row.get(0)?,
            decision: row.get(1)?,
            policy_reference: row.get(2)?,
            scope: row.get(3)?,
            reason: row.get(4)?,
            decided_by: row.get(5)?,
            decided_by_display: None,
            decided_at: row.get(6)?,
            causal_sequence: row.get(7)?,
            discrepancies: Vec::new(),
        })
    })?
    .collect()
}

pub(crate) fn discrepancies(
    conn: &Connection,
    decision_id: ReceiptDecisionId,
) -> rusqlite::Result<Vec<ReceiptDiscrepancyView>> {
    conn.prepare_cached(
        "SELECT code, note FROM spc_receipt_discrepancies WHERE decision_id = ?1 ORDER BY code",
    )?
    .query_map([decision_id], |row| {
        Ok(ReceiptDiscrepancyView {
            code: row.get(0)?,
            note: row.get(1)?,
        })
    })?
    .collect()
}

/// Missing causal links are unresolved legacy evidence, not timestamp order.
pub(crate) fn chronology_uncertain(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> rusqlite::Result<bool> {
    conn.query_row("SELECT EXISTS(SELECT 1 FROM spc_receipt_decisions d LEFT JOIN spc_receipt_order o ON o.decision_id = d.id WHERE d.specimen_id = ?1 AND o.decision_id IS NULL)", [specimen_id], |r| r.get(0))
}
