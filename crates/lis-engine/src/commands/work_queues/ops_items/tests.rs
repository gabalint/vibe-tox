//! Native SQLite adapter regression: history display order is not current health.
use super::*;
use lis_core::{Capability, UtcTimestamp};
use lis_ops::health::HealthEventId;
use rusqlite::{Connection, params};

fn database() -> Connection {
    let conn = Connection::open_in_memory().unwrap();
    for migration in lis_ops::SCHEMA.migrations {
        conn.execute_batch(migration.sql).unwrap();
    }
    for migration in lis_ops::RUNTIME_MIGRATIONS {
        conn.execute_batch(migration.sql).unwrap();
    }
    conn
}

fn observed(conn: &Connection, state: &str, time: &str, current: bool) {
    let id = HealthEventId::generate();
    let at: UtcTimestamp = time.parse().unwrap();
    conn.execute("INSERT INTO ops_health_events(id,check_code,state,observed_at) VALUES(?1,'backup_age',?2,?3)",params![id,state,at]).unwrap();
    if current {
        conn.execute("INSERT INTO ops_health_current(check_code,event_id) VALUES('backup_age',?1) ON CONFLICT(check_code) DO UPDATE SET event_id=excluded.event_id",[id]).unwrap();
    }
}

fn alerts(conn: &Connection, principal: &Principal) -> Vec<WorkQueueItem> {
    let definition = QueueDefinition {
        code: "ops_alerts",
        label: "Operational alerts",
        capability: Capability::BackupRestore,
        next_action: "Investigate",
    };
    alert_items(conn, definition, principal).unwrap()
}

#[test]
fn resolved_alert_after_clock_rollback_leaves_queue_and_legacy_current_is_unknown() {
    let conn = database();
    let owner = Principal::system("synthetic-queue-reader", [Capability::BackupRestore]);
    observed(&conn, "alert", "2026-10-14T12:00:00.000000000Z", false);
    assert!(
        alerts(&conn, &owner).is_empty(),
        "legacy history must not invent current state"
    );
    observed(&conn, "warning", "2026-10-14T11:00:00.000000000Z", true);
    let warning = alerts(&conn, &owner);
    assert_eq!(warning.len(), 1);
    assert_eq!(warning[0].state, "warning");
    assert_eq!(
        warning[0].since.to_string(),
        "2026-10-14T11:00:00.000000000Z"
    );
    observed(&conn, "ok", "2026-10-14T10:00:00.000000000Z", true);
    assert!(
        alerts(&conn, &owner).is_empty(),
        "resolved current state wins over a later historical alert timestamp"
    );
    observed(&conn, "alert", "2026-10-14T10:00:00.000000000Z", true);
    assert_eq!(
        alerts(&conn, &owner).len(),
        1,
        "equal-time changed state remains actionable"
    );
    let denied = Principal::system("synthetic-no-owner", []);
    assert!(alerts(&conn, &denied).is_empty());
}
