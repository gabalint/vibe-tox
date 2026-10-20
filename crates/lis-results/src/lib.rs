//! Results module: import attempts, measurements, processing revisions,
//! quarantine, result revisions, validity assessments and technical review
//! (REQ-RES-001–005). Table prefix `res_`. Design: docs/planning/m2-design-brief.md §2.5.
//!
//! This part is the database-free model:
//! - [`parse_value`] reads a source value text into a typed form (quantity,
//!   censored or qualitative) exactly as received, or a [`MalformedValue`]
//!   reason; nothing is trimmed, rounded or coerced (REQ-RES-001, NFR-10).
//! - [`classify`] compares a value with the method version's cutoff using
//!   exact decimals, and refuses to decide where rounding or a censored bound
//!   leaves the answer uncertain (REQ-RES-003). Value form, method certainty
//!   and output scale stay separate inputs (REQ-RES-002).
//! - [`astm::extract`] turns an ASTM result message into rows of received
//!   text for the importer, which quarantines rows it cannot place
//!   (REQ-ANA-003); [`delimited::extract_file`] does the same for a result
//!   file under its source's layout, and [`delimited::read_value`] holds the
//!   result-file value rules (result files brief F3–F7).
//! - [`au::extract_artifact`] preserves AU400/AU640 fixed-width result fields.
//!   Only approved numeric test definitions supply declared units; raw source
//!   units remain absent. AU reruns need explicit reconciliation, and later
//!   evidence selection uses the original immutable source declaration.
//! - Patient evidence is admitted only while its analytical execution can
//!   still accept data; QC re-export conflicts remain solely in the
//!   analytical conflict queue (REQ-ANA-001, REQ-ANA-004).
//! - [`specimen_release_lines`], [`current_validity`] and
//!   [`out_of_scope_detections`] expose transaction-safe release and billing
//!   projections without duplicating analytical or specimen gates
//!   (REQ-RPT-001, REQ-BIL-005, NFR-01).
//! - [`record_reflex_decision`] records a person's reflex decision on a
//!   current, reviewed result revision through orders' laboratory-result
//!   entry points; nothing infers it from the classification (REQ-ORD-003,
//!   REQ-RPT-005).
//! - The ready-review queue keeps an indexed projection of result lines that
//!   have never been reviewed; review status and every safety gate are still
//!   recomputed by their authoritative queries (REQ-OPS-004, NFR-05).

#![warn(clippy::wildcard_enum_match_arm)]

pub mod astm;
pub mod au;
mod classification;
mod commands;
pub mod delimited;
mod error;
mod import;
mod model;
mod queries;
mod result_value;
mod value;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use classification::{Classification, NotClassifiableReason, classify};
pub use commands::{
    RETRY_IMPORT_TOPIC, RecordReflexDecisionInput, RecordResultDispositionInput,
    RecordValidityAssessmentInput, ReflexDecision, ReflexTriggerView, ResolveQuarantineItemInput,
    RetryImportInput, SelectResultEvidenceInput, SubmitTechnicalReviewInput,
    UseMeasuredResultInput, pending_reflex_triggers, record_reflex_decision,
    record_result_disposition, record_validity_assessment, resolve_quarantine_item, retry_import,
    select_result_evidence, submit_technical_review, use_measured_result,
};
pub use error::ResultError;
pub use import::{ImportReceiptInput, ImportSource, import_receipt};
pub use model::{
    ImportAttemptId, ImportFailureCode, ImportOutcome, MeasurementForm, MeasurementId,
    ProcessingRevisionId, QuarantineItemId, QuarantineReason, QuarantineResolution,
    QuarantineState, ResultCause, ResultId, ResultOutcome, ResultRevisionId, ReviewDecision,
    ReviewId, ReviewQueueFilter, SetAsideKind, SetAsideRowId, TechnicalReviewId,
    ValidityAssessmentId,
};
pub use queries::{AwaitingResultRow, awaiting_results};
pub use queries::{
    ResultExceptionQueueItem, ResultReviewQueueItem, ReviewQueueCursor, ReviewQueuePage,
    ReviewWorkQueuePage, StaleReason, current_validity, get_import_attempt, get_specimen_results,
    latest_import_attempt_for_receipt, list_import_attempts, list_import_attempts_page,
    open_quarantine_queue, out_of_scope_detections, out_of_scope_queue, result_revision_run_ids,
    review_blocked_queue, review_gates, review_queue, review_queue_page, review_ready_queue,
    review_stale_queue, review_status, review_work_queue_page, scan_review_queue,
    scan_review_work_queue, specimen_release_lines, stale_reasons_after,
};
pub use value::{MalformedValue, SourceValue, parse_value};
pub use views::{
    AddOnComponentView, CurrentValidity, ImportAttemptDetail, ImportAttemptSummary, ImportFileView,
    MeasuredNotOrderedView, MeasurementView, OutOfScopeDetection, ReleaseLine, ReleaseReviewStatus,
    ReleaseRevision, ResultLineView, ResultRevisionView, ReviewQueueEntry, ReviewStatus,
    ReviewView, SetAsideRowView, SpecimenResults, ValidityAssessmentView,
};

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "results",
        sql: include_str!("../migrations/0001_results.sql"),
    },
    Migration {
        version: 2,
        name: "quarantine_conflict_context",
        sql: include_str!("../migrations/0002_quarantine_conflict_context.sql"),
    },
    Migration {
        version: 3,
        name: "review_safety_and_release_view",
        sql: include_str!("../migrations/0003_review_safety_and_release_view.sql"),
    },
    Migration {
        version: 4,
        name: "open_review_queue",
        sql: include_str!("../migrations/0004_open_review_queue.sql"),
    },
    Migration {
        version: 5,
        name: "result_files",
        sql: include_str!("../migrations/0005_result_files.sql"),
    },
    Migration {
        version: 6,
        name: "set_aside_dilution",
        sql: include_str!("../migrations/0006_set_aside_dilution.sql"),
    },
    Migration {
        version: 7,
        name: "review_commit_order",
        sql: include_str!("../migrations/0007_review_commit_order.sql"),
    },
    Migration {
        version: 8,
        name: "review_causal_evidence",
        sql: include_str!("../migrations/0008_review_causal_evidence.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "results",
    table_prefix: "res_",
    migrations: MIGRATIONS,
};
