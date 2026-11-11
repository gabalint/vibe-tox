//! Fixtures for tests in this and other crates: a temporary database, a fixed
//! clock and principals with chosen capabilities. Enable with the `testing`
//! feature from `[dev-dependencies]`.

use std::net::{Ipv4Addr, TcpListener};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use lis_core::{
    Capability, FixedClock, ModuleSchema, Principal, SessionId, UserId, UtcTimestamp, WorkstationId,
};
use tempfile::TempDir;

use crate::database::Database;
use crate::error::StoreError;

/// The instant every fixed test clock starts at.
pub const TEST_START: &str = "2026-01-02T03:04:05.000000000Z";

/// A database in a temporary directory, removed when dropped.
pub struct TestDatabase {
    /// The open database.
    pub db: Database,
    /// Its clock; move it with `clock.set(…)`.
    pub clock: Arc<FixedClock>,
    modules: Vec<ModuleSchema>,
    dir: TempDir,
}

impl TestDatabase {
    /// Opens a fresh database with `modules` (after the built-in audit and store schemas).
    pub fn open(modules: &[ModuleSchema]) -> Result<Self, StoreError> {
        let dir = tempfile::tempdir()?;
        let clock = Arc::new(FixedClock::new(test_start()));
        let db = Database::open(dir.path(), modules, clock.clone())?;
        Ok(Self {
            db,
            clock,
            modules: modules.to_vec(),
            dir,
        })
    }

    /// Closes the database without any cleanup, as a crash would leave it, and opens it again.
    pub fn reopen(self) -> Result<Self, StoreError> {
        let Self {
            db,
            clock,
            modules,
            dir,
        } = self;
        drop(db);
        let db = Database::open(dir.path(), &modules, clock.clone())?;
        Ok(Self {
            db,
            clock,
            modules,
            dir,
        })
    }

    /// The data directory.
    #[must_use]
    pub fn data_dir(&self) -> &Path {
        self.dir.path()
    }
}

/// [`TEST_START`] as a timestamp.
#[must_use]
pub fn test_start() -> UtcTimestamp {
    #[expect(clippy::expect_used, reason = "TEST_START is a valid constant")]
    UtcTimestamp::parse(TEST_START).expect("TEST_START is canonical")
}

/// A signed-in user holding exactly `capabilities`, with fresh IDs.
#[must_use]
pub fn principal(capabilities: &[Capability]) -> Principal {
    Principal::user(
        UserId::generate(),
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

/// The first port [`local_port`] hands out, and how many it cycles through: below the
/// ephemeral ranges of Linux (32768 up), macOS and Windows (49152 up).
const PORTS_FROM: u32 = 20_000;
const PORTS: u32 = 12_000;

/// A loopback TCP port that nothing listens on now, for a listener or printer the code
/// under test opens later. Asking the OS for port 0 and closing it is not enough: until
/// the code binds, the OS may give the same port to another test or use it for an
/// outgoing connection. These ports lie outside the ephemeral range, each is handed out
/// once per process, and a process starts at an offset of its own.
#[must_use]
pub fn local_port() -> u16 {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let offset = std::process::id().wrapping_mul(7_919) % PORTS;
    loop {
        let next = NEXT.fetch_add(1, Ordering::Relaxed);
        assert!(next < PORTS, "the test process used every local port");
        let candidate = PORTS_FROM + (offset + next) % PORTS;
        #[expect(clippy::expect_used, reason = "the range is below u16::MAX")]
        let port = u16::try_from(candidate).expect("port fits in u16");
        // Skip a port another program on this computer already uses.
        if TcpListener::bind((Ipv4Addr::LOCALHOST, port)).is_ok() {
            return port;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Mutex;

    use super::*;

    // Tests running side by side never share a port, and none is an ephemeral port.
    #[test]
    fn local_ports_are_distinct_and_outside_the_ephemeral_range() {
        let ports = Mutex::new(Vec::new());
        let take_fifty = || {
            let mine: Vec<u16> = (0..50).map(|_| local_port()).collect();
            ports.lock().unwrap().extend(mine);
        };
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(take_fifty);
            }
        });
        let ports = ports.into_inner().unwrap();
        let distinct: BTreeSet<u16> = ports.iter().copied().collect();
        assert_eq!(distinct.len(), ports.len());
        assert!(ports.iter().all(|&port| (20_000..32_000).contains(&port)));
    }
}
