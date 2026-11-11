//! Key-confirmed downloaded updates retain package, authorization and backup gates
//! (REQ-OPS-006, REQ-IDN-004, NFR-19/24; ADR 0019/0020).

use super::*;
use lis_core::Clock;
use lis_engine::RecordUpdateApprovalInput;
use lis_ops::backup::RunId;
use rusqlite::params;
use sha2::{Digest, Sha256};
use std::path::PathBuf;

const PACKAGE: &[u8] = b"synthetic downloaded update package";

fn staged_package(f: &Fixture) -> PathBuf {
    let path = f.dir.path().join("synthetic-update-9.9.9.pkg");
    std::fs::write(&path, PACKAGE).unwrap();
    path
}

async fn approval_action(f: &Fixture, run: RunId) -> SecurityKeyActionInput {
    SecurityKeyActionInput {
        credential_id: f.key_id().await,
        action: SecurityKeyAction::ApproveDownloadedUpdate(RecordUpdateApprovalInput {
            version: "9.9.9".to_owned(),
            risk_class_text: "Development build".to_owned(),
            release_notes_reviewed: true,
            pre_update_run_id: run,
            notes: "Synthetic update approval".to_owned(),
            password: secret("").into(),
        }),
    }
}

/// Seed synthetic backup evidence through the tables' required state transitions.
/// No disk backup or hardware qualification is claimed by these ceremony tests.
fn verified_backup(f: &Fixture, finished: UtcTimestamp) -> RunId {
    let conn = f.conn();
    let run = RunId::generate();
    let target = RunId::generate().to_string();
    conn.execute(
        "INSERT INTO ops_targets (id, label, kind, purpose, path, created_by, \
         created_at, revision) VALUES (?1, 'Synthetic target', 'removable', \
         'backup_and_anchors', ?2, 'synthetic', ?3, 1)",
        params![target, format!("/synthetic/{target}"), finished],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ops_backup_runs (id, kind, state, requested_by, requested_at, revision) \
         VALUES (?1, 'pre_update', 'requested', 'synthetic', ?2, 1)",
        params![run, finished],
    )
    .unwrap();
    conn.execute(
        "UPDATE ops_backup_runs SET state = 'running', started_at = ?2, revision = 2 \
         WHERE id = ?1",
        params![run, finished],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ops_backup_copies (run_id, target_id, set_id, state, written_bytes, \
         verified_at) VALUES (?1, ?2, 'synthetic-set', 'succeeded', 10, ?3)",
        params![run, target, finished],
    )
    .unwrap();
    conn.execute(
        "UPDATE ops_backup_runs SET state = 'succeeded', finished_at = ?2, \
         snapshot_sha256 = ?3, revision = 3 WHERE id = ?1",
        params![run, finished, "a".repeat(64)],
    )
    .unwrap();
    run
}

fn assert_no_approval(f: &Fixture) {
    let count: i64 = f
        .conn()
        .query_row("SELECT count(*) FROM ops_update_approvals", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(count, 0);
}

#[tokio::test]
async fn downloaded_update_key_approval_uses_the_staged_package_and_audits_confirmation() {
    let (f, _) = Fixture::signed_in().await;
    let run = verified_backup(&f, f.clock.now());
    let value = f
        .engine
        .security_key_action(
            approval_action(&f, run).await,
            Some(staged_package(&f)),
            &f.key,
        )
        .await
        .unwrap();
    assert_eq!(value["version"], "9.9.9");
    assert_eq!(value["packageFileName"], "synthetic-update-9.9.9.pkg");
    assert_eq!(value["packageSha256"], hex::encode(Sha256::digest(PACKAGE)));
    assert_eq!(value["preUpdateRunId"], run.to_string());
    assert!(value.get("keyProof").is_none());
    assert_eq!(f.engine.list_update_approvals().await.unwrap().len(), 1);
    let confirmed: i64 = f
        .conn()
        .query_row(
            "SELECT count(*) FROM audit_events r JOIN audit_events a \
             ON a.correlation_id = r.correlation_id \
             WHERE r.action = 'auth.reauthenticate' AND r.outcome = 'succeeded' \
             AND json_extract(r.details, '$.purpose') = 'update_approval' \
             AND a.action = 'ops.approve_update' AND a.outcome = 'succeeded' \
             AND a.reason = 'Synthetic update approval'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(confirmed, 1);
}

struct WithoutVerification<'a>(&'a TestKey);
impl SecurityKeyClient for WithoutVerification<'_> {
    async fn register(&self, _: String, _: Value) -> Result<Value, CommandError> {
        panic!("update approval must not register a key")
    }

    async fn authenticate(
        &self,
        origin: String,
        mut options: Value,
    ) -> Result<Value, CommandError> {
        options["publicKey"]["userVerification"] = Value::String("discouraged".to_owned());
        self.0.authenticate(origin, options).await
    }
}

#[tokio::test]
async fn cancelled_or_unverified_key_cannot_approve_a_downloaded_update() {
    for cancel in [true, false] {
        let (f, _) = Fixture::signed_in().await;
        let run = verified_backup(&f, f.clock.now());
        let action = approval_action(&f, run).await;
        let package = Some(staged_package(&f));
        let result = if cancel {
            f.engine.security_key_action(action, package, &Cancel).await
        } else {
            f.engine
                .security_key_action(action, package, &WithoutVerification(&f.key))
                .await
        };
        assert!(result.is_err());
        assert_no_approval(&f);
    }
}

#[tokio::test]
async fn downloaded_update_approval_requires_capability_before_contacting_the_key() {
    let (f, _) = Fixture::signed_in().await;
    let action = approval_action(&f, RunId::generate()).await;
    f.conn()
        .execute(
            "UPDATE auth_role_grants SET revoked_at = ?1, revoked_by = user_id \
             WHERE revoked_at IS NULL",
            [f.clock.now()],
        )
        .unwrap();
    let error = f
        .engine
        .security_key_action(action, Some(staged_package(&f)), &NeverCalled)
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert_no_approval(&f);
}

#[tokio::test]
async fn missing_staged_package_is_refused_before_contacting_the_key() {
    let (f, _) = Fixture::signed_in().await;
    let error = f
        .engine
        .security_key_action(
            approval_action(&f, RunId::generate()).await,
            None,
            &NeverCalled,
        )
        .await
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    assert_no_approval(&f);
}

#[tokio::test]
async fn downloaded_update_key_approval_keeps_missing_stale_and_pruned_backup_gates() {
    for state in ["missing", "stale", "pruned"] {
        let (f, _) = Fixture::signed_in().await;
        let run = match state {
            "missing" => RunId::generate(),
            "stale" => verified_backup(
                &f,
                UtcTimestamp::parse("2026-10-13T08:00:00.000000000Z").unwrap(),
            ),
            _ => {
                let run = verified_backup(&f, f.clock.now());
                f.conn()
                    .execute(
                        "UPDATE ops_backup_copies SET pruned_at = ?2 WHERE run_id = ?1",
                        params![run, f.clock.now()],
                    )
                    .unwrap();
                run
            }
        };
        let error = f
            .engine
            .security_key_action(
                approval_action(&f, run).await,
                Some(staged_package(&f)),
                &f.key,
            )
            .await
            .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Conflict, "{state}");
        assert_no_approval(&f);
    }
}
