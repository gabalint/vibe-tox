//! The preflight the desktop shell runs before a native dialog (M8; M4 D10): no session,
//! a locked session, a missing capability and a restore onto an installation in use are
//! refused before any dialog opens. Synthetic data only.

use lis_core::ErrorCode;
use lis_engine::{BeginRestoreInput, DialogCommand, KeyProvider};
use lis_ops::restore::RestoreSessionId;

use crate::fixture::code_of;
use crate::m4_runtime::{Host, key};

const SIGNED_IN: [DialogCommand; 5] = [
    DialogCommand::AddBackupTarget,
    DialogCommand::ExportRecoveryKit,
    DialogCommand::RequestExitExport,
    DialogCommand::VerifyAuditAnchors,
    DialogCommand::RecordUpdateApproval,
];

// M8: without a session, and with a locked one, no dialog of a signed-in command opens.
#[tokio::test]
async fn dialogs_need_an_unlocked_session_with_the_capability() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    for command in SIGNED_IN {
        assert_eq!(
            code_of(engine.preflight_dialog(command).await),
            ErrorCode::Unauthenticated,
            "{command:?}"
        );
    }
    host.enrol_admin(&engine).await;
    engine
        .preflight_dialog(DialogCommand::AddBackupTarget)
        .await
        .unwrap();
    engine
        .preflight_dialog(DialogCommand::RecordUpdateApproval)
        .await
        .unwrap();
    engine.lock_session().await.unwrap();
    assert_eq!(
        code_of(
            engine
                .preflight_dialog(DialogCommand::AddBackupTarget)
                .await
        ),
        ErrorCode::SessionLocked
    );
}

// M8, REQ-OPS-002: the restore folder dialogs open only on a new installation, for a live
// restore session, and the anchor check only once a set is staged.
#[tokio::test]
async fn restore_dialogs_need_a_new_installation_and_a_live_session() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let unknown = RestoreSessionId::generate();
    assert_eq!(
        code_of(
            engine
                .preflight_dialog(DialogCommand::ChooseRestoreSource(unknown))
                .await
        ),
        ErrorCode::Conflict
    );
    let session = engine
        .begin_restore(BeginRestoreInput {
            installation_key: lis_ops::kit::key_text(&key(2)).as_str().to_owned(),
        })
        .await
        .unwrap()
        .restore_session_id;
    engine
        .preflight_dialog(DialogCommand::ChooseRestoreSource(session))
        .await
        .unwrap();
    assert_eq!(
        code_of(
            engine
                .preflight_dialog(DialogCommand::CheckRestoreAnchors(session))
                .await
        ),
        ErrorCode::Conflict
    );

    host.enrol_admin(&engine).await;
    assert_eq!(
        code_of(
            engine
                .preflight_dialog(DialogCommand::ChooseRestoreSource(session))
                .await
        ),
        ErrorCode::Conflict
    );
}
