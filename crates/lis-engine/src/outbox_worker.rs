//! The outbox dispatcher (NFR-02, NFR-06): leases ready items one at a time on
//! the writer, runs the topic's handler outside any transaction, then marks
//! the item done or failed with bounded exponential backoff. The store
//! dead-letters an item after its last attempt or a permanent failure. It
//! acts as the `outbox-worker` system principal and stops gracefully: the
//! item in hand finishes first.
//!
//! Topics with an external effect (label printing) wait while a recovery
//! reconciliation is open after a restore (REQ-OPS-002; M4 design brief §3.8):
//! a restored database may re-issue numbers already on physical labels.

use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use lis_core::{CorrelationId, Principal};
use lis_store::outbox::{self, Failure, LeasedItem, OutboxState};
use lis_store::{Database, StoreError};
use tokio::sync::watch;
use tokio::task::JoinHandle;

/// The worker's principal; it holds no capability.
const COMPONENT: &str = "outbox-worker";
/// How often the dispatcher looks for ready items.
const POLL_INTERVAL: Duration = Duration::from_secs(2);
/// How long a lease lasts before another attempt may take the item.
const LEASE: Duration = Duration::from_secs(5 * 60);
/// Delay before the second attempt; doubled per attempt up to [`MAX_BACKOFF`].
const FIRST_BACKOFF: Duration = Duration::from_secs(10);
const MAX_BACKOFF: Duration = Duration::from_secs(15 * 60);
/// Topics held while a recovery reconciliation is open.
const HELD_DURING_RECOVERY: [&str; 1] = [lis_specimens::LABEL_PRINT_TOPIC];

/// What a handler's future resolves to.
pub type HandlerFuture = Pin<Box<dyn Future<Output = Result<(), HandlerError>> + Send>>;

/// Runs the follow-up work of one topic.
#[derive(Clone)]
pub struct TopicHandler {
    /// The outbox topic, such as `report.deliver`.
    pub topic: &'static str,
    /// Does the work for one item. It must be safe to repeat (at-least-once).
    pub handle: Arc<dyn Fn(LeasedItem) -> HandlerFuture + Send + Sync>,
}

/// How a handler failed. Codes are code-defined and PHI-free.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HandlerError {
    /// Try again later.
    Retry(&'static str),
    /// Never retry; dead-letter now.
    Permanent(&'static str),
}

/// Delay before attempt `attempt + 1`.
fn backoff(attempt: u32) -> Duration {
    let doublings = attempt.saturating_sub(1).min(16);
    FIRST_BACKOFF
        .saturating_mul(1 << doublings)
        .min(MAX_BACKOFF)
}

pub(crate) struct Dispatcher {
    db: Arc<Database>,
    handlers: Vec<TopicHandler>,
}

impl Dispatcher {
    pub(crate) fn new(db: Arc<Database>, handlers: Vec<TopicHandler>) -> Self {
        Self { db, handlers }
    }

    async fn write<T, F>(&self, command: F) -> Result<T, StoreError>
    where
        F: FnOnce(&mut lis_store::CommandContext<'_>) -> Result<T, StoreError> + Send + 'static,
        T: Send + 'static,
    {
        self.db
            .write(
                Principal::system(COMPONENT, []),
                CorrelationId::generate(),
                command,
            )
            .await
    }

    /// Handles every item that is ready now; returns how many were handled.
    pub(crate) async fn dispatch_ready(&self) -> Result<usize, StoreError> {
        let mut handled = 0;
        let holds = self
            .handlers
            .iter()
            .any(|handler| HELD_DURING_RECOVERY.contains(&handler.topic));
        let held = holds && self.recovery_open().await;
        for handler in &self.handlers {
            let topic = handler.topic;
            if held && HELD_DURING_RECOVERY.contains(&topic) {
                continue;
            }
            while let Some(item) = self
                .write(move |ctx| outbox::lease(ctx, topic, 1, LEASE).map(|mut items| items.pop()))
                .await?
            {
                self.process(handler, item).await?;
                handled += 1;
            }
        }
        Ok(handled)
    }

    /// True while a recovery reconciliation is open. A failure to tell counts as open:
    /// holding a label is safer than printing a re-issued number.
    async fn recovery_open(&self) -> bool {
        self.db
            .read(lis_ops::recovery::is_open)
            .await
            .unwrap_or_else(|error| {
                tracing::warn!(%error, "recovery state unreadable; holding external effects");
                true
            })
    }

    async fn process(&self, handler: &TopicHandler, item: LeasedItem) -> Result<(), StoreError> {
        let outcome = (handler.handle)(item.clone()).await;
        let (id, attempt) = (item.id, item.attempt);
        let state = match outcome {
            Ok(()) => {
                self.write(move |ctx| outbox::complete(ctx, &item)).await?;
                OutboxState::Done
            }
            Err(HandlerError::Retry(error_code)) => {
                self.write(move |ctx| {
                    let retry_at = ctx
                        .now()
                        .checked_add(backoff(item.attempt))
                        .ok_or(StoreError::TimeOutOfRange)?;
                    let failure = Failure::Retry {
                        error_code,
                        retry_at,
                    };
                    outbox::fail(ctx, &item, failure)
                })
                .await?
            }
            Err(HandlerError::Permanent(error_code)) => {
                self.write(move |ctx| outbox::fail(ctx, &item, Failure::Permanent { error_code }))
                    .await?
            }
        };
        if state == OutboxState::Dead {
            tracing::warn!(topic = handler.topic, item_id = %id, attempt, "outbox item dead-lettered");
        } else {
            tracing::info!(topic = handler.topic, item_id = %id, attempt, state = ?state, "outbox item handled");
        }
        Ok(())
    }

    /// Runs until `shutdown` turns true, polling for ready items.
    pub(crate) fn spawn(self, shutdown: watch::Receiver<bool>) -> JoinHandle<()> {
        tokio::spawn(self.run(shutdown))
    }

    async fn run(self, mut shutdown: watch::Receiver<bool>) {
        while !*shutdown.borrow() {
            if let Err(error) = self.dispatch_ready().await {
                tracing::warn!(%error, "outbox dispatch failed; retrying");
            }
            tokio::select! {
                changed = shutdown.changed() => {
                    // A dropped sender also means stop.
                    if changed.is_err() {
                        return;
                    }
                }
                () = tokio::time::sleep(POLL_INTERVAL) => {}
            }
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod recovery_hold_tests {
    use std::sync::Mutex;

    use lis_core::{FixedClock, UtcTimestamp};
    use lis_store::OutboxItem;

    use super::*;

    fn handler(calls: Arc<Mutex<u32>>) -> TopicHandler {
        TopicHandler {
            topic: lis_specimens::LABEL_PRINT_TOPIC,
            handle: Arc::new(move |_item| {
                *calls.lock().unwrap() += 1;
                Box::pin(async { Ok(()) })
            }),
        }
    }

    async fn write(db: &Database, sql: &'static str) {
        db.write(
            Principal::system("test", []),
            CorrelationId::generate(),
            move |ctx| ctx.tx().execute_batch(sql).map_err(StoreError::from),
        )
        .await
        .unwrap();
    }

    const OPEN_RESTORE: &str = "INSERT INTO ops_restores (id, set_id, set_created_at, \
        set_kind, snapshot_sha256, backup_app_version, staged_at, applied_at, recorded_at, \
        anchors_compared, anchors_matched, anchors_lost_interval, anchors_unverified, \
        anchors_mismatched, anchors_missing, anchor_conflicts, anchor_gaps, anchors_unanchored, \
        applied_event_seq, applied_event_hash, accession_floor, container_floor) VALUES \
        ('r1', 's1', 't', 'scheduled', printf('%064d', 0), '0.1.0', 't', 't', 't', \
        0, 0, 0, 0, 0, 0, 0, 0, 0, 1, printf('%064d', 0), 'A26001000', 'C00005000');";

    const CLOSE_RESTORE: &str = "INSERT INTO ops_recovery_attestations (restore_id, item_code) \
        SELECT 'r1', value FROM json_each('[\"production_computer\", \"lost_interval_listed\", \
        \"numbers_checked\", \"analyzer_results_resent\", \"reports_reconciled\", \
        \"orders_reentered\", \"billing_reconciled\", \"accounts_reapplied\", \
        \"configuration_reapplied\", \"anchors_reviewed\", \"labels_waiting\"]'); \
        INSERT INTO ops_recovery_closures (restore_id, closed_by, closed_at) \
        VALUES ('r1', 'synthetic', 't');";

    #[tokio::test]
    async fn label_printing_waits_while_a_recovery_reconciliation_is_open() {
        let dir = tempfile::tempdir().unwrap();
        let start = UtcTimestamp::parse("2026-10-13T07:00:00.000000000Z").unwrap();
        let clock = Arc::new(FixedClock::new(start));
        let db = Arc::new(Database::open(dir.path(), crate::engine::MODULES, clock).unwrap());
        write(&db, OPEN_RESTORE).await;
        let item = OutboxItem {
            topic: lis_specimens::LABEL_PRINT_TOPIC,
            dedup_key: "job-1".to_owned(),
            payload: serde_json::json!({ "jobId": "synthetic" }),
        };
        db.write(
            Principal::system("test", []),
            CorrelationId::generate(),
            move |ctx| ctx.enqueue(item),
        )
        .await
        .unwrap();
        let calls = Arc::new(Mutex::new(0));
        let dispatcher = Dispatcher::new(Arc::clone(&db), vec![handler(calls.clone())]);
        assert_eq!(dispatcher.dispatch_ready().await.unwrap(), 0, "held");
        assert_eq!(*calls.lock().unwrap(), 0);
        write(&db, CLOSE_RESTORE).await;
        assert_eq!(
            dispatcher.dispatch_ready().await.unwrap(),
            1,
            "printed once closed"
        );
        assert_eq!(*calls.lock().unwrap(), 1);
    }
}
