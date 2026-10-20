//! The recovery reconciliation (REQ-OPS-002; M4 design brief D5, §3.8): every recorded
//! restore opens it, its gate blocks outbound work until a quality manager closes it with
//! every item attested and a note, and its records are append-only.

use lis_core::{Capability, CorrelationId, ErrorCode, Principal};
use lis_ops::backup::BackupError;
use lis_ops::recovery::{
    self, CLOSE_RECOVERY, ChecklistItem, GATE_CODE, GATE_MESSAGE, close_recovery_reconciliation,
    get_recovery_reconciliation,
};
use lis_ops::restore::RestoreId;
use lis_store::StoreError;
use lis_store::testing::principal;

use crate::support::{Restored, backed_up, restore_into_new};

fn restored() -> Restored {
    let (f, target, _) = backed_up();
    restore_into_new(&f, &target)
}

fn close(
    restored: &Restored,
    who: Principal,
    restore_id: RestoreId,
    items: &[ChecklistItem],
    note: &str,
) -> Result<(), BackupError> {
    let items = items.to_vec();
    let note = note.to_owned();
    restored
        .db
        .write_blocking(who, CorrelationId::generate(), move |ctx| {
            close_recovery_reconciliation(ctx, restore_id, &items, &note)
        })
}

fn quality() -> Principal {
    principal(&[Capability::ManageQuality])
}

fn rows(restored: &Restored) -> (i64, i64) {
    restored
        .db
        .read_blocking(|conn| -> Result<_, StoreError> {
            let count = |sql: &str| conn.query_row(sql, [], |row| row.get::<_, i64>(0));
            Ok((
                count("SELECT count(*) FROM ops_recovery_attestations")?,
                count("SELECT count(*) FROM ops_recovery_closures")?,
            ))
        })
        .unwrap()
}

#[test]
fn a_recorded_restore_opens_the_gate_and_the_checklist() {
    let restored = restored();
    let gate = restored
        .db
        .read_blocking(recovery::outbound_gate)
        .unwrap()
        .unwrap();
    assert_eq!(gate.code, GATE_CODE);
    assert_eq!(gate.message, GATE_MESSAGE);
    let view = restored
        .db
        .read_blocking(get_recovery_reconciliation)
        .unwrap()
        .unwrap();
    assert_eq!(view.restore_id, restored.restore_id);
    assert_eq!(view.items.len(), 11);
    assert!(
        view.items
            .iter()
            .all(|item| !item.guidance().is_empty() && !item.label().is_empty())
    );
    assert_eq!(view.floors.accession, "A26001001");
    assert!(view.anchors.matched >= 1);
    assert!(view.closed.is_none());
    assert!(view.set_created_at < view.applied_at);
}

#[test]
fn closing_needs_manage_quality_a_note_and_every_item() {
    let restored = restored();
    let id = restored.restore_id;
    let all = ChecklistItem::ALL;
    let forbidden = close(
        &restored,
        principal(&[Capability::BackupRestore]),
        id,
        &all,
        "Done",
    );
    assert_eq!(forbidden.unwrap_err().code(), ErrorCode::Forbidden);
    let no_note = close(&restored, quality(), id, &all, "  ").unwrap_err();
    assert_eq!(no_note.code(), ErrorCode::Validation);
    let missing = close(&restored, quality(), id, &all[..10], "Done").unwrap_err();
    assert_eq!(missing.code(), ErrorCode::Validation);
    let doubled = [all[..10].to_vec(), vec![all[0]]].concat();
    let repeated = close(&restored, quality(), id, &doubled, "Done").unwrap_err();
    assert_eq!(repeated.code(), ErrorCode::Validation);
    let unknown = close(&restored, quality(), RestoreId::generate(), &all, "Done");
    assert!(matches!(unknown.unwrap_err(), BackupError::RecoveryNotOpen));
    assert_eq!(rows(&restored), (0, 0));
    assert!(restored.db.read_blocking(recovery::is_open).unwrap());
}

#[test]
fn closing_with_every_item_attested_lifts_the_gate_once() {
    let restored = restored();
    let id = restored.restore_id;
    close(
        &restored,
        quality(),
        id,
        &ChecklistItem::ALL,
        "Lost interval re-entered",
    )
    .unwrap();
    assert!(
        restored
            .db
            .read_blocking(recovery::outbound_gate)
            .unwrap()
            .is_none()
    );
    assert!(!restored.db.read_blocking(recovery::is_open).unwrap());
    let view = restored
        .db
        .read_blocking(get_recovery_reconciliation)
        .unwrap()
        .unwrap();
    assert!(view.closed.is_some());
    let (stream, reason): (String, Option<String>) = restored
        .db
        .read_blocking(|conn| -> Result<_, StoreError> {
            Ok(conn.query_row(
                "SELECT stream, reason FROM audit_events WHERE action = ?1",
                [CLOSE_RECOVERY],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?)
        })
        .unwrap();
    assert_eq!(stream, "clinical");
    assert_eq!(reason.as_deref(), Some("Lost interval re-entered"));
    let again = close(&restored, quality(), id, &ChecklistItem::ALL, "Again");
    assert!(matches!(again.unwrap_err(), BackupError::RecoveryNotOpen));
    assert_eq!(rows(&restored), (11, 1));
}

#[test]
fn the_tables_refuse_an_incomplete_closure_and_any_change() {
    let restored = restored();
    let id = restored.restore_id.to_string();
    let attempts = [
        format!("INSERT INTO ops_recovery_closures VALUES ('{id}', 'u', 't')"),
        "UPDATE ops_restores SET set_id = 'other'".to_owned(),
        "DELETE FROM ops_restores".to_owned(),
        "DELETE FROM ops_restore_heads".to_owned(),
    ];
    for sql in attempts {
        let result = restored.db.write_blocking(
            Principal::system("test", []),
            CorrelationId::generate(),
            move |ctx| -> Result<usize, StoreError> { Ok(ctx.tx().execute(&sql, [])?) },
        );
        assert!(result.is_err());
    }
    assert!(restored.db.read_blocking(recovery::is_open).unwrap());
}
