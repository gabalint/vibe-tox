//! Restore tests, restore onto a new installation, the recovery reconciliation and the
//! exit export, on real temporary folders and SQLite files (REQ-OPS-002, REQ-OPS-003,
//! REQ-OPS-007, REQ-OPS-010, NFR-04, NFR-12, NFR-30). Encryption, digests, integrity
//! checks, `Database::open`, chain verification and anchor comparison are the real ones;
//! all data is synthetic.

#![cfg(test)]

mod apply;
mod exit_export;
mod findings;
mod reconciliation;
mod support;
