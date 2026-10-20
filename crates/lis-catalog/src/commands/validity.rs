//! Validity policy version commands (REQ-SPC-008). The lifecycle is the one
//! methods and orderables use; a policy belongs to a specimen type.

use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::commands::lifecycle::{
    begin_approval, begin_draft, begin_edit, begin_retirement, finish_approval, finish_draft,
    finish_edit, finish_retirement,
};
use crate::content::ValidityPolicyContent;
use crate::error::CatalogError;
use crate::lifecycle::VersionRef;
use crate::model::{
    REASON_MAX, SpecimenTypeId, ValidityPolicyVersion, ValidityPolicyVersionId, text,
};
use crate::repo::validity::{self as repo, NewPolicyVersion};

/// Input of [`draft_validity_policy_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct DraftValidityPolicyVersion {
    /// The specimen type the policy applies to.
    pub specimen_type_id: SpecimenTypeId,
    /// The draft's categories.
    pub content: ValidityPolicyContent,
}

/// Opens the next draft policy version of a specimen type; one draft at a time.
pub fn draft_validity_policy_version(
    ctx: &mut CommandContext<'_>,
    input: DraftValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CatalogError> {
    let (author, version) = begin_draft::<ValidityPolicyVersion>(ctx, input.specimen_type_id)?;
    let content = input.content.validate()?;
    let id = ValidityPolicyVersionId::generate();
    repo::insert_version(
        ctx.tx(),
        &NewPolicyVersion {
            id,
            specimen_type_id: input.specimen_type_id,
            version,
            created_by: author,
            created_at: ctx.now(),
        },
    )?;
    repo::replace_categories(ctx.tx(), id, &content)?;
    finish_draft(ctx, id, input.specimen_type_id, version)
}

/// Input of [`update_validity_policy_draft`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateValidityPolicyDraft {
    /// The draft.
    pub policy_version_id: ValidityPolicyVersionId,
    /// The revision the author saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The replacement categories.
    pub content: ValidityPolicyContent,
}

/// Replaces a draft's categories. Only its author may edit it.
pub fn update_validity_policy_draft(
    ctx: &mut CommandContext<'_>,
    input: UpdateValidityPolicyDraft,
) -> Result<VersionRef<ValidityPolicyVersion>, CatalogError> {
    let pending = begin_edit(ctx, input.policy_version_id, input.expected_revision)?;
    let content = input.content.validate()?;
    repo::replace_categories(ctx.tx(), input.policy_version_id, &content)?;
    finish_edit(ctx, &pending)
}

/// Input of [`approve_validity_policy_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ApproveValidityPolicyVersion {
    /// The draft.
    pub policy_version_id: ValidityPolicyVersionId,
    /// The revision the approver reviewed.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they wrote the draft and approve it with no second
    /// person, as the laboratory's separation-of-duties policy allows (ADR 0015);
    /// needed only when they wrote it.
    pub self_approval: bool,
}

/// Approves a draft (someone other than its author, unless the laboratory's
/// separation-of-duties policy lets the author, ADR 0015). It becomes immutable
/// and current for its specimen type; the previous current version's period ends.
pub fn approve_validity_policy_version(
    ctx: &mut CommandContext<'_>,
    input: ApproveValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CatalogError> {
    let pending = begin_approval(
        ctx,
        input.policy_version_id,
        input.expected_revision,
        input.self_approval,
    )?;
    finish_approval(ctx, &pending)
}

/// Input of [`retire_validity_policy_version`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RetireValidityPolicyVersion {
    /// The approved version.
    pub policy_version_id: ValidityPolicyVersionId,
    /// The revision the user saw.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Why it is retired.
    pub reason: String,
}

/// Retires an approved version: no longer current, still readable by ID for
/// assessments that used it.
pub fn retire_validity_policy_version(
    ctx: &mut CommandContext<'_>,
    input: RetireValidityPolicyVersion,
) -> Result<VersionRef<ValidityPolicyVersion>, CatalogError> {
    let pending = begin_retirement(ctx, input.policy_version_id, input.expected_revision)?;
    let reason = text("reason", &input.reason, REASON_MAX)?;
    finish_retirement(ctx, &pending, reason)
}
