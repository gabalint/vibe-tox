use lis_engine::SecurityKeyClient;

use super::*;

#[test]
fn boundary_error_logs_the_returned_reference_without_message_data() {
    let log = tempfile::NamedTempFile::new().unwrap();
    let subscriber = tracing_subscriber::fmt()
        .without_time()
        .with_ansi(false)
        .with_writer(Mutex::new(log.reopen().unwrap()))
        .finish();
    // Match the application's global logger. A thread-local capture can miss the
    // first callsite registration while other tests invoke this same helper.
    tracing::subscriber::set_global_default(subscriber).unwrap();
    let error = boundary_error(ErrorCode::Validation, "synthetic-sensitive-message");
    let serialized = serde_json::to_value(error).unwrap();
    let captured = std::fs::read_to_string(log.path()).unwrap();
    assert!(
        captured.contains(serialized["correlationId"].as_str().unwrap()),
        "diagnostic output did not contain the returned reference: {captured}"
    );
    assert!(captured.contains("validation"));
    assert!(!captured.contains("synthetic-sensitive-message"));
}

#[test]
fn stale_operation_id_cannot_cancel_or_receive_pin_for_a_new_operation() {
    let service = SecurityKeyOperations::default();
    let old = service.begin().unwrap();
    let stale_id = old.id.clone();
    drop(old);
    let active = service.begin().unwrap();
    assert!(
        service
            .cancel(&SecurityKeyOperationInput {
                operation_id: stale_id.clone()
            })
            .is_err()
    );
    assert!(
        service
            .submit_pin(SecurityKeyPinInput {
                operation_id: stale_id,
                pin: "synthetic".into()
            })
            .is_err()
    );
    assert!(active.seal().is_ok());
}

#[test]
fn cancellation_before_commit_wins_and_after_commit_is_refused() {
    let service = SecurityKeyOperations::default();
    let cancelled = service.begin().unwrap();
    service
        .cancel(&SecurityKeyOperationInput {
            operation_id: cancelled.id.clone(),
        })
        .unwrap();
    assert!(cancelled.finish().is_err());
    assert!(cancelled.start_native().is_err());
    drop(cancelled);
    let committed = service.begin().unwrap();
    committed.finish().unwrap();
    assert!(
        service
            .cancel(&SecurityKeyOperationInput {
                operation_id: committed.id.clone()
            })
            .is_err()
    );
}

#[test]
fn reservation_and_native_handle_release_on_error_or_drop() {
    let service = SecurityKeyOperations::default();
    let operation = service.begin().unwrap();
    assert!(service.begin().is_err());
    let _interaction = operation.start_native().unwrap();
    assert!(operation.start_native().is_err());
    let control = service
        .0
        .lock()
        .unwrap()
        .as_ref()
        .unwrap()
        .native
        .as_ref()
        .unwrap()
        .0
        .clone();
    drop(operation);
    assert_eq!(
        control.submit_pin("synthetic".into()),
        Err(SecurityKeyError::Cancelled)
    );
    assert!(service.status().unwrap().is_none());
    assert!(service.begin().is_ok());
}

#[tokio::test]
async fn spare_registration_waits_for_explicit_continuation_and_cancel_interrupts_it() {
    let service = SecurityKeyOperations::default();
    let operation = service.begin().unwrap();
    operation.authenticated().unwrap();
    let wait = operation.await_replacement();
    tokio::pin!(wait);
    tokio::select! {
        _ = &mut wait => panic!("replacement must require confirmation"),
        () = tokio::task::yield_now() => {}
    }
    let earlier_phase_id = service.status().unwrap().unwrap().operation_id;
    service
        .continue_registration(&SecurityKeyOperationInput {
            operation_id: earlier_phase_id.clone(),
        })
        .unwrap();
    wait.await.unwrap();

    let wait = operation.await_replacement();
    tokio::pin!(wait);
    tokio::select! {
        _ = &mut wait => panic!("replacement must require confirmation"),
        () = tokio::task::yield_now() => {}
    }
    assert!(
        service
            .cancel(&SecurityKeyOperationInput {
                operation_id: earlier_phase_id.clone()
            })
            .is_err()
    );
    assert!(
        service
            .submit_pin(SecurityKeyPinInput {
                operation_id: earlier_phase_id,
                pin: "stale-pin".into()
            })
            .is_err()
    );
    service
        .cancel(&SecurityKeyOperationInput {
            operation_id: service.status().unwrap().unwrap().operation_id,
        })
        .unwrap();
    assert!(wait.await.is_err());
}
