//! One anchor export pass (M4 design brief §4, ADR 0008 item 3). Blocking: the engine
//! calls it inside `spawn_blocking`, from the job runner after each seal or for
//! `export_anchors_now`. Seal and plan in one transaction, write the files, then record
//! them in a second transaction; no file I/O runs inside either.

use std::collections::HashSet;
use std::fs;
use std::path::Path;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Seal, Stream};
use lis_core::{Actor, Capability, CorrelationId, InstallationId, Principal, UtcTimestamp};
use lis_store::{CommandContext, Database};
use serde::Serialize;
use ts_rs::TS;

use super::repo::{self, NewExport, SealRange};
use super::{AnchorError, AnchorExportId, EXPORT_ANCHORS};
use crate::backup::durable::write_file;
use crate::installation_id;
use crate::targets::{self, Target, TargetId};

/// The folder of anchor files inside a target's `VibeToxLIS` folder.
pub(super) const ANCHORS_FOLDER: &str = "anchors";
/// The extension of anchor files; anything else under `anchors/` is not one.
pub(super) const ANCHOR_FILE_EXTENSION: &str = "jsonl";

/// What one pass wrote (`export_anchors_now`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ExportReport {
    /// Targets that received a new anchor file.
    #[ts(type = "number")]
    pub targets_written: usize,
    /// Distinct seals written, whatever the number of targets.
    #[ts(type = "number")]
    pub seals_written: usize,
    /// Available targets whose file could not be written; their seals stay pending.
    #[ts(type = "number")]
    pub targets_failed: usize,
    /// Active targets that are not reachable now; their seals stay pending.
    #[ts(type = "number")]
    pub targets_unavailable: usize,
}

/// Seals the audit streams and writes every pending seal to every available target
/// (ADR 0008 item 3). `principal` is the job runner's system principal for a scheduled
/// pass, or a user holding `backup_restore` for a manual one, who is then recorded.
///
/// One `ops.export_anchors` event records the pass: always for a manual pass, and for a
/// scheduled pass that wrote or failed something. A manual pass with no available target
/// fails with [`AnchorError::NoTargetAvailable`]. If recording fails after files were
/// written, those files stay; the next pass writes the same seals again, and identical
/// lines count as one anchor.
pub fn export_pending(db: &Database, principal: Principal) -> Result<ExportReport, AnchorError> {
    let correlation = CorrelationId::generate();
    let plan = db.write_blocking(principal.clone(), correlation, plan)?;
    let (report, written) = write_files(&plan);
    let manual = plan.exported_by.is_some();
    if manual && report.targets_unavailable == plan.targets.len() {
        return Err(AnchorError::NoTargetAvailable);
    }
    if manual || report.targets_written + report.targets_failed > 0 {
        db.write_blocking(principal, correlation, move |ctx| {
            record(ctx, &plan, &written, report)
        })?;
    }
    tracing::info!(
        targets_written = report.targets_written,
        seals_written = report.seals_written,
        targets_failed = report.targets_failed,
        targets_unavailable = report.targets_unavailable,
        "anchor export pass finished"
    );
    Ok(report)
}

/// Writes one file to each available target with pending seals (no transaction open).
fn write_files(plan: &Plan) -> (ExportReport, Vec<WrittenFile>) {
    let mut report = ExportReport::default();
    let mut written = Vec::new();
    let mut seals = HashSet::new();
    for pending in &plan.targets {
        if !targets::is_available(&pending.target) {
            report.targets_unavailable += 1;
            continue;
        }
        if pending.seals.is_empty() {
            continue;
        }
        match write_anchor_file(pending, plan.installation, plan.now) {
            Some(file) => {
                report.targets_written += 1;
                seals.extend(pending.seals.iter().map(|seal| (seal.stream, seal.seal_no)));
                written.push(file);
            }
            None => report.targets_failed += 1,
        }
    }
    report.seals_written = seals.len();
    (report, written)
}

/// The sealed streams and, per active target, the seals it has not received.
struct Plan {
    installation: InstallationId,
    now: UtcTimestamp,
    exported_by: Option<String>,
    targets: Vec<PendingTarget>,
}

struct PendingTarget {
    target: Target,
    /// In stream order, then seal order.
    seals: Vec<Seal>,
}

/// A written, not yet recorded, anchor file.
struct WrittenFile {
    id: AnchorExportId,
    target_id: TargetId,
    file_name: String,
    ranges: Vec<SealRange>,
}

fn plan(ctx: &mut CommandContext<'_>) -> Result<Plan, AnchorError> {
    let exported_by = match ctx.principal().actor() {
        Actor::User { user_id, .. } => Some(user_id.to_string()),
        Actor::Instrument { .. } | Actor::System { .. } => None,
    };
    if exported_by.is_some() {
        ctx.require(Capability::BackupRestore)?;
    }
    let installation = installation_id(ctx.tx())?.ok_or(AnchorError::NoInstallation)?;
    let now = ctx.now();
    lis_audit::seal(ctx.tx(), now)?;
    let mut pending = Vec::new();
    for target in targets::active(ctx.tx())? {
        let mut seals = Vec::new();
        for stream in Stream::ALL {
            let after = repo::last_exported_seal(ctx.tx(), target.id, stream)?.unwrap_or(0);
            seals.extend(lis_audit::seals(ctx.tx(), stream, after)?);
        }
        pending.push(PendingTarget { target, seals });
    }
    Ok(Plan {
        installation,
        now,
        exported_by,
        targets: pending,
    })
}

/// Writes one new anchor file to the target; `None`, logged, when a seal is refused as a
/// line or the file cannot be written.
fn write_anchor_file(
    pending: &PendingTarget,
    installation: InstallationId,
    now: UtcTimestamp,
) -> Option<WrittenFile> {
    let mut bytes = Vec::new();
    for seal in &pending.seals {
        let Ok(line) = lis_audit::to_line(seal, installation) else {
            tracing::error!(
                target_id = %pending.target.id,
                stream = seal.stream.as_str(),
                seal_no = seal.seal_no,
                "seal refused as an anchor line; nothing written to the target"
            );
            return None;
        };
        bytes.extend_from_slice(line.as_bytes());
        bytes.push(b'\n');
    }
    let id = AnchorExportId::generate();
    let (day, file_name) = file_name(now, id);
    let root = targets::root(&pending.target);
    let folder = root.join(ANCHORS_FOLDER).join(day);
    let result =
        create_folder(&root, &folder).and_then(|()| write_file(&folder.join(&file_name), &bytes));
    if let Err(error) = result {
        tracing::warn!(target_id = %pending.target.id, error_kind = ?error.kind(), "anchor file not written");
        return None;
    }
    Some(WrittenFile {
        id,
        target_id: pending.target.id,
        file_name,
        ranges: ranges(&pending.seals),
    })
}

/// `(<YYYY-MM-DD>, <YYYYMMDDTHHMMSSZ>-<exportId>.jsonl)` for a file written at `now`.
fn file_name(now: UtcTimestamp, id: AnchorExportId) -> (String, String) {
    let text = now.to_string();
    let day = text.chars().take(10).collect();
    let digits: String = text.chars().take(19).filter(char::is_ascii_digit).collect();
    let (date, time) = digits.split_at(digits.len().min(8));
    (day, format!("{date}T{time}Z-{id}.{ANCHOR_FILE_EXTENSION}"))
}

/// Creates the day folder and makes the new folders durable (Unix; see `durable`).
fn create_folder(root: &Path, folder: &Path) -> std::io::Result<()> {
    fs::create_dir_all(folder)?;
    lis_store::sync_dir(root)?;
    lis_store::sync_dir(&root.join(ANCHORS_FOLDER))
}

/// One range per stream; the seals of a stream are consecutive.
fn ranges(seals: &[Seal]) -> Vec<SealRange> {
    let mut ranges: Vec<SealRange> = Vec::new();
    for seal in seals {
        match ranges.last_mut() {
            Some(range) if range.stream == seal.stream => range.last_seal_no = seal.seal_no,
            _ => ranges.push(SealRange {
                stream: seal.stream,
                first_seal_no: seal.seal_no,
                last_seal_no: seal.seal_no,
            }),
        }
    }
    ranges
}

fn record(
    ctx: &mut CommandContext<'_>,
    plan: &Plan,
    written: &[WrittenFile],
    report: ExportReport,
) -> Result<(), AnchorError> {
    if plan.exported_by.is_some() {
        ctx.require(Capability::BackupRestore)?;
    }
    for file in written {
        let export = NewExport {
            id: file.id,
            target_id: file.target_id,
            file_name: &file.file_name,
            exported_at: plan.now,
            exported_by: plan.exported_by.as_deref(),
        };
        repo::insert_export(ctx.tx(), &export, &file.ranges)?;
    }
    let outcome = if report.targets_failed == 0 {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    };
    ctx.audit(
        AuditEvent::new(Stream::Security, EXPORT_ANCHORS, outcome)
            .detail(
                "targets_written",
                AuditDetail::count(report.targets_written),
            )
            .detail("seals_written", AuditDetail::count(report.seals_written))
            .detail("targets_failed", AuditDetail::count(report.targets_failed))
            .detail(
                "targets_unavailable",
                AuditDetail::count(report.targets_unavailable),
            ),
    )?;
    Ok(())
}
