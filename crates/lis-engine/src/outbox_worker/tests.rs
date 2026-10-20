//! Dispatcher tests with a test handler: no production topic exists in M1.

use std::sync::Mutex;

use lis_core::{Clock, FixedClock, UtcTimestamp};
use lis_store::OutboxItem;
use lis_store::outbox::MAX_ATTEMPTS;
use tempfile::TempDir;

use super::*;

const TOPIC: &str = "test.echo";

fn start() -> UtcTimestamp {
    UtcTimestamp::parse("2026-10-12T10:00:00.000000000Z").unwrap()
}

fn open() -> (TempDir, Arc<FixedClock>, Arc<Database>) {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(FixedClock::new(start()));
    let db = Database::open(dir.path(), &[], clock.clone()).unwrap();
    (dir, clock, Arc::new(db))
}

/// A handler that answers with `outcome` and counts its calls.
fn handler(outcome: Result<(), HandlerError>, calls: Arc<Mutex<u32>>) -> TopicHandler {
    TopicHandler {
        topic: TOPIC,
        handle: Arc::new(move |_item| {
            *calls.lock().unwrap() += 1;
            Box::pin(async move { outcome })
        }),
    }
}

async fn enqueue(db: &Database, key: &str) {
    let item = OutboxItem {
        topic: TOPIC,
        dedup_key: key.to_owned(),
        payload: serde_json::json!({ "key": key }),
    };
    let principal = Principal::system("test", []);
    let _: lis_store::OutboxItemId = db
        .write(principal, CorrelationId::generate(), move |ctx| {
            ctx.enqueue(item)
        })
        .await
        .unwrap();
}

async fn state(db: &Database) -> (String, u32, String) {
    db.read(|conn| {
        Ok::<_, StoreError>(conn.query_row(
            "SELECT state, attempts, available_at FROM store_outbox",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    })
    .await
    .unwrap()
}

#[tokio::test]
async fn handled_item_is_marked_done() {
    let (_dir, _clock, db) = open();
    enqueue(&db, "a").await;
    let calls = Arc::new(Mutex::new(0));
    let dispatcher = Dispatcher::new(Arc::clone(&db), vec![handler(Ok(()), calls.clone())]);
    assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 1);
    assert_eq!(state(&db).await.0, "done");
    assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 0);
    assert_eq!(*calls.lock().unwrap(), 1);
}

#[tokio::test]
async fn retry_waits_with_backoff_and_dead_letters_after_the_last_attempt() {
    let (_dir, clock, db) = open();
    enqueue(&db, "b").await;
    let calls = Arc::new(Mutex::new(0));
    let failing = handler(Err(HandlerError::Retry("timeout")), calls.clone());
    let dispatcher = Dispatcher::new(Arc::clone(&db), vec![failing]);
    assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 1);
    let (state_after_one, attempts, available_at) = state(&db).await;
    assert_eq!((state_after_one.as_str(), attempts), ("pending", 1));
    assert_eq!(available_at, "2026-10-12T10:00:10.000000000Z");
    assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 0, "not due yet");
    for _ in 1..MAX_ATTEMPTS {
        clock.set(clock.now().checked_add(MAX_BACKOFF).unwrap());
        assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 1);
    }
    let dead = db
        .read(|conn| outbox::dead_letters(conn, 10))
        .await
        .unwrap();
    assert_eq!(dead.len(), 1);
    assert_eq!(dead[0].last_error_code.as_deref(), Some("timeout"));
    assert_eq!(*calls.lock().unwrap(), MAX_ATTEMPTS);
}

#[tokio::test]
async fn permanent_failure_dead_letters_at_once() {
    let (_dir, _clock, db) = open();
    enqueue(&db, "c").await;
    let permanent = handler(Err(HandlerError::Permanent("rejected")), Arc::default());
    Dispatcher::new(Arc::clone(&db), vec![permanent])
        .dispatch_ready()
        .await
        .unwrap();
    assert_eq!(state(&db).await.0, "dead");
}

#[test]
fn backoff_doubles_and_is_bounded() {
    assert_eq!(backoff(1), Duration::from_secs(10));
    assert_eq!(backoff(2), Duration::from_secs(20));
    assert_eq!(backoff(4), Duration::from_secs(80));
    assert_eq!(backoff(8), MAX_BACKOFF);
    assert_eq!(backoff(u32::MAX), MAX_BACKOFF);
}

#[tokio::test]
async fn spawned_dispatcher_handles_items_and_stops_on_shutdown() {
    let (_dir, _clock, db) = open();
    enqueue(&db, "d").await;
    let (stop, stopped) = watch::channel(false);
    let task =
        Dispatcher::new(Arc::clone(&db), vec![handler(Ok(()), Arc::default())]).spawn(stopped);
    // The first pass runs before the first poll wait.
    while state(&db).await.0 != "done" {
        tokio::task::yield_now().await;
    }
    stop.send(true).unwrap();
    task.await.unwrap();
}
