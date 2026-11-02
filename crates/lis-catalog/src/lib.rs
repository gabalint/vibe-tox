//! Catalog and clinical configuration (REQ-ANA-002, REQ-ANA-009, REQ-ORD-002,
//! NFR-10, REQ-OPS-001; INV-07). Table prefix `cat_`.
//!
//! - **Reference data**: specimen types, analytes (local code, name, LOINC
//!   and drug class codes as provided), methods and orderables (tests and
//!   panels). Rows are immutable once created (triggers), except analytes:
//!   [`update_analyte`] corrects or retires one as a new revision with a
//!   reason and an immutable history. Its code is permanent once anything
//!   refers to the analyte; a retired analyte is refused for new method
//!   versions and source test-code maps.
//! - **Method versions** hold the specimen type, method certainty
//!   (presumptive or definitive), output scale (qualitative or quantitative),
//!   the threshold rule (positive at ≥ or >) and whether the comparison uses
//!   the value before or after rounding, plus per analyte: UCUM unit text,
//!   cutoff, limits of detection and quantitation, reportable range and
//!   display scale. Every limit is optional and a lexical decimal
//!   ([`lis_core::DecimalText`]), never REAL.
//! - **Orderable versions** hold the component manifest: which method version
//!   each component uses, which of its analytes it expects, and its
//!   [`ComponentPurpose`] (analytical or validity, REQ-SPC-004), which orders
//!   copy when they freeze the component.
//! - **Validity policy versions** (REQ-SPC-008) belong to a specimen type and
//!   hold lab-approved categories, each mapped to one [`ValidityAction`]; a
//!   qualifier action carries the report qualifier text. The product ships no
//!   categories. [`approved_validity_policy`] returns the current one by
//!   specimen type code, or none.
//!
//! **Lifecycle** (all three version kinds): a *draft* is editable by its author
//! (`manage_configuration`), one draft per parent at a time. *Approval*
//! (`approve_configuration`) needs a different user from the author unless the
//! laboratory's separation-of-duties policy lets the author approve and they
//! confirm it (`selfApproval`; ADR 0015, audited as a self-approval), makes the
//! content immutable, starts its effective period and ends the previous
//! approved version's period, recording which version replaced which. Method
//! supersession is blocked while a current orderable uses the prior version.
//! An orderable draft may use a current approved or a draft method version;
//! it is approved only when every component uses a current approved method
//! version (REQ-ANA-009, minimal form). *Retirement*
//! (`approve_configuration`, with a reason) blocks new use and changes no
//! history. Database triggers require initial draft state, legal lifecycle
//! transitions and the command context's attributable audit event; they also
//! enforce immutable approved content and refuse approver = author unless the
//! policy in force (`auth_v_separation_policy`) allows it.
//!
//! **Standard Catalog** (REQ-OPS-014, proposed): the app ships a versioned
//! file ([`StandardFile`]) of published reference content and published
//! cutoffs, each with its source. Other limits stay blank, and the threshold
//! rule and rounding are starter values. [`load_standard_catalog`] loads the
//! chosen items and what they need **only as drafts**, through the ordinary
//! commands, and records the load. **Catalog review** (REQ-OPS-015,
//! proposed): [`catalog_preview_snapshot`] gathers every value with its
//! source for the printed report, [`record_catalog_preview`] records which
//! drafts it lists, and [`approve_reviewed_catalog`] approves exactly those
//! drafts, by a user who authored none of them or, as the policy allows, who
//! confirms the self-approval.
//!
//! Every command checks its capability first and appends a clinical-stream
//! audit event in its transaction, with old and new revisions and the reason
//! where one is required. Queries perform no authorization; the engine checks
//! capabilities before calling them. Catalog content holds no PHI. IPC types
//! derive `ts_rs::TS`; names other areas could reuse carry a `Catalog` prefix
//! in TypeScript.

#![warn(clippy::wildcard_enum_match_arm)]

mod approval_references;
mod commands;
mod content;
mod error;
mod lifecycle;
mod model;
mod preview;
mod previews;
pub mod queries;
mod repo;
mod standard;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use commands::analytes::{
    AnalyteHistoryEntry, AnalyteInput, AnalyteMethodUse, AnalyteUsage, UpdateAnalyte,
    analyte_history, analyte_usage, update_analyte,
};
pub use commands::methods::{
    ApproveMethodVersion, DraftMethodVersion, RetireMethodVersion, UpdateMethodDraft,
    approve_method_version, draft_method_version, retire_method_version, update_method_draft,
};
pub use commands::orderables::{
    ApproveOrderableVersion, DraftOrderableVersion, RetireOrderableVersion, UpdateOrderableDraft,
    approve_orderable_version, draft_orderable_version, retire_orderable_version,
    update_orderable_draft,
};
pub use commands::reference::{
    CodedName, CreateAnalyte, CreateOrderable, create_analyte, create_method, create_orderable,
    create_specimen_type,
};
pub use commands::review::{
    ApproveReviewedCatalog, CatalogBlankCutoff, CatalogReviewApproved, RecordCatalogPreview,
    approve_reviewed_catalog, catalog_preview_capability, record_catalog_preview,
};
pub use commands::standard::{
    LoadStandardCatalog, StandardCatalogLoaded, StandardOmittedAnalyte, load_standard_catalog,
};
pub use commands::validity::{
    ApproveValidityPolicyVersion, DraftValidityPolicyVersion, RetireValidityPolicyVersion,
    UpdateValidityPolicyDraft, approve_validity_policy_version, draft_validity_policy_version,
    retire_validity_policy_version, update_validity_policy_draft,
};
pub use content::{
    ComponentInput, MethodAnalyte, MethodVersionContent, OrderableVersionContent, ValidityCategory,
    ValidityPolicyContent,
};
pub use error::CatalogError;
pub use lifecycle::{VersionLifecycle, VersionRef, VersionSummary};
pub use model::{
    Analyte, AnalyteId, CatalogDraft, CatalogPreview, CatalogPreviewId, CatalogReview,
    CatalogReviewId, ComponentPurpose, Method, MethodCertainty, MethodId, MethodVersion,
    MethodVersionId, Orderable, OrderableId, OrderableKind, OrderableVersion, OrderableVersionId,
    OutputScale, RangePolicy, SpecimenType, SpecimenTypeId, StandardCatalogLoad,
    StandardCatalogLoadId, ThresholdRounding, ThresholdRule, ValidityAction, ValidityPolicyVersion,
    ValidityPolicyVersionId, VersionStatus,
};
pub use preview::{
    CatalogGap, CatalogGapField, CatalogPreviewAnalyte, CatalogPreviewMethod,
    CatalogPreviewMethodAnalyte, CatalogPreviewMethodVersion, CatalogPreviewOrderable,
    CatalogPreviewSnapshot, CatalogValueSource, CatalogValueSourceKind, DemoValues,
    catalog_preview_snapshot,
};
pub use previews::{
    ARTIFACT_COLUMNS, CatalogPreviewBlankCutoff, CatalogPreviewSummary, catalog_preview_artifact,
    list_catalog_previews,
};
pub use queries::{
    approved_validity_policy, audit_record_label, get_method_version, get_orderable_version,
    list_active_orderables, list_analytes, list_method_versions, list_methods,
    list_orderable_versions, list_orderables, list_specimen_types, list_validity_policy_versions,
    validity_policy,
};
pub use standard::state::{StandardItemState, StandardItemStatus};
pub use standard::tree::{
    StandardAnalyteItem, StandardCatalogLoadView, StandardCatalogStatus, StandardCatalogTree,
    StandardComponentItem, StandardMethodAnalyteItem, StandardMethodItem, StandardOrderableItem,
    StandardSpecimenTypeItem, standard_catalog_status, standard_catalog_tree,
};
pub use standard::{StandardCatalogInfo, StandardFile, StandardReviewStatus};
pub use views::{
    AnalyteView, ComponentAnalyteView, ComponentView, MethodAnalyteView, MethodVersionView,
    MethodView, OrderableVersionView, OrderableView, SpecimenTypeView, ValidityPolicyView,
};

/// This module's migrations, in version order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "catalog",
        sql: include_str!("../migrations/0001_catalog.sql"),
    },
    Migration {
        version: 2,
        name: "purpose_and_validity",
        sql: include_str!("../migrations/0002_purpose_and_validity.sql"),
    },
    Migration {
        version: 3,
        name: "standard_catalog_loads",
        sql: include_str!("../migrations/0003_standard_catalog_loads.sql"),
    },
    Migration {
        version: 4,
        name: "catalog_reviews",
        sql: include_str!("../migrations/0004_catalog_reviews.sql"),
    },
    Migration {
        version: 5,
        name: "self_approval",
        sql: include_str!("../migrations/0005_self_approval.sql"),
    },
    Migration {
        version: 6,
        name: "report_content",
        sql: include_str!("../migrations/0006_report_content.sql"),
    },
    Migration {
        version: 7,
        name: "external_approval_references",
        sql: include_str!("../migrations/0007_external_approval_references.sql"),
    },
    Migration {
        version: 8,
        name: "analyte_corrections",
        sql: include_str!("../migrations/0008_analyte_corrections.sql"),
    },
];

/// This module's schema descriptor.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "catalog",
    table_prefix: "cat_",
    migrations: MIGRATIONS,
};
