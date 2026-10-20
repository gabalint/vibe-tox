//! Outbox durability, leases, retries and dead letters (NFR-02, NFR-06, ADR 0012 item 2).

use std::time::Duration;

use lis_core::{CorrelationId, UtcTimestamp};
use lis_store::outbox::{
    self, DeadLetter, Failure, LeasedItem, MAX_ATTEMPTS, OutboxState, TopicBacklog,
};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{CommandContext, OutboxItem, OutboxItemId, StoreError};
use serde_json::json;

use crate::{DEMO, count};

const TOPIC: &str = "demo.deliver";
const LEASE: Duration = Duration::from_secs(30);

fn item(key: &str, payload: serde_json::Value) -> OutboxItem {
    OutboxItem {
        topic: TOPIC,
        dedup_key: key.to_owned(),
        payload,
    }
}

/// Runs `f` as a write by an outbox worker.
fn write<T: Send + 'static>(
    t: &TestDatabase,
    f: impl FnOnce(&mut CommandContext<'_>) -> Result<T, StoreError> + Send + 'static,
) -> Result<T, StoreError> {
    t.db.write_blocking(principal(&[]), CorrelationId::generate(), f)
}

fn enqueue(t: &TestDatabase, key: &'static str) -> OutboxItemId {
    write(t, move |ctx| ctx.enqueue(item(key, json!({ "key": key })))).unwrap()
}

fn lease(t: &TestDatabase) -> Vec<LeasedItem> {
    write(t, |ctx| outbox::lease(ctx, TOPIC, 10, LEASE)).unwrap()
}

fn advance(t: &TestDatabase, by: Duration) {
    t.clock.set(now(t).checked_add(by).unwrap());
}

fn now(t: &TestDatabase) -> UtcTimestamp {
    use lis_core::Clock;
    t.clock.now()
}

fn dead_letters(t: &TestDatabase) -> Vec<DeadLetter> {
    t.db.read_blocking(|conn| outbox::dead_letters(conn, 10))
        .unwrap()
}

#[test]
fn committed_item_survives_a_crash_and_is_leasable_after_reopen() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let id = write(&t, |ctx| {
        ctx.tx()
            .execute("INSERT INTO demo_items (label) VALUES ('released')", [])?;
        ctx.enqueue(item("report-1", json!({ "report": 1 })))
    })
    .unwrap();
    // Simulated crash between commit and dispatch: no worker ever ran.
    let t = t.reopen().unwrap();
    assert_eq!(count(&t.db, "demo_items"), 1);
    let leased = lease(&t);
    assert_eq!(leased.len(), 1);
    assert_eq!(leased[0].id, id);
    assert_eq!(leased[0].payload, json!({ "report": 1 }));
    assert_eq!(leased[0].attempt, 1);
}

#[test]
fn expired_lease_becomes_leasable_again_and_fences_the_old_attempt() {
    let t = TestDatabase::open(&[]).unwrap();
    let id = enqueue(&t, "a");
    let first = lease(&t).remove(0);
    assert!(lease(&t).is_empty(), "a live lease is not handed out twice");
    advance(&t, LEASE);
    let second = lease(&t).remove(0);
    assert_eq!((second.id, second.attempt), (id, 2));
    let stale = write(&t, move |ctx| outbox::complete(ctx, &first));
    assert!(matches!(stale, Err(StoreError::OutboxNotLeased)));
    write(&t, move |ctx| outbox::complete(ctx, &second)).unwrap();
    assert!(lease(&t).is_empty());
    assert!(
        t.db.read_blocking(move |c| outbox::backlog(c, test_now()))
            .unwrap()
            .is_empty()
    );
}

fn test_now() -> UtcTimestamp {
    lis_store::testing::test_start()
}

#[test]
fn exhausted_retries_reach_the_dead_letter_state() {
    let t = TestDatabase::open(&[]).unwrap();
    let id = enqueue(&t, "a");
    for attempt in 1..=MAX_ATTEMPTS {
        let leased = lease(&t).remove(0);
        assert_eq!(leased.attempt, attempt);
        let retry_at = now(&t);
        let state = write(&t, move |ctx| {
            let failure = Failure::Retry {
                error_code: "timeout",
                retry_at,
            };
            outbox::fail(ctx, &leased, failure)
        })
        .unwrap();
        let expected = if attempt < MAX_ATTEMPTS {
            OutboxState::Pending
        } else {
            OutboxState::Dead
        };
        assert_eq!(state, expected);
    }
    assert!(lease(&t).is_empty());
    let letters = dead_letters(&t);
    assert_eq!(letters.len(), 1);
    assert_eq!(letters[0].id, id);
    assert_eq!(letters[0].attempts, MAX_ATTEMPTS);
    assert_eq!(letters[0].last_error_code.as_deref(), Some("timeout"));
}

#[test]
fn worker_crashing_on_every_attempt_still_reaches_dead_letter() {
    let t = TestDatabase::open(&[]).unwrap();
    enqueue(&t, "poison");
    let mut last = None;
    for attempt in 1..=MAX_ATTEMPTS {
        let leased = lease(&t).remove(0);
        assert_eq!(leased.attempt, attempt);
        last = Some(leased);
        advance(&t, LEASE);
    }
    assert!(lease(&t).is_empty());
    // The worker of the final attempt can no longer complete or fail the item.
    let last = last.unwrap();
    let late = last.clone();
    let completed = write(&t, move |ctx| outbox::complete(ctx, &late));
    assert!(matches!(completed, Err(StoreError::OutboxNotLeased)));
    let failed = write(&t, move |ctx| {
        outbox::fail(ctx, &last, Failure::Permanent { error_code: "late" })
    });
    assert!(matches!(failed, Err(StoreError::OutboxNotLeased)));
    let letters = dead_letters(&t);
    assert_eq!(letters.len(), 1);
    assert_eq!(letters[0].last_error_code.as_deref(), Some("lease_expired"));
}

#[test]
fn retry_waits_until_retry_at_and_permanent_failure_dead_letters_at_once() {
    let t = TestDatabase::open(&[]).unwrap();
    enqueue(&t, "a");
    let leased = lease(&t).remove(0);
    let retry_at = now(&t).checked_add(Duration::from_secs(300)).unwrap();
    write(&t, move |ctx| {
        outbox::fail(
            ctx,
            &leased,
            Failure::Retry {
                error_code: "unreachable",
                retry_at,
            },
        )
    })
    .unwrap();
    assert!(lease(&t).is_empty());
    advance(&t, Duration::from_secs(300));
    let leased = lease(&t).remove(0);
    let state = write(&t, move |ctx| {
        outbox::fail(
            ctx,
            &leased,
            Failure::Permanent {
                error_code: "rejected",
            },
        )
    })
    .unwrap();
    assert_eq!(state, OutboxState::Dead);
    assert_eq!(
        dead_letters(&t)[0].last_error_code.as_deref(),
        Some("rejected")
    );
}

#[test]
fn dead_letters_are_bounded_oldest_first() {
    let t = TestDatabase::open(&[]).unwrap();
    let older = enqueue(&t, "a");
    enqueue(&t, "b");
    for leased in lease(&t) {
        advance(&t, Duration::from_secs(1));
        write(&t, move |ctx| {
            outbox::fail(
                ctx,
                &leased,
                Failure::Permanent {
                    error_code: "rejected",
                },
            )
        })
        .unwrap();
    }
    let oldest =
        t.db.read_blocking(|conn| outbox::dead_letters(conn, 1))
            .unwrap();
    assert_eq!(oldest.len(), 1);
    assert_eq!(oldest[0].id, older);
    assert_eq!(dead_letters(&t).len(), 2);
}

#[test]
fn same_dedup_key_is_a_no_op_and_a_different_payload_is_a_conflict() {
    let t = TestDatabase::open(&[DEMO]).unwrap();
    let first = enqueue(&t, "a");
    assert_eq!(enqueue(&t, "a"), first);
    // Key order inside the payload does not matter.
    let reordered = write(&t, |ctx| {
        let mut payload = serde_json::Map::new();
        payload.insert("z".into(), json!(1));
        payload.insert("key".into(), json!("a"));
        ctx.enqueue(item("ordered", json!({ "key": "a", "z": 1 })))?;
        ctx.enqueue(item("ordered", payload.into()))
    });
    assert!(reordered.is_ok());
    assert_eq!(count(&t.db, "store_outbox"), 2);
    let conflict = write(&t, |ctx| {
        ctx.tx()
            .execute("INSERT INTO demo_items (label) VALUES ('x')", [])?;
        ctx.enqueue(item("a", json!({ "key": "different" })))
    });
    assert!(matches!(conflict, Err(StoreError::OutboxConflict)));
    assert_eq!(
        count(&t.db, "demo_items"),
        0,
        "the conflicting command rolled back"
    );
}

#[test]
fn backlog_reports_outstanding_items_and_the_oldest_age() {
    let t = TestDatabase::open(&[]).unwrap();
    enqueue(&t, "old");
    advance(&t, Duration::from_secs(10));
    enqueue(&t, "new");
    write(&t, |ctx| {
        ctx.enqueue(OutboxItem {
            topic: "demo.other",
            dedup_key: "b".into(),
            payload: json!(null),
        })
    })
    .unwrap();
    let done = enqueue(&t, "done");
    let leased = lease(&t);
    let finished = leased.into_iter().find(|item| item.id == done).unwrap();
    write(&t, move |ctx| outbox::complete(ctx, &finished)).unwrap();

    let start = test_now();
    let at = start.checked_add(Duration::from_secs(60)).unwrap();
    let backlog = t.db.read_blocking(move |c| outbox::backlog(c, at)).unwrap();
    let later = start.checked_add(Duration::from_secs(10)).unwrap();
    assert_eq!(
        backlog,
        [
            TopicBacklog {
                topic: TOPIC.into(),
                outstanding: 2,
                oldest_created_at: start,
                oldest_age: Duration::from_secs(60),
            },
            TopicBacklog {
                topic: "demo.other".into(),
                outstanding: 1,
                oldest_created_at: later,
                oldest_age: Duration::from_secs(50),
            },
        ]
    );
}
