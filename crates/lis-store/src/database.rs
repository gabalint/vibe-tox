//! Opening the database, and the write and read paths.

use std::fs::{self, File, OpenOptions, TryLockError};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use lis_core::{Clock, CorrelationId, ModuleSchema, Principal};
use rusqlite::{Connection, OpenFlags};
use tokio::sync::oneshot;

use crate::artifacts::{ArtifactStore, MAX_ARTIFACT_BYTES};
use crate::context::{CommandContext, run_unit_of_work};
use crate::error::StoreError;
use crate::migrate::migrate;
use crate::snapshot::{Snapshot, pre_migration_snapshot, snapshot};
use crate::threads::{ConnectionThreads, Job};
use crate::transaction_guard::TransactionControlGuard;

/// Database file name inside the data directory.
pub const DATABASE_FILE: &str = "lis.sqlite3";
/// Lock file that keeps a second instance from opening the same data directory.
const LOCK_FILE: &str = "lis.lock";
/// Directory of content-addressed artifacts inside the data directory.
pub const ARTIFACTS_DIR: &str = "artifacts";
/// How long a connection waits for a lock before failing.
pub(crate) const BUSY_TIMEOUT: Duration = Duration::from_secs(5);
/// Number of read-only connections.
const READ_CONNECTIONS: usize = 4;

/// The schemas every database has, applied before the caller's: audit, then store.
pub(crate) fn built_in_schemas() -> [ModuleSchema; 2] {
    [lis_audit::SCHEMA, crate::SCHEMA]
}

/// The open system of record (NFR-01, NFR-02, ADR 0012).
///
/// One OS thread owns the only write connection; every write runs on it in a
/// `BEGIN IMMEDIATE` transaction. Reads run on a small pool of read-only
/// (`query_only`) connections and see committed snapshots. Dropping the database lets queued
/// work finish, stops the threads and then releases the lock.
pub struct Database {
    writer: ConnectionThreads,
    readers: ConnectionThreads,
    artifacts: ArtifactStore,
    clock: Arc<dyn Clock>,
    database_file: PathBuf,
    // Declared last so it is released after the threads have stopped.
    _lock: File,
}

impl Database {
    /// Opens the database in `data_dir`, creating it if needed.
    ///
    /// Takes the single-instance lock, sets the pragmas (WAL, `synchronous=FULL`,
    /// foreign keys, busy timeout) and applies the audit and store schemas and
    /// then `modules`, in the given (crate-map) order.
    ///
    /// When migrations are pending on a database that already has applied
    /// ones, it first keeps an integrity-checked copy in
    /// [`PRE_MIGRATION_DIR`](crate::PRE_MIGRATION_DIR) (newest 3) and refuses
    /// to migrate if that fails (NFR-14).
    pub fn open(
        data_dir: &Path,
        modules: &[ModuleSchema],
        clock: Arc<dyn Clock>,
    ) -> Result<Self, StoreError> {
        fs::create_dir_all(data_dir)?;
        let lock = lock(&data_dir.join(LOCK_FILE))?;
        let database_file = data_dir.join(DATABASE_FILE);
        let mut writer = Connection::open(&database_file)?;
        configure_writer(&writer)?;
        let schemas: Vec<ModuleSchema> = built_in_schemas()
            .into_iter()
            .chain(modules.iter().copied())
            .collect();
        let now = clock.now();
        migrate(&mut writer, &schemas, now, || {
            pre_migration_snapshot(data_dir, &database_file, now)
        })?;
        let readers = (0..READ_CONNECTIONS)
            .map(|_| open_reader(&database_file))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            writer: ConnectionThreads::spawn("writer", vec![writer])?,
            readers: ConnectionThreads::spawn("reader", readers)?,
            artifacts: ArtifactStore::open(data_dir.join(ARTIFACTS_DIR), MAX_ARTIFACT_BYTES)?,
            clock,
            database_file,
            _lock: lock,
        })
    }

    /// Runs `command` on the writer thread in a `BEGIN IMMEDIATE` transaction
    /// and awaits its result: commit on `Ok`, roll back on `Err`.
    ///
    /// Once submitted, the command runs to completion even if this future is
    /// dropped. If a command panics, its transaction rolls back, the writer
    /// stops, and this and every later write return
    /// [`StoreError::WriterStopped`].
    pub async fn write<T, E, F>(
        &self,
        principal: Principal,
        correlation_id: CorrelationId,
        command: F,
    ) -> Result<T, E>
    where
        F: WriteCommand<T, E>,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        let response = self.submit_write(principal, correlation_id, command)?;
        response.await.map_err(|_| StoreError::WriterStopped)?
    }

    // jscpd:ignore-start (sync twin of `write`: same signature by design; bodies share `submit_write`)
    /// [`write`](Self::write) for synchronous callers. Called from an async
    /// task it panics (tokio refuses to block a runtime thread). Called from
    /// inside a command it returns [`StoreError::NestedWrite`] instead of
    /// waiting on itself forever.
    pub fn write_blocking<T, E, F>(
        &self,
        principal: Principal,
        correlation_id: CorrelationId,
        command: F,
    ) -> Result<T, E>
    where
        F: WriteCommand<T, E>,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        let response = self.submit_write(principal, correlation_id, command)?;
        response
            .blocking_recv()
            .map_err(|_| StoreError::WriterStopped)?
    }
    // jscpd:ignore-end

    /// Runs `query` on a read-only connection inside one read transaction,
    /// so it sees a single committed snapshot, and awaits its result.
    pub async fn read<T, E, F>(&self, query: F) -> Result<T, E>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        let response = self.submit_read(query)?;
        response.await.map_err(|_| StoreError::ReadersStopped)?
    }

    /// [`read`](Self::read) for synchronous callers. Called from an async task
    /// it panics (tokio refuses to block a runtime thread).
    pub fn read_blocking<T, E, F>(&self, query: F) -> Result<T, E>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        let response = self.submit_read(query)?;
        response
            .blocking_recv()
            .map_err(|_| StoreError::ReadersStopped)?
    }

    /// The content-addressed artifact store in the data directory.
    #[must_use]
    pub fn artifacts(&self) -> &ArtifactStore {
        &self.artifacts
    }

    /// Writes a `VACUUM INTO` snapshot to `destination`, which must not exist,
    /// and runs `PRAGMA integrity_check` on it.
    ///
    /// Blocking file I/O: call it outside async tasks.
    pub fn snapshot(&self, destination: &Path) -> Result<Snapshot, StoreError> {
        snapshot(&self.database_file, destination)
    }

    fn submit_write<T, E, F>(
        &self,
        principal: Principal,
        correlation_id: CorrelationId,
        command: F,
    ) -> Result<Response<T, E>, E>
    where
        F: WriteCommand<T, E>,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        if self.writer.runs_on_current_thread() {
            return Err(StoreError::NestedWrite.into());
        }
        let (respond, response) = oneshot::channel();
        let clock = Arc::clone(&self.clock);
        let job: Job = Box::new(move |connection| {
            let result = run_unit_of_work(
                connection,
                &principal,
                correlation_id,
                clock.as_ref(),
                command,
            );
            // The caller may have stopped waiting; the outcome is already durable or rolled back.
            let _ = respond.send(result);
        });
        if self.writer.submit(job) {
            Ok(response)
        } else {
            Err(StoreError::WriterStopped.into())
        }
    }

    fn submit_read<T, E, F>(&self, query: F) -> Result<Response<T, E>, E>
    where
        F: FnOnce(&Connection) -> Result<T, E> + Send + 'static,
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
    {
        let (respond, response) = oneshot::channel();
        let job: Job = Box::new(move |connection| {
            let result = run_read_callback(connection, query);
            // The caller may have stopped waiting; a read changes nothing.
            let _ = respond.send(result);
        });
        if self.readers.submit(job) {
            Ok(response)
        } else {
            Err(StoreError::ReadersStopped.into())
        }
    }
}

fn run_read_callback<T, E, F>(connection: &mut Connection, query: F) -> Result<T, E>
where
    F: FnOnce(&Connection) -> Result<T, E>,
    E: From<StoreError>,
{
    let tx = connection
        .transaction()
        .map_err(|error| E::from(StoreError::from(error)))?;
    let guard = TransactionControlGuard::install(&tx).map_err(E::from)?;
    let result = query(&tx);
    if guard.finish().map_err(E::from)? {
        return Err(StoreError::TransactionEnded.into());
    }
    if tx.is_autocommit() {
        return Err(StoreError::TransactionEnded.into());
    }
    result
}

/// A unit of work for the single writer: it runs inside one `BEGIN IMMEDIATE`
/// transaction that commits on `Ok` and rolls back on `Err`.
pub trait WriteCommand<T, E>:
    FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static
{
}

impl<T, E, F> WriteCommand<T, E> for F where
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static
{
}

/// The pending result of a job submitted to the writer or a reader thread.
type Response<T, E> = oneshot::Receiver<Result<T, E>>;

/// Takes an exclusive lock on `path`, held until the returned file is dropped (REQ-OPS-013).
fn lock(path: &Path) -> Result<File, StoreError> {
    let file = OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .open(path)?;
    match file.try_lock() {
        Ok(()) => Ok(file),
        Err(TryLockError::WouldBlock) => Err(StoreError::AlreadyOpen),
        Err(TryLockError::Error(error)) => Err(error.into()),
    }
}

fn configure_writer(conn: &Connection) -> Result<(), StoreError> {
    let mode: String =
        conn.pragma_update_and_check(None, "journal_mode", "WAL", |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("wal") {
        return Err(StoreError::WalUnavailable);
    }
    configure_connection(conn)
}

/// Read-only at open and `query_only`, so a query cannot write even by changing a pragma.
fn open_reader(database_file: &Path) -> Result<Connection, StoreError> {
    let flags = OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX;
    let conn = Connection::open_with_flags(database_file, flags)?;
    configure_connection(&conn)?;
    conn.pragma_update(None, "query_only", true)?;
    // Measured historical review scans reuse more than SQLite's default 16 statements.
    // This retains query plans only; transaction guards still flush at both boundaries.
    conn.set_prepared_statement_cache_capacity(64);
    Ok(conn)
}

/// Per-connection settings. WAL is a property of the file, set by the writer.
fn configure_connection(conn: &Connection) -> Result<(), StoreError> {
    // FULL, not NORMAL: NORMAL can lose committed transactions on power loss (ENG-99).
    conn.pragma_update(None, "synchronous", "FULL")?;
    // On macOS, plain fsync does not flush the drive cache; F_FULLFSYNC does.
    // Both pragmas have no effect on other systems.
    conn.pragma_update(None, "fullfsync", true)?;
    conn.pragma_update(None, "checkpoint_fullfsync", true)?;
    conn.pragma_update(None, "foreign_keys", true)?;
    conn.busy_timeout(BUSY_TIMEOUT)?;
    Ok(())
}
