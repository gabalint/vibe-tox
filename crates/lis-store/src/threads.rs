//! OS threads that each own one SQLite connection and run submitted jobs.
//!
//! The writer is one thread with the only write connection, so writes are
//! serialized. Readers are several threads with read-only connections that
//! share one job queue.

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex, PoisonError};
use std::thread::{self, JoinHandle};

use rusqlite::Connection;

/// Work for a connection thread. It reports its own result through a channel it captured.
pub(crate) type Job = Box<dyn FnOnce(&mut Connection) + Send>;

pub(crate) struct ConnectionThreads {
    role: &'static str,
    sender: Option<Sender<Job>>,
    handles: Vec<JoinHandle<()>>,
}

impl ConnectionThreads {
    /// Starts one thread per connection.
    pub(crate) fn spawn(role: &'static str, connections: Vec<Connection>) -> std::io::Result<Self> {
        let (sender, receiver) = mpsc::channel::<Job>();
        let receiver = Arc::new(Mutex::new(receiver));
        let mut handles = Vec::with_capacity(connections.len());
        for (index, connection) in connections.into_iter().enumerate() {
            let receiver = Arc::clone(&receiver);
            let handle = thread::Builder::new()
                .name(format!("lis-db-{role}-{index}"))
                .spawn(move || serve(connection, &receiver))?;
            handles.push(handle);
        }
        Ok(Self {
            role,
            sender: Some(sender),
            handles,
        })
    }

    /// True when called from one of these threads.
    pub(crate) fn runs_on_current_thread(&self) -> bool {
        let current = thread::current().id();
        self.handles
            .iter()
            .any(|handle| handle.thread().id() == current)
    }

    /// Queues `job`. Returns false when every thread has stopped.
    pub(crate) fn submit(&self, job: Job) -> bool {
        self.sender
            .as_ref()
            .is_some_and(|sender| sender.send(job).is_ok())
    }
}

/// Runs jobs until the queue closes. A panicking job unwinds and ends the
/// thread: its transaction rolls back, its caller sees the thread as stopped,
/// and the panic reaches the panic hook. It is never caught and resumed.
fn serve(mut connection: Connection, receiver: &Mutex<Receiver<Job>>) {
    loop {
        // The guard is a temporary, so the lock is released as soon as a job arrives.
        let next = receiver
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .recv();
        let Ok(job) = next else {
            return;
        };
        job(&mut connection);
    }
}

impl Drop for ConnectionThreads {
    /// Closes the queue, lets queued jobs finish and joins every thread.
    fn drop(&mut self) {
        drop(self.sender.take());
        for handle in self.handles.drain(..) {
            if handle.join().is_err() {
                tracing::error!(role = self.role, "database thread ended with a panic");
            }
        }
    }
}
