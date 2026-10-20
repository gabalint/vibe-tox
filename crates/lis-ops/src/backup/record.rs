//! Recording a run's outcome: copy rows, heads, pruning, the run state and the
//! `ops.backup` event (M4 design brief §3.1 step 8), in one short transaction.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_store::CommandContext;

use super::repo::{self, CopyRecord, SnapshotRecord};
use super::run::{Copies, Produced, RunReport, Started};
use super::{BACKUP, BackupError, RunFailure, RunState, interrupted_event};

/// Records the outcome and audits `ops.backup` (§3.1 step 8).
pub(super) fn finish(
    ctx: &mut CommandContext<'_>,
    run: &Started,
    outcome: Produced,
) -> Result<RunReport, BackupError> {
    let mut report = RunReport {
        run_id: run.run_id,
        kind: run.kind,
        state: RunState::Failed,
        failure: None,
        copies_verified: 0,
        copies_failed: 0,
        artifact_problems: 0,
        sets_pruned: 0,
    };
    let copies = match outcome {
        Produced::Interrupted => {
            repo::interrupt(ctx.tx(), run.run_id, ctx.now())?;
            ctx.audit(interrupted_event(run.run_id, run.revision))?;
            report.state = RunState::Interrupted;
            return Ok(report);
        }
        Produced::Failed(failure) => {
            let revision = repo::finish(
                ctx.tx(),
                run.run_id,
                RunState::Failed,
                Some(failure),
                None,
                ctx.now(),
            )?;
            report.failure = Some(failure);
            ctx.audit(completion_event(run, &report, revision, None)?)?;
            return Ok(report);
        }
        Produced::Copied(copies) => copies,
    };
    record_copies(ctx, run, &copies, &mut report)?;
    report.state = if report.copies_verified > 0 {
        RunState::Succeeded
    } else {
        report.failure = Some(RunFailure::AllCopiesFailed);
        RunState::Failed
    };
    report.artifact_problems = copies.artifact_problems;
    let snapshot = SnapshotRecord {
        sha256: &copies.snapshot_sha256,
        bytes: copies.snapshot_bytes,
        artifact_count: copies.artifact_count,
        artifact_problems: copies.artifact_problems,
    };
    let revision = repo::finish(
        ctx.tx(),
        run.run_id,
        report.state,
        report.failure,
        Some(&snapshot),
        ctx.now(),
    )?;
    let event = completion_event(run, &report, revision, Some(&copies))?;
    ctx.audit(event)?;
    Ok(report)
}

fn record_copies(
    ctx: &CommandContext<'_>,
    run: &Started,
    copies: &Copies,
    report: &mut RunReport,
) -> Result<(), BackupError> {
    repo::insert_heads(ctx.tx(), run.run_id, &copies.heads)?;
    for copy in &copies.copies {
        let (set_id, failure, verified_at) = match copy.result {
            Ok(verified_at) => {
                report.copies_verified += 1;
                (Some(copies.set_id.as_str()), None, Some(verified_at))
            }
            Err(failure) => {
                report.copies_failed += 1;
                (None, Some(failure), None)
            }
        };
        let record = CopyRecord {
            target_id: copy.target_id,
            set_id,
            failure,
            written_bytes: copy.written_bytes,
            verified_at,
        };
        repo::insert_copy(ctx.tx(), run.run_id, &record)?;
    }
    for (target_id, set_ids) in &copies.pruned {
        for set_id in set_ids {
            repo::mark_pruned(ctx.tx(), *target_id, set_id, ctx.now())?;
        }
        report.sets_pruned += set_ids.len();
    }
    Ok(())
}

fn completion_event(
    run: &Started,
    report: &RunReport,
    revision: i64,
    copies: Option<&Copies>,
) -> Result<AuditEvent, BackupError> {
    let outcome = if report.state == RunState::Succeeded {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    };
    let mut event = AuditEvent::new(Stream::Security, BACKUP, outcome)
        .record("backup_run", run.run_id)
        .old_revision(revision - 1)
        .new_revision(revision)
        .detail("kind", AuditDetail::code(run.kind.as_str()))
        .detail("state", AuditDetail::code(report.state.as_str()))
        .detail(
            "copies_verified",
            AuditDetail::count(report.copies_verified),
        )
        .detail("copies_failed", AuditDetail::count(report.copies_failed))
        .detail(
            "artifact_problems",
            AuditDetail::count(report.artifact_problems),
        )
        .detail("sets_pruned", AuditDetail::count(report.sets_pruned));
    if let Some(failure) = report.failure {
        event = event.detail("failure_code", AuditDetail::code(failure.as_str()));
    }
    if let Some(requested_by) = &run.requested_by {
        event = event.detail("requested_by", AuditDetail::machine_token(requested_by)?);
    }
    if let Some(copies) = copies {
        event = event
            .detail(
                "snapshot_sha256",
                AuditDetail::sha256(&copies.snapshot_sha256)?,
            )
            .detail("prune_failures", AuditDetail::count(copies.prune_failures));
    }
    Ok(event)
}
