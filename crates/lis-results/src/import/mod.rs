//! Transactional semantic import of a durable receipt: an ASTM or AU message,
//! or a result file (REQ-ANA-003, REQ-RES-001–005, REQ-INT-003, REQ-INT-005).
//! Parsing happens before this command; this module resolves typed rows,
//! preserves exact source evidence and applies explicit re-export semantics.
//! A file source is read under its approved template: its header check
//! already happened in the reader, so the ASTM sender check does not apply,
//! and its rows follow the result files brief §5 in that order.

use std::collections::{BTreeMap, BTreeSet};

use lis_analytical::reexport::{RowDecision, SourceRow};
use lis_analytical::{FileLayout, PositionMatch, PositionRef, PositionRole, QcRow, RunId};
use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_catalog::{AnalyteId, MethodAnalyte, MethodVersionView};
use lis_core::{Capability, GateReason, InstrumentId};
use lis_interfaces::ReceiptId;
use lis_specimens::SpecimenId;
use lis_store::CommandContext;
use rusqlite::{OptionalExtension, params};

use crate::astm::{ExtractFailure, ExtractedImport, ExtractedRow};
use crate::delimited::FileExtractFailure;
use crate::error::ResultError;
use crate::model::{
    ImportAttemptId, ImportFailureCode, MeasurementId, ProcessingRevisionId, QuarantineItemId,
    QuarantineReason, ResultCause, ResultId, ResultRevisionId, SetAsideKind,
};
use crate::queries::ScopeComponent;
use crate::result_value::ResultValue;
use crate::views::ImportAttemptSummary;

mod attempt;
mod file_rows;
mod patient;
mod planning;
mod quarantine_acceptance;
mod writing;

use attempt::{
    Attempt, Counts, attempt_by_key, audit_attempt, insert_attempt, record_failed_attempt,
    supersede_prior_quarantine,
};
use planning::{Planner, count_plans, plan_patient, plan_rows, record_qc, source_row};
pub(crate) use quarantine_acceptance::accept_quarantined_as_reprocessing;
use writing::write_plans;

const ASTM_PARSER_VERSION: &str = "astm-result-v1";
pub(crate) const AU_PARSER_VERSION: &str = "au-result-v1";
/// The result-file reader (brief §13).
const FILE_PARSER_VERSION: &str = "delimited-v1";
const NOT_PERFORMED_STATUS: &str = "X";

/// The parser result passed into the short database transaction.
#[derive(Debug)]
pub enum ImportSource {
    /// A structurally valid ASTM result message.
    Extracted(ExtractedImport),
    /// Structural parsing failed; exact bytes remain in the receipt artifact.
    Malformed(ExtractFailure),
    /// AU fixed-width results, qualified by the receipt's frozen protocol
    /// profile. AU carries no ASTM sender header or reported unit.
    Au {
        /// Exact protocol settings frozen when the bytes were received.
        profile: lis_au::AuConfig,
        extracted: Result<ExtractedImport, crate::au::AuExtractFailure>,
    },
    /// A result file read under the registration's template, or why it
    /// yields no rows (brief F3, F6, F14); its bytes remain in the artifact.
    File(Result<ExtractedImport, FileExtractFailure>),
    /// A result file whose instrument has no active file registration to
    /// read it with (`unregistered_source`; Retry import after registration).
    UnregisteredFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ImportFormat {
    Astm,
    Au,
    File,
}

struct ReadableRows {
    rows: Vec<ExtractedRow>,
    malformed_rows: Vec<String>,
    format: ImportFormat,
}

/// One idempotent semantic attempt of a durable receipt.
#[derive(Debug)]
pub struct ImportReceiptInput {
    pub receipt_id: ReceiptId,
    pub attempt_no: i64,
    pub instrument_id: InstrumentId,
    /// The exact immutable registration profile used to parse the artifact.
    pub registration: Option<lis_analytical::SourceRegistrationView>,
    pub source: ImportSource,
}

#[derive(Debug)]
struct PatientPlan {
    row: ExtractedRow,
    position: PositionRef,
    specimen_id: SpecimenId,
    analyte_id: AnalyteId,
    method: MethodVersionView,
    analyte: MethodAnalyte,
    /// The ordered components expecting the analyte; empty for a file row
    /// kept as measured, not ordered (F17).
    components: Vec<ScopeComponent>,
    value: ResultValue,
    decision: RowDecision,
    current_measurement: Option<MeasurementId>,
    measurement_id: Option<MeasurementId>,
}

impl PatientPlan {
    /// A measurement kept outside the ordered scope: it gets no result line.
    fn kept(&self) -> bool {
        self.components.is_empty()
    }
}

#[derive(Debug)]
struct PatientTarget {
    position: PositionRef,
    source_row: SourceRow,
    specimen_id: Option<SpecimenId>,
}

#[derive(Debug)]
struct QcPlan {
    observation: QcRow,
}

#[derive(Debug, Clone, Copy)]
struct QuarantineContext {
    position: Option<lis_analytical::PositionId>,
    analyte: Option<AnalyteId>,
    conflicting_measurement: Option<MeasurementId>,
    reason: QuarantineReason,
}

#[derive(Debug)]
enum Plan {
    Qc(Box<QcPlan>),
    Patient(Box<PatientPlan>),
    Quarantine {
        row: Box<ExtractedRow>,
        context: QuarantineContext,
    },
    /// A file row whose analyte is on the not-a-result list (F10): counted only.
    NotResult,
    /// A file row listed on the attempt without a quarantine item (F9, F17).
    SetAside {
        row: Box<ExtractedRow>,
        kind: SetAsideKind,
        reason: Option<QuarantineReason>,
    },
}

/// Imports typed rows in one `BEGIN IMMEDIATE` unit of work. A repeated
/// `(receipt_id, attempt_no)` returns the original attempt without another
/// audit event or clinical effect (NFR-02).
pub fn import_receipt(
    ctx: &mut CommandContext<'_>,
    input: ImportReceiptInput,
) -> Result<ImportAttemptSummary, ResultError> {
    ctx.require(Capability::InstrumentIntake)?;
    if let Some(existing) = attempt_by_key(ctx.tx(), input.receipt_id, input.attempt_no)? {
        return Ok(existing);
    }
    if input.attempt_no < 1 {
        return Err(crate::error::invalid(
            "attemptNo",
            "out_of_range",
            "The attempt number must be at least one.",
        ));
    }
    let active_registration = lis_analytical::active_registration(ctx.tx(), input.instrument_id)?;
    if active_registration != input.registration {
        return Err(ResultError::RevisionConflict);
    }
    let mut attempt = Attempt::of(&input);
    let registration = input.registration.as_ref();
    let ReadableRows {
        rows,
        malformed_rows,
        format,
    } = match readable_rows(input.source, registration) {
        Ok(rows) => rows,
        Err((failure, rows_total)) => {
            if failure == ImportFailureCode::UnregisteredSource {
                attempt.registration = None;
            }
            return record_failed_attempt(ctx, &attempt, failure, rows_total);
        }
    };
    let registration = registration.ok_or(ResultError::WrongState("source registration"))?;
    let mut planner = Planner::new(input.receipt_id, registration, format);
    let mut plans = plan_rows(ctx, &mut planner, rows)?;
    plans.extend(malformed_rows.into_iter().map(planning::malformed_row));
    let mut counts = Counts::total(plans.len());
    record_qc(ctx, &mut plans, &mut counts)?;
    count_plans(&plans, &mut counts);
    let attempt_id = ImportAttemptId::generate();
    let summary = insert_attempt(ctx, attempt_id, &attempt, None, &counts)?;
    write_plans(ctx, attempt_id, plans)?;
    let superseded = supersede_prior_quarantine(ctx, input.receipt_id, input.attempt_no)?;
    audit_attempt(ctx, &summary, superseded)?;
    Ok(summary)
}

/// The rows to place and the references of malformed file records, or the
/// whole-attempt failure with its row count (brief §5; M2 §3.2).
fn readable_rows(
    source: ImportSource,
    registration: Option<&lis_analytical::SourceRegistrationView>,
) -> Result<ReadableRows, (ImportFailureCode, usize)> {
    let layout: Option<&FileLayout> = registration.and_then(|item| item.file_layout.as_ref());
    match source {
        ImportSource::Malformed(_) => Err((ImportFailureCode::MalformedMessage, 0)),
        ImportSource::Au {
            extracted: Err(_), ..
        } => Err((ImportFailureCode::MalformedMessage, 0)),
        ImportSource::Au {
            profile,
            extracted: Ok(extracted),
        } => {
            let rows = extracted.rows.len();
            // Sender/software names in this registration are setup declarations,
            // not fields received in an AU header. An approved test-code map is
            // still required; an ASTM correction-status policy cannot apply.
            let Some(registration) = registration.filter(|_| layout.is_none()) else {
                return Err((ImportFailureCode::UnregisteredSource, rows));
            };
            if registration.revision_indicator != lis_analytical::RevisionIndicator::None {
                return Err((ImportFailureCode::UnregisteredSource, rows));
            }
            let Some(approved) = registration.au_profile.as_ref() else {
                return Err((ImportFailureCode::UnregisteredSource, rows));
            };
            if approved.protocol != profile {
                return Err((ImportFailureCode::LayoutMismatch, rows));
            }
            Ok(ReadableRows {
                rows: extracted.rows,
                malformed_rows: extracted.malformed_rows,
                format: ImportFormat::Au,
            })
        }
        ImportSource::UnregisteredFile => Err((ImportFailureCode::UnregisteredSource, 0)),
        ImportSource::File(Err(failure)) => Err((file_failure(failure), 0)),
        ImportSource::File(Ok(extracted)) => {
            let rows = extracted.rows.len() + extracted.malformed_rows.len();
            if layout.is_none() {
                return Err((ImportFailureCode::UnregisteredSource, rows));
            }
            Ok(ReadableRows {
                rows: extracted.rows,
                malformed_rows: extracted.malformed_rows,
                format: ImportFormat::File,
            })
        }
        ImportSource::Extracted(extracted) => {
            let rows = extracted.rows.len();
            let Some(registration) =
                registration.filter(|item| layout.is_none() && item.au_profile.is_none())
            else {
                return Err((ImportFailureCode::UnregisteredSource, rows));
            };
            if extracted.sender_name.as_deref() != Some(registration.sender_name.as_str())
                || extracted.sender_version.as_deref() != Some(registration.sender_version.as_str())
            {
                return Err((ImportFailureCode::SenderMismatch, rows));
            }
            Ok(ReadableRows {
                rows: extracted.rows,
                malformed_rows: Vec::new(),
                format: ImportFormat::Astm,
            })
        }
    }
}

const fn file_failure(failure: FileExtractFailure) -> ImportFailureCode {
    match failure {
        FileExtractFailure::UndecodableText => ImportFailureCode::UndecodableText,
        FileExtractFailure::LayoutMismatch => ImportFailureCode::LayoutMismatch,
        FileExtractFailure::LimitsExceeded => ImportFailureCode::LimitsExceeded,
    }
}
