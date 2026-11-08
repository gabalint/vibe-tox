//! The incoming-order reader (ticket #4555 step 3c; REQ-INT-002, REQ-INT-003, REQ-ORD-001).
//!
//! From engine start, whoever is signed in (also nobody), every [`PASS_INTERVAL`] it reads
//! each started HL7 Orders connection whose destination check covers its folder or server,
//! once the connection's `check every` has passed: up to [`inbox::MAX_FILES`] complete
//! `*.hl7` files. A file's bytes are stored first; then one transaction records what the
//! message became, with the order it made ([`decide`]); then the file moves to Processed or
//! Failed › year › month and the move is recorded. A file read again after a crash before
//! its move is only moved. Messages waiting for their patient are tried again at each
//! reading. Nothing is read while a recovery reconciliation is open. Each reading's first
//! problem, or none, is kept for the connection views ([`PartnerReadings`]). Logs carry IDs
//! and codes, never file names, folders or content.

pub(crate) mod acceptance;
mod decide;
mod inbox;
mod order_input;
mod readings;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_interfaces::partner::{
    IntakeSource, NewReceived, PartnerConnectionId, PartnerError, PartnerReceivedId,
    ReceivedOutcome,
};
use lis_store::{ArtifactDigest, Database};

pub(crate) use self::decide::ReadFile;
use self::inbox::Inbox;
pub(crate) use self::inbox::candidate;
pub(crate) use self::readings::PartnerReadings;
use crate::engine::Engine;
use crate::ops_worker::{Operations, locked};
use crate::partner_delivery::{held_by_recovery, lab_zone, stamp};
use crate::poller::{Poller, Scan, scan_off_runtime};

/// How often connections are looked at; each is read at its own `check every`.
const PASS_INTERVAL: Duration = Duration::from_secs(5);
/// The worker's name in audit events and logs.
const COMPONENT: &str = "partner-intake";

/// Why a step of reading failed; never shown with patient data.
#[derive(Debug, thiserror::Error)]
pub(crate) enum IntakeError {
    #[error(transparent)]
    Partner(#[from] PartnerError),
    #[error(transparent)]
    Order(#[from] lis_orders::OrderError),
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    #[error(transparent)]
    Store(#[from] lis_store::StoreError),
}

/// The reader's shared part.
pub(crate) struct PartnerIntake {
    ops: Arc<Operations>,
    /// Raised at shutdown; a pass stops between files.
    cancel: AtomicBool,
    /// When each connection was last read.
    last_read: Mutex<HashMap<PartnerConnectionId, UtcTimestamp>>,
    /// What each reading found, shared with the connection views.
    readings: Arc<PartnerReadings>,
}

/// Starts the reader, whose first pass runs now. Must run inside a tokio runtime.
pub(crate) fn start(ops: &Arc<Operations>, readings: &Arc<PartnerReadings>) -> Poller {
    Poller::start(Arc::new(PartnerIntake::new(ops, readings)), PASS_INTERVAL)
}

impl Scan for PartnerIntake {
    fn scan(&self) {
        if let Err(error) = self.pass() {
            tracing::warn!(%error, "incoming orders not read");
        }
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
    }
}

fn worker() -> Principal {
    Principal::system(COMPONENT, [Capability::EnterOrders])
}

impl PartnerIntake {
    fn new(ops: &Arc<Operations>, readings: &Arc<PartnerReadings>) -> Self {
        Self {
            ops: Arc::clone(ops),
            cancel: AtomicBool::new(false),
            last_read: Mutex::new(HashMap::new()),
            readings: Arc::clone(readings),
        }
    }

    fn pass(&self) -> Result<(), IntakeError> {
        let db = &self.ops.db;
        if held_by_recovery(db) {
            return Ok(());
        }
        let now = self.ops.clock.now();
        for source in db.read_blocking(lis_interfaces::partner::intake_sources)? {
            if self.cancel.load(Ordering::Acquire) {
                return Ok(());
            }
            if self.due(&source, now) {
                self.read_source(&source);
            }
        }
        Ok(())
    }

    /// Whether `check every` has passed since the connection was last read; marks it read.
    fn due(&self, source: &IntakeSource, now: UtcTimestamp) -> bool {
        let mut last_read = locked(&self.last_read);
        let every = Duration::from_secs(u64::from(source.intake.check_every_seconds));
        let due = last_read
            .get(&source.connection_id)
            .and_then(|last| last.checked_add(every))
            .is_none_or(|next| next <= now);
        if due {
            last_read.insert(source.connection_id, now);
        }
        due
    }

    fn read_source(&self, source: &IntakeSource) {
        // A listener receives its messages as they come; only its waiting ones are read here.
        if source.destination.mllp_listener.is_some() {
            self.retry_waiting(source);
            return;
        }
        let connection_id = source.connection_id;
        let key = locked(&self.ops.key).usable().ok();
        let opened = Inbox::open(&self.ops.db, key.as_ref(), &source.destination);
        let names = opened.and_then(|mut inbox| inbox.list().map(|names| (inbox, names)));
        let problem = match names {
            Ok((mut inbox, names)) => self.read_files(&mut inbox, source, &names),
            Err(code) => {
                tracing::warn!(%connection_id, error_code = code, "incoming orders not read");
                Some(code)
            }
        };
        let now = self.ops.clock.now();
        self.readings.record(connection_id, now, problem);
        self.retry_waiting(source);
    }

    /// Reads the files in order; the first problem, if any.
    fn read_files(
        &self,
        inbox: &mut Inbox,
        source: &IntakeSource,
        names: &[String],
    ) -> Option<&'static str> {
        let mut problem = None;
        for name in names {
            if self.cancel.load(Ordering::Acquire) {
                break;
            }
            problem = problem.or(self.read_file(inbox, source, name));
        }
        problem
    }

    /// Stores, decides and moves one file; what stopped it, if anything. A file not read,
    /// stored or recorded stays in the input folder and is read again at the next reading.
    fn read_file(
        &self,
        inbox: &mut Inbox,
        source: &IntakeSource,
        name: &str,
    ) -> Option<&'static str> {
        let connection_id = source.connection_id;
        let db = &self.ops.db;
        let bytes = match inbox.read(name) {
            Ok(bytes) => bytes,
            Err(code) => {
                tracing::warn!(%connection_id, error_code = code, "incoming file not read");
                return Some(code);
            }
        };
        let sha256 = match db.artifacts().put(&bytes) {
            Ok(stored) => stored.digest.to_string(),
            Err(error) => {
                tracing::warn!(%connection_id, %error, "incoming file not stored");
                return Some("store_failed");
            }
        };
        let digest = sha256.clone();
        let earlier = db.read_blocking(move |conn| {
            lis_interfaces::partner::earlier_read(conn, connection_id, &digest)
        });
        let decided = match earlier {
            // Read before a crash interrupted its move: it is only moved now.
            Ok(Some(earlier)) if earlier.moved_to.is_none() => Some((earlier.id, earlier.rejected)),
            _ => {
                let file = ReadFile {
                    name: name.to_owned(),
                    sha256,
                    bytes,
                    peer: None,
                };
                receive(db, source, file).map(|(id, outcome)| (id, outcome.failed()))
            }
        };
        match decided {
            Some((id, failed)) => self.move_file(inbox, id, name, failed),
            None => Some("not_recorded"),
        }
    }

    /// Moves the file to Processed or Failed › year › month and records where it went; the
    /// move's problem, if it failed.
    fn move_file(
        &self,
        inbox: &mut Inbox,
        id: PartnerReceivedId,
        name: &str,
        failed: bool,
    ) -> Option<&'static str> {
        let db = &self.ops.db;
        let (date, _) = stamp(self.ops.clock.now(), lab_zone(db).as_ref());
        let moved = inbox.move_out(name, failed, (&date[..4], &date[4..6]));
        let path = match moved {
            Ok(path) => path,
            Err(code) => {
                tracing::warn!(received_id = %id, error_code = code, "incoming file not moved");
                return Some(code);
            }
        };
        let recorded = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
            lis_interfaces::partner::record_moved(ctx, id, &path)
        });
        match recorded {
            Ok(()) => None,
            Err(error) => {
                tracing::error!(received_id = %id, error_code = error.code().as_str(), "file move not recorded");
                Some("not_recorded")
            }
        }
    }

    /// Tries this connection's messages waiting for their patient again.
    fn retry_waiting(&self, source: &IntakeSource) {
        let db = &self.ops.db;
        let waiting = db
            .read_blocking(lis_interfaces::partner::waiting_messages)
            .unwrap_or_default();
        for message in waiting
            .into_iter()
            .filter(|message| message.connection_id == source.connection_id)
        {
            let Some(bytes) = message
                .artifact_sha256
                .parse::<ArtifactDigest>()
                .ok()
                .and_then(|digest| db.artifacts().read(&digest).ok())
            else {
                continue;
            };
            let read = source.clone();
            let retried = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
                decide::retry_waiting(ctx, &read, &message, &bytes)
            });
            if let Err(error) = retried {
                tracing::warn!(%error, "waiting order not tried again");
            }
        }
    }
}

/// Records what a stored message became, with the order it made; an order the order rules
/// refuse is recorded as refused, without an order. `None` when nothing could be recorded.
/// Blocking.
pub(crate) fn receive(
    db: &Database,
    source: &IntakeSource,
    file: ReadFile,
) -> Option<(PartnerReceivedId, ReceivedOutcome)> {
    let (kept, read) = (source.clone(), source.clone());
    let (name, sha256, peer) = (file.name.clone(), file.sha256.clone(), file.peer.clone());
    let decided = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
        decide::receive(ctx, &read, &file)
    });
    let refused = match decided {
        Ok(decided) => return Some(decided),
        Err(error) => error,
    };
    let connection_id = source.connection_id;
    tracing::warn!(%connection_id, error = %refused, "incoming order refused");
    let outcome = ReceivedOutcome::Rejected {
        reason: "order_refused",
    };
    let kept_outcome = outcome.clone();
    let recorded = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
        lis_interfaces::partner::record_received(
            ctx,
            NewReceived {
                connection_id: kept.connection_id,
                file_name: name,
                artifact_sha256: sha256,
                control_id: None,
                placer_order: None,
                outcome: kept_outcome,
                peer,
            },
        )
    });
    match recorded {
        Ok(id) => Some((id, outcome)),
        Err(error) => {
            tracing::error!(%connection_id, error_code = error.code().as_str(), "incoming message not recorded");
            None
        }
    }
}

impl Engine {
    /// One reading of every connection due now, as the reader does every 5 seconds from
    /// start. Scenario tests drive the reader with it and the clock.
    pub async fn read_partner_orders(&self) {
        let intake = PartnerIntake::new(&self.ops, &self.partner_readings);
        scan_off_runtime(Arc::new(intake)).await;
    }
}
