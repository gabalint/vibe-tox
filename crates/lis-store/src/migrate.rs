//! Per-module migration runner with checksums.
//!
//! Applied migrations are recorded in `store_migrations`. Opening refuses to
//! continue when an applied migration's SQL changed, when a version is missing
//! (a gap, or an applied migration this build does not have), or when a
//! module's migrations are not numbered 1, 2, 3… Each pending migration runs in
//! its own `BEGIN IMMEDIATE` transaction together with its record. Before the
//! first migration changes a database that already has applied migrations,
//! the caller's `before_upgrade` step runs (the pre-migration snapshot,
//! NFR-14); if it fails, nothing is migrated.

use std::collections::{BTreeMap, HashSet};

use lis_core::{Migration, ModuleSchema, UtcTimestamp};
use rusqlite::{Connection, TransactionBehavior, params};
use sha2::{Digest, Sha256};

use crate::database::built_in_schemas;
use crate::error::{MigrationProblem, StoreError};
use crate::transaction_guard::TransactionControlGuard;

/// Creates the migration ledger. Its shape is fixed forever, so it is not itself a migration.
pub(crate) const BOOTSTRAP_SQL: &str = "CREATE TABLE IF NOT EXISTS store_migrations (
    module     TEXT    NOT NULL,
    version    INTEGER NOT NULL CHECK (version >= 1),
    name       TEXT    NOT NULL,
    checksum   TEXT    NOT NULL,
    applied_at TEXT    NOT NULL,
    PRIMARY KEY (module, version)
) STRICT;";

/// A recorded migration: `(name, checksum)` by `(module, version)`.
type Ledger = BTreeMap<(String, u32), (String, String)>;

/// Validates `modules` against the ledger, then applies pending migrations in
/// order. Runs `before_upgrade` first when migrations are pending on a
/// database that already has applied ones; a new database skips it.
pub(crate) fn migrate(
    conn: &mut Connection,
    modules: &[ModuleSchema],
    now: UtcTimestamp,
    before_upgrade: impl FnOnce() -> Result<(), StoreError>,
) -> Result<(), StoreError> {
    check_order(modules)?;
    conn.execute_batch(BOOTSTRAP_SQL)?;
    let ledger = read_ledger(conn)?;
    check_ledger(modules, &ledger)?;
    let pending: Vec<_> = modules
        .iter()
        .flat_map(|module| module.migrations.iter().map(move |m| (module, m)))
        .filter(|(module, m)| !ledger.contains_key(&(module.module.to_owned(), m.version)))
        .collect();
    if !ledger.is_empty() && !pending.is_empty() {
        before_upgrade()?;
    }
    for (module, migration) in pending {
        apply(conn, module, migration, now)?;
    }
    Ok(())
}

/// Lowercase hex SHA-256 of the migration's SQL.
pub(crate) fn checksum(migration: &Migration) -> String {
    hex::encode(Sha256::digest(migration.sql.as_bytes()))
}

fn problem(module: &str, version: u32, problem: MigrationProblem) -> StoreError {
    StoreError::Migration {
        module: module.to_owned(),
        version,
        problem,
    }
}

/// Module names are unique and each module's versions are exactly 1..=n in order.
fn check_order(modules: &[ModuleSchema]) -> Result<(), StoreError> {
    let mut seen = HashSet::new();
    for module in modules {
        if !seen.insert(module.module) {
            return Err(problem(module.module, 0, MigrationProblem::OutOfOrder));
        }
        for (expected, migration) in (1..).zip(module.migrations) {
            if migration.version != expected {
                return Err(problem(
                    module.module,
                    migration.version,
                    MigrationProblem::OutOfOrder,
                ));
            }
        }
    }
    Ok(())
}

/// One row of the migration ledger (`store_migrations`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppliedMigration {
    /// Module name.
    pub module: String,
    /// Migration version within the module.
    pub version: u32,
    /// Migration name.
    pub name: String,
    /// Lowercase hex SHA-256 of the migration's SQL when it was applied.
    pub checksum: String,
}

/// The migration ledger of `conn`, ordered by module and version: what backup
/// and exit manifests record (REQ-OPS-007, REQ-OPS-010).
pub fn applied_migrations(conn: &Connection) -> Result<Vec<AppliedMigration>, StoreError> {
    let mut statement = conn.prepare(
        "SELECT module, version, name, checksum FROM store_migrations ORDER BY module, version",
    )?;
    let rows = statement.query_map([], |row| {
        Ok(AppliedMigration {
            module: row.get(0)?,
            version: row.get(1)?,
            name: row.get(2)?,
            checksum: row.get(3)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<_>>()?)
}

/// Checks a ledger read from another database file as `Database::open` with
/// `modules` would: `Ok` when this build has every applied migration, with
/// the same name and checksum and without gaps. A restore refuses a database
/// whose ledger fails, because this build is older than the one that wrote it.
pub fn check_applied(
    modules: &[ModuleSchema],
    ledger: &[AppliedMigration],
) -> Result<(), StoreError> {
    let schemas: Vec<ModuleSchema> = built_in_schemas()
        .into_iter()
        .chain(modules.iter().copied())
        .collect();
    check_ledger(&schemas, &to_ledger(ledger.iter().cloned()))
}

fn to_ledger(rows: impl Iterator<Item = AppliedMigration>) -> Ledger {
    rows.map(|row| ((row.module, row.version), (row.name, row.checksum)))
        .collect()
}

fn read_ledger(conn: &Connection) -> Result<Ledger, StoreError> {
    Ok(to_ledger(applied_migrations(conn)?.into_iter()))
}

/// Every recorded migration exists in this build with the same name and
/// checksum, and each module's recorded versions are 1..=k without gaps.
fn check_ledger(modules: &[ModuleSchema], ledger: &Ledger) -> Result<(), StoreError> {
    let mut next_expected: BTreeMap<&str, u32> = BTreeMap::new();
    for ((module_name, version), (name, recorded_checksum)) in ledger {
        let module = modules.iter().find(|m| m.module == module_name.as_str());
        let Some(module) = module else {
            return Err(problem(module_name, *version, MigrationProblem::Missing));
        };
        let expected = next_expected.entry(module.module).or_insert(1);
        let migration = module.migrations.iter().find(|m| m.version == *version);
        let Some(migration) = migration.filter(|_| *version == *expected) else {
            return Err(problem(module_name, *expected, MigrationProblem::Missing));
        };
        if migration.name != name || checksum(migration) != *recorded_checksum {
            return Err(problem(
                module_name,
                *version,
                MigrationProblem::ChecksumChanged,
            ));
        }
        *expected += 1;
    }
    Ok(())
}

/// Runs one migration the way SQLite documents for schema changes that
/// rebuild a table other tables reference: foreign-key enforcement is off for
/// the migration's transaction (the pragma cannot change inside one), and the
/// migration must not add a foreign-key violation before it commits.
/// Enforcement is back on afterwards, whatever the outcome.
fn apply(
    conn: &mut Connection,
    module: &ModuleSchema,
    migration: &Migration,
    now: UtcTimestamp,
) -> Result<(), StoreError> {
    conn.pragma_update(None, "foreign_keys", false)?;
    let result = apply_checked(conn, module, migration, now);
    conn.pragma_update(None, "foreign_keys", true)?;
    result
}

/// Foreign-key violations by child and parent table. Compared before and
/// after a migration, so an inconsistency that predates it (which this store
/// never writes, but an older or repaired database might hold) does not block
/// every later upgrade, while one the migration adds always does. Row IDs are
/// not compared: a rebuilt table gets new ones.
fn foreign_key_violations(conn: &Connection) -> rusqlite::Result<BTreeMap<(String, String), i64>> {
    conn.prepare("SELECT \"table\", parent, count(*) FROM pragma_foreign_key_check GROUP BY 1, 2")?
        .query_map([], |row| Ok(((row.get(0)?, row.get(1)?), row.get(2)?)))?
        .collect()
}

fn apply_checked(
    conn: &mut Connection,
    module: &ModuleSchema,
    migration: &Migration,
    now: UtcTimestamp,
) -> Result<(), StoreError> {
    let tx = conn.transaction_with_behavior(TransactionBehavior::Immediate)?;
    let before = foreign_key_violations(&tx)?;
    let guard = TransactionControlGuard::install(&tx)?;
    let result = tx.execute_batch(migration.sql);
    if guard.finish()? {
        return Err(StoreError::TransactionEnded);
    }
    result?;
    if tx.is_autocommit() {
        return Err(StoreError::TransactionEnded);
    }
    let added = foreign_key_violations(&tx)?
        .into_iter()
        .any(|(key, count)| count > before.get(&key).copied().unwrap_or(0));
    if added {
        return Err(problem(
            module.module,
            migration.version,
            MigrationProblem::ForeignKeyViolation,
        ));
    }
    tx.execute(
        "INSERT INTO store_migrations (module, version, name, checksum, applied_at) \
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![
            module.module,
            migration.version,
            migration.name,
            checksum(migration),
            now
        ],
    )?;
    tx.commit()?;
    Ok(())
}
