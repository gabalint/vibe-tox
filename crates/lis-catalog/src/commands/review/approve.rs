//! Dependency-ordered activation of exactly the reviewed drafts.
use crate::commands::methods::{
    ApproveMethodVersion, approve_method_version_deferring_use_check,
    ensure_current_orderables_use_current_methods,
};
use crate::commands::orderables::{ApproveOrderableVersion, approve_orderable_version};
use crate::commands::validity::{ApproveValidityPolicyVersion, approve_validity_policy_version};
use crate::{error::CatalogError, model::CatalogDraft};
use lis_store::CommandContext;

/// Approves the listed drafts in dependency order, passing on the reviewer's
/// `self_approval` confirmation; returns the counts of validity policies,
/// methods and orderables approved.
pub(super) fn approve_drafts(
    ctx: &mut CommandContext<'_>,
    drafts: &[CatalogDraft],
    self_approval: bool,
    reference: Option<&str>,
) -> Result<[usize; 3], CatalogError> {
    let mut approved = [0; 3];
    for draft in drafts {
        if let CatalogDraft::ValidityPolicyVersion { id, revision } = *draft {
            let input = ApproveValidityPolicyVersion {
                policy_version_id: id,
                expected_revision: revision,
                self_approval,
            };
            approve_validity_policy_version(ctx, input)?;
            approved[0] += 1;
        }
    }
    for draft in drafts {
        if let CatalogDraft::MethodVersion { id, revision } = *draft {
            let input = ApproveMethodVersion {
                external_approval_reference: reference.map(str::to_owned),
                method_version_id: id,
                expected_revision: revision,
                self_approval,
            };
            // Tests moving to this version are approved below; the use
            // check runs once, after them.
            approve_method_version_deferring_use_check(ctx, input)?;
            approved[1] += 1;
        }
    }
    // After the methods: an orderable is approved only once they are current.
    for draft in drafts {
        if let CatalogDraft::OrderableVersion { id, revision } = *draft {
            let input = ApproveOrderableVersion {
                orderable_version_id: id,
                expected_revision: revision,
                self_approval,
            };
            approve_orderable_version(ctx, input)?;
            approved[2] += 1;
        }
    }
    // REQ-ANA-009: no test or panel left current on a replaced method version.
    ensure_current_orderables_use_current_methods(ctx.tx())?;
    Ok(approved)
}
