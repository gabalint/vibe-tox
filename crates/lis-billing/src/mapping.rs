//! Versioned analyte-to-class mapping lifecycle (REQ-BIL-005).

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_auth::Duty;
use lis_core::{Capability, UserId, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, params};
use serde::Deserialize;
use ts_rs::TS;

use crate::error::{BillingError, invalid};
use crate::mapping_support::{
    SUMMARY_COLUMNS, acting_user, current_mapping, ensure_revision, has_draft, insert_entries,
    load_summary, next_version, summary_row, validate_entries,
};
use crate::model::{
    BillingClassMappingEntry, BillingClassMappingVersionId, BillingClassMappingVersionSummary,
    BillingClassMappingVersionView, BillingVersionStatus,
};

const REASON_MAX: usize = 1000;
const RECORD_TYPE: &str = "billing_class_mapping_version";

/// Input for opening the single mapping draft.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DraftClassMappingVersion {
    pub entries: Vec<BillingClassMappingEntry>,
}

/// Input for replacing a mapping draft's complete content.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct UpdateClassMappingDraft {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub entries: Vec<BillingClassMappingEntry>,
}

/// Input for billing-owner approval of a mapping draft.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApproveClassMappingVersion {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they wrote the draft and approve it with no second
    /// person, as the laboratory's separation-of-duties policy allows (ADR 0015);
    /// needed only when they wrote it.
    pub self_approval: bool,
}

/// Input for laboratory confirmation of approved analyte identity.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ConfirmClassMappingVersion {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
}

/// Input for retiring an approved mapping without rewriting past derivations.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct RetireClassMappingVersion {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
}

/// Opens the one editable mapping draft. No mapping content ships with the product.
pub fn draft_class_mapping_version(
    ctx: &mut CommandContext<'_>,
    input: DraftClassMappingVersion,
) -> Result<BillingClassMappingVersionView, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let entries = validate_entries(ctx.tx(), input.entries)?;
    if has_draft(ctx.tx())? {
        return Err(BillingError::DraftExists);
    }
    let id = BillingClassMappingVersionId::generate();
    let version = next_version(ctx.tx())?;
    ctx.tx().execute(
        "INSERT INTO bil_class_mapping_versions
         (id, version, status, revision, created_by, created_at)
         VALUES (?1, ?2, 'draft', 1, ?3, ?4)",
        params![id, version, actor, ctx.now()],
    )?;
    insert_entries(ctx.tx(), id, &entries)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.draft_class_mapping",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, id)
        .new_revision(1)
        .detail("version", AuditDetail::integer(version))
        .detail("entry_count", AuditDetail::count(entries.len())),
    )?;
    get_class_mapping_version(ctx.tx(), id)
}

/// Replaces the complete draft, only for its author and expected revision.
pub fn update_class_mapping_draft(
    ctx: &mut CommandContext<'_>,
    input: UpdateClassMappingDraft,
) -> Result<BillingClassMappingVersionView, BillingError> {
    let (actor, current) = open_draft(ctx, input.version_id, input.expected_revision)?;
    if current.created_by != actor {
        return Err(BillingError::NotDraftAuthor);
    }
    let entries = validate_entries(ctx.tx(), input.entries)?;
    let next_revision = current.revision + 1;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.update_class_mapping_draft",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, input.version_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .detail("version", AuditDetail::integer(current.version))
        .detail("entry_count", AuditDetail::count(entries.len())),
    )?;
    ctx.tx().execute(
        "DELETE FROM bil_class_mapping_entries WHERE version_id = ?1",
        [input.version_id],
    )?;
    insert_entries(ctx.tx(), input.version_id, &entries)?;
    ctx.tx().execute(
        "UPDATE bil_class_mapping_versions SET revision = ?2 WHERE id = ?1",
        params![input.version_id, next_revision],
    )?;
    get_class_mapping_version(ctx.tx(), input.version_id)
}

/// The draft `version_id` at `expected_revision`, and the billing user acting on it.
fn open_draft(
    ctx: &mut CommandContext<'_>,
    version_id: BillingClassMappingVersionId,
    expected_revision: i64,
) -> Result<(UserId, BillingClassMappingVersionSummary), BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let current = load_summary(ctx.tx(), version_id)?;
    ensure_revision(&current, expected_revision)?;
    if current.status != BillingVersionStatus::Draft {
        return Err(BillingError::InvalidMappingState);
    }
    Ok((actor, current))
}

/// Records billing-owner approval: by someone other than the author, or by the
/// author as the laboratory's separation-of-duties policy allows, with
/// `self_approval` confirmed (ADR 0015). The rule is checked again in SQLite.
pub fn approve_class_mapping_version(
    ctx: &mut CommandContext<'_>,
    input: ApproveClassMappingVersion,
) -> Result<BillingClassMappingVersionView, BillingError> {
    let (actor, current) = open_draft(ctx, input.version_id, input.expected_revision)?;
    let self_approval = current.created_by == actor;
    if self_approval {
        // REQ-BIL-005, ADR 0015: the policy in force, read in this transaction.
        lis_auth::current_separation_policy(ctx.tx())?
            .authorize_self(Duty::ConfigurationApproval, input.self_approval)?;
    }
    let next_revision = current.revision + 1;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        "billing.approve_class_mapping",
        Outcome::Succeeded,
    )
    .record(RECORD_TYPE, input.version_id)
    .old_revision(current.revision)
    .new_revision(next_revision)
    .detail("version", AuditDetail::integer(current.version));
    if self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    ctx.tx().execute(
        "UPDATE bil_class_mapping_versions
         SET status = 'approved', revision = ?2, approved_by = ?3, approved_at = ?4
         WHERE id = ?1",
        params![input.version_id, next_revision, actor, ctx.now()],
    )?;
    get_class_mapping_version(ctx.tx(), input.version_id)
}

/// Confirms analyte identity under laboratory configuration authority and makes
/// the version current, ending the prior version's effective interval.
pub fn confirm_class_mapping_version(
    ctx: &mut CommandContext<'_>,
    input: ConfirmClassMappingVersion,
) -> Result<BillingClassMappingVersionView, BillingError> {
    ctx.require(Capability::ApproveConfiguration)?;
    let actor = acting_user(ctx.principal())?;
    let current = load_summary(ctx.tx(), input.version_id)?;
    ensure_revision(&current, input.expected_revision)?;
    if current.status != BillingVersionStatus::Approved {
        return Err(BillingError::ApprovalRequired);
    }
    if current.confirmed_by.is_some() {
        return Err(BillingError::InvalidMappingState);
    }
    if let Some(previous) = current_mapping(ctx.tx())? {
        let next_revision = previous.revision + 1;
        ctx.audit(
            AuditEvent::new(
                Stream::Clinical,
                "billing.supersede_class_mapping",
                Outcome::Succeeded,
            )
            .record(RECORD_TYPE, previous.version_id)
            .old_revision(previous.revision)
            .new_revision(next_revision)
            .detail("replaced_by", AuditDetail::id(input.version_id)),
        )?;
        ctx.tx().execute(
            "UPDATE bil_class_mapping_versions
             SET revision = ?2, effective_to = ?3 WHERE id = ?1",
            params![previous.version_id, next_revision, ctx.now()],
        )?;
    }
    let next_revision = current.revision + 1;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.confirm_class_mapping",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, input.version_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .detail("version", AuditDetail::integer(current.version)),
    )?;
    ctx.tx().execute(
        "UPDATE bil_class_mapping_versions
         SET revision = ?2, confirmed_by = ?3, confirmed_at = ?4, effective_from = ?4
         WHERE id = ?1",
        params![input.version_id, next_revision, actor, ctx.now()],
    )?;
    get_class_mapping_version(ctx.tx(), input.version_id)
}

/// Retires an approved mapping with a reason. Past derivations keep their frozen facts.
pub fn retire_class_mapping_version(
    ctx: &mut CommandContext<'_>,
    input: RetireClassMappingVersion,
) -> Result<BillingClassMappingVersionView, BillingError> {
    ctx.require(Capability::Billing)?;
    let actor = acting_user(ctx.principal())?;
    let current = load_summary(ctx.tx(), input.version_id)?;
    ensure_revision(&current, input.expected_revision)?;
    if current.status != BillingVersionStatus::Approved {
        return Err(BillingError::InvalidMappingState);
    }
    let reason = required_text("reason", &input.reason, REASON_MAX).map_err(invalid)?;
    let next_revision = current.revision + 1;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "billing.retire_class_mapping",
            Outcome::Succeeded,
        )
        .record(RECORD_TYPE, input.version_id)
        .old_revision(current.revision)
        .new_revision(next_revision)
        .reason(reason.clone())
        .detail("version", AuditDetail::integer(current.version)),
    )?;
    ctx.tx().execute(
        "UPDATE bil_class_mapping_versions
         SET status = 'retired', revision = ?2, retired_by = ?3, retired_at = ?4,
             retire_reason = ?5,
             effective_to = CASE WHEN effective_from IS NULL THEN NULL
                                 ELSE COALESCE(effective_to, ?4) END
         WHERE id = ?1",
        params![input.version_id, next_revision, actor, ctx.now(), reason],
    )?;
    get_class_mapping_version(ctx.tx(), input.version_id)
}

/// Lists all mapping versions, newest first. Mapping content contains no PHI.
pub fn list_class_mapping_versions(
    conn: &Connection,
) -> Result<Vec<BillingClassMappingVersionSummary>, BillingError> {
    Ok(conn
        .prepare_cached(&format!(
            "SELECT {SUMMARY_COLUMNS} FROM bil_class_mapping_versions ORDER BY version DESC"
        ))?
        .query_map([], summary_row)?
        .collect::<Result<_, _>>()?)
}

/// Gets one mapping version and its entries. Mapping content contains no PHI.
pub fn get_class_mapping_version(
    conn: &Connection,
    id: BillingClassMappingVersionId,
) -> Result<BillingClassMappingVersionView, BillingError> {
    let summary = load_summary(conn, id)?;
    let entries = conn
        .prepare_cached(
            "SELECT analyte_id, class_code, class_label FROM bil_class_mapping_entries
             WHERE version_id = ?1 ORDER BY analyte_id",
        )?
        .query_map([id], |row| {
            Ok(BillingClassMappingEntry {
                analyte_id: row.get(0)?,
                class_code: row.get(1)?,
                class_label: row.get(2)?,
            })
        })?
        .collect::<Result<_, _>>()?;
    Ok(BillingClassMappingVersionView {
        allowed_actions: allowed_actions(&summary),
        summary,
        entries,
    })
}

fn allowed_actions(summary: &BillingClassMappingVersionSummary) -> Vec<String> {
    let actions: &[&str] = match summary.status {
        BillingVersionStatus::Draft => &[
            "update_class_mapping_draft",
            "approve_class_mapping_version",
        ],
        BillingVersionStatus::Approved if summary.confirmed_by.is_none() => &[
            "confirm_class_mapping_version",
            "retire_class_mapping_version",
        ],
        BillingVersionStatus::Approved => &["retire_class_mapping_version"],
        BillingVersionStatus::Retired => &[],
    };
    actions.iter().map(|action| (*action).to_owned()).collect()
}

/// Returns the laboratory-confirmed mapping usable for new derivations.
pub fn current_class_mapping(
    conn: &Connection,
) -> Result<Option<BillingClassMappingVersionView>, BillingError> {
    current_mapping(conn)?
        .map(|summary| get_class_mapping_version(conn, summary.version_id))
        .transpose()
}
