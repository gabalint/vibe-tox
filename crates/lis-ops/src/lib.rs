//! Operations module (`ops_`; REQ-OPS-002, 003, 006, 007, 008, 009, 010, 013; NFR-18).
//! Design: docs/planning/m4-design-brief.md.
//!
//! - **Installation.** One row names this installation; audit anchors, backup sets and exit
//!   exports carry its ID ([`installation_id`], [`ensure_installation`]). Its kind,
//!   laboratory or demo, is fixed at the first start ([`open_installation`]).
//! - **Policy.** Defaults, setting ranges and fixed limits have one home ([`policy`]).
//! - **Areas.** `backup` (targets, encrypted copies, the recovery kit, restore and the
//!   recovery reconciliation, exit export), `runtime` (settings, app runs, host health,
//!   updates, disposals) and `anchors` (anchor export, lag and classification). Each area has
//!   its own schema module, so their migrations are numbered independently; every table
//!   keeps the `ops_` prefix.
//! - **Local.** Safety, backup, restore and exit run in the application; nothing here calls
//!   a network service.

mod installation;
pub mod policy;

// backup: begin
pub mod backup;
pub mod exit_export;
pub mod format;
pub mod kit;
pub mod restore;
pub mod targets;

pub use restore::recovery;
// backup: end

// runtime: begin
pub mod disposals;
pub mod health;
pub mod host;
pub mod runs;
mod runtime_error;
pub mod settings;
pub mod updates;

pub use runtime_error::RuntimeError;

/// Migrations of the `ops_runtime` schema module: settings, app runs, health history
/// and attestations, update records, media disposals, then clock jumps and the D-1/D-2
/// health checks.
pub const RUNTIME_MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "runtime",
        sql: include_str!("../migrations/runtime/0001_runtime.sql"),
    },
    Migration {
        version: 2,
        name: "clock_jumps",
        sql: include_str!("../migrations/runtime/0002_clock_jumps.sql"),
    },
    Migration {
        version: 3,
        name: "health_checks",
        sql: include_str!("../migrations/runtime/0003_health_checks.sql"),
    },
    Migration {
        version: 4,
        name: "health_current",
        sql: include_str!("../migrations/runtime/0004_health_current.sql"),
    },
    Migration {
        version: 5,
        name: "update_backup_eligibility",
        sql: include_str!("../migrations/runtime/0005_update_backup_eligibility.sql"),
    },
];

/// The runtime area's schema; applied after [`SCHEMA`], whose backup runs it references.
pub const RUNTIME_SCHEMA: ModuleSchema = ModuleSchema {
    module: "ops_runtime",
    table_prefix: "ops_",
    migrations: RUNTIME_MIGRATIONS,
};
// runtime: end

// anchors: begin
pub mod anchors;

/// Migrations of the `ops_anchors` schema module: anchor exports and their seal ranges.
pub const ANCHORS_MIGRATIONS: &[Migration] = &[Migration {
    version: 1,
    name: "anchors",
    sql: include_str!("../migrations/anchors/0001_anchors.sql"),
}];

/// The anchors area's schema; applied after [`SCHEMA`], whose targets it references.
pub const ANCHORS_SCHEMA: ModuleSchema = ModuleSchema {
    module: "ops_anchors",
    table_prefix: "ops_",
    migrations: ANCHORS_MIGRATIONS,
};
// anchors: end

use lis_core::{Migration, ModuleSchema};

pub use installation::{
    INSTALLATION_CREATE, InstallationError, InstallationKind, ensure_installation, installation_id,
    installation_kind, open_installation,
};

/// Migrations of the `ops` schema module: the installation row, the backup area, then the
/// installation kind.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "installation",
        sql: include_str!("../migrations/0001_installation.sql"),
    },
    // backup: begin
    Migration {
        version: 2,
        name: "backup",
        sql: include_str!("../migrations/0002_backup.sql"),
    },
    Migration {
        version: 3,
        name: "restore",
        sql: include_str!("../migrations/0003_restore.sql"),
    },
    Migration {
        version: 4,
        name: "restore_anchor_gap",
        sql: include_str!("../migrations/0004_restore_anchor_gap.sql"),
    },
    // backup: end
    Migration {
        version: 5,
        name: "installation_kind",
        sql: include_str!("../migrations/0005_installation_kind.sql"),
    },
];

/// The installation and backup schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "ops",
    table_prefix: "ops_",
    migrations: MIGRATIONS,
};
