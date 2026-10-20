//! The write path: commit and rollback of change, audit and outbox together;
//! authorization; the single writer; read connections; pragmas.

use std::sync::{Arc, mpsc};
use std::thread;

use lis_audit::{AuditEvent, Outcome, Stream, verify};
use lis_core::{Capability, CorrelationId, Denied};
use lis_store::testing::{TEST_START, TestDatabase, principal};
use lis_store::{CommandContext, OutboxItem, StoreError};

use crate::{DEMO, count};

#[derive(Debug, thiserror::Error)]
enum DemoError {
    #[error("refused by the demo rule")]
    Refused,
    #[error(transparent)]
    Store(#[from] StoreError),
}

fn insert_demo(ctx: &CommandContext<'_>, label: &str) -> Result<(), StoreError> {
    ctx.tx()
        .execute("INSERT INTO demo_items (label) VALUES (?1)", [label])?;
    Ok(())
}

fn demo_item(label: &str) -> OutboxItem {
    OutboxItem {
        topic: "demo.notify",
        dedup_key: label.to_owned(),
        payload: serde_json::json!({ "label": label }),
    }
}

fn demo_event() -> AuditEvent {
    AuditEvent::new(Stream::Clinical, "demo.create", Outcome::Succeeded).record("demo", "a")
}

/// Change, audit event and outbox item from one command.
fn full_command(ctx: &mut CommandContext<'_>) -> Result<(), StoreError> {
    ctx.require(Capability::Accession)?;
    insert_demo(ctx, "a")?;
    ctx.audit(demo_event())?;
    ctx.enqueue(demo_item("a"))?;
    Ok(())
}

fn assert_nothing_written(t: &TestDatabase) {
    assert_eq!(count(&t.db, "demo_items"), 0);
    assert_eq!(count(&t.db, "audit_events"), 0);
    assert_eq!(count(&t.db, "store_outbox"), 0);
}

#[test]
fn command_commits_change_audit_and_outbox_together() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let correlation_id = CorrelationId::generate();
    t.db.write_blocking(
        principal(&[Capability::Accession]),
        correlation_id,
        full_command,
    )
    .unwrap();
    assert_eq!(count(&t.db, "demo_items"), 1);
    assert_eq!(count(&t.db, "audit_events"), 1);
    assert_eq!(count(&t.db, "store_outbox"), 1);

    let (authority, correlation, occurred_at, kind): (String, String, String, String) =
        t.db.read_blocking(|conn| {
            conn.query_row(
                "SELECT authority, correlation_id, occurred_at, principal_kind FROM audit_events",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .map_err(StoreError::from)
        })
        .unwrap();
    assert_eq!(authority, "accession");
    assert_eq!(correlation, correlation_id.to_string());
    assert_eq!(occurred_at, TEST_START);
    assert_eq!(kind, "user");
    let report =
        t.db.read_blocking(|conn| verify(conn, Stream::Clinical).map_err(StoreError::from))
            .unwrap();
    assert!(report.is_intact());
}

#[test]
fn failing_command_rolls_back_change_audit_and_outbox() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result = t.db.write_blocking(
        principal(&[Capability::Accession]),
        CorrelationId::generate(),
        |ctx| -> Result<(), DemoError> {
            full_command(ctx)?;
            Err(DemoError::Refused)
        },
    );
    assert!(matches!(result, Err(DemoError::Refused)));
    assert_nothing_written(&t);
}

#[test]
fn command_without_capability_is_refused_and_writes_nothing() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    for capabilities in [&[][..], &[Capability::ViewAudit, Capability::ManageUsers]] {
        let result = t.db.write_blocking(
            principal(capabilities),
            CorrelationId::generate(),
            full_command,
        );
        assert!(
            matches!(
                result,
                Err(StoreError::Denied(Denied {
                    capability: Capability::Accession
                }))
            ),
            "{result:?}"
        );
    }
    assert_nothing_written(&t);
}

#[test]
fn writes_serialize_through_the_writer_while_reads_proceed() {
    let t = Arc::new(TestDatabase::open(&[DEMO]).unwrap());
    let (inserted, wait_inserted) = mpsc::channel();
    let (release, wait_release) = mpsc::channel::<()>();
    let holder = {
        let t = Arc::clone(&t);
        thread::spawn(move || {
            t.db.write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
                insert_demo(ctx, "held")?;
                inserted.send(()).unwrap();
                wait_release.recv().unwrap();
                Ok::<_, StoreError>(())
            })
        })
    };
    wait_inserted.recv().unwrap();
    // The write transaction is open and uncommitted: a read still runs, on a
    // reader thread, and sees only committed data.
    let (visible, reader) =
        t.db.read_blocking(|conn| {
            let visible: i64 =
                conn.query_row("SELECT count(*) FROM demo_items", [], |r| r.get(0))?;
            Ok::<_, StoreError>((visible, thread::current().name().map(str::to_owned)))
        })
        .unwrap();
    assert_eq!(visible, 0);
    assert!(reader.unwrap().starts_with("lis-db-reader-"));
    release.send(()).unwrap();
    holder.join().unwrap().unwrap();

    // Concurrent read-modify-write commands lose no update: they all run on the one writer thread.
    let writers: Vec<_> = (0..16)
        .map(|_| {
            let t = Arc::clone(&t);
            thread::spawn(move || {
                t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
                    assert_eq!(thread::current().name(), Some("lis-db-writer-0"));
                    let n: i64 =
                        ctx.tx()
                            .query_row("SELECT count(*) FROM demo_items", [], |r| r.get(0))?;
                    insert_demo(ctx, &format!("row-{n}"))?;
                    Ok::<_, StoreError>(())
                })
            })
        })
        .collect();
    for writer in writers {
        writer.join().unwrap().unwrap();
    }
    let distinct =
        t.db.read_blocking(|conn| {
            conn.query_row("SELECT count(DISTINCT label) FROM demo_items", [], |r| {
                r.get::<_, i64>(0)
            })
            .map_err(StoreError::from)
        })
        .unwrap();
    assert_eq!(distinct, 17);
}

#[tokio::test]
async fn async_write_and_read_await_their_results() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    t.db.write(
        principal(&[Capability::Accession]),
        CorrelationId::generate(),
        full_command,
    )
    .await
    .unwrap();
    let rows: i64 =
        t.db.read(|conn| {
            conn.query_row("SELECT count(*) FROM demo_items", [], |r| r.get(0))
                .map_err(StoreError::from)
        })
        .await
        .unwrap();
    assert_eq!(rows, 1);
}

#[test]
fn panicking_command_rolls_back_and_stops_the_writer() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result = t.db.write_blocking(
        principal(&[]),
        CorrelationId::generate(),
        |ctx| -> Result<(), StoreError> {
            insert_demo(ctx, "before the panic")?;
            panic!("synthetic panic inside a command");
        },
    );
    assert!(matches!(result, Err(StoreError::WriterStopped)));
    let later =
        t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
            insert_demo(ctx, "after")
        });
    assert!(matches!(later, Err(StoreError::WriterStopped)));
    // Reads continue; the panicked transaction left nothing behind.
    assert_eq!(count(&t.db, "demo_items"), 0);
}

#[test]
fn connections_use_the_production_pragmas() {
    let t = TestDatabase::open(&[]).unwrap();
    let read = |conn: &rusqlite::Connection| -> rusqlite::Result<(String, [i64; 5])> {
        let value = |name| conn.pragma_query_value(None, name, |r| r.get::<_, i64>(0));
        let journal_mode = conn.pragma_query_value(None, "journal_mode", |r| r.get(0))?;
        let settings = [
            value("synchronous")?,
            value("foreign_keys")?,
            value("busy_timeout")?,
            value("fullfsync")?,
            value("query_only")?,
        ];
        Ok((journal_mode, settings))
    };
    let writer =
        t.db.write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
            read(ctx.tx()).map_err(StoreError::from)
        })
        .unwrap();
    // synchronous 2 = FULL, foreign keys on, 5 s busy timeout, fullfsync on, writable.
    assert_eq!(writer, ("wal".to_owned(), [2, 1, 5000, 1, 0]));
    let reader =
        t.db.read_blocking(move |conn| read(conn).map_err(StoreError::from))
            .unwrap();
    assert_eq!(reader, ("wal".to_owned(), [2, 1, 5000, 1, 1]));
    let refused = t.db.read_blocking(|conn| {
        conn.execute("INSERT INTO store_outbox (id) VALUES ('x')", [])
            .map_err(StoreError::from)
    });
    assert!(refused.is_err());
}

#[tokio::test]
async fn panicking_async_command_stops_the_writer() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result =
        t.db.write(
            principal(&[]),
            CorrelationId::generate(),
            |_ctx| -> Result<(), StoreError> { panic!("synthetic panic inside an async command") },
        )
        .await;
    assert!(matches!(result, Err(StoreError::WriterStopped)));
}

#[test]
fn panicking_query_stops_only_its_reader() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result = t.db.read_blocking(|_conn| -> Result<(), StoreError> {
        panic!("synthetic panic inside a query")
    });
    assert!(matches!(result, Err(StoreError::ReadersStopped)));
    assert_eq!(
        count(&t.db, "demo_items"),
        0,
        "the other readers still serve"
    );
}

#[test]
fn command_that_ends_its_own_transaction_is_refused() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result = t.db.write_blocking(
        principal(&[Capability::Accession]),
        CorrelationId::generate(),
        |ctx| {
            full_command(ctx)?;
            ctx.tx().execute_batch("COMMIT")?;
            Ok::<_, StoreError>(())
        },
    );
    assert!(matches!(result, Err(StoreError::TransactionEnded)));
    assert_nothing_written(&t);
    // The writer keeps working afterwards.
    t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
        insert_demo(ctx, "after")
    })
    .unwrap();
}

#[test]
fn read_callback_cannot_commit_and_mix_snapshots() {
    let t = Arc::new(TestDatabase::open(&[DEMO]).unwrap());
    t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
        insert_demo(ctx, "first")
    })
    .unwrap();
    let (attempted, wait_attempted) = mpsc::channel();
    let (written, wait_written) = mpsc::channel();
    let reader = {
        let t = Arc::clone(&t);
        thread::spawn(move || {
            t.db.read_blocking(move |conn| {
                let before: i64 =
                    conn.query_row("SELECT count(*) FROM demo_items", [], |row| row.get(0))?;
                let commit_succeeded = conn.execute_batch("COMMIT").is_ok();
                attempted.send(commit_succeeded).unwrap();
                wait_written.recv().unwrap();
                let after: i64 =
                    conn.query_row("SELECT count(*) FROM demo_items", [], |row| row.get(0))?;
                Ok::<_, StoreError>((before, after))
            })
        })
    };
    assert!(!wait_attempted.recv().unwrap());
    t.db.write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
        insert_demo(ctx, "second")
    })
    .unwrap();
    written.send(()).unwrap();
    assert!(matches!(
        reader.join().unwrap(),
        Err(StoreError::TransactionEnded)
    ));
}

#[test]
fn write_started_inside_a_command_is_refused_instead_of_hanging() {
    let t = Arc::new(TestDatabase::open(&[DEMO]).unwrap());
    let inner = Arc::clone(&t);
    let result =
        t.db.write_blocking(principal(&[]), CorrelationId::generate(), move |_ctx| {
            inner
                .db
                .write_blocking(principal(&[]), CorrelationId::generate(), |ctx| {
                    insert_demo(ctx, "nested")
                })
        });
    assert!(matches!(result, Err(StoreError::NestedWrite)));
    assert_eq!(count(&t.db, "demo_items"), 0);
}

#[test]
fn query_cannot_write_even_after_clearing_query_only() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let result = t.db.read_blocking(|conn| {
        conn.execute_batch("PRAGMA query_only = 0")?;
        conn.execute("INSERT INTO demo_items (label) VALUES ('sneaky')", [])?;
        Ok::<_, StoreError>(())
    });
    assert!(result.is_err());
    assert_eq!(count(&t.db, "demo_items"), 0);
}
