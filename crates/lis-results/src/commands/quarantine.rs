//! Quarantine resolution and receipt retry.

use super::*;

pub fn resolve_quarantine_item(
    ctx: &mut CommandContext<'_>,
    input: ResolveQuarantineItemInput,
) -> Result<ImportAttemptDetail, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    let actor = require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    let (attempt_id, reason, state, revision): (
        crate::ImportAttemptId,
        QuarantineReason,
        QuarantineState,
        i64,
    ) = ctx
        .tx()
        .query_row(
            "SELECT import_attempt_id, reason, state, revision FROM res_quarantine_items WHERE id = ?1",
            [input.item_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .optional()?
        .ok_or(ResultError::NotFound("quarantine item"))?;
    if state != QuarantineState::Open || revision != input.expected_revision {
        return Err(ResultError::RevisionConflict);
    }
    let next_state = match input.resolution {
        QuarantineResolution::AcceptAsReprocessing => {
            if reason != QuarantineReason::ConflictingDuplicate {
                return Err(ResultError::WrongState("quarantine item"));
            }
            crate::import::accept_quarantined_as_reprocessing(ctx, input.item_id)?;
            QuarantineState::AcceptedAsReprocessing
        }
        QuarantineResolution::Reject => QuarantineState::Rejected,
    };
    ctx.tx().execute(
        "UPDATE res_quarantine_items SET state = ?2, resolved_by = ?3, resolved_at = ?4, \
         resolution_reason = ?5, revision = revision + 1 WHERE id = ?1",
        params![input.item_id, next_state, actor, ctx.now(), input.reason],
    )?;
    ctx.audit(
        clinical("result.resolve_quarantine")
            .record("quarantine_item", input.item_id)
            .old_revision(revision)
            .new_revision(revision + 1)
            .reason(input.reason)
            .detail("resolution", AuditDetail::code(input.resolution.as_str())),
    )?;
    get_import_attempt(ctx.tx(), attempt_id)
}

/// Enqueues a new attempt number; bytes remain in the original receipt artifact.
pub fn retry_import(
    ctx: &mut CommandContext<'_>,
    input: RetryImportInput,
) -> Result<i64, ResultError> {
    ctx.require(Capability::ReviewResults)?;
    require_user(ctx)?;
    require_reason("reason", &input.reason)?;
    let exists: bool = ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM ifc_receipts WHERE id = ?1 AND state = 'complete')",
        [input.receipt_id],
        |row| row.get(0),
    )?;
    if !exists {
        return Err(ResultError::NotFound("complete receipt"));
    }
    let attempt_no = next_attempt_no(ctx.tx(), input.receipt_id)?;
    ctx.enqueue(OutboxItem {
        topic: RETRY_IMPORT_TOPIC,
        dedup_key: format!("{}:{attempt_no}", input.receipt_id),
        payload: serde_json::json!({
            "receipt_id": input.receipt_id,
            "attempt_no": attempt_no,
        }),
    })?;
    ctx.audit(
        clinical("result.retry_import")
            .record("receipt", input.receipt_id)
            .reason(input.reason)
            .detail("attempt_no", AuditDetail::integer(attempt_no)),
    )?;
    Ok(attempt_no)
}
