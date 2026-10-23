//! The draft → approved → retired lifecycle shared by method, orderable and
//! validity policy versions (REQ-ANA-002, REQ-SPC-008). Pure rules here; SQL
//! in `repo::lifecycle`.

use lis_core::{Id, UserId, UtcTimestamp};
use serde::Serialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::model::{
    Method, MethodVersion, Orderable, OrderableVersion, SpecimenType, ValidityPolicyVersion,
    VersionStatus,
};

/// A versioned kind: its table, its parent and its audit vocabulary.
pub(crate) trait Versioned {
    /// The record the versions belong to.
    type Parent;
    const TABLE: &'static str;
    const PARENT_TABLE: &'static str;
    const PARENT_COLUMN: &'static str;
    /// `NotFound` name of the parent.
    const PARENT_RECORD_TYPE: &'static str;
    /// Audit record type and `NotFound` name.
    const RECORD_TYPE: &'static str;
    const DRAFT_ACTION: &'static str;
    const UPDATE_ACTION: &'static str;
    const APPROVE_ACTION: &'static str;
    const SUPERSEDE_ACTION: &'static str;
    const RETIRE_ACTION: &'static str;
}

impl Versioned for MethodVersion {
    type Parent = Method;
    const TABLE: &'static str = "cat_method_versions";
    const PARENT_TABLE: &'static str = "cat_methods";
    const PARENT_COLUMN: &'static str = "method_id";
    const PARENT_RECORD_TYPE: &'static str = "method";
    const RECORD_TYPE: &'static str = "method_version";
    const DRAFT_ACTION: &'static str = "catalog.draft_method_version";
    const UPDATE_ACTION: &'static str = "catalog.update_method_draft";
    const APPROVE_ACTION: &'static str = "catalog.approve_method_version";
    const SUPERSEDE_ACTION: &'static str = "catalog.supersede_method_version";
    const RETIRE_ACTION: &'static str = "catalog.retire_method_version";
}

impl Versioned for OrderableVersion {
    type Parent = Orderable;
    const TABLE: &'static str = "cat_orderable_versions";
    const PARENT_TABLE: &'static str = "cat_orderables";
    const PARENT_COLUMN: &'static str = "orderable_id";
    const PARENT_RECORD_TYPE: &'static str = "orderable";
    const RECORD_TYPE: &'static str = "orderable_version";
    const DRAFT_ACTION: &'static str = "catalog.draft_orderable_version";
    const UPDATE_ACTION: &'static str = "catalog.update_orderable_draft";
    const APPROVE_ACTION: &'static str = "catalog.approve_orderable_version";
    const SUPERSEDE_ACTION: &'static str = "catalog.supersede_orderable_version";
    const RETIRE_ACTION: &'static str = "catalog.retire_orderable_version";
}

impl Versioned for ValidityPolicyVersion {
    type Parent = SpecimenType;
    const TABLE: &'static str = "cat_validity_policy_versions";
    const PARENT_TABLE: &'static str = "cat_specimen_types";
    const PARENT_COLUMN: &'static str = "specimen_type_id";
    const PARENT_RECORD_TYPE: &'static str = "specimen_type";
    const RECORD_TYPE: &'static str = "validity_policy_version";
    const DRAFT_ACTION: &'static str = "catalog.draft_validity_policy_version";
    const UPDATE_ACTION: &'static str = "catalog.update_validity_policy_draft";
    const APPROVE_ACTION: &'static str = "catalog.approve_validity_policy_version";
    const SUPERSEDE_ACTION: &'static str = "catalog.supersede_validity_policy_version";
    const RETIRE_ACTION: &'static str = "catalog.retire_validity_policy_version";
}

/// Who created, approved, replaced and retired a version, and when.
///
/// The effective period starts at approval and ends when a newer version is
/// approved (`replaced_by`) or the version is retired.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", bound = "")]
#[ts(rename = "CatalogVersionLifecycle", concrete(K = MethodVersion), bound = "K: 'static")]
pub struct VersionLifecycle<K> {
    /// Draft, approved or retired.
    pub status: VersionStatus,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
    /// The author.
    pub created_by: UserId,
    /// Current account display name, resolved by the engine at read time.
    #[ts(optional)]
    pub created_by_display: Option<String>,
    /// When the draft was created.
    pub created_at: UtcTimestamp,
    /// The approver; the author only as the separation-of-duties policy allowed
    /// (ADR 0015).
    pub approved_by: Option<UserId>,
    /// Current approver display name, resolved by the engine at read time.
    #[ts(optional)]
    pub approved_by_display: Option<String>,
    /// Start of the effective period (the approval time).
    pub effective_from: Option<UtcTimestamp>,
    /// End of the effective period.
    pub effective_to: Option<UtcTimestamp>,
    /// The version whose approval ended this one's effective period.
    pub replaced_by: Option<Id<K>>,
    /// Who retired it.
    pub retired_by: Option<UserId>,
    /// Current retiring user's display name, resolved by the engine at read time.
    #[ts(optional)]
    pub retired_by_display: Option<String>,
    /// When it was retired.
    pub retired_at: Option<UtcTimestamp>,
    /// Why it was retired.
    pub retire_reason: Option<String>,
}

impl<K> VersionLifecycle<K> {
    /// Approved and its effective period still open: usable for new work.
    #[must_use]
    pub fn is_current(&self) -> bool {
        self.status == VersionStatus::Approved && self.effective_to.is_none()
    }

    pub(crate) fn ensure_draft(&self) -> Result<(), CatalogError> {
        match self.status {
            VersionStatus::Draft => Ok(()),
            VersionStatus::Approved | VersionStatus::Retired => Err(CatalogError::NotDraft),
        }
    }

    pub(crate) fn ensure_approved(&self) -> Result<(), CatalogError> {
        match self.status {
            VersionStatus::Approved => Ok(()),
            VersionStatus::Draft => Err(CatalogError::NotApproved),
            VersionStatus::Retired => Err(CatalogError::AlreadyRetired),
        }
    }

    pub(crate) fn ensure_revision(&self, expected: i64) -> Result<(), CatalogError> {
        if self.revision == expected {
            Ok(())
        } else {
            Err(CatalogError::RevisionConflict)
        }
    }
}

/// A version's identity and lifecycle, for version history.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", bound = "")]
#[ts(export, rename = "CatalogVersionSummary", concrete(K = MethodVersion), bound = "K: 'static")]
pub struct VersionSummary<K> {
    /// The version.
    pub id: Id<K>,
    /// Version number within its parent, from 1.
    #[ts(type = "number")]
    pub version: i64,
    /// Its lifecycle.
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle<K>,
}

/// A version after a command changed it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase", bound = "")]
#[ts(export, rename = "CatalogVersionRef", concrete(K = MethodVersion), bound = "K: 'static")]
pub struct VersionRef<K> {
    /// The version.
    pub id: Id<K>,
    /// Version number within its parent.
    #[ts(type = "number")]
    pub version: i64,
    /// Its new revision.
    #[ts(type = "number")]
    pub revision: i64,
}
