//! The partner delivery poller (REQ-RPT-003, REQ-RPT-011, REQ-RPT-012).
//!
//! From engine start, whoever is signed in (also nobody), every [`PASS_INTERVAL`] it takes
//! the deliveries `lis_interfaces` says may be written now (connection started, a
//! destination check covering its current folder, retry time come), keeps those whose
//! schedule allows the first attempt, and writes each message's stored file, or the HL7
//! message made for it ([`hl7_payload`]), into the connection's folder ([`folder_write`]).
//! Every attempt is recorded as the delivery worker,
//! with `export` only. Nothing is written while a recovery reconciliation is open, and a
//! report revision's PDF waits while its impact review is open (L13). Logs carry delivery
//! IDs and codes, never file names, folders or content.

mod folder_write;
pub(crate) mod hl7_message;
pub(crate) mod hl7_payload;

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use lis_core::{Capability, Clock, CorrelationId, Principal, UtcTimestamp};
use lis_interfaces::partner::{
    AttemptOutcome, DueDelivery, MessageSource, PartnerError, PartnerMessageType,
};
use lis_reporting::ReportRevisionId;
use lis_store::{ArtifactDigest, Database};

pub(crate) use self::folder_write::{WriteProblem, probe, write_file};
use self::hl7_payload::Payload;
use crate::engine::Engine;
use crate::poller::{Poller, Scan, scan_off_runtime};

/// How often waiting deliveries are looked at.
const PASS_INTERVAL: Duration = Duration::from_secs(10);
/// The worker's name in audit events and logs.
const COMPONENT: &str = "partner-delivery";

/// The poller's shared part.
pub(crate) struct PartnerDeliveries {
    db: Arc<Database>,
    clock: Arc<dyn Clock>,
    /// Raised at shutdown; a pass stops between deliveries.
    cancel: AtomicBool,
}

/// Starts the poller, whose first pass runs now. Must run inside a tokio runtime.
pub(crate) fn start(db: &Arc<Database>, clock: &Arc<dyn Clock>) -> Poller {
    Poller::start(
        Arc::new(PartnerDeliveries {
            db: Arc::clone(db),
            clock: Arc::clone(clock),
            cancel: AtomicBool::new(false),
        }),
        PASS_INTERVAL,
    )
}

impl Scan for PartnerDeliveries {
    fn scan(&self) {
        if let Err(error) = self.pass() {
            tracing::warn!(
                error_code = error.code().as_str(),
                "partner deliveries not read"
            );
        }
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

impl PartnerDeliveries {
    fn pass(&self) -> Result<(), PartnerError> {
        if self
            .db
            .read_blocking(|conn| lis_ops::recovery::outbound_gate(conn).map(|gate| gate.is_some()))
            .unwrap_or(true)
        {
            return Ok(());
        }
        let now = self.clock.now();
        let zone = lab_zone(&self.db);
        let due = self.db.read_blocking(move |conn| {
            lis_interfaces::partner::due_partner_deliveries(conn, now)
        })?;
        for delivery in due {
            if self.cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            let scheduled = delivery.attempts > 0
                || lis_interfaces::partner::schedule_due(
                    &delivery.schedule,
                    delivery.created_at,
                    now,
                    zone.as_ref(),
                );
            if scheduled && !self.paused(&delivery) {
                self.deliver(&delivery, zone.as_ref());
            }
        }
        Ok(())
    }

    /// L13: a report revision's PDF waits, unrecorded, while its impact review is open or
    /// a correction is due; every pass asks again.
    fn paused(&self, delivery: &DueDelivery) -> bool {
        if delivery.source != MessageSource::ReportRevision {
            return false;
        }
        let Some(revision_id) = delivery
            .source_id
            .as_deref()
            .and_then(|id| id.parse::<ReportRevisionId>().ok())
        else {
            return true;
        };
        let delivery_id = delivery.delivery_id;
        self.db
            .read_blocking(move |conn| lis_reporting::revision_delivery_paused(conn, revision_id))
            .unwrap_or_else(|error| {
                tracing::warn!(%delivery_id, error_code = error.code().as_str(), "report pause not read; delivery waits");
                true
            })
    }

    /// Writes one delivery and records the attempt. An HL7 delivery held for a missing
    /// mapping is left as it is.
    fn deliver(&self, delivery: &DueDelivery, zone: Option<&TimeZone>) {
        let bytes = match delivery.message_type {
            PartnerMessageType::Hl7Oru => match hl7_payload::payload(&self.db, delivery) {
                Payload::Ready(bytes) => Some(bytes),
                Payload::Lost => None,
                Payload::Held => return,
            },
            PartnerMessageType::BillingJson | PartnerMessageType::ReportPdf => {
                stored(&self.db, &delivery.artifact_sha256)
            }
        };
        let outcome = match bytes {
            Some(bytes) => {
                let (date, time) = stamp(delivery.created_at, zone);
                let name = delivery.file_name(&date, &time);
                written(write_file(Path::new(&delivery.folder), &name, &bytes))
            }
            None => AttemptOutcome::Failed {
                error_code: "file_unavailable",
            },
        };
        let delivery_id = delivery.delivery_id;
        let failure = match &outcome {
            AttemptOutcome::Written { .. } => None,
            AttemptOutcome::Failed { error_code } => Some(*error_code),
        };
        let recorded = self.db.write_blocking(
            Principal::system(COMPONENT, [Capability::Export]),
            CorrelationId::generate(),
            move |ctx| lis_interfaces::partner::record_delivery_attempt(ctx, delivery_id, outcome),
        );
        match (recorded, failure) {
            (Ok(state), None) => {
                tracing::info!(%delivery_id, state = state.as_str(), "partner message written")
            }
            (Ok(state), Some(code)) => {
                tracing::warn!(%delivery_id, state = state.as_str(), error_code = code, "partner message not written")
            }
            (Err(error), _) => {
                tracing::error!(%delivery_id, error_code = error.code().as_str(), "partner delivery attempt not recorded")
            }
        }
    }
}

/// A stored file's bytes, verified against their digest.
fn stored(db: &Database, digest: &str) -> Option<Vec<u8>> {
    let digest: ArtifactDigest = digest.parse().ok()?;
    db.artifacts().read(&digest).ok()
}

/// The attempt outcome of a write.
pub(crate) fn written(result: Result<std::path::PathBuf, WriteProblem>) -> AttemptOutcome {
    match result {
        Ok(path) => AttemptOutcome::Written {
            path: path.to_string_lossy().into_owned(),
        },
        Err(problem) => AttemptOutcome::Failed {
            error_code: problem.code(),
        },
    }
}

/// The laboratory's time zone, for daily schedules and file names; `None` without one.
pub(crate) fn lab_zone(db: &Database) -> Option<TimeZone> {
    let profile = db
        .read_blocking(lis_identity::current_laboratory_profile)
        .ok()??;
    TimeZone::get(&profile.time_zone).ok()
}

/// `YYYYMMDD` and `HHMMSS` of `at`, in the laboratory's zone or else in UTC.
pub(crate) fn stamp(at: UtcTimestamp, zone: Option<&TimeZone>) -> (String, String) {
    let instant = Timestamp::from_second(at.unix_seconds()).unwrap_or(Timestamp::UNIX_EPOCH);
    let local = instant.to_zoned(zone.cloned().unwrap_or(TimeZone::UTC));
    (
        local.strftime("%Y%m%d").to_string(),
        local.strftime("%H%M%S").to_string(),
    )
}

impl Engine {
    /// One delivery pass now, as the poller runs every 10 seconds from start. Scenario tests
    /// drive the poller with it and the clock.
    pub async fn deliver_partner_messages(&self) {
        scan_off_runtime(Arc::new(PartnerDeliveries {
            db: Arc::clone(&self.db),
            clock: Arc::clone(&self.clock),
            cancel: AtomicBool::new(false),
        }))
        .await;
    }
}
