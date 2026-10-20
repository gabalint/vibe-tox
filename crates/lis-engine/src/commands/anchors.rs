//! Audit anchor commands (NFR-18, ADR 0008, REQ-OPS-008; M4 contract "Audit anchors").
//!
//! - `get_anchor_status` and `list_anchor_exports` (`view_audit`) show sequence numbers,
//!   times, counts and target labels only, so they are not audited (M4 brief §2.5).
//! - `export_anchors_now` (`backup_restore`) runs one export pass as the signed-in user;
//!   `lis_ops::anchors::export_pending` rechecks the capability in its transactions and
//!   records `ops.export_anchors`.
//! - `verify_audit_anchors` (`view_audit`) is an audited read (`audit.verify_anchors`, like
//!   `verify_audit_chain`): it reads the anchor files of the folder the desktop shell's
//!   dialog returned, or of every available target, outside any transaction, then compares
//!   them with the local chains and explains restores from the restore records.
//!
//! Every path to a file is checked against the caller's capability before any file is
//! read or written. Audit details and logs hold counts and booleans, never paths or labels.

use std::path::PathBuf;

use lis_audit::AuditDetail;
use lis_core::{Capability, CommandError};
use lis_ops::anchors::{
    self, AnchorError, AnchorExportView, AnchorStatusView, AnchorVerification, ExportReport,
    ReadAnchors,
};
use lis_ops::targets;
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{Failure, access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const VIEW_AUDIT: Access = Access::Capability(Capability::ViewAudit);

pub(crate) const GET_ANCHOR_STATUS: CommandSpec =
    spec("get_anchor_status", "audit.get_anchor_status", VIEW_AUDIT);
pub(crate) const LIST_ANCHOR_EXPORTS: CommandSpec = spec(
    "list_anchor_exports",
    "audit.list_anchor_exports",
    VIEW_AUDIT,
);
pub(crate) const EXPORT_ANCHORS_NOW: CommandSpec = spec(
    "export_anchors_now",
    anchors::EXPORT_ANCHORS,
    Access::Capability(Capability::BackupRestore),
);
pub(crate) const VERIFY_AUDIT_ANCHORS: CommandSpec =
    spec("verify_audit_anchors", "audit.verify_anchors", VIEW_AUDIT);

domain_error!(AnchorError, |_: &AnchorError| Vec::new());

/// Input of `list_anchor_exports`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListAnchorExportsInput {
    /// Newest files first, at most this many; the operations module bounds it to 1–200.
    pub limit: u32,
}

/// Input of `verify_audit_anchors`. The desktop shell reads it: when `chooseFolder`, it
/// opens a folder dialog and passes the chosen folder to the engine; otherwise the engine
/// reads every available target.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct VerifyAuditAnchorsInput {
    /// Read the anchors of one folder chosen in a dialog instead of the targets.
    pub choose_folder: bool,
}

impl Engine {
    /// `get_anchor_status`: chain, seal and off-host anchor positions and the lag (NFR-18).
    pub async fn get_anchor_status(&self) -> Result<AnchorStatusView, CommandError> {
        self.run(&GET_ANCHOR_STATUS, |call| async move {
            let now = self.clock.now();
            self.read(&call, move |conn| anchors::get_anchor_status(conn, now))
                .await
        })
        .await
    }

    /// `list_anchor_exports`: the newest anchor files written to targets.
    pub async fn list_anchor_exports(
        &self,
        input: ListAnchorExportsInput,
    ) -> Result<Vec<AnchorExportView>, CommandError> {
        self.run(&LIST_ANCHOR_EXPORTS, |call| async move {
            self.read(&call, move |conn| {
                anchors::list_anchor_exports(conn, input.limit)
            })
            .await
        })
        .await
    }

    /// `export_anchors_now`: seals the streams and writes every pending seal to every
    /// available target (ADR 0008 item 3). Refused when no target is available.
    pub async fn export_anchors_now(&self) -> Result<ExportReport, CommandError> {
        self.run(&EXPORT_ANCHORS_NOW, |call| async move {
            self.authorize(&call).await?;
            let db = self.db.clone();
            let principal = call.principal.clone();
            blocking(move || anchors::export_pending(&db, principal)).await
        })
        .await
    }

    /// `verify_audit_anchors`: compares the anchors under `folder`, or on every available
    /// target when `None`, with the local chains (NFR-18, VAL-40). The desktop shell
    /// passes the folder its dialog returned; the UI never sends a path.
    pub async fn verify_audit_anchors(
        &self,
        folder: Option<PathBuf>,
    ) -> Result<AnchorVerification, CommandError> {
        self.run(&VERIFY_AUDIT_ANCHORS, |call| async move {
            self.authorize(&call).await?;
            let chosen = folder.is_some();
            let active = match folder {
                Some(_) => Vec::new(),
                None => self
                    .db
                    .read(targets::active)
                    .await
                    .map_err(AnchorError::from)?,
            };
            let read = blocking(move || {
                let roots = match folder {
                    Some(folder) => vec![folder],
                    // Reads one small marker file per target, outside any transaction.
                    None => active
                        .iter()
                        .filter(|target| targets::is_available(target))
                        .map(targets::root)
                        .collect(),
                };
                anchors::read_anchors(&roots)
            })
            .await?;
            self.audited_read(
                &call,
                move |conn| verify(conn, &read),
                move |verification| verify_event(verification, chosen),
            )
            .await
        })
        .await
    }
}

fn verify(
    conn: &rusqlite::Connection,
    read: &ReadAnchors,
) -> Result<AnchorVerification, AnchorError> {
    let restores = lis_ops::restore::restore_records(conn)?;
    anchors::verify_anchors(conn, read, &restores)
}

/// The `access` event of a verification: counts and whether a folder was chosen; never
/// the folder.
fn verify_event(verification: &AnchorVerification, chosen_folder: bool) -> lis_audit::AuditEvent {
    let summary = &verification.summary;
    access(VERIFY_AUDIT_ANCHORS.action)
        .detail("chosen_folder", AuditDetail::boolean(chosen_folder))
        .detail("anchors_read", count(verification.anchors_read))
        .detail("compared", count(summary.compared))
        .detail("matched", count(summary.matched))
        .detail("lost_interval", count(summary.lost_interval))
        .detail("problems", AuditDetail::count(verification.problems()))
}

fn count(value: u64) -> AuditDetail {
    AuditDetail::count(usize::try_from(value).unwrap_or(usize::MAX))
}

/// Runs blocking file I/O off the async runtime.
async fn blocking<T: Send + 'static>(
    step: impl FnOnce() -> Result<T, AnchorError> + Send + 'static,
) -> Result<T, Failure> {
    tokio::task::spawn_blocking(step)
        .await
        .map_err(|_| Failure::Internal("anchor task stopped".to_owned()))?
        .map_err(Failure::from)
}
