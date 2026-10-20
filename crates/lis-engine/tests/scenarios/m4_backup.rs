//! Backup-area command authorization and restore safety guards (REQ-OPS-002/003/007/010;
//! NFR-04/12/30). All values are synthetic.

use std::fs;
use std::path::{Path, PathBuf};

use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::{
    AddBackupTargetInput, BackupRunKind, BeginRestoreInput, ChecklistItem, CloseRecoveryInput,
    ConfirmRecoveryKitStorageInput, ConfirmRestoreInput, Engine, ExportRecoveryKitInput,
    LimitInput, ListAppRunsInput, ListBackupRunsInput, RaiseNumberFloorsInput, RequestBackupInput,
    RequestExitExportInput, RequestRestoreTestInput, RestoreId, RestoreSessionId,
    RestoreSessionInput, RetireBackupTargetInput, StageRestoreInput, TargetId, TargetInput,
    TargetKind, TargetPurpose,
};

use lis_ops::backup::RunId;
use lis_ops::targets;

use crate::fixture::{ADMIN, PASSWORD, TestEngine};

pub(crate) async fn backup_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = target_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = kit_and_restore_call(engine, name).await {
        return Some(result);
    }
    recovery_and_exit_call(engine, name).await
}

/// Backup targets, backups and restore tests.
async fn target_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let target_id = TargetId::generate();
    Some(match name {
        "list_backup_targets" => engine.list_backup_targets().await.map(drop),
        "add_backup_target" => engine
            .add_backup_target(
                AddBackupTargetInput {
                    label: "Synthetic backup".to_owned(),
                    kind: TargetKind::LocalDisk,
                    purpose: TargetPurpose::BackupAndAnchors,
                },
                None,
            )
            .await
            .map(drop),
        "retire_backup_target" => engine
            .retire_backup_target(RetireBackupTargetInput {
                target_id,
                expected_revision: 1,
                reason: "Synthetic test".to_owned(),
            })
            .await
            .map(drop),
        "get_backup_status" => engine.get_backup_status().await.map(drop),
        "request_backup" => engine
            .request_backup(RequestBackupInput {
                kind: BackupRunKind::Manual,
            })
            .await
            .map(drop),
        "list_backup_runs" => engine
            .list_backup_runs(ListBackupRunsInput {
                before_run_id: None,
                run_id: None,
                limit: 10,
                before_requested_at: None,
            })
            .await
            .map(drop),
        "list_backup_sets" => engine
            .list_backup_sets(TargetInput { target_id })
            .await
            .map(drop),
        "request_restore_test" => engine
            .request_restore_test(RequestRestoreTestInput {
                target_id,
                set_id: None,
            })
            .await
            .map(drop),
        "list_restore_tests" => engine
            .list_restore_tests(lis_engine::ListRestoreTestsInput {
                limit: 10,
                before_test_id: None,
                test_id: None,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// The recovery kit and the restore steps.
async fn kit_and_restore_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let restore_session_id = RestoreSessionId::generate();
    Some(match name {
        "get_recovery_kit_status" => engine.get_recovery_kit_status().await.map(drop),
        "export_recovery_kit" => engine
            .export_recovery_kit(
                ExportRecoveryKitInput {
                    disk_encryption_reference: "Synthetic disk encryption".to_owned(),
                    password: secrecy::SecretString::from(PASSWORD).into(),
                },
                None,
            )
            .await
            .map(drop),
        "confirm_recovery_kit_storage" => engine
            .confirm_recovery_kit_storage(ConfirmRecoveryKitStorageInput {
                kit_no: 1,
                note: "Synthetic off-computer storage".to_owned(),
            })
            .await
            .map(drop),
        "get_restore_availability" => engine.get_restore_availability().await.map(drop),
        "begin_restore" => engine
            .begin_restore(BeginRestoreInput {
                installation_key: "not-a-key".to_owned(),
            })
            .await
            .map(drop),
        "choose_restore_source" => engine
            .choose_restore_source(RestoreSessionInput { restore_session_id }, None)
            .await
            .map(drop),
        "stage_restore" => engine
            .stage_restore(StageRestoreInput {
                restore_session_id,
                set_id: "synthetic-set".to_owned(),
            })
            .await
            .map(drop),
        "check_restore_anchors" => engine
            .check_restore_anchors(RestoreSessionInput { restore_session_id }, None)
            .await
            .map(drop),
        "confirm_restore" => engine
            .confirm_restore(ConfirmRestoreInput {
                restore_session_id,
                confirmation: "RESTORE".to_owned(),
            })
            .await
            .map(drop),
        "cancel_restore" => {
            engine
                .cancel_restore(RestoreSessionInput { restore_session_id })
                .await
        }
        _ => return None,
    })
}

/// Recovery reconciliation, number floors and the exit export.
async fn recovery_and_exit_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "get_recovery_reconciliation" => engine.get_recovery_reconciliation().await.map(drop),
        "close_recovery_reconciliation" => {
            engine
                .close_recovery_reconciliation(CloseRecoveryInput {
                    restore_id: RestoreId::generate(),
                    attested_items: ChecklistItem::ALL.to_vec(),
                    note: "Synthetic reconciliation".to_owned(),
                    password: secrecy::SecretString::from(PASSWORD).into(),
                })
                .await
        }
        "raise_number_floors" => engine
            .raise_number_floors(RaiseNumberFloorsInput {
                accession_floor: Some("A26001000".to_owned()),
                container_floor: Some("C00001000".to_owned()),
                reason: "Synthetic physical labels".to_owned(),
            })
            .await
            .map(drop),
        "request_exit_export" => engine
            .request_exit_export(
                RequestExitExportInput {
                    reason: "Synthetic exit test".to_owned(),
                    password: secrecy::SecretString::from(PASSWORD).into(),
                },
                None,
            )
            .await
            .map(drop),
        "list_exit_exports" => engine
            .list_exit_exports(LimitInput { limit: 10 })
            .await
            .map(drop),
        _ => return None,
    })
}

#[tokio::test]
async fn every_restore_step_refuses_an_installation_with_users() {
    let test = TestEngine::start().await;
    test.bootstrap().await;
    let session = RestoreSessionInput {
        restore_session_id: RestoreSessionId::generate(),
    };

    let results = [
        test.engine
            .begin_restore(BeginRestoreInput {
                installation_key: "not-a-key".to_owned(),
            })
            .await
            .map(drop),
        test.engine
            .choose_restore_source(session, None)
            .await
            .map(drop),
        test.engine
            .stage_restore(StageRestoreInput {
                restore_session_id: session.restore_session_id,
                set_id: "synthetic-set".to_owned(),
            })
            .await
            .map(drop),
        test.engine
            .check_restore_anchors(session, None)
            .await
            .map(drop),
        test.engine
            .confirm_restore(ConfirmRestoreInput {
                restore_session_id: session.restore_session_id,
                confirmation: "RESTORE".to_owned(),
            })
            .await
            .map(drop),
        test.engine.cancel_restore(session).await,
    ];

    for result in results {
        assert_eq!(result.unwrap_err().code(), ErrorCode::Conflict);
    }
}

/// The folder the app owns inside the target's chosen folder.
pub(crate) fn target_root(test: &TestEngine, target_id: TargetId) -> PathBuf {
    let target = targets::active(&test.conn())
        .unwrap()
        .into_iter()
        .find(|target| target.id == target_id)
        .unwrap();
    targets::root(&target)
}

fn names(folder: &Path) -> Vec<String> {
    match fs::read_dir(folder) {
        Ok(entries) => entries
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect(),
        Err(_) => Vec::new(),
    }
}

/// Removes every copy row, as a power loss between a set's final rename and its copy
/// row's commit leaves the installation: the set is published but unrecorded.
fn forget_copies(test: &TestEngine) {
    let conn = test.conn();
    let trigger: String = conn
        .query_row(
            "SELECT sql FROM sqlite_schema WHERE name = 'ops_backup_copies_no_delete'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    conn.execute_batch("DROP TRIGGER ops_backup_copies_no_delete; DELETE FROM ops_backup_copies;")
        .unwrap();
    conn.execute_batch(&trigger).unwrap();
}

/// What a crash in the middle of a backup leaves behind.
struct Crashed {
    target_id: TargetId,
    root: PathBuf,
    run_id: RunId,
    unrecorded_set: String,
}

/// One backup whose set is published but unrecorded, then a second backup that is
/// running, with a half-written `.tmp` set and a staged snapshot, when the engine stops.
async fn crash_mid_backup(test: &TestEngine, folder: &Path) -> Crashed {
    let target = test
        .engine
        .add_backup_target(
            AddBackupTargetInput {
                label: "Synthetic backup folder".to_owned(),
                kind: TargetKind::LocalDisk,
                purpose: TargetPurpose::BackupAndAnchors,
            },
            Some(folder.to_owned()),
        )
        .await
        .unwrap()
        .unwrap();
    let manual = || RequestBackupInput {
        kind: BackupRunKind::Manual,
    };
    test.engine.request_backup(manual()).await.unwrap();
    test.engine.run_operations_pass().await;
    let root = target_root(test, target.target_id);
    let mut published = names(&root.join("sets"));
    assert_eq!(published.len(), 1);
    forget_copies(test);

    let run = test.engine.request_backup(manual()).await.unwrap();
    test.conn()
        .execute(
            "UPDATE ops_backup_runs SET state = 'running', started_at = requested_at, \
             revision = revision + 1 WHERE id = ?1",
            [run.run_id.to_string()],
        )
        .unwrap();
    let half_set = root.join("sets").join(HALF_SET);
    fs::create_dir_all(&half_set).unwrap();
    fs::write(half_set.join("database.vtbk"), b"partial").unwrap();
    let staging = test.data_dir().join("ops/staging");
    fs::create_dir_all(&staging).unwrap();
    fs::write(staging.join("snapshot.sqlite3"), b"partial").unwrap();
    Crashed {
        target_id: target.target_id,
        root,
        run_id: run.run_id,
        unrecorded_set: published.remove(0),
    }
}

/// A set folder still being written when the engine stopped.
const HALF_SET: &str = "20261012T080000Z-0000abcd.tmp";

/// M4 exit scenario 5 with the startup reconciliation (REQ-OPS-007, NFR-04; M4 brief
/// §6.4 step 8; review finding M5): the engine stops without shutdown while a backup
/// runs, after an earlier set was published but never recorded. At the next start the
/// run is `interrupted`, the stop is unclean, and the unrecorded set is in quarantine
/// before anything lists it; the `.tmp` set is never listed. The next backup clears the
/// staged snapshot and the `.tmp` set and lists only its own set.
#[tokio::test]
async fn a_crash_during_a_backup_leaves_no_usable_set_and_the_next_start_reconciles() {
    let test = TestEngine::start().await;
    let totp = test.admin_session().await;
    let folder = tempfile::tempdir().unwrap();
    let crashed = crash_mid_backup(&test, folder.path()).await;

    let test = test.crash_and_restart().await;
    test.sign_in_with(ADMIN, PASSWORD, &totp).await;
    assert!(
        !test.data_dir().join("ops/staging").exists(),
        "startup removes plaintext staging before another backup"
    );
    let first_page = test
        .engine
        .list_backup_runs(ListBackupRunsInput {
            before_run_id: None,
            run_id: None,
            limit: 1,
            before_requested_at: None,
        })
        .await
        .unwrap();
    assert_eq!(first_page.len(), 2, "the extra row proves more runs exist");
    let runs = || {
        test.engine.list_backup_runs(ListBackupRunsInput {
            before_run_id: None,
            run_id: None,
            limit: 10,
            before_requested_at: None,
        })
    };
    let listed = runs().await.unwrap();
    let run = listed.iter().find(|run| run.run_id == crashed.run_id);
    assert_eq!(run.unwrap().state, "interrupted");
    let app_runs = test
        .engine
        .list_app_runs(ListAppRunsInput {
            run_id: None,
            before_run_id: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert!(app_runs[0].previous_unclean);
    let sets = || {
        test.engine.list_backup_sets(TargetInput {
            target_id: crashed.target_id,
        })
    };
    assert!(sets().await.unwrap().is_empty());
    let quarantined = names(&crashed.root.join("quarantine"));
    assert_eq!(quarantined.len(), 1);
    let unrecorded = format!("unrecorded-{}", crashed.unrecorded_set);
    assert!(quarantined[0].starts_with(&unrecorded), "{quarantined:?}");

    test.engine
        .request_backup(RequestBackupInput {
            kind: BackupRunKind::Manual,
        })
        .await
        .unwrap();
    test.engine.run_operations_pass().await;
    assert_eq!(runs().await.unwrap()[0].state, "succeeded");
    assert!(names(&test.data_dir().join("ops/staging")).is_empty());
    let sets = sets().await.unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(names(&crashed.root.join("sets")), [sets[0].set_id.clone()]);
}
