//! Typed result identities and closed state codes.

use lis_core::{Id, text_enum};

pub enum ImportAttempt {}
pub enum ProcessingRevision {}
pub enum Measurement {}
pub enum QuarantineItem {}
pub enum ResultLine {}
pub enum ResultRevision {}
pub enum TechnicalReview {}
pub enum ValidityAssessment {}
pub enum SetAsideRow {}

/// One semantic processing attempt of a durable receipt.
pub type ImportAttemptId = Id<ImportAttempt>;
/// A linked revision of one run's imported processing.
pub type ProcessingRevisionId = Id<ProcessingRevision>;
/// One immutable source observation from a patient position.
pub type MeasurementId = Id<Measurement>;
/// One row requiring human resolution.
pub type QuarantineItemId = Id<QuarantineItem>;
/// A result line for one frozen component and analyte.
pub type ResultId = Id<ResultLine>;
/// One immutable version of a result line.
pub type ResultRevisionId = Id<ResultRevision>;
/// One technical review decision.
pub type TechnicalReviewId = Id<TechnicalReview>;
/// One specimen-validity assessment.
pub type ValidityAssessmentId = Id<ValidityAssessment>;
/// One file row an import attempt lists without quarantining it.
pub type SetAsideRowId = Id<SetAsideRow>;

text_enum!(/// Overall import disposition.
    ImportOutcome { Processed = "processed", ProcessedWithQuarantine = "processed_with_quarantine", Quarantined = "quarantined", });
text_enum!(/// Whole-message failure.
ImportFailureCode {
    MalformedMessage = "malformed_message",
    UnregisteredSource = "unregistered_source",
    SenderMismatch = "sender_mismatch",
    UndecodableText = "undecodable_text",
    LayoutMismatch = "layout_mismatch",
    LimitsExceeded = "limits_exceeded",
});
text_enum!(/// Why one row needs human attention.
QuarantineReason {
    UnknownSample = "unknown_sample",
    AmbiguousSample = "ambiguous_sample",
    RunClosed = "run_closed",
    ExecutionFailed = "execution_failed",
    UnknownAnalyte = "unknown_analyte",
    NotInOrderedScope = "not_in_ordered_scope",
    IncompatibleUnit = "incompatible_unit",
    MalformedValue = "malformed_value",
    ScaleMismatch = "scale_mismatch",
    ConflictingDuplicate = "conflicting_duplicate",
    DilutionNotSupported = "dilution_not_supported",
    MalformedRow = "malformed_row",
    RoleMismatch = "role_mismatch",
    DilutionMismatch = "dilution_mismatch",
});
text_enum!(/// How a measurement holds its source value; `word` keeps an approved
    /// value word as received (result files brief F7).
    MeasurementForm { Quantity = "quantity", Censored = "censored", Qualitative = "qualitative", NotPerformed = "not_performed", Word = "word", });
text_enum!(/// Why an import attempt lists a row without quarantining it (F9, F17).
    SetAsideKind { BatchRowNotPlaced = "batch_row_not_placed", NotOrderedUnusable = "not_ordered_unusable", });
text_enum!(/// State of a quarantined source row.
    QuarantineState { Open = "open", AcceptedAsReprocessing = "accepted_as_reprocessing", Rejected = "rejected", SupersededByRetry = "superseded_by_retry", });
text_enum!(/// A reviewer's resolution of a quarantined row.
    QuarantineResolution { AcceptAsReprocessing = "accept_as_reprocessing", Reject = "reject", });
text_enum!(/// Why a result revision exists.
    ResultCause { Initial = "initial", Reprocessed = "reprocessed", Selected = "selected", Disposition = "disposition", });
text_enum!(/// Explicit result outcome forms (REQ-RES-001).
    ResultOutcome { Quantity = "quantity", Censored = "censored", Qualitative = "qualitative", NotPerformed = "not_performed", Invalid = "invalid", Indeterminate = "indeterminate", Missing = "missing", });
text_enum!(/// Technical review decision.
    ReviewDecision { Approved = "approved", Rejected = "rejected", });
text_enum!(/// Result-review worklist filter.
    ReviewQueueFilter { Ready = "ready", Stale = "stale", Blocked = "blocked", Quarantine = "quarantine", });

/// Backwards-compatible public name used by the IPC contract.
pub type ReviewId = TechnicalReviewId;
