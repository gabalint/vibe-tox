//! The import attempt record: counts, outcome, audit and idempotency.

use super::*;
use crate::model::ImportOutcome;

/// What identifies an attempt and how its bytes were read.
pub(super) struct Attempt {
    pub(super) receipt_id: ReceiptId,
    pub(super) attempt_no: i64,
    pub(super) instrument_id: InstrumentId,
    pub(super) registration: Option<lis_analytical::SourceRegistrationId>,
    pub(super) parser_version: &'static str,
    /// The identical earlier receipt of a `duplicate` attempt.
    pub(super) duplicate_of: Option<ReceiptId>,
}

impl Attempt {
    /// The attempt of `input`; the parser follows the source's format.
    pub(super) fn of(input: &ImportReceiptInput) -> Self {
        // Nothing in a duplicate is read, so it names no registration.
        let (parser_version, duplicate_of) = match &input.source {
            ImportSource::Extracted(_) | ImportSource::Malformed(_) => (ASTM_PARSER_VERSION, None),
            ImportSource::Au { .. } => (AU_PARSER_VERSION, None),
            ImportSource::File(_) | ImportSource::UnregisteredFile => (FILE_PARSER_VERSION, None),
            ImportSource::Duplicate(earlier) => (NOT_READ_PARSER_VERSION, Some(*earlier)),
        };
        let registration = input
            .registration
            .as_ref()
            .filter(|_| duplicate_of.is_none())
            .map(|item| item.registration_id);
        Self {
            receipt_id: input.receipt_id,
            attempt_no: input.attempt_no,
            instrument_id: input.instrument_id,
            registration,
            parser_version,
            duplicate_of,
        }
    }
}

/// Every row ends in exactly one count (brief §5).
#[derive(Debug, Default)]
pub(super) struct Counts {
    pub(super) total: i64,
    pub(super) new: i64,
    pub(super) duplicate: i64,
    pub(super) reprocessed: i64,
    pub(super) qc: i64,
    pub(super) quarantined: i64,
    /// File rows whose analyte is not a result (F10).
    pub(super) not_result: i64,
    /// File rows listed on the attempt without a quarantine item (F9, F17).
    pub(super) set_aside: i64,
    /// New measurements kept outside the ordered scope (F17).
    pub(super) retained: i64,
}

impl Counts {
    pub(super) fn total(rows: usize) -> Self {
        Self {
            total: count(rows),
            ..Self::default()
        }
    }

    fn outcome(&self, failed: bool) -> ImportOutcome {
        if failed || self.quarantined == self.total && self.total > 0 {
            ImportOutcome::Quarantined
        } else if self.quarantined > 0 {
            ImportOutcome::ProcessedWithQuarantine
        } else {
            ImportOutcome::Processed
        }
    }
}

fn count(rows: usize) -> i64 {
    i64::try_from(rows).unwrap_or(i64::MAX)
}

pub(super) fn supersede_prior_quarantine(
    ctx: &CommandContext<'_>,
    receipt_id: ReceiptId,
    attempt_no: i64,
) -> Result<usize, ResultError> {
    if attempt_no == 1 {
        return Ok(0);
    }
    Ok(ctx.tx().execute(
        "UPDATE res_quarantine_items SET state = 'superseded_by_retry', \
         resolved_by = ?3, resolved_at = ?4, resolution_reason = 'superseded by import retry', \
         revision = revision + 1 WHERE state = 'open' AND import_attempt_id IN \
           (SELECT id FROM res_import_attempts WHERE receipt_id = ?1 AND attempt_no < ?2)",
        params![
            receipt_id,
            attempt_no,
            ctx.principal().actor().id_text(),
            ctx.now()
        ],
    )?)
}

/// A message identical to an earlier receipt: listed with no rows, no
/// registration and no clinical effect, naming the earlier receipt whose
/// attempts hold its rows (REQ-INT-003, NFR-02). The earlier receipt is the
/// first complete one with these bytes, so it is never a duplicate itself.
pub(super) fn record_duplicate_attempt(
    ctx: &mut CommandContext<'_>,
    attempt: &Attempt,
) -> Result<ImportAttemptSummary, ResultError> {
    if attempt.duplicate_of == Some(attempt.receipt_id) {
        return Err(ResultError::WrongState("duplicate receipt"));
    }
    let summary = insert_attempt(
        ctx,
        ImportAttemptId::generate(),
        attempt,
        None,
        &Counts::default(),
    )?;
    audit_attempt(ctx, &summary, 0)?;
    Ok(summary)
}

/// A whole-attempt failure: nothing in the source is imported, and every
/// row read counts as quarantined.
pub(super) fn record_failed_attempt(
    ctx: &mut CommandContext<'_>,
    attempt: &Attempt,
    failure: ImportFailureCode,
    rows_total: usize,
) -> Result<ImportAttemptSummary, ResultError> {
    let counts = Counts {
        quarantined: count(rows_total),
        ..Counts::total(rows_total)
    };
    let summary = insert_attempt(
        ctx,
        ImportAttemptId::generate(),
        attempt,
        Some(failure),
        &counts,
    )?;
    audit_attempt(ctx, &summary, 0)?;
    Ok(summary)
}

pub(super) fn insert_attempt(
    ctx: &CommandContext<'_>,
    id: ImportAttemptId,
    attempt: &Attempt,
    failure: Option<ImportFailureCode>,
    counts: &Counts,
) -> Result<ImportAttemptSummary, ResultError> {
    let outcome = if attempt.duplicate_of.is_some() {
        ImportOutcome::Duplicate
    } else {
        counts.outcome(failure.is_some())
    };
    ctx.tx().execute(
        "INSERT INTO res_import_attempts (id, receipt_id, attempt_no, instrument_id, registration_id, \
         outcome, failure_code, rows_total, rows_new, rows_duplicate, rows_reprocessed, rows_qc, \
         rows_quarantined, rows_not_result, rows_set_aside, rows_retained, parser_version, \
         processed_at, duplicate_of_receipt_id) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16, ?17, ?18, ?19)",
        params![id, attempt.receipt_id, attempt.attempt_no, attempt.instrument_id,
            attempt.registration, outcome, failure, counts.total, counts.new, counts.duplicate,
            counts.reprocessed, counts.qc, counts.quarantined, counts.not_result,
            counts.set_aside, counts.retained, attempt.parser_version, ctx.now(),
            attempt.duplicate_of],
    )?;
    attempt_by_key(ctx.tx(), attempt.receipt_id, attempt.attempt_no)?
        .ok_or(ResultError::NotFound("import attempt"))
}

pub(super) fn audit_attempt(
    ctx: &CommandContext<'_>,
    summary: &ImportAttemptSummary,
    quarantine_superseded: usize,
) -> Result<(), ResultError> {
    let mut event = AuditEvent::new(Stream::Clinical, "result.import", Outcome::Succeeded)
        .record("import_attempt", summary.import_attempt_id)
        .detail("receipt", AuditDetail::id(summary.receipt_id))
        .detail("outcome", AuditDetail::code(summary.outcome.as_str()))
        .detail(
            "quarantine_superseded",
            AuditDetail::count(quarantine_superseded),
        );
    for (name, value) in [
        ("rows_total", summary.rows_total),
        ("rows_new", summary.rows_new),
        ("rows_duplicate", summary.rows_duplicate),
        ("rows_reprocessed", summary.rows_reprocessed),
        ("rows_qc", summary.rows_qc),
        ("rows_quarantined", summary.rows_quarantined),
        ("rows_not_result", summary.rows_not_result),
        ("rows_set_aside", summary.rows_set_aside),
        ("rows_retained", summary.rows_retained),
    ] {
        event = event.detail(name, AuditDetail::integer(value));
    }
    if let Some(failure) = summary.failure_code {
        event = event.detail("failure_code", AuditDetail::code(failure.as_str()));
    }
    if let Some(earlier) = summary.duplicate_of_attempt_id {
        event = event.detail("duplicate_of_attempt", AuditDetail::id(earlier));
    }
    ctx.audit(event)?;
    Ok(())
}

pub(super) fn attempt_by_key(
    conn: &rusqlite::Connection,
    receipt_id: ReceiptId,
    attempt_no: i64,
) -> Result<Option<ImportAttemptSummary>, ResultError> {
    Ok(conn
        .query_row(
            &format!(
                "SELECT {} FROM res_import_attempts WHERE receipt_id = ?1 AND attempt_no = ?2",
                crate::queries::ATTEMPT_SUMMARY_COLUMNS
            ),
            params![receipt_id, attempt_no],
            crate::queries::attempt_summary,
        )
        .optional()?)
}
