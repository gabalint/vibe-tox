//! Orderable version commands (REQ-ANA-002, REQ-ANA-009, REQ-ORD-002).

use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::commands::lifecycle::{
    begin_approval, begin_draft, begin_edit, begin_retirement, finish_approval, finish_draft,
    finish_edit, finish_retirement,
};
use crate::content::OrderableVersionContent;
use crate::error::{CatalogError, invalid};
use crate::lifecycle::VersionRef;
use crate::model::{
    MethodVersion, OrderableId, OrderableKind, OrderableVersion, OrderableVersionId, REASON_MAX,
    VersionStatus, text,
};
use crate::repo::orderables as repo;
use crate::repo::{lifecycle, methods, reference};

/// Input of [`draft_orderable_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DraftOrderableVersion {
    /// The orderable.
    pub orderable_id: OrderableId,
    /// The draft's component manifest.
    pub content: OrderableVersionContent,
}

/// Opens the next draft version of an orderable; one draft per orderable at a time.
pub fn draft_orderable_version(
    ctx: &mut CommandContext<'_>,
    input: DraftOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CatalogError> {
    let (author, version) = begin_draft::<OrderableVersion>(ctx, input.orderable_id)?;
    check_manifest(ctx.tx(), input.orderable_id, &input.content)?;
    let id = OrderableVersionId::generate();
    repo::insert_version(ctx.tx(), id, input.orderable_id, version, author, ctx.now())?;
    repo::insert_components(ctx.tx(), id, &input.content)?;
    finish_draft(ctx, id, input.orderable_id, version)
}

/// Input of [`update_orderable_draft`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateOrderableDraft {
    /// The draft.
    pub orderable_version_id: OrderableVersionId,
    /// The revision the author saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The replacement manifest.
    pub content: OrderableVersionContent,
}

/// Replaces a draft's manifest. Only its author may edit it.
pub fn update_orderable_draft(
    ctx: &mut CommandContext<'_>,
    input: UpdateOrderableDraft,
) -> Result<VersionRef<OrderableVersion>, CatalogError> {
    let pending = begin_edit(ctx, input.orderable_version_id, input.expected_revision)?;
    check_manifest(ctx.tx(), pending.parent, &input.content)?;
    repo::delete_components(ctx.tx(), input.orderable_version_id)?;
    repo::insert_components(ctx.tx(), input.orderable_version_id, &input.content)?;
    finish_edit(ctx, &pending)
}

/// Input of [`approve_orderable_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ApproveOrderableVersion {
    /// The draft.
    pub orderable_version_id: OrderableVersionId,
    /// The revision the approver reviewed.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they wrote the draft and approve it with no second
    /// person, as the laboratory's separation-of-duties policy allows (ADR 0015);
    /// needed only when they wrote it.
    pub self_approval: bool,
}

/// Approves a draft (someone other than its author, unless the laboratory's
/// separation-of-duties policy lets the author, ADR 0015) once every component
/// still uses a current approved method version (REQ-ANA-009). The previous
/// current version's effective period ends; orders that froze it keep it.
pub fn approve_orderable_version(
    ctx: &mut CommandContext<'_>,
    input: ApproveOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CatalogError> {
    let pending = begin_approval(
        ctx,
        input.orderable_version_id,
        input.expected_revision,
        input.self_approval,
    )?;
    // REQ-ANA-009: activation needs complete dependent configuration, rechecked here.
    if repo::components_not_current(ctx.tx(), input.orderable_version_id)? > 0 {
        return Err(CatalogError::IncompleteConfiguration);
    }
    finish_approval(ctx, &pending)
}

/// Input of [`retire_orderable_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RetireOrderableVersion {
    /// The approved version.
    pub orderable_version_id: OrderableVersionId,
    /// The revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is retired.
    pub reason: String,
}

/// Retires an approved version: no longer offered for new orders, still
/// readable by ID for orders that froze it.
pub fn retire_orderable_version(
    ctx: &mut CommandContext<'_>,
    input: RetireOrderableVersion,
) -> Result<VersionRef<OrderableVersion>, CatalogError> {
    let pending = begin_retirement(ctx, input.orderable_version_id, input.expected_revision)?;
    let reason = text("reason", &input.reason, REASON_MAX)?;
    finish_retirement(ctx, &pending, reason)
}

/// Validates the manifest against the orderable's kind, and refuses analytes
/// a method version does not measure and method versions that are neither
/// current approved nor an open draft. A draft may use a method draft, so a
/// loaded catalog needs one review, not two; approval still requires every
/// component to be current approved (REQ-ANA-009). Retired and superseded
/// versions are refused.
fn check_manifest(
    conn: &Connection,
    orderable_id: OrderableId,
    content: &OrderableVersionContent,
) -> Result<(), CatalogError> {
    let kind: OrderableKind = reference::orderable_kind(conn, orderable_id)?
        .ok_or(CatalogError::NotFound("orderable"))?;
    content.validate(kind)?;
    for (index, component) in content.components.iter().enumerate() {
        let field = format!("components[{index}]");
        let method = lifecycle::version_state::<MethodVersion>(conn, component.method_version_id)?;
        let usable = method.is_some_and(|stored| {
            let lifecycle = &stored.version.lifecycle;
            lifecycle.is_current() || lifecycle.status == VersionStatus::Draft
        });
        if !usable {
            return Err(invalid(
                &format!("{field}.methodVersionId"),
                "not_current",
                "Choose a current approved or draft method version.",
            ));
        }
        for (position, analyte) in component.analyte_ids.iter().enumerate() {
            if !methods::measures(conn, component.method_version_id, *analyte)? {
                return Err(invalid(
                    &format!("{field}.analyteIds[{position}]"),
                    "not_measured",
                    "The method version does not measure this analyte.",
                ));
            }
        }
    }
    Ok(())
}
