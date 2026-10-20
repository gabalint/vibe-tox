//! Billing commands (REQ-BIL-001/002/005, REQ-OPS-001): the class-mapping
//! lifecycle here, derivations and billing work in [`services`], handoffs
//! in [`handoffs`]. Every command needs `billing`, except mapping
//! confirmation, which needs `approve_configuration`. Reads that return
//! subject identity record an access event.

mod handoffs;
mod services;

use std::collections::BTreeSet;

pub use handoffs::{BillingHandoffInput, ExportedHandoffFile};
pub use services::GetServiceDerivationInput;

use lis_billing::{
    ApproveClassMappingVersion, BillingClassMappingVersionId, BillingClassMappingVersionSummary,
    BillingClassMappingVersionView, BillingError, ConfirmClassMappingVersion,
    DraftClassMappingVersion, RetireClassMappingVersion, UpdateClassMappingDraft,
};
use lis_core::{Capability, CommandError};
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::domain_error;
use crate::registry::{Access, CommandSpec, spec};

const BILLING: Access = Access::Capability(Capability::Billing);
const CONFIRM: Access = Access::Capability(Capability::ApproveConfiguration);

pub(crate) const LIST_CLASS_MAPPING_VERSIONS: CommandSpec = spec(
    "list_class_mapping_versions",
    "billing.list_class_mapping_versions",
    BILLING,
);
pub(crate) const GET_CLASS_MAPPING_VERSION: CommandSpec = spec(
    "get_class_mapping_version",
    "billing.get_class_mapping_version",
    BILLING,
);
pub(crate) const DRAFT_CLASS_MAPPING_VERSION: CommandSpec = spec(
    "draft_class_mapping_version",
    "billing.draft_class_mapping",
    BILLING,
);
pub(crate) const UPDATE_CLASS_MAPPING_DRAFT: CommandSpec = spec(
    "update_class_mapping_draft",
    "billing.update_class_mapping_draft",
    BILLING,
);
pub(crate) const APPROVE_CLASS_MAPPING_VERSION: CommandSpec = spec(
    "approve_class_mapping_version",
    "billing.approve_class_mapping",
    BILLING,
);
pub(crate) const CONFIRM_CLASS_MAPPING_VERSION: CommandSpec = spec(
    "confirm_class_mapping_version",
    "billing.confirm_class_mapping",
    CONFIRM,
);
pub(crate) const RETIRE_CLASS_MAPPING_VERSION: CommandSpec = spec(
    "retire_class_mapping_version",
    "billing.retire_class_mapping",
    BILLING,
);
pub(crate) const LIST_BILLING_WORK: CommandSpec =
    spec("list_billing_work", "billing.list_billing_work", BILLING);
pub(crate) const DERIVE_SERVICES: CommandSpec = spec("derive_services", "billing.derive", BILLING);
pub(crate) const GET_SERVICE_DERIVATION: CommandSpec = spec(
    "get_service_derivation",
    "billing.get_service_derivation",
    BILLING,
);
pub(crate) const APPROVE_SERVICE_DERIVATION: CommandSpec = spec(
    "approve_service_derivation",
    "billing.approve_service_derivation",
    BILLING,
);
pub(crate) const CREATE_BILLING_HANDOFF: CommandSpec =
    spec("create_billing_handoff", "billing.create_handoff", BILLING);
pub(crate) const GET_BILLING_HANDOFF: CommandSpec =
    spec("get_billing_handoff", "billing.get_handoff", BILLING);
pub(crate) const EXPORT_BILLING_HANDOFF: CommandSpec =
    spec("export_billing_handoff", "billing.export_handoff", BILLING);
pub(crate) const RECORD_HANDOFF_OUTCOME: CommandSpec = spec(
    "record_handoff_outcome",
    "billing.record_handoff_outcome",
    BILLING,
);

pub(crate) const RESOLVE_BILLING_CONTEXT_CONFLICT: CommandSpec = spec(
    "resolve_billing_context_conflict",
    "billing.resolve_context_conflict",
    BILLING,
);
pub(crate) const RECORD_HANDOFF_FOLLOWUP: CommandSpec = spec(
    "record_handoff_followup",
    "billing.record_handoff_followup",
    BILLING,
);

/// Input for the mapping detail query.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetClassMappingVersionInput {
    pub version_id: BillingClassMappingVersionId,
}

domain_error!(
    BillingError,
    |error: &BillingError| match error {
        BillingError::Invalid(field) => vec![field.clone()],
        _ => Vec::new(),
    },
    BillingError::gate_reasons
);

impl Engine {
    /// Lists every billing class mapping version; mapping content contains no PHI.
    pub async fn list_class_mapping_versions(
        &self,
    ) -> Result<Vec<BillingClassMappingVersionSummary>, CommandError> {
        self.run(&LIST_CLASS_MAPPING_VERSIONS, |call| async move {
            self.read(&call, |conn| -> Result<_, BillingError> {
                let mut versions = lis_billing::list_class_mapping_versions(conn)?;
                resolve_mapping_names(conn, &mut versions)?;
                Ok(versions)
            })
            .await
        })
        .await
    }

    /// Gets one billing class mapping version; mapping content contains no PHI.
    pub async fn get_class_mapping_version(
        &self,
        input: GetClassMappingVersionInput,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&GET_CLASS_MAPPING_VERSION, |call| async move {
            self.read(&call, move |conn| -> Result<_, BillingError> {
                let mut version = lis_billing::get_class_mapping_version(conn, input.version_id)?;
                resolve_mapping_names(conn, std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// Opens the one billing class mapping draft.
    pub async fn draft_class_mapping_version(
        &self,
        input: DraftClassMappingVersion,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&DRAFT_CLASS_MAPPING_VERSION, |call| async move {
            self.write(&call, move |ctx| -> Result<_, BillingError> {
                let mut version = lis_billing::draft_class_mapping_version(ctx, input)?;
                resolve_mapping_names(ctx.tx(), std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// Replaces the mapping draft's complete content.
    pub async fn update_class_mapping_draft(
        &self,
        input: UpdateClassMappingDraft,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&UPDATE_CLASS_MAPPING_DRAFT, |call| async move {
            self.write(&call, move |ctx| -> Result<_, BillingError> {
                let mut version = lis_billing::update_class_mapping_draft(ctx, input)?;
                resolve_mapping_names(ctx.tx(), std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// Records billing-owner approval by someone other than the author.
    pub async fn approve_class_mapping_version(
        &self,
        input: ApproveClassMappingVersion,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&APPROVE_CLASS_MAPPING_VERSION, |call| async move {
            self.write(&call, move |ctx| -> Result<_, BillingError> {
                let mut version = lis_billing::approve_class_mapping_version(ctx, input)?;
                resolve_mapping_names(ctx.tx(), std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// Confirms analyte identity under laboratory configuration authority.
    pub async fn confirm_class_mapping_version(
        &self,
        input: ConfirmClassMappingVersion,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&CONFIRM_CLASS_MAPPING_VERSION, |call| async move {
            self.write(&call, move |ctx| -> Result<_, BillingError> {
                let mut version = lis_billing::confirm_class_mapping_version(ctx, input)?;
                resolve_mapping_names(ctx.tx(), std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// Retires a mapping without rewriting any prior derivation.
    pub async fn retire_class_mapping_version(
        &self,
        input: RetireClassMappingVersion,
    ) -> Result<BillingClassMappingVersionView, CommandError> {
        self.run(&RETIRE_CLASS_MAPPING_VERSION, |call| async move {
            self.write(&call, move |ctx| -> Result<_, BillingError> {
                let mut version = lis_billing::retire_class_mapping_version(ctx, input)?;
                resolve_mapping_names(ctx.tx(), std::slice::from_mut(&mut version.summary))?;
                Ok(version)
            })
            .await
        })
        .await
    }
}

fn resolve_mapping_names(
    conn: &rusqlite::Connection,
    versions: &mut [BillingClassMappingVersionSummary],
) -> Result<(), BillingError> {
    let mut users = BTreeSet::new();
    for version in versions.iter() {
        users.insert(version.created_by);
        users.extend(version.approved_by);
    }
    let names = lis_auth::display_names(conn, &users).map_err(|error| match error {
        lis_auth::AuthError::Store(error) => BillingError::Store(error),
        _ => BillingError::Inconsistent("mapping actor"),
    })?;
    for version in versions {
        version.created_by_display = names.get(&version.created_by).cloned();
        version.approved_by_display = version
            .approved_by
            .and_then(|approved_by| names.get(&approved_by).cloned());
    }
    Ok(())
}
