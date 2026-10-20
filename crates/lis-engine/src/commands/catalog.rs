//! Catalog configuration (REQ-ANA-002, REQ-ANA-009, REQ-SPC-008,
//! REQ-OPS-001): specimen types, analytes, methods, orderables and validity
//! policies, with their draft → approved → retired versions.
//!
//! Catalog content holds no patient data. Any signed-in user may read it,
//! because order entry, analysis, review and approval all need it; drafting
//! needs `manage_configuration` and approval or retirement
//! `approve_configuration`, checked in each domain command.

pub(crate) mod preview;
mod preview_document;
pub(crate) mod standard;
pub(crate) mod versions;

use lis_catalog::{
    AnalyteId, AnalyteView, ApproveValidityPolicyVersion, CatalogError, CodedName, CreateAnalyte,
    CreateOrderable, DraftValidityPolicyVersion, MethodId, MethodView, OrderableId,
    OrderableVersionView, OrderableView, RetireValidityPolicyVersion, SpecimenTypeId,
    SpecimenTypeView, UpdateValidityPolicyDraft, ValidityPolicyVersion, ValidityPolicyVersionId,
    ValidityPolicyView, VersionRef, VersionSummary,
};
use lis_core::{Capability, CommandError};
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::domain_error;
use crate::registry::{Access, CommandSpec, spec};

const MANAGE: Access = Access::Capability(Capability::ManageConfiguration);
const APPROVE: Access = Access::Capability(Capability::ApproveConfiguration);
const READ: Access = Access::Session;

pub(crate) const LIST_SPECIMEN_TYPES: CommandSpec =
    spec("list_specimen_types", "catalog.list_specimen_types", READ);
pub(crate) const CREATE_SPECIMEN_TYPE: CommandSpec = spec(
    "create_specimen_type",
    "catalog.create_specimen_type",
    MANAGE,
);
pub(crate) const LIST_ANALYTES: CommandSpec = spec("list_analytes", "catalog.list_analytes", READ);
pub(crate) const CREATE_ANALYTE: CommandSpec =
    spec("create_analyte", "catalog.create_analyte", MANAGE);
pub(crate) const LIST_METHODS: CommandSpec = spec("list_methods", "catalog.list_methods", READ);
pub(crate) const CREATE_METHOD: CommandSpec =
    spec("create_method", "catalog.create_method", MANAGE);
pub(crate) const LIST_ORDERABLES: CommandSpec =
    spec("list_orderables", "catalog.list_orderables", READ);
pub(crate) const CREATE_ORDERABLE: CommandSpec =
    spec("create_orderable", "catalog.create_orderable", MANAGE);
pub(crate) const LIST_ACTIVE_ORDERABLES: CommandSpec = spec(
    "list_active_orderables",
    "catalog.list_active_orderables",
    READ,
);
pub(crate) const LIST_VALIDITY_POLICY_VERSIONS: CommandSpec = spec(
    "list_validity_policy_versions",
    "catalog.list_validity_policy_versions",
    READ,
);
pub(crate) const GET_VALIDITY_POLICY: CommandSpec =
    spec("get_validity_policy", "catalog.get_validity_policy", READ);
pub(crate) const DRAFT_VALIDITY_POLICY_VERSION: CommandSpec = spec(
    "draft_validity_policy_version",
    "catalog.draft_validity_policy_version",
    MANAGE,
);
pub(crate) const UPDATE_VALIDITY_POLICY_DRAFT: CommandSpec = spec(
    "update_validity_policy_draft",
    "catalog.update_validity_policy_draft",
    MANAGE,
);
pub(crate) const APPROVE_VALIDITY_POLICY_VERSION: CommandSpec = spec(
    "approve_validity_policy_version",
    "catalog.approve_validity_policy_version",
    APPROVE,
);
pub(crate) const RETIRE_VALIDITY_POLICY_VERSION: CommandSpec = spec(
    "retire_validity_policy_version",
    "catalog.retire_validity_policy_version",
    APPROVE,
);

/// Input of `list_validity_policy_versions`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListValidityPolicyVersionsInput {
    /// The specimen type.
    pub specimen_type_id: SpecimenTypeId,
}

/// Input of `get_validity_policy`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetValidityPolicyInput {
    /// The version.
    pub policy_version_id: ValidityPolicyVersionId,
}

domain_error!(CatalogError, |error: &CatalogError| match error {
    CatalogError::Invalid(field) => vec![field.clone()],
    _ => Vec::new(),
});

impl Engine {
    /// `list_specimen_types`.
    pub async fn list_specimen_types(&self) -> Result<Vec<SpecimenTypeView>, CommandError> {
        self.run(&LIST_SPECIMEN_TYPES, |call| async move {
            self.read(&call, lis_catalog::list_specimen_types).await
        })
        .await
    }

    /// `create_specimen_type`.
    pub async fn create_specimen_type(
        &self,
        input: CodedName,
    ) -> Result<SpecimenTypeId, CommandError> {
        self.run(&CREATE_SPECIMEN_TYPE, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::create_specimen_type(ctx, input)
            })
            .await
        })
        .await
    }

    /// `list_analytes`.
    pub async fn list_analytes(&self) -> Result<Vec<AnalyteView>, CommandError> {
        self.run(&LIST_ANALYTES, |call| async move {
            self.read(&call, lis_catalog::list_analytes).await
        })
        .await
    }

    /// `create_analyte`.
    pub async fn create_analyte(&self, input: CreateAnalyte) -> Result<AnalyteId, CommandError> {
        self.run(&CREATE_ANALYTE, |call| async move {
            self.write(&call, move |ctx| lis_catalog::create_analyte(ctx, input))
                .await
        })
        .await
    }

    /// `list_methods`.
    pub async fn list_methods(&self) -> Result<Vec<MethodView>, CommandError> {
        self.run(&LIST_METHODS, |call| async move {
            self.read(&call, lis_catalog::list_methods).await
        })
        .await
    }

    /// `create_method`.
    pub async fn create_method(&self, input: CodedName) -> Result<MethodId, CommandError> {
        self.run(&CREATE_METHOD, |call| async move {
            self.write(&call, move |ctx| lis_catalog::create_method(ctx, input))
                .await
        })
        .await
    }

    /// `list_orderables`.
    pub async fn list_orderables(&self) -> Result<Vec<OrderableView>, CommandError> {
        self.run(&LIST_ORDERABLES, |call| async move {
            self.read(&call, lis_catalog::list_orderables).await
        })
        .await
    }

    /// `create_orderable`.
    pub async fn create_orderable(
        &self,
        input: CreateOrderable,
    ) -> Result<OrderableId, CommandError> {
        self.run(&CREATE_ORDERABLE, |call| async move {
            self.write(&call, move |ctx| lis_catalog::create_orderable(ctx, input))
                .await
        })
        .await
    }

    /// `list_active_orderables`: what order entry may offer now.
    pub async fn list_active_orderables(&self) -> Result<Vec<OrderableVersionView>, CommandError> {
        self.run(&LIST_ACTIVE_ORDERABLES, |call| async move {
            self.read(&call, |conn| -> Result<_, CatalogError> {
                let mut versions = lis_catalog::list_active_orderables(conn)?;
                versions::resolve_lifecycles_names(
                    conn,
                    versions.iter_mut().map(|version| &mut version.lifecycle),
                )?;
                Ok(versions)
            })
            .await
        })
        .await
    }
}

/// Validity policy versions (REQ-SPC-008).
impl Engine {
    /// `list_validity_policy_versions`: every policy version of a specimen type.
    pub async fn list_validity_policy_versions(
        &self,
        input: ListValidityPolicyVersionsInput,
    ) -> Result<Vec<VersionSummary<ValidityPolicyVersion>>, CommandError> {
        self.run(&LIST_VALIDITY_POLICY_VERSIONS, |call| async move {
            self.read(&call, move |conn| -> Result<_, CatalogError> {
                let mut versions =
                    lis_catalog::list_validity_policy_versions(conn, input.specimen_type_id)?;
                versions::resolve_version_names(conn, &mut versions)?;
                Ok(versions)
            })
            .await
        })
        .await
    }

    /// `get_validity_policy`: a policy version with its categories.
    pub async fn get_validity_policy(
        &self,
        input: GetValidityPolicyInput,
    ) -> Result<ValidityPolicyView, CommandError> {
        self.run(&GET_VALIDITY_POLICY, |call| async move {
            self.read(&call, move |conn| -> Result<_, CatalogError> {
                let mut version = lis_catalog::validity_policy(conn, input.policy_version_id)?;
                versions::resolve_lifecycle_names(conn, &mut version.lifecycle)?;
                Ok(version)
            })
            .await
        })
        .await
    }

    /// `draft_validity_policy_version`.
    pub async fn draft_validity_policy_version(
        &self,
        input: DraftValidityPolicyVersion,
    ) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
        self.run(&DRAFT_VALIDITY_POLICY_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::draft_validity_policy_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `update_validity_policy_draft`.
    pub async fn update_validity_policy_draft(
        &self,
        input: UpdateValidityPolicyDraft,
    ) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
        self.run(&UPDATE_VALIDITY_POLICY_DRAFT, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::update_validity_policy_draft(ctx, input)
            })
            .await
        })
        .await
    }

    /// `approve_validity_policy_version`.
    pub async fn approve_validity_policy_version(
        &self,
        input: ApproveValidityPolicyVersion,
    ) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
        self.run(&APPROVE_VALIDITY_POLICY_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::approve_validity_policy_version(ctx, input)
            })
            .await
        })
        .await
    }

    /// `retire_validity_policy_version`.
    pub async fn retire_validity_policy_version(
        &self,
        input: RetireValidityPolicyVersion,
    ) -> Result<VersionRef<ValidityPolicyVersion>, CommandError> {
        self.run(&RETIRE_VALIDITY_POLICY_VERSION, |call| async move {
            self.write(&call, move |ctx| {
                lis_catalog::retire_validity_policy_version(ctx, input)
            })
            .await
        })
        .await
    }
}
