//! Billing handoffs (REQ-BIL-002, brief §5.3). File work never happens in
//! a transaction:
//!
//! - **Create**: the file is built on a read connection and stored as an
//!   artifact; one transaction then rebuilds it, checks its digest and
//!   every derivation again, and records the handoff.
//! - **Export**: the stored bytes are read back with digest verification
//!   and published as a new `billing-handoff-<id>.json` in the folder the
//!   desktop shell resolves (the OS Downloads folder) after an audited
//!   intent, then the export is recorded on the security stream, through
//!   the one export path of `report_rendering`. A billing handoff export is
//!   not a report delivery.

use std::collections::BTreeSet;
use std::path::PathBuf;
use std::sync::Arc;

use lis_audit::AuditDetail;
use lis_billing::{
    BillingError, BillingHandoffId, BillingHandoffSummary, CreateBillingHandoff, HandoffView,
    ListBillingHandoffs, RecordHandoffOutcome,
};
use lis_core::{CommandError, ErrorCode};
use lis_store::{ArtifactDigest, CommandContext, StoreError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::{
    CREATE_BILLING_HANDOFF, EXPORT_BILLING_HANDOFF, GET_BILLING_HANDOFF, LIST_BILLING_HANDOFFS,
    RECORD_HANDOFF_OUTCOME,
};
use crate::engine::Engine;
use crate::pipeline::{Failure, access};
use crate::report_rendering::ExportFile;

const RECORD_TYPE: &str = "billing_handoff";

/// Input of `get_billing_handoff` and `export_billing_handoff`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct BillingHandoffInput {
    pub handoff_id: BillingHandoffId,
}

/// Where a billing handoff file was written: always the user's Downloads
/// folder, under a PHI-free name that labels it a billing handoff.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportedHandoffFile {
    pub file_name: String,
    #[ts(type = "\"downloads\"")]
    pub folder: &'static str,
}

impl Engine {
    /// Retains a later billing-owner response to a previously rejected handoff item.
    pub async fn record_handoff_followup(
        &self,
        input: lis_billing::RecordHandoffFollowupInput,
    ) -> Result<HandoffView, CommandError> {
        self.run(&super::RECORD_HANDOFF_FOLLOWUP, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_billing::record_handoff_followup(ctx, input)?;
                handoff_read(ctx, super::RECORD_HANDOFF_FOLLOWUP.action, id)
            })
            .await
        })
        .await
    }

    /// `create_billing_handoff`: build, store, then recheck and record.
    pub async fn create_billing_handoff(
        &self,
        input: CreateBillingHandoff,
    ) -> Result<HandoffView, CommandError> {
        self.run(&CREATE_BILLING_HANDOFF, |call| async move {
            self.authorize(&call).await?;
            let plan = lis_billing::plan_handoff(input, self.clock.now())?;
            let read_plan = plan.clone();
            let bytes = self
                .read(&call, move |conn| {
                    lis_billing::handoff_file(conn, &read_plan)
                })
                .await?;
            let db = Arc::clone(&self.db);
            let stored = tokio::task::spawn_blocking(move || db.artifacts().put(&bytes))
                .await
                .map_err(|_| Failure::Internal("handoff store task stopped".to_owned()))??;
            let digest = stored.digest.to_string();
            self.write(&call, move |ctx| {
                ensure_outbound(ctx.tx())?;
                let id = lis_billing::create_billing_handoff(ctx, &plan, &digest)?;
                handoff_read(ctx, CREATE_BILLING_HANDOFF.action, id)
            })
            .await
        })
        .await
    }

    /// `list_billing_handoffs`: PHI-free past handoffs, newest first. The
    /// search text may be an accession number, so only its use is audited.
    pub async fn list_billing_handoffs(
        &self,
        input: ListBillingHandoffs,
    ) -> Result<Vec<BillingHandoffSummary>, CommandError> {
        self.run(&LIST_BILLING_HANDOFFS, |call| async move {
            let searched = input
                .search
                .as_deref()
                .is_some_and(|text| !text.trim().is_empty());
            self.audited_read(
                &call,
                move |conn| lis_billing::list_billing_handoffs(conn, input),
                move |rows| {
                    access(LIST_BILLING_HANDOFFS.action)
                        .detail("searched", AuditDetail::boolean(searched))
                        .detail("returned", AuditDetail::count(rows.len()))
                },
            )
            .await
        })
        .await
    }

    /// `get_billing_handoff`: an audited read of the items and outcomes.
    pub async fn get_billing_handoff(
        &self,
        input: BillingHandoffInput,
    ) -> Result<HandoffView, CommandError> {
        self.run(&GET_BILLING_HANDOFF, |call| async move {
            self.audited_read(
                &call,
                move |conn| handoff_view(conn, input.handoff_id),
                |view| access(GET_BILLING_HANDOFF.action).record(RECORD_TYPE, view.handoff_id),
            )
            .await
        })
        .await
    }

    /// `record_handoff_outcome`: one outcome per item, append-only.
    pub async fn record_handoff_outcome(
        &self,
        input: RecordHandoffOutcome,
    ) -> Result<HandoffView, CommandError> {
        self.run(&RECORD_HANDOFF_OUTCOME, |call| async move {
            self.write(&call, move |ctx| {
                let id = lis_billing::record_handoff_outcome(ctx, input)?;
                handoff_read(ctx, RECORD_HANDOFF_OUTCOME.action, id)
            })
            .await
        })
        .await
    }

    /// `export_billing_handoff`: verified bytes to a new file in `folder`,
    /// the Downloads folder the desktop shell resolves (`None` when the OS
    /// has none).
    pub async fn export_billing_handoff(
        &self,
        input: BillingHandoffInput,
        folder: Option<PathBuf>,
    ) -> Result<ExportedHandoffFile, CommandError> {
        self.run(&EXPORT_BILLING_HANDOFF, |call| async move {
            self.authorize(&call).await?;
            let folder = folder.ok_or_else(|| {
                Failure::refused(
                    ErrorCode::Unavailable,
                    "The Downloads folder is not available on this computer.",
                )
            })?;
            let handoff_id = input.handoff_id;
            let source = self
                .read(&call, move |conn| {
                    lis_billing::handoff_export_source(conn, handoff_id)
                })
                .await?;
            let bytes = self.verified_handoff_file(&source.artifact_sha256).await?;
            let file = ExportFile {
                folder,
                stem: source.file_stem.clone(),
                extension: "json",
                sha256: source.artifact_sha256.clone(),
                source: ("handoff_id", AuditDetail::id(handoff_id)),
            };
            let file_name = self
                .export_file(&call, file, bytes, move |ctx, _name| {
                    lis_billing::record_handoff_export(ctx, handoff_id, &source)
                })
                .await?;
            Ok(ExportedHandoffFile {
                file_name,
                folder: "downloads",
            })
        })
        .await
    }

    /// The stored handoff bytes, verified against their digest.
    async fn verified_handoff_file(&self, digest: &str) -> Result<Vec<u8>, Failure> {
        let parsed: ArtifactDigest = digest.parse().map_err(Failure::from)?;
        let db = Arc::clone(&self.db);
        match tokio::task::spawn_blocking(move || db.artifacts().read(&parsed))
            .await
            .map_err(|_| Failure::Internal("handoff read task stopped".to_owned()))?
        {
            Ok(bytes) => Ok(bytes),
            Err(StoreError::ArtifactNotFound | StoreError::ArtifactCorrupt) => {
                Err(BillingError::HandoffFileUnavailable.into())
            }
            Err(error) => Err(error.into()),
        }
    }
}

fn ensure_outbound(conn: &rusqlite::Connection) -> Result<(), BillingError> {
    let gate = lis_ops::recovery::outbound_gate(conn).map_err(|error| match error {
        lis_ops::backup::BackupError::Store(error) => BillingError::Store(error),
        _ => BillingError::Inconsistent("recovery reconciliation"),
    })?;
    match gate {
        Some(gate) => Err(BillingError::Reporting(
            lis_reporting::ReportingError::GateBlocked(vec![gate]),
        )),
        None => Ok(()),
    }
}

/// The handoff as it is now, with the access event of the read.
fn handoff_read(
    ctx: &CommandContext<'_>,
    action: &'static str,
    id: BillingHandoffId,
) -> Result<HandoffView, BillingError> {
    let view = handoff_view(ctx.tx(), id)?;
    ctx.audit(access(action).record(RECORD_TYPE, id))?;
    Ok(view)
}

/// Resolves current account display names on every read; handoffs retain only
/// immutable user IDs and never cache mutable names (REQ-BIL-002).
fn handoff_view(
    conn: &rusqlite::Connection,
    id: BillingHandoffId,
) -> Result<HandoffView, BillingError> {
    let mut view = lis_billing::get_billing_handoff(conn, id)?;
    let mut users = BTreeSet::from([view.created_by]);
    for item in &view.items {
        for outcome in item
            .outcome
            .iter()
            .chain(item.outcome_history.iter().flatten())
        {
            users.insert(outcome.recorded_by);
        }
    }
    let names = lis_auth::display_names(conn, &users).map_err(|error| match error {
        lis_auth::AuthError::Store(store) => BillingError::Store(store),
        _ => BillingError::Inconsistent("handoff actor"),
    })?;
    view.created_by_display = names.get(&view.created_by).cloned();
    for item in &mut view.items {
        for outcome in item
            .outcome
            .iter_mut()
            .chain(item.outcome_history.iter_mut().flatten())
        {
            outcome.recorded_by_display = names.get(&outcome.recorded_by).cloned();
        }
    }
    Ok(view)
}
