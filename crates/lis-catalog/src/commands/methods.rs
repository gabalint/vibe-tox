//! Method version commands (REQ-ANA-002).

use rusqlite::Connection;
use serde::Deserialize;
use ts_rs::TS;

use crate::commands::lifecycle::{
    begin_approval, begin_draft, begin_edit, begin_retirement, finish_approval, finish_draft,
    finish_edit, finish_retirement,
};
use crate::content::MethodVersionContent;
use crate::error::{CatalogError, invalid};
use crate::lifecycle::VersionRef;
use crate::model::{MethodId, MethodVersion, MethodVersionId, REASON_MAX, text};
use crate::repo::methods::{self as repo, NewMethodVersion};
use crate::repo::{analytes, reference};
use lis_store::CommandContext;

/// Input of [`draft_method_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DraftMethodVersion {
    /// The method.
    pub method_id: MethodId,
    /// The draft's content.
    pub content: MethodVersionContent,
}

/// Opens the next draft version of a method; one draft per method at a time.
pub fn draft_method_version(
    ctx: &mut CommandContext<'_>,
    input: DraftMethodVersion,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let (author, version) = begin_draft::<MethodVersion>(ctx, input.method_id)?;
    let content = input.content.validate()?;
    check_references(ctx.tx(), &content)?;
    let id = MethodVersionId::generate();
    repo::insert_version(
        ctx.tx(),
        &NewMethodVersion {
            id,
            method_id: input.method_id,
            version,
            content: &content,
            created_by: author,
            created_at: ctx.now(),
        },
    )?;
    finish_draft(ctx, id, input.method_id, version)
}

/// Input of [`update_method_draft`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateMethodDraft {
    /// The draft.
    pub method_version_id: MethodVersionId,
    /// The revision the author saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The replacement content.
    pub content: MethodVersionContent,
}

/// Replaces a draft's content. Only its author may edit it; approved and
/// retired versions are immutable.
pub fn update_method_draft(
    ctx: &mut CommandContext<'_>,
    input: UpdateMethodDraft,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let pending = begin_edit(ctx, input.method_version_id, input.expected_revision)?;
    let content = input.content.validate()?;
    check_references(ctx.tx(), &content)?;
    check_analytes_in_use(ctx.tx(), input.method_version_id, &content)?;
    repo::replace_content(ctx.tx(), input.method_version_id, &content)?;
    finish_edit(ctx, &pending)
}

/// Input of [`approve_method_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ApproveMethodVersion {
    /// The draft.
    pub method_version_id: MethodVersionId,
    /// The revision the approver reviewed.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they wrote the draft and approve it with no second
    /// person, as the laboratory's separation-of-duties policy allows (ADR 0015);
    /// needed only when they wrote it.
    pub self_approval: bool,
    /// Optional reference to laboratory approval records held outside the LIS (1–1000 characters).
    #[serde(default)]
    #[ts(optional)]
    pub external_approval_reference: Option<String>,
}

/// Approves a draft (someone other than its author, unless the laboratory's
/// separation-of-duties policy lets the author, ADR 0015). It becomes immutable
/// and current, and the previous current version's effective period ends.
pub fn approve_method_version(
    ctx: &mut CommandContext<'_>,
    input: ApproveMethodVersion,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let pending = begin_approval(
        ctx,
        input.method_version_id,
        input.expected_revision,
        input.self_approval,
    )?;
    ensure_no_blocking_orderables(repo::blocking_orderables_for_supersession(
        ctx.tx(),
        pending.parent,
    )?)?;
    finish_method_approval(ctx, &pending, input.external_approval_reference.as_deref())
}

/// [`approve_method_version`] without the per-method in-use check, for All
/// Data Reviewed, which approves the tests that move to the new version in
/// the same transaction. The caller must call
/// [`ensure_current_orderables_use_current_methods`] after its last approval.
pub(crate) fn approve_method_version_deferring_use_check(
    ctx: &mut CommandContext<'_>,
    input: ApproveMethodVersion,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let pending = begin_approval(
        ctx,
        input.method_version_id,
        input.expected_revision,
        input.self_approval,
    )?;
    finish_method_approval(ctx, &pending, input.external_approval_reference.as_deref())
}

fn finish_method_approval(
    ctx: &CommandContext<'_>,
    pending: &crate::commands::lifecycle::Pending<MethodVersion>,
    external_reference: Option<&str>,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let reference = crate::approval_references::validate(external_reference)?;
    let approved = finish_approval(ctx, pending)?;
    crate::approval_references::record(
        ctx,
        "method_version",
        pending.version.id,
        reference.as_deref(),
    )?;
    Ok(approved)
}

/// Refuses when a current approved test or panel uses a method version that
/// is no longer current (REQ-ANA-009), naming each as "CODE vN".
pub(crate) fn ensure_current_orderables_use_current_methods(
    conn: &Connection,
) -> Result<(), CatalogError> {
    let records: Vec<String> = repo::current_orderables_on_replaced_methods(conn)?
        .into_iter()
        .map(|(code, version)| format!("{code} v{version}"))
        .collect();
    if records.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::OrderablesOnReplacedMethods { records })
    }
}

/// Input of [`retire_method_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RetireMethodVersion {
    /// The approved version.
    pub method_version_id: MethodVersionId,
    /// The revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is retired.
    pub reason: String,
}

/// Retires an approved version: blocked for new use, still readable by ID.
/// Refused while a current orderable version uses it.
pub fn retire_method_version(
    ctx: &mut CommandContext<'_>,
    input: RetireMethodVersion,
) -> Result<VersionRef<MethodVersion>, CatalogError> {
    let pending = begin_retirement(ctx, input.method_version_id, input.expected_revision)?;
    let reason = text("reason", &input.reason, REASON_MAX)?;
    ensure_no_blocking_orderables(repo::blocking_current_orderables(
        ctx.tx(),
        input.method_version_id,
    )?)?;
    finish_retirement(ctx, &pending, reason)
}

fn ensure_no_blocking_orderables(
    blocking_orderables: Vec<crate::model::OrderableId>,
) -> Result<(), CatalogError> {
    if blocking_orderables.is_empty() {
        Ok(())
    } else {
        Err(CatalogError::MethodInUse {
            blocking_orderables,
        })
    }
}

/// Refuses removing an analyte that a test or panel draft expects from this
/// method draft, naming the tests and panels.
fn check_analytes_in_use(
    conn: &Connection,
    id: MethodVersionId,
    content: &MethodVersionContent,
) -> Result<(), CatalogError> {
    let mut users: Vec<String> = Vec::new();
    for (analyte, code, version) in repo::analytes_in_use(conn, id)? {
        let label = format!("{code} v{version}");
        let kept = content.analytes.iter().any(|a| a.analyte_id == analyte);
        if !kept && !users.contains(&label) {
            users.push(label);
        }
    }
    if users.is_empty() {
        return Ok(());
    }
    Err(invalid(
        "analytes",
        "in_use_by_orderable",
        &format!(
            "Tests or panels expect an analyte this edit removes: {}.",
            users.join(", ")
        ),
    ))
}

/// Refuses unknown specimen types and unknown or retired analytes as input errors.
fn check_references(conn: &Connection, content: &MethodVersionContent) -> Result<(), CatalogError> {
    if !reference::specimen_type_exists(conn, content.specimen_type_id)? {
        return Err(invalid(
            "specimenTypeId",
            "unknown",
            "Choose an existing specimen type.",
        ));
    }
    for (index, analyte) in content.analytes.iter().enumerate() {
        let field = format!("analytes[{index}].analyteId");
        match analytes::get(conn, analyte.analyte_id)?.map(|found| found.active) {
            None => return Err(invalid(&field, "unknown", "Choose an existing analyte.")),
            Some(false) => return Err(invalid(&field, "retired", "Choose an active analyte.")),
            Some(true) => {}
        }
    }
    Ok(())
}
