//! Fixtures for tests in this and other crates: a temporary database, a fixed
//! clock and principals with chosen capabilities. Enable with the `testing`
//! feature from `[dev-dependencies]`.

use std::path::Path;
use std::sync::Arc;

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
