//! Instrument configuration, durable AU, ASTM and result-file receipts, listener
//! status, and startup reconciliation for the local LIS (REQ-INT-003,
//! REQ-INT-005, REQ-INT-009, REQ-OPS-013, NFR-02, NFR-06).
//!
//! Exact accepted frame bytes commit through the single database writer before
//! protocol session is told they are durable, so its frame ACK cannot precede
//! the receipt row. Complete assembled messages are synced to the
//! content-addressed artifact store before the receipt references them.
//! Accepted analyzer transfers act as [`lis_core::Principal::instrument`]; peer
//! rejection and startup recovery act as narrowly capable system principals.
//! Listeners do not consult a signed-in session. Status and diagnostic errors
//! contain codes and opaque identifiers only, never payload bytes.
//!
//! TCP listeners require an explicit source-IP allowlist and reject other peers
//! before protocol processing or instrument-principal construction. Source IP is
//! not cryptographic authentication: a host able to spoof an allowed address on
//! the lab network can still impersonate an analyzer, so deployments needing
//! stronger identity must place the listener behind an authenticated gateway or
//! VPN.

#![warn(clippy::wildcard_enum_match_arm)]

mod artifact_references;
#[cfg(test)]
#[path = "au_dedup/tests.rs"]
mod au_dedup_tests;
mod au_evidence;
mod au_session;
mod commands;
mod error;
mod instrument_commands;
mod listener;
mod model;
mod peer_rejections;
mod queries;
mod receipt_writer;
mod reconciliation;
mod repo;
mod result_files;
mod serial;
mod session;
mod status;
mod tcp;
mod validation;

pub use lis_core::InstrumentId;
use lis_core::{Migration, ModuleSchema};

pub use artifact_references::{ARTIFACT_COLUMNS, referenced_artifacts};
pub use error::{InterfaceError, ListenerErrorCode};
pub use instrument_commands::{
    register_instrument, set_instrument_enabled, set_result_folder, update_instrument_connection,
};
pub use listener::{DEFAULT_ESTABLISHMENT_TIMEOUT, ListenerSupervisor, ListenerSupervisorError};
pub use model::{
    AbortCode, AuConnectionSettings, ConnectionSettings, DataBits, FileFacts, FileOrigin,
    FileSource, Instrument, InstrumentTransport, Parity, PresentFile, Receipt, ReceiptFileView,
    ReceiptFrame, ReceiptId, ReceiptState, ReceiveResultFileInput, RegisterInstrument,
    SetInstrumentEnabled, SetResultFolder, StopBits, UpdateInstrumentConnection, WatchedFolder,
};
pub use queries::{
    get_instrument, get_receipt, instrument_au_profile, instrument_names, instrument_transport,
    list_instruments, list_receipt_frames, receipt_au_profile,
};
pub use reconciliation::{ReconciliationSummary, reconcile_receipts};
pub use result_files::{
    present_at_selection, receipt_file, receivable_instrument, receive_result_file,
    record_oversize_file, result_file_instruments, seen_file,
};
pub use status::{ListenerDetailCode, ListenerState, ListenerStatus, StatusReport};

/// Durable-work topic consumed by the M2 receipt importer.
pub const RECEIPT_COMPLETED_TOPIC: &str = "ifc.receipt.completed";

/// Largest result file the LIS reads, in bytes (20 MiB). Intake checks it
/// before reading; a larger file becomes an aborted receipt (F14).
pub const MAX_RESULT_FILE_BYTES: u64 = 20 * 1024 * 1024;

/// This module's forward-only schema changes.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "instruments_and_receipts",
        sql: include_str!("../migrations/0001_instruments_and_receipts.sql"),
    },
    Migration {
        version: 2,
        name: "result_files",
        sql: include_str!("../migrations/0002_result_files.sql"),
    },
    Migration {
        version: 3,
        name: "folder_baseline",
        sql: include_str!("../migrations/0003_folder_baseline.sql"),
    },
    Migration {
        version: 4,
        name: "au_protocol",
        sql: include_str!("../migrations/0004_au_protocol.sql"),
    },
];

/// Schema descriptor for tables owned by this crate.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "interfaces",
    table_prefix: "ifc_",
    migrations: MIGRATIONS,
};
