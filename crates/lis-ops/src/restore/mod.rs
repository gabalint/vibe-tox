//! Restore tests, restore onto a new installation and the recovery reconciliation
//! (REQ-OPS-002, REQ-OPS-007, NFR-04, NFR-30; M4 design brief D4, D5, §3.6–§3.8).
//!
//! - **Staging** ([`stage`]) decrypts one set into a scratch folder inside the data
//!   folder and checks it exactly as a restore would: manifest and key, schema ledger,
//!   database digest and integrity, `Database::open` with this build's migrations, every
//!   chain against the manifest's heads and seals, the anchors on the target and other
//!   folders, every artifact, and every artifact the database references. It returns the
//!   finding codes of §3.6 ([`FindingCode`]); a set passes with none but `lost_interval`.
//! - **Restore tests** ([`request_restore_test`], [`run_restore_test`]) stage into
//!   `ops/restore-tests/<id>/`, record the elapsed time and findings, and delete the
//!   scratch folder; live data is never touched.
//! - **Restore onto a new installation**: a [`RestoreSession`] holds the key typed from the
//!   kit for 30 minutes; [`read_source`] lists a chosen folder's sets; staging goes to
//!   [`restore_staging_dir`]; [`confirm_restore`] writes `READY.json`; [`apply_staged`]
//!   runs at the next start before the database opens; [`record_applied_restore`] records
//!   it after the open and opens the reconciliation.
//! - **Recovery reconciliation** ([`recovery`]): the checklist, the outbound gate and the
//!   closing.
//!
//! Invariants: only a new installation (no user accounts, rechecked by the engine when
//! confirming and here when applying) is restored onto; a replaced database is moved to
//! `ops/replaced/`, never deleted; every recorded restore opens a reconciliation in the
//! same transaction that raises the number floors; audit details hold IDs, codes, counts
//! and digests, logs IDs and codes, never paths or key material.

mod anchors;
mod apply;
mod artifacts;
mod checks;
mod findings;
mod record;
pub mod recovery;
mod session;
mod stage;
mod test_repo;
mod test_runs;

use lis_audit::Stream;

pub use apply::{
    ApplyFailure, ApplyOutcome, MarkerState, PendingRestore, Restore, RestoreId, RestoreMarker,
    apply_staged, cancel_restore, clean_scratch, confirm_restore, pending_restore,
    restore_staging_dir,
};
pub use checks::ChainCheck;
pub use findings::{Finding, FindingCode, Findings};
pub use record::{
    RESTORE_APPLIED, RESTORE_COMPONENT, RestoreFloors, applied_restore, finish_recorded_restore,
    record_applied_restore, restore_records,
};
pub use session::{
    RestoreSession, RestoreSessionId, RestoreSource, RestoreWizard, SetSummary, read_source,
    summarize_sets,
};
pub use stage::{StageInput, StagedRestore, stage};
pub use test_repo::{
    RestoreTestRecord, get_restore_test, list_restore_tests, list_restore_tests_page,
};
pub use test_runs::{
    REQUEST_RESTORE_TEST, RESTORE_TEST, RestoreTestId, RestoreTestJob, RestoreTestReport,
    RestoreTestRun, TestFailure, TestState, request_restore_test, run_restore_test,
};

pub(crate) use test_repo::{interrupt as interrupt_test, unfinished as unfinished_tests};
pub(crate) use test_runs::interrupted_event as interrupted_test_event;

/// The stream named `text` in stored rows and manifests.
fn stream_named(text: &str) -> Option<Stream> {
    Stream::ALL
        .into_iter()
        .find(|stream| stream.as_str() == text)
}
