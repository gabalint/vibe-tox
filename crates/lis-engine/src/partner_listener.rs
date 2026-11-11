//! The MLLP listeners of HL7 Orders connections (ticket #4555 step 4b; REQ-INT-002,
//! REQ-INT-003, REQ-INT-006).
//!
//! From engine start, whoever is signed in (also nobody), every [`PASS_INTERVAL`] the
//! supervisor compares the running listeners with the started HL7 Orders connections whose
//! destination check covers their listener. It stops a listener whose connection stopped or
//! whose settings changed, and starts each missing one on its lab-network address and port;
//! one that cannot start is tried again at the next pass. Nothing listens while a recovery
//! reconciliation is open. Each listener serves its allowed partners ([`serve`]); whether
//! it listens shows on its connection. Logs carry IDs and codes, never addresses or content.

mod serve;

use std::collections::HashMap;
use std::io;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use lis_core::UtcTimestamp;
use lis_interfaces::partner::{
    ConnectionStatus, IntakeSource, ListenerView, MllpListener, PartnerConnection,
    PartnerConnectionId, valid_mllp_listener,
};
use tokio::net::TcpListener;
use tokio::sync::watch;
use tokio::task::JoinHandle;

use crate::engine::Engine;
use crate::ops_worker::{Operations, locked};
use crate::partner_delivery::held_by_recovery;
use crate::poller::{Poller, Scan, scan_off_runtime};

/// How often the listeners are compared with their connections.
const PASS_INTERVAL: Duration = Duration::from_secs(5);

/// One connection's listener: listening, or why it cannot.
struct Listener {
    source: IntakeSource,
    view: ListenerView,
    /// `None` when it could not start.
    running: Option<Running>,
}

/// A started listener's task and its stop signal: dropping the sender stops the listener and
/// its partners' connections, each after the message it is answering.
struct Running {
    stop: watch::Sender<bool>,
    task: JoinHandle<()>,
}

/// The supervisor's shared part.
pub(crate) struct PartnerListeners {
    ops: Arc<Operations>,
    /// Raised at shutdown; nothing listens after it.
    cancel: AtomicBool,
    listeners: Mutex<HashMap<PartnerConnectionId, Listener>>,
}

/// Starts the supervisor, whose first pass runs now. Must run inside a tokio runtime.
pub(crate) fn start(listeners: &Arc<PartnerListeners>) -> Poller {
    let scanner: Arc<dyn Scan> = Arc::clone(listeners) as Arc<dyn Scan>;
    Poller::start(scanner, PASS_INTERVAL)
}

impl Scan for PartnerListeners {
    fn scan(&self) {
        self.reconcile();
    }

    fn cancel(&self) {
        self.cancel.store(true, Ordering::Release);
        locked(&self.listeners).clear();
    }
}

impl PartnerListeners {
    pub(crate) fn new(ops: &Arc<Operations>) -> Self {
        Self {
            ops: Arc::clone(ops),
            cancel: AtomicBool::new(false),
            listeners: Mutex::new(HashMap::new()),
        }
    }

    /// The connections to listen for now; `None` when they cannot be read, which leaves
    /// the listeners as they are.
    fn wanted(&self) -> Option<Vec<IntakeSource>> {
        let db = &self.ops.db;
        if self.cancel.load(Ordering::Acquire) || held_by_recovery(db) {
            return Some(Vec::new());
        }
        match db.read_blocking(lis_interfaces::partner::intake_sources) {
            Ok(sources) => Some(
                sources
                    .into_iter()
                    .filter(|source| source.destination.mllp_listener.is_some())
                    .collect(),
            ),
            Err(error) => {
                tracing::warn!(
                    error_code = error.code().as_str(),
                    "partner listeners not read"
                );
                None
            }
        }
    }

    /// Stops the listeners no longer wanted as they are, then starts the missing ones.
    fn reconcile(&self) {
        let Some(wanted) = self.wanted() else {
            return;
        };
        let mut listeners = locked(&self.listeners);
        let ended: Vec<PartnerConnectionId> = listeners
            .iter()
            .filter(|(_, listener)| !wanted.contains(&listener.source))
            .map(|(id, _)| *id)
            .collect();
        for id in ended {
            if let Some(listener) = listeners.remove(&id) {
                stop(id, listener);
            }
        }
        let now = self.ops.clock.now();
        for source in wanted {
            let id = source.connection_id;
            if listeners
                .get(&id)
                .is_some_and(|kept| kept.running.is_some())
            {
                continue;
            }
            let earlier = listeners.remove(&id);
            listeners.insert(id, self.open(source, earlier.as_ref(), now));
        }
    }

    /// Binds the connection's address and port and starts serving it. A listener that
    /// still cannot start for the same reason keeps the time it first failed.
    fn open(
        &self,
        source: IntakeSource,
        earlier: Option<&Listener>,
        now: UtcTimestamp,
    ) -> Listener {
        let id = source.connection_id;
        let bound = source
            .destination
            .mllp_listener
            .as_ref()
            .ok_or("listener_failed")
            .and_then(bind);
        match bound {
            Ok(socket) => {
                let (stop, stopped) = watch::channel(false);
                let task = tokio::spawn(serve::listen(
                    socket,
                    Arc::clone(&self.ops),
                    source.clone(),
                    stopped,
                ));
                tracing::info!(connection_id = %id, "partner listener started");
                Listener {
                    source,
                    view: ListenerView {
                        listening: true,
                        problem: None,
                        since: now,
                    },
                    running: Some(Running { stop, task }),
                }
            }
            Err(code) => {
                let since = earlier
                    .filter(|earlier| earlier.view.problem.as_deref() == Some(code))
                    .map(|earlier| earlier.view.since);
                if since.is_none() {
                    tracing::warn!(connection_id = %id, error_code = code, "partner listener not started");
                }
                Listener {
                    source,
                    view: ListenerView {
                        listening: false,
                        problem: Some(code.to_owned()),
                        since: since.unwrap_or(now),
                    },
                    running: None,
                }
            }
        }
    }

    /// The connection's listener as it is now; `None` while it does not try to listen.
    pub(crate) fn view(&self, id: PartnerConnectionId) -> Option<ListenerView> {
        locked(&self.listeners)
            .get(&id)
            .map(|listener| listener.view.clone())
    }

    /// Whether the connection's own listener listens on `socket` now.
    pub(crate) fn listens_on(&self, id: PartnerConnectionId, listener: &MllpListener) -> bool {
        locked(&self.listeners).get(&id).is_some_and(|running| {
            running.running.is_some()
                && running
                    .source
                    .destination
                    .mllp_listener
                    .as_ref()
                    .and_then(MllpListener::socket)
                    == listener.socket()
        })
    }

    /// Adds the listener's state to `connection`; one that cannot listen is in error.
    pub(crate) fn add_view(&self, connection: &mut PartnerConnection) {
        connection.listener = self.view(connection.id);
        if connection
            .listener
            .as_ref()
            .is_some_and(|listener| !listener.listening)
        {
            connection.status = ConnectionStatus::Error;
        }
    }
}

/// Stops a listener and waits until its port is free. Blocking.
fn stop(id: PartnerConnectionId, listener: Listener) {
    let Some(Running { stop, task }) = listener.running else {
        return;
    };
    drop(stop);
    if tokio::runtime::Handle::current().block_on(task).is_err() {
        tracing::error!(connection_id = %id, "partner listener ended with a panic");
    }
    tracing::info!(connection_id = %id, "partner listener stopped");
}

/// Binds the listener's lab-network address and port, or says why it cannot: a PHI-free
/// code such as `port_in_use`.
pub(crate) fn bind(listener: &MllpListener) -> Result<TcpListener, &'static str> {
    let socket = valid_mllp_listener(listener.clone())
        .ok()
        .and_then(|listener| listener.socket())
        .ok_or("not_lab_network")?;
    std::net::TcpListener::bind(socket)
        .and_then(|bound| {
            bound.set_nonblocking(true)?;
            TcpListener::from_std(bound)
        })
        .map_err(|error| bind_problem(&error))
}

fn bind_problem(error: &io::Error) -> &'static str {
    match error.kind() {
        io::ErrorKind::AddrInUse => "port_in_use",
        io::ErrorKind::AddrNotAvailable => "address_not_on_this_computer",
        io::ErrorKind::PermissionDenied => "port_not_allowed",
        _ => "listener_failed",
    }
}

impl Engine {
    /// One supervisor pass now, as it runs every 5 seconds from start. Scenario tests start
    /// and stop the listeners with it.
    pub async fn listen_for_partners(&self) {
        let scanner: Arc<dyn Scan> = Arc::clone(&self.partner_listeners) as Arc<dyn Scan>;
        scan_off_runtime(scanner).await;
    }
}
