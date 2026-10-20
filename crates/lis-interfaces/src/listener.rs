use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use lis_core::{Clock, InstrumentId, SystemClock};
use lis_store::Database;

use crate::model::{ConnectionSettings, Instrument};
use crate::session::StreamContext;
use crate::status::{ListenerDetailCode, ListenerState, ListenerStatus, StatusReport};
use crate::tcp::TcpListenerConfig;
use crate::{InterfaceError, ListenerErrorCode, repo};

pub(crate) const IO_POLL: Duration = Duration::from_millis(50);
/// Maximum time a new TCP connection may remain idle before sending ENQ.
pub const DEFAULT_ESTABLISHMENT_TIMEOUT: Duration = Duration::from_secs(5);
pub(crate) type StatusMap = Arc<Mutex<BTreeMap<InstrumentId, ListenerStatus>>>;

/// Supervisor lifecycle failures; per-listener failures remain visible in status.
#[derive(Debug, thiserror::Error)]
pub enum ListenerSupervisorError {
    /// Reading configured instruments failed.
    #[error(transparent)]
    Interface(#[from] InterfaceError),
    /// Start was requested while workers from an earlier start still exist.
    #[error("listener supervisor is already running")]
    AlreadyRunning,
    /// At least one configured listener could not be opened or spawned.
    #[error("one or more listeners failed to start")]
    StartFailed,
    /// A listener thread panicked during shutdown.
    #[error("a listener thread stopped unexpectedly")]
    ThreadStopped,
}

/// Owns one blocking worker thread per enabled analyzer connection.
///
/// Call [`start_all`](Self::start_all) after startup reconciliation and
/// [`stop_all`](Self::stop_all) before closing the database. Status snapshots
/// require no signed-in application session (REQ-OPS-013).
pub struct ListenerSupervisor {
    database: Arc<Database>,
    statuses: StatusMap,
    workers: Mutex<Vec<Worker>>,
    establishment_timeout: Duration,
}

impl ListenerSupervisor {
    /// Creates an idle supervisor around the open local system of record.
    #[must_use]
    pub fn new(database: Arc<Database>) -> Self {
        Self {
            database,
            statuses: Arc::new(Mutex::new(BTreeMap::new())),
            workers: Mutex::new(Vec::new()),
            establishment_timeout: DEFAULT_ESTABLISHMENT_TIMEOUT,
        }
    }

    #[cfg(test)]
    pub(crate) fn with_establishment_timeout(
        database: Arc<Database>,
        establishment_timeout: Duration,
    ) -> Self {
        Self {
            database,
            statuses: Arc::new(Mutex::new(BTreeMap::new())),
            workers: Mutex::new(Vec::new()),
            establishment_timeout,
        }
    }

    /// Starts every enabled configured listener and records failures in status.
    /// Successfully opened listeners remain active if another listener fails.
    pub fn start_all(&self) -> Result<(), ListenerSupervisorError> {
        let mut workers = self.workers.lock().unwrap_or_else(PoisonError::into_inner);
        if !workers.is_empty() {
            return Err(ListenerSupervisorError::AlreadyRunning);
        }
        self.start_locked(&mut workers)
    }

    /// Stops every listener, then starts every enabled one with the stored
    /// configuration, so a configuration change takes effect (REQ-INT-009).
    /// A transfer in progress is interrupted like at [`stop_all`](Self::stop_all).
    /// The lock is held throughout, so concurrent restarts run one after the
    /// other and the last one reads the latest configuration.
    pub fn restart_all(&self) -> Result<(), ListenerSupervisorError> {
        let mut workers = self.workers.lock().unwrap_or_else(PoisonError::into_inner);
        let stopped = self.stop_locked(&mut workers);
        let started = self.start_locked(&mut workers);
        stopped.and(started)
    }

    fn start_locked(&self, workers: &mut Vec<Worker>) -> Result<(), ListenerSupervisorError> {
        let instruments = self.database.read_blocking(repo::all_instruments)?;
        let mut failed = false;
        for instrument in instruments {
            if !instrument.enabled {
                set_status(
                    &self.statuses,
                    instrument.id,
                    ListenerState::Disabled,
                    Some(ListenerDetailCode::ConfigurationDisabled),
                );
                continue;
            }
            match spawn_worker(
                Arc::clone(&self.database),
                Arc::clone(&self.statuses),
                instrument,
                self.establishment_timeout,
            ) {
                Ok(Some(worker)) => workers.push(worker),
                Ok(None) => {}
                Err(()) => failed = true,
            }
        }
        if failed {
            Err(ListenerSupervisorError::StartFailed)
        } else {
            Ok(())
        }
    }

    /// Signals every listener, waits for bounded polling to stop, and joins it.
    pub fn stop_all(&self) -> Result<(), ListenerSupervisorError> {
        let mut workers = self.workers.lock().unwrap_or_else(PoisonError::into_inner);
        self.stop_locked(&mut workers)
    }

    fn stop_locked(&self, workers: &mut Vec<Worker>) -> Result<(), ListenerSupervisorError> {
        for worker in &*workers {
            worker.stop.store(true, Ordering::Release);
        }
        let mut thread_stopped = false;
        for worker in workers.drain(..) {
            if worker.handle.join().is_err() {
                thread_stopped = true;
            }
            set_status(
                &self.statuses,
                worker.instrument_id,
                ListenerState::Disabled,
                Some(ListenerDetailCode::ListenerStopped),
            );
        }
        if thread_stopped {
            Err(ListenerSupervisorError::ThreadStopped)
        } else {
            Ok(())
        }
    }

    /// Records the folder poller's state for a result-file instrument (F13). It
    /// never replaces the status of a running ASTM listener, which a
    /// configuration change may have started for the same instrument meanwhile.
    pub fn report_folder_status(&self, instrument_id: InstrumentId, report: StatusReport) {
        let workers = self.workers.lock().unwrap_or_else(PoisonError::into_inner);
        if workers
            .iter()
            .all(|worker| worker.instrument_id != instrument_id)
        {
            put_status(&self.statuses, instrument_id, report);
        }
    }

    /// Returns a PHI-free, instrument-ordered point-in-time status snapshot.
    #[must_use]
    pub fn status_snapshot(&self) -> Vec<ListenerStatus> {
        self.statuses
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .values()
            .cloned()
            .collect()
    }
}

struct Worker {
    instrument_id: InstrumentId,
    stop: Arc<AtomicBool>,
    handle: JoinHandle<()>,
}

/// Starts the listener thread of an ASTM connection. A result-file instrument
/// has none: the engine's folder poller watches it.
fn spawn_worker(
    database: Arc<Database>,
    statuses: StatusMap,
    instrument: Instrument,
    establishment_timeout: Duration,
) -> Result<Option<Worker>, ()> {
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let instrument_id = instrument.id;
    let name = format!("ifc-listener-{instrument_id}");
    let failure_statuses = Arc::clone(&statuses);
    let transport = instrument.connection.transport();
    let au = instrument.connection.au_settings().cloned();
    let handle = match instrument.connection {
        ConnectionSettings::AstmTcp {
            bind_address,
            port,
            allowed_peers,
        }
        | ConnectionSettings::AuTcp {
            bind_address,
            port,
            allowed_peers,
            ..
        } => crate::tcp::spawn(
            name,
            TcpListenerConfig {
                database,
                statuses,
                instrument_id,
                bind_address,
                port,
                allowed_peers,
                stop: thread_stop,
                establishment_timeout,
                au,
            },
        ),
        ConnectionSettings::AstmSerial {
            port_path,
            baud_rate,
            data_bits,
            parity,
            stop_bits,
        }
        | ConnectionSettings::AuSerial {
            port_path,
            baud_rate,
            data_bits,
            parity,
            stop_bits,
            ..
        } => {
            let port = crate::serial::open(port_path, baud_rate, data_bits, parity, stop_bits);
            let Ok(mut port) = port else {
                return start_failed(&statuses, instrument_id);
            };
            thread::Builder::new().name(name).spawn(move || {
                let context = StreamContext {
                    database: &database,
                    statuses: &statuses,
                    instrument_id,
                    transport,
                    peer: None,
                    stop: &thread_stop,
                    establishment_timeout,
                };
                crate::serial::run(port.as_mut(), &context, au.as_ref());
            })
        }
        ConnectionSettings::ResultFile {} => return Ok(None),
    };
    match handle {
        Ok(handle) => Ok(Some(Worker {
            instrument_id,
            stop,
            handle,
        })),
        Err(_) => start_failed(&failure_statuses, instrument_id),
    }
}

fn start_failed(statuses: &StatusMap, instrument_id: InstrumentId) -> Result<Option<Worker>, ()> {
    set_status(
        statuses,
        instrument_id,
        ListenerState::Error,
        Some(ListenerDetailCode::OpenFailed),
    );
    tracing::warn!(instrument_id = %instrument_id, error_code = ?ListenerErrorCode::OpenFailed, "listener failed to start");
    Err(())
}

pub(crate) fn detail_for_error(code: ListenerErrorCode) -> ListenerDetailCode {
    match code {
        ListenerErrorCode::OpenFailed => ListenerDetailCode::OpenFailed,
        ListenerErrorCode::TransportFailed => ListenerDetailCode::TransportFailed,
        ListenerErrorCode::PersistenceFailed => ListenerDetailCode::PersistenceFailed,
        ListenerErrorCode::ProtocolFailed => ListenerDetailCode::ProtocolFailed,
        ListenerErrorCode::EstablishmentTimeout => ListenerDetailCode::EstablishmentTimeout,
        ListenerErrorCode::UnexpectedPeer => ListenerDetailCode::UnexpectedPeerRejected,
    }
}

pub(crate) fn set_status(
    statuses: &StatusMap,
    instrument_id: InstrumentId,
    state: ListenerState,
    detail_code: Option<ListenerDetailCode>,
) {
    let report = StatusReport {
        state,
        detail_code,
        files_waiting: None,
        files_too_large: None,
    };
    put_status(statuses, instrument_id, report);
}

fn put_status(statuses: &StatusMap, instrument_id: InstrumentId, report: StatusReport) {
    statuses
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            instrument_id,
            ListenerStatus {
                instrument_id,
                state: report.state,
                last_activity_at: Some(SystemClock.now()),
                detail_code: report.detail_code,
                files_waiting: report.files_waiting,
                files_too_large: report.files_too_large,
            },
        );
}

pub(crate) fn touch_status(statuses: &StatusMap, instrument_id: InstrumentId) {
    if let Some(status) = statuses
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .get_mut(&instrument_id)
    {
        status.last_activity_at = Some(SystemClock.now());
    }
}

#[cfg(test)]
mod tests;
