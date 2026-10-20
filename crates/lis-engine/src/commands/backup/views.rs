//! The backup screens' views of targets, runs, sets and restore tests.

use lis_core::UtcTimestamp;
use lis_ops::backup::{RunRecord, TargetRecord};
use lis_ops::restore::RestoreTestRecord;

use super::{
    BackupRunView, BackupSetView, CopyView, FindingView, RestoreTestView, TargetFailureView,
    TargetView,
};

pub(super) fn target_available(record: &TargetRecord) -> bool {
    !record.retired && lis_ops::targets::is_available(&record.target)
}

pub(super) fn target_view(record: TargetRecord) -> TargetView {
    let available = target_available(&record);
    let mut allowed_actions = Vec::new();
    if !record.retired {
        allowed_actions.push("retire_backup_target".to_owned());
        if available && record.target.purpose.takes_backups() {
            allowed_actions.push("request_restore_test".to_owned());
        }
    }
    TargetView {
        target_id: record.target.id,
        label: record.target.label,
        kind: record.target.kind,
        purpose: record.target.purpose,
        location: record.target.path.display().to_string(),
        available,
        last_verified_at: record.last_verified_at,
        last_failure: record.last_failure.map(|(code, at)| TargetFailureView {
            code: code.as_str().to_owned(),
            at,
        }),
        retired: record.retired,
        revision: record.target.revision,
        allowed_actions,
    }
}

pub(super) fn run_view(run: RunRecord, now: UtcTimestamp) -> BackupRunView {
    let update_approval_eligible = lis_ops::updates::update_approval_eligible(&run, now);
    BackupRunView {
        update_approval_eligible,
        run_id: run.run_id,
        kind: run.kind.as_str().to_owned(),
        state: run.state.as_str().to_owned(),
        requested_by: run.requested_by,
        requested_at: run.requested_at,
        started_at: run.started_at,
        finished_at: run.finished_at,
        database_bytes: run.database_bytes,
        artifact_count: run.artifact_count,
        artifact_problems: run.artifact_problems,
        failure_code: run.failure.map(|failure| failure.as_str().to_owned()),
        copies: run
            .copies
            .into_iter()
            .map(|copy| CopyView {
                target_id: copy.target_id,
                target_label: copy.target_label,
                set_id: copy.set_id,
                state: if copy.succeeded {
                    "succeeded"
                } else {
                    "failed"
                }
                .to_owned(),
                failure_code: copy.failure.map(|failure| failure.as_str().to_owned()),
                verified_at: copy.verified_at,
                pruned: copy.pruned,
            })
            .collect(),
    }
}

pub(super) fn set_view(set: lis_ops::restore::SetSummary) -> BackupSetView {
    BackupSetView {
        set_id: set.set_id,
        created_at: set.created_at,
        kind: set.kind.as_str().to_owned(),
        app_version: set.app_version,
        database_bytes: set.database_bytes,
        artifact_count: set.artifact_count,
        compatible: set.incompatible.is_none(),
        incompatible_reason: set.incompatible.map(|reason| reason.as_str().to_owned()),
    }
}

pub(super) fn restore_test_view(test: RestoreTestRecord) -> RestoreTestView {
    let passed = test.passed();
    RestoreTestView {
        test_id: test.test_id,
        target_label: test.target_label,
        set_id: test.set_id,
        set_created_at: test.set_created_at,
        state: test.state.as_str().to_owned(),
        requested_by: test.requested_by,
        requested_at: test.requested_at,
        started_at: test.started_at,
        finished_at: test.finished_at,
        elapsed_ms: test.elapsed_ms,
        failure_code: test.failure.map(|failure| failure.as_str().to_owned()),
        anchors: test.anchors,
        findings: test
            .findings
            .list()
            .iter()
            .map(|finding| FindingView {
                code: finding.code.as_str().to_owned(),
                count: finding.count,
            })
            .collect(),
        passed,
    }
}
