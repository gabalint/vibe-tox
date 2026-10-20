//! The recovery reconciliation after a restore (REQ-OPS-002; M4 design brief D5, §3.8):
//! the one home of its checklist codes, labels and guidance, its outbound gate and its
//! closing.
//!
//! Invariants: every recorded restore opens a reconciliation; while one is open,
//! [`outbound_gate`] blocks release, delivery attempts, delivery exports and billing
//! handoffs, and the dispatcher holds label printing; only a quality manager closes it,
//! with every checklist item attested and a note (the note is the audit reason; the
//! engine adds fresh authentication). The closure is append-only and its insert trigger
//! refuses a closure without all eleven attestations.

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, FieldError, GateReason, UtcTimestamp, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use super::anchors::summary_at;
use super::apply::RestoreId;
use super::record::RestoreFloors;
use crate::anchors::AnchorSummary;
use crate::backup::BackupError;

/// Audit action of closing a reconciliation (clinical stream).
pub const CLOSE_RECOVERY: &str = "ops.close_recovery";
/// Gate code while a reconciliation is open.
pub const GATE_CODE: &str = "recovery_reconciliation_open";
/// Gate message while a reconciliation is open (§3.8).
pub const GATE_MESSAGE: &str = "Restored from a backup: release, delivery and billing handoff wait until the laboratory closes the recovery reconciliation.";
const NOTE_MAX_CHARS: usize = 2000;

lis_core::text_enum! {
    /// An item attested when the reconciliation is closed (§3.8).
    ChecklistItem {
        /// This computer replaces production.
        ProductionComputer = "production_computer",
        /// The lost interval is listed.
        LostIntervalListed = "lost_interval_listed",
        /// The number floors cover every physical label.
        NumbersChecked = "numbers_checked",
        /// Analyzer results of the lost interval were resent.
        AnalyzerResultsResent = "analyzer_results_resent",
        /// Reports released or delivered in the lost interval are reconciled.
        ReportsReconciled = "reports_reconciled",
        /// Orders, decisions, QC and reviews were re-entered.
        OrdersReentered = "orders_reentered",
        /// Billing handoffs of the lost interval are reconciled.
        BillingReconciled = "billing_reconciled",
        /// Account changes made after the backup were reapplied.
        AccountsReapplied = "accounts_reapplied",
        /// Configuration changes made after the backup were reapplied.
        ConfigurationReapplied = "configuration_reapplied",
        /// The anchor comparison was reviewed.
        AnchorsReviewed = "anchors_reviewed",
        /// The held label jobs were reviewed.
        LabelsWaiting = "labels_waiting",
    }
}

impl ChecklistItem {
    /// Every item, in checklist order.
    pub const ALL: [Self; 11] = [
        Self::ProductionComputer,
        Self::LostIntervalListed,
        Self::NumbersChecked,
        Self::AnalyzerResultsResent,
        Self::ReportsReconciled,
        Self::OrdersReentered,
        Self::BillingReconciled,
        Self::AccountsReapplied,
        Self::ConfigurationReapplied,
        Self::AnchorsReviewed,
        Self::LabelsWaiting,
    ];

    /// Short label for the checklist.
    #[must_use]
    pub fn label(self) -> &'static str {
        match self {
            Self::ProductionComputer => "This computer is the production computer",
            Self::LostIntervalListed => "Lost interval listed",
            Self::NumbersChecked => "Number floors checked",
            Self::AnalyzerResultsResent => "Analyzer results resent",
            Self::ReportsReconciled => "Reports reconciled",
            Self::OrdersReentered => "Orders and work re-entered",
            Self::BillingReconciled => "Billing handoffs reconciled",
            Self::AccountsReapplied => "Account changes reapplied",
            Self::ConfigurationReapplied => "Configuration changes reapplied",
            Self::AnchorsReviewed => "Audit anchors reviewed",
            Self::LabelsWaiting => "Waiting labels reviewed",
        }
    }

    /// What attesting the item means.
    #[must_use]
    pub fn guidance(self) -> &'static str {
        match self {
            Self::ProductionComputer => {
                "This computer replaces production. Never close the reconciliation on a test copy: closing lets it write to production targets and print."
            }
            Self::LostIntervalListed => {
                "List the interval between the backup set's time and the incident: work done then is not in the restored database."
            }
            Self::NumbersChecked => {
                "The accession and container number floors cover every physical label issued; raise them if needed."
            }
            Self::AnalyzerResultsResent => {
                "Have analyzers resend results from the lost interval; see the \"results expected\" queue."
            }
            Self::ReportsReconciled => {
                "Reconcile reports released or delivered in the lost interval; never resend an older revision."
            }
            Self::OrdersReentered => {
                "Re-enter orders, decisions, QC and reviews from the lost interval."
            }
            Self::BillingReconciled => "Reconcile billing handoffs from the lost interval.",
            Self::AccountsReapplied => {
                "Reapply account changes made after the backup; for example, a user disabled after the snapshot is active again."
            }
            Self::ConfigurationReapplied => {
                "Reapply catalog, instrument, destination, settings and backup target changes made after the backup."
            }
            Self::AnchorsReviewed => {
                "Review the anchor comparison: matched, lost interval, mismatched or not checked."
            }
            Self::LabelsWaiting => {
                "Label jobs held while the reconciliation is open print when it closes; review how many are waiting."
            }
        }
    }
}

/// True while a recorded restore has no closure.
pub fn is_open(conn: &Connection) -> Result<bool, BackupError> {
    Ok(conn
        .prepare_cached("SELECT 1 FROM ops_v_recovery_open")?
        .exists([])?)
}

/// The gate the engine composes into release, delivery attempts, delivery exports and
/// billing handoffs; read inside the command's transaction.
pub fn outbound_gate(conn: &Connection) -> Result<Option<GateReason>, BackupError> {
    Ok(is_open(conn)?.then(|| GateReason {
        code: GATE_CODE.to_owned(),
        message: GATE_MESSAGE.to_owned(),
    }))
}

/// Closes the open reconciliation of `restore_id` (`ops.close_recovery`, clinical
/// stream): every checklist item attested and a note, which becomes the audit reason.
pub fn close_recovery_reconciliation(
    ctx: &mut CommandContext<'_>,
    restore_id: RestoreId,
    attested: &[ChecklistItem],
    note: &str,
) -> Result<(), BackupError> {
    ctx.require(Capability::ManageQuality)?;
    let note = required_text("note", note, NOTE_MAX_CHARS).map_err(BackupError::Invalid)?;
    let attested: BTreeSet<&str> = attested.iter().map(|item| item.as_str()).collect();
    if attested.len() != ChecklistItem::ALL.len() {
        return Err(BackupError::Invalid(FieldError {
            field: "attestedItems".to_owned(),
            code: "incomplete".to_owned(),
            message: "Attest every checklist item before closing.".to_owned(),
        }));
    }
    let open = ctx
        .tx()
        .prepare_cached("SELECT 1 FROM ops_v_recovery_open WHERE restore_id = ?1")?
        .exists([restore_id])?;
    if !open {
        return Err(BackupError::RecoveryNotOpen);
    }
    let mut insert = ctx.tx().prepare_cached(
        "INSERT INTO ops_recovery_attestations (restore_id, item_code) VALUES (?1, ?2)",
    )?;
    for item in ChecklistItem::ALL {
        insert.execute(params![restore_id, item])?;
    }
    ctx.tx()
        .prepare_cached(
            "INSERT INTO ops_recovery_closures (restore_id, closed_by, closed_at) \
             VALUES (?1, ?2, ?3)",
        )?
        .execute(params![
            restore_id,
            ctx.principal().actor().id_text(),
            ctx.now()
        ])?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, CLOSE_RECOVERY, Outcome::Succeeded)
            .record("restore", restore_id)
            .detail("items_attested", AuditDetail::count(attested.len()))
            .reason(note),
    )?;
    Ok(())
}

/// The newest restore's reconciliation, as the recovery screen shows it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecoveryView {
    /// The restore.
    pub restore_id: RestoreId,
    /// When the restored set was taken: the lost interval starts here.
    pub set_created_at: UtcTimestamp,
    /// When the restore was applied.
    pub applied_at: UtcTimestamp,
    /// The checklist, in order.
    pub items: [ChecklistItem; 11],
    /// The anchors compared while staging.
    pub anchors: AnchorSummary,
    /// The floors raised when the restore was recorded.
    pub floors: RestoreFloors,
    /// Who closed it and when (stored actor text); `None` while open.
    pub closed: Option<(String, UtcTimestamp)>,
}

/// The newest restore's reconciliation, open or closed; `None` without a restore.
pub fn get_recovery_reconciliation(conn: &Connection) -> Result<Option<RecoveryView>, BackupError> {
    let view = conn
        .prepare_cached(
            "SELECT r.id, r.set_created_at, r.applied_at, r.accession_floor, r.container_floor, \
             c.closed_by, c.closed_at, r.anchors_compared, r.anchors_matched, \
             r.anchors_lost_interval, r.anchors_unverified, r.anchors_mismatched, \
             r.anchors_missing, r.anchor_conflicts, r.anchor_gaps, r.anchors_unanchored \
             FROM ops_restores r LEFT JOIN ops_recovery_closures c ON c.restore_id = r.id \
             ORDER BY r.applied_at DESC, r.id DESC LIMIT 1",
        )?
        .query_row([], |row| {
            let closed_by: Option<String> = row.get(5)?;
            let closed_at: Option<UtcTimestamp> = row.get(6)?;
            Ok(RecoveryView {
                restore_id: row.get(0)?,
                set_created_at: row.get(1)?,
                applied_at: row.get(2)?,
                items: ChecklistItem::ALL,
                anchors: summary_at(row, 7)?.unwrap_or_default(),
                floors: RestoreFloors {
                    accession: row.get(3)?,
                    container: row.get(4)?,
                },
                closed: closed_by.zip(closed_at),
            })
        })
        .optional()?;
    Ok(view)
}
