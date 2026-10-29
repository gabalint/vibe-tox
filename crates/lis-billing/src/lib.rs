//! Billing handoff module (`bil_`; REQ-BIL-001/002/005, VAL-23). Design:
//! docs/planning/m3-design-brief.md §2.2, §5, §7.
//!
//! - **Mapping.** Versioned analyte-to-class mappings, approved by a billing
//!   user other than the author (or by the author, confirmed, as the
//!   laboratory's separation-of-duties policy allows; ADR 0015) and confirmed
//!   under `approve_configuration` (`mapping`). No mapping, payer rule or
//!   procedure code ships.
//! - **Derivation.** [`derive`] is pure and deterministic: classes tested
//!   per specimen and certainty, never positives, repeats or panel names.
//!   Inputs are gathered inside the transaction through owner functions
//!   only, and the date of service is the collection's local date
//!   (`policy`). The same inputs digest returns the same derivation; a
//!   new digest gives a new number, supersedes what was not handed off, and
//!   adjusts the newest handed-off derivation. Facts are sealed during
//!   construction and never edited or extended afterward.
//! - **Handoff.** A `bil-handoff-v2` file rebuilt and digest-checked in the
//!   recording transaction, with each derivation approved, current and not
//!   handed off; outcomes per item are append-only, including attributed
//!   follow-ups to rejected items. Stored v1 artifacts remain exportable.
//! - **No clinical effect.** Billing writes only `bil_` tables, so no billing
//!   flag can hold testing, review or release.

#![warn(clippy::wildcard_enum_match_arm)]

mod artifact_references;
mod derivation;
mod derivation_digest;
mod derivation_repo;
mod derivation_view;
mod derivations;
mod error;
mod financial_context;
mod handoff_document;
mod handoff_list;
mod handoff_view;
mod handoffs;
mod inputs;
mod mapping;
mod mapping_support;
mod model;
mod policy;
mod reconciliation;
mod work;

use lis_core::{Migration, ModuleSchema};

pub use artifact_references::{ARTIFACT_COLUMNS, referenced_artifacts};
pub use derivation::{
    AnalyteInput, ComponentInput, DerivationOutcome, ExecutionInput, InputRef, Inputs, ReviewInput,
    SpecimenInput, derive,
};
pub use derivation_view::{
    DerivationClassView, DerivationComponentView, DerivationRef, DerivationSpecimenView,
    DerivationView, MappingVersionRef, get_service_derivation,
};
pub use derivations::{
    ApproveServiceDerivation, DeriveServices, approve_service_derivation, derive_services,
};
pub use error::BillingError;
pub use handoff_list::{BillingHandoffSummary, ListBillingHandoffs, list_billing_handoffs};
pub use handoff_view::{
    HandoffItemView, HandoffOutcomeView, HandoffView, audit_record_label, get_billing_handoff,
};
pub use handoffs::{
    CreateBillingHandoff, HandoffExportSource, HandoffPlan, RecordHandoffOutcome,
    create_billing_handoff, handoff_export_source, handoff_file, plan_handoff,
    record_handoff_export, record_handoff_outcome,
};
pub use mapping::{
    ApproveClassMappingVersion, ConfirmClassMappingVersion, DraftClassMappingVersion,
    RetireClassMappingVersion, UpdateClassMappingDraft, approve_class_mapping_version,
    confirm_class_mapping_version, current_class_mapping, draft_class_mapping_version,
    get_class_mapping_version, list_class_mapping_versions, retire_class_mapping_version,
    update_class_mapping_draft,
};
pub use model::{
    BillingClass, BillingClassMappingEntry, BillingClassMappingVersion,
    BillingClassMappingVersionId, BillingClassMappingVersionSummary,
    BillingClassMappingVersionView, BillingComponentDisposition, BillingDerivation,
    BillingDerivationId, BillingDerivationState, BillingFlag, BillingHandoffId,
    BillingHandoffItemId, BillingInputKind, BillingVersionStatus, ClassFact,
    DerivationCompleteness, DerivationFacts, DerivedComponent, DerivedSpecimen, HandoffItemKind,
    HandoffOutcome, HandoffStatus,
};
pub use reconciliation::{
    BillingContextResolution, RecordHandoffFollowupInput, ResolveBillingContextConflictInput,
    record_handoff_followup, resolve_billing_context_conflict,
};
pub use work::{
    BillingWorkCursor, BillingWorkFilter, BillingWorkItem, BillingWorkState, BillingWorkTarget,
    ListBillingWork, billing_work_queue, list_billing_work,
};

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "billing",
        sql: include_str!("../migrations/0001_billing.sql"),
    },
    Migration {
        version: 2,
        name: "handoff_number",
        sql: include_str!("../migrations/0002_handoff_number.sql"),
    },
    Migration {
        version: 3,
        name: "self_approval",
        sql: include_str!("../migrations/0003_self_approval.sql"),
    },
    Migration {
        version: 4,
        name: "billing_context",
        sql: include_str!("../migrations/0004_billing_context.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "billing",
    table_prefix: "bil_",
    migrations: MIGRATIONS,
};

#[cfg(test)]
mod tests {
    #[test]
    fn schema_passes_lint() -> Result<(), lis_store::StoreError> {
        assert_eq!(
            lis_store::lint_schema(&[lis_auth::SCHEMA, lis_catalog::SCHEMA, super::SCHEMA])?,
            Vec::new()
        );
        Ok(())
    }
}
