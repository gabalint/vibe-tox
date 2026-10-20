//! Integration tests on real SQLite files with the production pragmas.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod artifacts;
mod lint;
mod migrations;
mod outbox;
mod pre_migration;
mod unit_of_work;

use lis_core::{Migration, ModuleSchema};
use lis_store::{Database, StoreError};

/// A throwaway module with one table, for tests that need a domain change.
pub(crate) const DEMO: ModuleSchema = ModuleSchema {
    module: "demo",
    table_prefix: "demo_",
    migrations: &[DEMO_V1],
};

pub(crate) const DEMO_V1: Migration = Migration {
    version: 1,
    name: "demo_items",
    sql: "CREATE TABLE demo_items (id INTEGER PRIMARY KEY, label TEXT NOT NULL) STRICT;",
};

/// Row count of `table`, read on a read connection.
pub(crate) fn count(db: &Database, table: &'static str) -> i64 {
    db.read_blocking(move |conn| {
        conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .map_err(StoreError::from)
    })
    .unwrap()
}
