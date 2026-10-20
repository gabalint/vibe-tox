//! Restore tests (REQ-OPS-007, NFR-30; M4 design brief §3.5, §3.6): a requested test
//! stages one target's set into `ops/restore-tests/<id>/`, records the elapsed time, the
//! anchor comparison and the findings, and deletes the scratch folder. Live data is never
//! touched. The job runner calls [`run_restore_test`] inside `spawn_blocking`; long file
//! I/O runs between two short write transactions.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::AtomicBool;
use std::time::Instant;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Capability, Clock, CorrelationId, FieldError, Id, ModuleSchema, Principal};
use lis_store::{CommandContext, Database};

use super::findings::Findings;
use super::stage::{StageInput, StagedRestore, stage};
use super::test_repo::{self as repo, Finished};
use crate::backup::{BackupError, JOBS_COMPONENT};
use crate::format::{BackupKey, FormatError};
use crate::targets::{self, Target, TargetId};

/// Audit action of a restore-test request.
pub const REQUEST_RESTORE_TEST: &str = "ops.request_restore_test";
/// Audit action of a test's completion: passed, failed or interrupted.
pub const RESTORE_TEST: &str = "ops.restore_test";
/// Scratch folders of restore tests, inside the data folder.
pub(super) const TESTS_DIR: [&str; 2] = ["ops", "restore-tests"];
const SET_ID_MAX_CHARS: usize = 64;

/// Marker type of restore test IDs.
pub enum RestoreTestRun {}
/// Identifies a restore test.
pub type RestoreTestId = Id<RestoreTestRun>;

lis_core::text_enum! {
    /// Where a restore test stands (M4 design brief §2.3).
    TestState {
        /// Waiting for the job runner.
        Requested = "requested",
        /// Being checked.
        Running = "running",
        /// No finding other than the lost interval.
        Passed = "passed",
        /// A finding, or the test could not run; see the failure code.
        Failed = "failed",
        /// Stopped by shutdown or a crash.
        Interrupted = "interrupted",
    }
}

lis_core::text_enum! {
    /// Why a restore test could not check its set.
    TestFailure {
        /// The installation key is not loaded.
        KeyUnavailable = "key_unavailable",
        /// The target folder is not reachable or holds no marker naming it.
        TargetUnavailable = "target_unavailable",
        /// The requested set, or any set, is not on the target.
        SetNotFound = "set_not_found",
        /// The scratch folder or the scratch database failed.
        ScratchFailed = "scratch_failed",
    }
}

/// Queues a restore test of `target_id`'s set `set_id`, or of its newest set
/// (`ops.request_restore_test`). Refused without an active target that takes backups,
/// or while another test waits.
pub fn request_restore_test(
    ctx: &mut CommandContext<'_>,
    target_id: TargetId,
    set_id: Option<&str>,
) -> Result<RestoreTestId, BackupError> {
    ctx.require(Capability::BackupRestore)?;
    let set_id = set_id
        .map(|text| text.trim())
        .filter(|text| !text.is_empty());
    if set_id.is_some_and(|text| text.chars().count() > SET_ID_MAX_CHARS) {
        return Err(BackupError::Invalid(FieldError {
            field: "setId".to_owned(),
            code: "too_long".to_owned(),
            message: "This is not a backup set name.".to_owned(),
        }));
    }
    let target = targets::active(ctx.tx())?
        .into_iter()
        .find(|target| target.id == target_id && target.purpose.takes_backups())
        .ok_or(BackupError::NotFound("backup target"))?;
    if repo::oldest_request(ctx.tx())?.is_some() {
        return Err(BackupError::RestoreTestQueued);
    }
    let test_id = RestoreTestId::generate();
    let requested_by = ctx.principal().actor().id_text();
    repo::insert_request(
        ctx.tx(),
        test_id,
        target.id,
        set_id,
        &requested_by,
        ctx.now(),
    )?;
    let mut event = AuditEvent::new(Stream::Security, REQUEST_RESTORE_TEST, Outcome::Succeeded)
        .record("restore_test", test_id)
        .new_revision(1)
        .detail("target_id", AuditDetail::id(target.id));
    if let Some(set_id) = set_id {
        event = event.detail("set_id", set_id_detail(set_id)?);
    }
    ctx.audit(event)?;
    Ok(test_id)
}

/// What the job runner hands a restore test.
pub struct RestoreTestJob<'a> {
    /// The live database, only written to record the test.
    pub db: &'a Database,
    /// The data folder; the scratch folder is created inside it.
    pub data_dir: &'a Path,
    /// The backup key, `None` when the installation key is not loaded.
    pub key: Option<&'a BackupKey>,
    /// This build's schema modules (the engine's `MODULES`).
    pub modules: &'a [ModuleSchema],
    /// Every column holding artifact digests, as `(table, column)`.
    pub artifact_columns: &'a [(&'a str, &'a str)],
    /// The clock the scratch database opens with.
    pub clock: Arc<dyn Clock>,
    /// Raised by shutdown; the test then ends `interrupted`.
    pub cancel: &'a AtomicBool,
}

/// How a restore test ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreTestReport {
    /// The test.
    pub test_id: RestoreTestId,
    /// `passed`, `failed` or `interrupted`.
    pub state: TestState,
    /// Why it could not check its set.
    pub failure: Option<TestFailure>,
    /// What it found.
    pub findings: Findings,
}

/// Runs the oldest waiting restore test, if any, and records it (`ops.restore_test`,
/// system principal `ops-jobs`). The scratch folder is deleted whatever happens.
pub fn run_restore_test(
    job: &RestoreTestJob<'_>,
) -> Result<Option<RestoreTestReport>, BackupError> {
    let correlation = CorrelationId::generate();
    let Some(started) = job.db.write_blocking(jobs(), correlation, start)? else {
        return Ok(None);
    };
    let begun = Instant::now();
    let outcome = check(job, &started);
    let elapsed_ms = u64::try_from(begun.elapsed().as_millis()).unwrap_or(u64::MAX);
    let report = job.db.write_blocking(jobs(), correlation, move |ctx| {
        record(ctx, &started, outcome, elapsed_ms)
    })?;
    tracing::info!(
        test_id = %report.test_id,
        state = report.state.as_str(),
        failure = report.failure.map(TestFailure::as_str),
        findings = report.findings.list().len(),
        "restore test finished"
    );
    Ok(Some(report))
}

fn jobs() -> Principal {
    Principal::system(JOBS_COMPONENT, [])
}

/// A test moved to `running`, with its target.
struct Started {
    test_id: RestoreTestId,
    set_id: Option<String>,
    revision: i64,
    target: Option<Target>,
    others: Vec<Target>,
}

fn start(ctx: &mut CommandContext<'_>) -> Result<Option<Started>, BackupError> {
    let Some(request) = repo::oldest_request(ctx.tx())? else {
        return Ok(None);
    };
    let revision = repo::start(ctx.tx(), request.id, ctx.now())?;
    let (target, others): (Vec<Target>, Vec<Target>) = targets::active(ctx.tx())?
        .into_iter()
        .partition(|target| target.id == request.target_id);
    Ok(Some(Started {
        test_id: request.id,
        set_id: request.set_id,
        revision,
        target: target.into_iter().next(),
        others,
    }))
}

/// What checking produced.
enum Checked {
    Staged(Box<StagedRestore>),
    Failed(TestFailure),
    Interrupted,
}

/// Stages into the scratch folder and removes it again.
fn check(job: &RestoreTestJob<'_>, started: &Started) -> Checked {
    let Some(key) = job.key else {
        return Checked::Failed(TestFailure::KeyUnavailable);
    };
    let Some(target) = started
        .target
        .as_ref()
        .filter(|target| targets::is_available(target))
    else {
        return Checked::Failed(TestFailure::TargetUnavailable);
    };
    let scratch = TESTS_DIR
        .iter()
        .fold(job.data_dir.to_path_buf(), |path, part| path.join(part))
        .join(started.test_id.to_string());
    let anchor_roots: Vec<PathBuf> = started
        .others
        .iter()
        .filter(|other| targets::is_available(other))
        .map(targets::root)
        .collect();
    let input = StageInput {
        source_root: &targets::root(target),
        set_id: started.set_id.as_deref(),
        key,
        scratch_dir: &scratch,
        modules: job.modules,
        artifact_columns: job.artifact_columns,
        anchor_roots: &anchor_roots,
        keep_artifacts: false,
        clock: Arc::clone(&job.clock),
        cancel: job.cancel,
    };
    let staged = stage(&input);
    if let Err(error) = std::fs::remove_dir_all(&scratch) {
        tracing::warn!(test_id = %started.test_id, error_kind = ?error.kind(), "restore test scratch not removed");
    }
    match staged {
        Ok(staged) => Checked::Staged(Box::new(staged)),
        Err(BackupError::Format(FormatError::Cancelled)) => Checked::Interrupted,
        Err(BackupError::NotFound(_)) => Checked::Failed(TestFailure::SetNotFound),
        Err(error) => {
            tracing::warn!(test_id = %started.test_id, error = %error, "restore test could not run");
            let gone = !targets::is_available(target);
            Checked::Failed(if gone {
                TestFailure::TargetUnavailable
            } else {
                TestFailure::ScratchFailed
            })
        }
    }
}

fn record(
    ctx: &mut CommandContext<'_>,
    started: &Started,
    checked: Checked,
    elapsed_ms: u64,
) -> Result<RestoreTestReport, BackupError> {
    let (state, failure, staged) = match checked {
        Checked::Interrupted => (TestState::Interrupted, None, None),
        Checked::Failed(failure) => (TestState::Failed, Some(failure), None),
        Checked::Staged(staged) if staged.findings.passed() => {
            (TestState::Passed, None, Some(staged))
        }
        Checked::Staged(staged) => (TestState::Failed, None, Some(staged)),
    };
    let findings = staged
        .as_ref()
        .map(|staged| staged.findings.clone())
        .unwrap_or_default();
    for finding in findings.list() {
        repo::insert_finding(ctx.tx(), started.test_id, finding.code, finding.count)?;
    }
    let finished = Finished {
        state,
        failure,
        staged: staged.as_deref(),
        elapsed_ms,
    };
    let revision = repo::finish(ctx.tx(), started.test_id, &finished, ctx.now())?;
    ctx.audit(completion_event(started, &finished, revision, &findings))?;
    Ok(RestoreTestReport {
        test_id: started.test_id,
        state,
        failure,
        findings,
    })
}

fn completion_event(
    started: &Started,
    finished: &Finished<'_>,
    revision: i64,
    findings: &Findings,
) -> AuditEvent {
    let outcome = if finished.state == TestState::Passed {
        Outcome::Succeeded
    } else {
        Outcome::Failed
    };
    let codes: Vec<&'static str> = findings
        .list()
        .iter()
        .map(|finding| finding.code.as_str())
        .collect();
    let mut event = AuditEvent::new(Stream::Security, RESTORE_TEST, outcome)
        .record("restore_test", started.test_id)
        .old_revision(started.revision)
        .new_revision(revision)
        .detail("state", AuditDetail::code(finished.state.as_str()))
        .detail("findings", AuditDetail::codes(codes))
        .detail("elapsed_ms", AuditDetail::unsigned(finished.elapsed_ms));
    if let Some(failure) = finished.failure {
        event = event.detail("failure_code", AuditDetail::code(failure.as_str()));
    }
    if let Some(anchors) = finished.staged.and_then(|staged| staged.anchors) {
        event = event.detail("anchors_compared", AuditDetail::unsigned(anchors.compared));
    }
    event
}

/// A set name as an audit detail: machine tokens are lowercase.
pub(super) fn set_id_detail(set_id: &str) -> Result<AuditDetail, lis_audit::AuditError> {
    AuditDetail::machine_token(&set_id.to_ascii_lowercase())
}

/// The audit event of a test a stop or crash left `requested` or `running`.
pub(crate) fn interrupted_event(test_id: RestoreTestId, revision: i64) -> AuditEvent {
    AuditEvent::new(Stream::Security, RESTORE_TEST, Outcome::Failed)
        .record("restore_test", test_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .detail("state", AuditDetail::code(TestState::Interrupted.as_str()))
}
