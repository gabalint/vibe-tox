//! Schema migration descriptors, applied by `lis-store`'s migration runner.

/// One forward-only schema change, numbered from 1 within its module.
///
/// SQL lives in `migrations/NNNN_name.sql` and is included with `include_str!`.
/// Never edit a migration once integrated: the runner refuses changed checksums.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Migration {
    /// Sequential version within the module, starting at 1.
    pub version: u32,
    /// Short snake_case description, matching the file name.
    pub name: &'static str,
    /// The SQL batch to run.
    pub sql: &'static str,
}

/// A module's schema: its name, table prefix and ordered migrations.
///
/// Each crate that owns tables exposes one as `pub const SCHEMA`. Every table
/// its migrations create must start with `table_prefix` (crate map in
/// `docs/planning/implementation-plan.md`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ModuleSchema {
    /// Module name recorded with each applied migration, such as `audit`.
    pub module: &'static str,
    /// Prefix of every table the module owns, such as `audit_`.
    pub table_prefix: &'static str,
    /// Migrations in version order.
    pub migrations: &'static [Migration],
}
