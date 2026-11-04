//! The incoming-order reader (ticket #4555 step 3c; REQ-INT-002, REQ-INT-003, REQ-ORD-001).
//!
//! From engine start, whoever is signed in (also nobody), every [`PASS_INTERVAL`] it reads
//! each started HL7 Orders connection whose destination check covers its folder or server,
//! once the connection's `check every` has passed: up to [`inbox::MAX_FILES`] complete
//! `*.hl7` files. A file's bytes are stored first; then one transaction records what the
//! message became, with the order it made ([`decide`]); then the file moves to Processed or
//! Failed › year › month and the move is recorded. A file read again after a crash before
//! its move is only moved. Messages waiting for their patient are tried again at each
//! reading. Nothing is read while a recovery reconciliation is open. Logs carry IDs and
//! codes, never file names, folders or content.

mod decide;
mod inbox;
mod order_input;

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lis_core::{Capability, CorrelationId, Principal, UtcTimestamp};
use lis_interfaces::partner::{
    IntakeSource, NewReceived, PartnerConnectionId, PartnerError, PartnerReceivedId,
    ReceivedOutcome,
};
use lis_store::ArtifactDigest;

use self::decide::ReadFile;
use self::inbox::Inbox;
pub(crate) use self::inbox::candidate;
use crate::engine::Engine;
use crate::ops_worker::{Operations, locked};
use crate::partner_delivery::{lab_zone, stamp};
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
}

/// Starts the reader, whose first pass runs now. Must run inside a tokio runtime.
pub(crate) fn start(ops: &Arc<Operations>) -> Poller {
    Poller::start(Arc::new(PartnerIntake::new(ops)), PASS_INTERVAL)
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
    fn new(ops: &Arc<Operations>) -> Self {
        Self {
            ops: Arc::clone(ops),
            cancel: AtomicBool::new(false),
            last_read: Mutex::new(HashMap::new()),
        }
    }

    fn pass(&self) -> Result<(), IntakeError> {
        let db = &self.ops.db;
        if db
            .read_blocking(|conn| lis_ops::recovery::outbound_gate(conn).map(|gate| gate.is_some()))
            .unwrap_or(true)
        {
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
        let connection_id = source.connection_id;
        let key = locked(&self.ops.key).usable().ok();
        let opened = Inbox::open(&self.ops.db, key.as_ref(), &source.destination);
        let names = opened.and_then(|mut inbox| inbox.list().map(|names| (inbox, names)));
        match names {
            Ok((mut inbox, names)) => self.read_files(&mut inbox, source, &names),
            Err(code) => {
                tracing::warn!(%connection_id, error_code = code, "incoming orders not read");
            }
        }
        self.retry_waiting(source);
    }

    fn read_files(&self, inbox: &mut Inbox, source: &IntakeSource, names: &[String]) {
        for name in names {
            if self.cancel.load(Ordering::Acquire) {
                return;
            }
            self.read_file(inbox, source, name);
        }
    }

    /// Stores, decides and moves one file.
    fn read_file(&self, inbox: &mut Inbox, source: &IntakeSource, name: &str) {
        let connection_id = source.connection_id;
        let db = &self.ops.db;
        let bytes = match inbox.read(name) {
            Ok(bytes) => bytes,
            Err(code) => {
                tracing::warn!(%connection_id, error_code = code, "incoming file not read");
                return;
            }
        };
        let sha256 = match db.artifacts().put(&bytes) {
            Ok(stored) => stored.digest.to_string(),
            Err(error) => {
                tracing::warn!(%connection_id, %error, "incoming file not stored");
                return;
            }
        };
        let digest = sha256.clone();
        let earlier = db.read_blocking(move |conn| {
            lis_interfaces::partner::earlier_read(conn, connection_id, &digest)
        });
        let decided = match earlier {
            // Read before a crash interrupted its move: it is only moved now.
            Ok(Some(earlier)) if earlier.moved_to.is_none() => Some((earlier.id, earlier.rejected)),
            _ => self.receive(
                source,
                ReadFile {
                    name: name.to_owned(),
                    sha256,
                    bytes,
                },
            ),
        };
        if let Some((id, failed)) = decided {
            self.move_file(inbox, id, name, failed);
        }
    }

    /// Records what the message became; an order the order rules refuse is recorded as
    /// refused, without an order. Returns the record and whether the file goes to Failed.
    fn receive(&self, source: &IntakeSource, file: ReadFile) -> Option<(PartnerReceivedId, bool)> {
        let db = &self.ops.db;
        let (kept, read) = (source.clone(), source.clone());
        let (name, sha256) = (file.name.clone(), file.sha256.clone());
        let decided = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
            decide::receive(ctx, &read, &file)
        });
        let refused = match decided {
            Ok((id, outcome)) => return Some((id, outcome.failed())),
            Err(error) => error,
        };
        let connection_id = source.connection_id;
        tracing::warn!(%connection_id, error = %refused, "incoming order refused");
        let recorded = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
            lis_interfaces::partner::record_received(
                ctx,
                NewReceived {
                    connection_id: kept.connection_id,
                    file_name: name,
                    artifact_sha256: sha256,
                    control_id: None,
                    placer_order: None,
                    outcome: ReceivedOutcome::Rejected {
                        reason: "order_refused",
                    },
                },
            )
        });
        match recorded {
            Ok(id) => Some((id, true)),
            Err(error) => {
                tracing::error!(%connection_id, error_code = error.code().as_str(), "incoming message not recorded");
                None
            }
        }
    }

    /// Moves the file to Processed or Failed › year › month and records where it went.
    fn move_file(&self, inbox: &mut Inbox, id: PartnerReceivedId, name: &str, failed: bool) {
        let db = &self.ops.db;
        let (date, _) = stamp(self.ops.clock.now(), lab_zone(db).as_ref());
        let moved = inbox.move_out(name, failed, (&date[..4], &date[4..6]));
        let path = match moved {
            Ok(path) => path,
            Err(code) => {
                tracing::warn!(received_id = %id, error_code = code, "incoming file not moved");
                return;
            }
        };
        let recorded = db.write_blocking(worker(), CorrelationId::generate(), move |ctx| {
            lis_interfaces::partner::record_moved(ctx, id, &path)
        });
        if let Err(error) = recorded {
            tracing::error!(received_id = %id, error_code = error.code().as_str(), "file move not recorded");
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

impl Engine {
    /// One reading of every connection due now, as the reader does every 5 seconds from
    /// start. Scenario tests drive the reader with it and the clock.
    pub async fn read_partner_orders(&self) {
        scan_off_runtime(Arc::new(PartnerIntake::new(&self.ops))).await;
    }
}
