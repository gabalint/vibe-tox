//! Label print requests and their outcomes (REQ-SPC-001).

use std::collections::BTreeSet;

use lis_audit::AuditDetail;
use lis_core::Capability;
use lis_store::{CommandContext, OutboxItem, StoreError};
use serde::Deserialize;
use ts_rs::TS;

use super::{actor, clinical, require_label_printer, require_user};
use crate::actions;
use crate::error::{SpecimenError, invalid};
use crate::input::{self, TEXT_MAX};
use crate::label_delivery::{LABEL_PRINT_TOPIC, LabelDelivery, SPECIMEN_TEMPLATE};
use crate::model::{ContainerId, LabelJobId, LabelJobKind, PrinterId};
use crate::repo::labels::{self as repo, NewJob, NewOutcome};
use crate::repo::{config, containers};
use crate::views::LabelJobView;

/// Most containers in one print request.
const MAX_LABELS: usize = 50;

/// Input of [`print_labels`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct PrintLabelsInput {
    /// The containers, each once.
    pub container_ids: Vec<ContainerId>,
    /// An active printer.
    pub printer_id: PrinterId,
    /// `print` for containers never labelled; `reprint` otherwise.
    pub kind: LabelJobKind,
    /// Required for a reprint.
    #[serde(default)]
    #[ts(optional)]
    pub reason: Option<String>,
}

/// Requests labels for containers that are not disposed. Each job is
/// recorded and an `spc.label.print` outbox item is enqueued in the same
/// transaction; the label worker renders and sends it after commit. A
/// container's first label is a `print`; any later one is a `reprint` with a
/// reason, and keeps the same label code (REQ-SPC-001).
pub fn print_labels(
    ctx: &mut CommandContext<'_>,
    input: PrintLabelsInput,
) -> Result<Vec<LabelJobView>, SpecimenError> {
    require_user(ctx, Capability::Accession)?;
    let reason = input::optional_text("reason", input.reason.as_deref(), TEXT_MAX)?;
    if input.kind == LabelJobKind::Reprint && reason.is_none() {
        return Err(invalid(
            "reason",
            "required",
            "Enter the reason for the reprint.",
        ));
    }
    let unique: BTreeSet<ContainerId> = input.container_ids.iter().copied().collect();
    if unique.is_empty() || unique.len() != input.container_ids.len() || unique.len() > MAX_LABELS {
        return Err(invalid(
            "containerIds",
            "invalid_selection",
            "Select from 1 to 50 containers, each once.",
        ));
    }
    let printer =
        config::printer(ctx.tx(), input.printer_id)?.ok_or(SpecimenError::NotFound("printer"))?;
    if !printer.active {
        return Err(SpecimenError::Inactive("printer"));
    }
    let mut jobs = Vec::new();
    for container_id in &input.container_ids {
        jobs.push(request_label(
            ctx,
            *container_id,
            &input,
            reason.as_deref(),
        )?);
    }
    Ok(jobs)
}

fn request_label(
    ctx: &CommandContext<'_>,
    container_id: ContainerId,
    input: &PrintLabelsInput,
    reason: Option<&str>,
) -> Result<LabelJobView, SpecimenError> {
    let container = containers::container(ctx.tx(), container_id)?
        .ok_or(SpecimenError::NotFound("container"))?;
    container.state.ensure_not_disposed()?;
    if input.kind == LabelJobKind::Print && repo::container_has_job(ctx.tx(), container_id)? {
        return Err(SpecimenError::AlreadyPrinted);
    }
    let id = LabelJobId::generate();
    let by = actor(ctx);
    repo::insert_job(
        ctx.tx(),
        &NewJob {
            id,
            container_id,
            printer_id: input.printer_id,
            template_id: SPECIMEN_TEMPLATE.0,
            template_version: SPECIMEN_TEMPLATE.1,
            kind: input.kind,
            reason,
            by: &by,
            at: ctx.now(),
        },
    )?;
    ctx.enqueue(OutboxItem {
        topic: LABEL_PRINT_TOPIC,
        dedup_key: id.to_string(),
        payload: serde_json::json!({ "labelJobId": id }),
    })?;
    let mut event = clinical(actions::PRINT_LABEL)
        .record("label_job", id)
        .detail("container", AuditDetail::id(container_id))
        .detail("printer", AuditDetail::id(input.printer_id))
        .detail("kind", AuditDetail::code(input.kind.as_str()));
    if let Some(reason) = reason {
        event = event.reason(reason);
    }
    ctx.audit(event)?;
    repo::job(ctx.tx(), id)?.ok_or(SpecimenError::NotFound("label job"))
}

/// Records what happened to a label job; run by the label worker after it
/// tried to hand the label to the printer. `handed_off` means the bytes
/// reached the printer or spooler, not that a label printed. Each job has
/// one outcome.
pub fn record_label_outcome(
    ctx: &mut CommandContext<'_>,
    delivery: LabelDelivery,
) -> Result<(), SpecimenError> {
    require_label_printer(ctx)?;
    repo::job(ctx.tx(), delivery.job_id)?.ok_or(SpecimenError::NotFound("label job"))?;
    if repo::job_has_outcome(ctx.tx(), delivery.job_id)? {
        return Err(SpecimenError::OutcomeRecorded);
    }
    repo::insert_outcome(
        ctx.tx(),
        &NewOutcome {
            job_id: delivery.job_id,
            outcome: delivery.outcome,
            error_code: delivery.error_code,
            label_sha256: delivery.label_sha256.as_deref(),
            truncated: delivery.truncated,
            at: ctx.now(),
        },
    )?;
    let mut event = clinical(actions::RECORD_LABEL_OUTCOME)
        .record("label_job", delivery.job_id)
        .detail("outcome", AuditDetail::code(delivery.outcome.as_str()));
    if let Some(error_code) = delivery.error_code {
        event = event.detail("error_code", AuditDetail::code(error_code));
    }
    if let Some(digest) = &delivery.label_sha256 {
        event = event.detail(
            "label_sha256",
            AuditDetail::sha256(digest).map_err(StoreError::from)?,
        );
    }
    if let Some(truncated) = delivery.truncated {
        event = event.detail("truncated", AuditDetail::boolean(truncated));
    }
    ctx.audit(event)?;
    Ok(())
}
