//! The installation key around encrypted data (§3.9, NFR-19): a key is never created
//! over a database that holds a recovery kit or a backup target, a restored key must
//! belong to the data, and a key restore whose completion could not be audited is
//! completed at the next start (M7). Synthetic data only.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{CorrelationId, Principal};
use lis_engine::{Engine, KeyProvider, KeyState, MemoryKeyStore, RestoreInstallationKeyInput};
use lis_ops::format::BackupKey;
use lis_store::{Database, StoreError};

use crate::m4_anchors::MODULES;
use crate::m4_runtime::{Host, json, key, key_line, key_state};

/// Starts and stops the engine once with `store`, so the database exists.
async fn start_once(host: &Host, store: &MemoryKeyStore) {
    let engine = host.start(KeyProvider::Memory(store.clone())).await;
    stop(engine).await;
}

async fn stop(engine: Engine) {
    engine.shutdown().await;
    drop(engine);
}

/// A recovery kit recorded for the key `byte` (the kit export itself is the backup area's).
fn record_kit(host: &Host, byte: u8) {
    let key_id = BackupKey::derive(&key(byte)).key_id().to_string();
    host.conn()
        .execute(
            "INSERT INTO ops_recovery_kits (id, kit_no, key_id, targets_digest, \
             disk_encryption_reference, file_name, exported_by, exported_at) VALUES \
             ('01890000-0000-7000-8000-0000000000a1', 1, ?1, ?2, 'Synthetic reference', \
             'kit.txt', 'synthetic-user', '2026-10-13T00:00:00.000000000Z')",
            rusqlite::params![key_id, "b".repeat(64)],
        )
        .unwrap();
}

async fn restore(engine: &Engine, byte: u8) -> Result<KeyState, lis_core::CommandError> {
    engine
        .restore_installation_key(RestoreInstallationKeyInput {
            installation_key: key_line(&key(byte)),
        })
        .await
        .map(|view| view.key_state)
}

// Never a new key over encrypted data: a kit's key ID makes the key `missing`, and only
// the kit's key restores it.
#[tokio::test]
async fn a_recorded_kit_prevents_a_new_key_and_only_its_key_restores() {
    let host = Host::new();
    start_once(&host, &MemoryKeyStore::new(Some(key(1)))).await;
    record_kit(&host, 1);

    let empty = MemoryKeyStore::new(None);
    let engine = host.start(KeyProvider::Memory(empty.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Missing);
    assert!(empty.key().is_none(), "no new key over a recorded kit");
    let foreign = restore(&engine, 2).await.unwrap_err();
    assert_eq!(json(&foreign)["fieldErrors"][0]["code"], "does_not_open");
    assert!(empty.key().is_none());
    assert_eq!(restore(&engine, 1).await.unwrap(), KeyState::Ok);
    assert_eq!(empty.key().unwrap().expose(), key(1).expose());
    stop(engine).await;

    let engine = host
        .start(KeyProvider::Memory(MemoryKeyStore::new(Some(key(3)))))
        .await;
    assert_eq!(key_state(&engine).await, KeyState::Mismatch);
}

// Never a new key over encrypted data: a backup target's marker and sets carry the key ID.
#[tokio::test]
async fn a_backup_target_prevents_a_new_key() {
    let host = Host::new();
    start_once(&host, &MemoryKeyStore::new(Some(key(1)))).await;
    host.conn()
        .execute(
            "INSERT INTO ops_targets (id, label, kind, purpose, path, created_by, created_at, \
             revision) VALUES ('01890000-0000-7000-8000-0000000000b1', 'Synthetic USB', \
             'removable', 'backup_and_anchors', '/synthetic/usb', 'synthetic-user', \
             '2026-10-13T00:00:00.000000000Z', 1)",
            [],
        )
        .unwrap();
    let empty = MemoryKeyStore::new(None);
    let engine = host.start(KeyProvider::Memory(empty.clone())).await;
    assert_eq!(key_state(&engine).await, KeyState::Missing);
    assert!(empty.key().is_none(), "no new key over a backup target");
    let unchecked = restore(&engine, 1).await.unwrap_err();
    assert_eq!(json(&unchecked)["fieldErrors"][0]["code"], "cannot_check");
}

/// `(action, outcome, record_id, details)` of the key-restore events, in order.
fn key_restore_events(host: &Host) -> Vec<(String, String, String, String)> {
    host.conn()
        .prepare(
            "SELECT action, outcome, record_id, details FROM audit_events \
             WHERE record_type = 'installation_key_restore' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

// M7: the intent commits before the key store is written and the completion after it.
#[tokio::test]
async fn a_key_restore_commits_its_intent_before_its_completion() {
    let host = Host::new();
    start_once(&host, &MemoryKeyStore::new(Some(key(1)))).await;
    record_kit(&host, 1);
    let engine = host
        .start(KeyProvider::Memory(MemoryKeyStore::new(None)))
        .await;
    restore(&engine, 1).await.unwrap();
    let events = key_restore_events(&host);
    let actions: Vec<(&str, &str)> = events
        .iter()
        .map(|(action, outcome, _, _)| (action.as_str(), outcome.as_str()))
        .collect();
    assert_eq!(
        actions,
        [
            ("auth.restore_installation_key_intent", "succeeded"),
            ("auth.restore_installation_key", "succeeded")
        ]
    );
    assert_eq!(events[0].2, events[1].2, "one attempt");
}

// M7: a key stored after its intent whose completion was never audited (a crash or an
// audit failure) is completed at the next start from the key then found, once.
#[tokio::test]
async fn an_unfinished_key_restore_is_completed_at_the_next_start() {
    let host = Host::new();
    let store = MemoryKeyStore::new(Some(key(1)));
    start_once(&host, &store).await;
    let db = Database::open(host.dir.path(), MODULES, host.clock.clone()).unwrap();
    db.write(
        Principal::system("synthetic-crash", []),
        CorrelationId::generate(),
        |ctx| -> Result<(), StoreError> {
            let intent = AuditEvent::new(
                Stream::Security,
                "auth.restore_installation_key_intent",
                Outcome::Succeeded,
            )
            .record("installation_key_restore", "synthetic-attempt");
            ctx.audit(intent).map(drop)
        },
    )
    .await
    .unwrap();
    drop(db);

    start_once(&host, &store).await;
    start_once(&host, &store).await;
    let events = key_restore_events(&host);
    assert_eq!(events.len(), 2, "{events:?}");
    let (action, outcome, record, details) = &events[1];
    assert_eq!(
        (action.as_str(), outcome.as_str(), record.as_str()),
        (
            "auth.restore_installation_key",
            "succeeded",
            "synthetic-attempt"
        )
    );
    assert!(details.contains("\"reconciled\":true"), "{details}");
}
