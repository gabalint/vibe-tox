//! Published result queries and the single review-gate implementation
//! (REQ-RES-005, NFR-01). Safety callers use these on their write transaction.

use std::collections::{BTreeMap, BTreeSet};

use lis_catalog::{AnalyteId, ComponentPurpose};
use lis_core::{GateReason, InstrumentId, UtcTimestamp};
use lis_interfaces::ReceiptId;
use lis_orders::OrderComponentId;
use lis_specimens::{ContainerId, SpecimenId};
use rusqlite::{Connection, OptionalExtension, params};

use crate::error::{ResultError, gate};
use crate::model::{
    ImportAttemptId, MeasurementId, ResultId, ResultRevisionId, ReviewQueueFilter,
    TechnicalReviewId, ValidityAssessmentId,
};
use crate::views::{
    ImportAttemptDetail, ImportAttemptSummary, ImportFileView, MeasurementView, PendingAnalyteView,
    QuarantineItemView, ResultLineView, ResultRevisionView, ReviewQueueEntry, ReviewStatus,
    SetAsideRowView, SpecimenResults, ValidityAssessmentView,
};

pub(crate) mod awaiting;
mod exception_queues;
mod imports;
mod invalidations;
mod measured;
mod measurements;
mod queues;
mod range;
mod read_context;
mod release;
mod review;
mod review_chronology;
mod snapshot;
mod specimen;

pub use awaiting::{AwaitingResultRow, awaiting_results};
pub use exception_queues::{ResultExceptionQueueItem, open_quarantine_queue, out_of_scope_queue};
pub(crate) use imports::{ATTEMPT_SUMMARY_COLUMNS, attempt_summary, next_attempt_no};
pub use imports::{
    get_import_attempt, latest_import_attempt_for_receipt, list_import_attempts,
    list_import_attempts_page,
};
pub use invalidations::{StaleReason, stale_reasons_after};
pub(crate) use measured::measured_not_ordered;
pub use measurements::result_revision_run_ids;
pub(crate) use measurements::{
    ScopeComponent, all_measurements, measurement_component_ids, measurements_for_revision,
    scoped_components,
};
pub use queues::{
    ResultReviewQueueItem, ReviewQueueCursor, ReviewQueuePage, ReviewWorkQueuePage,
    review_blocked_queue, review_queue_page, review_ready_queue, review_stale_queue,
    review_work_queue_page, scan_review_queue, scan_review_work_queue,
};
pub use release::{current_validity, out_of_scope_detections, specimen_release_lines};
use review::{component_active, components_require_validity, current_validity_id};
pub(crate) use review::{current_result, reflex_parent};
pub use review::{review_gates, review_status};
pub(crate) use snapshot::dependency_snapshots;
pub(crate) use specimen::specimen_components;
pub use specimen::{get_specimen_results, review_queue};
