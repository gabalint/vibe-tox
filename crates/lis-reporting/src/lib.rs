//! Reporting module (`rpt_`; REQ-RPT-001/002/003/005/011/012/013 local,
//! NFR-01, NFR-07, REQ-IDN-004 fresh authentication). Design:
//! docs/planning/m3-design-brief.md §2.1, §3, §4.
//!
//! - **One report per specimen** (L1), numbered accession number `-`
//!   specimen type code (L2). Its expected components come from
//!   [`manifest`], the one home of that rule, published to billing.
//! - **Candidates.** [`prepare_candidate`] (T1) rechecks authority and the
//!   report-level gates, freezes a [`snapshot::ReportSnapshot`] (canonical
//!   JSON and SHA-256) and the exact template input, and supersedes the open
//!   candidate. The engine renders and stores the PDF outside any
//!   transaction; [`record_rendered`] or [`record_render_failure`] (T2)
//!   closes the preparation.
//! - **Release** ([`release_candidate`]) runs every check of the brief's
//!   §3.4 inside its own transaction through the owners' functions: current
//!   grants, fresh authentication bound to the candidate (password per
//!   release, no grace period), candidate state, revision and expiry, the
//!   verified artifact digest, the rebuilt content digest, identity,
//!   specimen, order, validity and review gates, the two-person rule (a
//!   reviewer releases only as the laboratory's separation-of-duties policy
//!   allows, with their confirmation; ADR 0015) and partial acknowledgement.
//!   The `report.release` audit event must exist before the revision row
//!   (trigger), and obligations commit with it.
//! - **Corrections are new revisions** ([`relationship`]): supplement or
//!   correction (with a reason); identical content is refused. Released
//!   rows, content and bytes never change; bytes are never regenerated.
//! - **Withdrawal** commits [`preflight_withdrawal`] and fresh authentication
//!   before rendering; [`stage_withdrawal_notice`] durably reconciles the
//!   stored notice before [`withdraw_report`] makes the report terminal. It is
//!   never gated by holds and notifies every past destination.
//! - **Impact** after release is computed, keyed and assessed by quality;
//!   open impact pauses delivery (L13).
//! - **Delivery** is staff-operated: destinations verified by a second
//!   person, or by their creator where the separation-of-duties policy
//!   allows it, one obligation per destination and revision, attested
//!   attempts, and one label function ([`delivery::labels`]) that always
//!   names channel and evidence and never says "Delivered". An attempt that
//!   finds the address wrong blocks the destination for every obligation
//!   until it is replaced or retired and the obligation redirected.
//! - **Stored content** (a candidate's canonical content and printed data)
//!   is verified against its digests before every parse; a mismatch is a
//!   preservation incident and is never shown (NFR-07). Release also
//!   requires each reported line's frozen review attribution to be the
//!   current one, so the two-person rule judges the reviews it names.
//! - **Preservation incidents** are recorded once while open (a loss
//!   accepted is not recorded again) and closed by quality staff with a
//!   resolution and a reason ([`close_preservation_incident`], REQ-OPS-004);
//!   closures are append-only. A revision whose stored content fails its
//!   digest is left out of the impact and delivery queues, which keep
//!   working; the incidents queue lists it, and the report stays readable
//!   without its impact.
//! - **Validity lines** need no limit: they print the measured value, the
//!   method limits only when defined, and the current validity assessment.
//!
//! Printed wording lives in [`display`], the fixed policy in [`policy`].

#![warn(clippy::wildcard_enum_match_arm)]

pub mod actions;
mod artifact_references;
mod authority;
mod commands;
mod condition_review;
pub mod delivery;
pub mod display;
pub mod dispositions;
mod error;
pub mod gates;
mod impact;
pub mod manifest;
mod model;
pub mod policy;
mod queries;
pub mod relationship;
pub mod render;
mod repo;
pub mod report_data;
pub mod snapshot;
mod stored;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use artifact_references::{ARTIFACT_COLUMNS, referenced_artifacts};
pub use commands::assess::{RecordImpactAssessmentInput, record_impact_assessment};
pub use commands::destinations::{
    CreateDeliveryDestinationInput, ReplaceDeliveryDestinationInput,
    RetireDeliveryDestinationInput, VerifyDeliveryDestinationInput, create_delivery_destination,
    replace_delivery_destination, retire_delivery_destination, verify_delivery_destination,
};
pub use commands::exports::{ExportReportPdfInput, ExportSource, export_source, record_export};
pub use commands::incidents::{
    ClosePreservationIncidentInput, close_preservation_incident, record_content_incident,
    record_preservation_incident,
};
pub use commands::obligations::{
    AddDeliveryObligationInput, RecordDeliveryAttemptInput, RedirectDeliveryObligationInput,
    add_delivery_obligation, record_delivery_attempt, redirect_delivery_obligation,
};
pub use commands::prepare::{
    PrepareReportCandidateInput, PreparedCandidate, RenderFailure, RenderedArtifact,
    prepare_candidate, record_render_failure, record_rendered,
};
pub use commands::release::{ReleaseReportInput, release_candidate};
pub use commands::withdraw::{
    NoticeArtifact, NoticeDraft, PreparedWithdrawal, WithdrawReportInput, preflight_withdrawal,
    stage_withdrawal_notice, withdraw_report,
};
pub use condition_review::{ConditionReportDecision, ConditionReportReview, ConditionReviewSource};
pub use error::ReportingError;
pub use lis_report::RenderedPdf;
pub use manifest::{ExpectedComponent, ExpectedTrigger, Manifest, manifest};
pub use model::*;
pub use queries::billing::{CurrentFinalRelease, ReportedLine, current_final_release};
pub use queries::candidate::{candidate_artifact, get_candidate};
pub use queries::delivery::{
    DeliveryQueueFilter, DeliveryQueuePage, ListDeliveryObligationsInput, delivery_queue,
    delivery_queue_page,
};
pub use queries::destinations::{
    DestinationQueueItem, DestinationState, ListDeliveryDestinationsInput,
    destinations_needing_attention, list_destinations,
};
pub use queries::incidents::{
    IncidentQueueItem, IncidentQueueSubject, preservation_incidents, unrecorded_content_incident,
};
pub use queries::obligation::get_delivery_obligation;
pub use queries::queues::{
    ReleaseQueue, ReleaseQueueItem, ReleaseReconciliation, RevisionQueueItem, impact_queue,
    partial_reports, release_queue, release_reconciliation, revisions_without_destination,
};
pub use queries::report::{
    ListReportsInput, ReportListFilter, audit_record_label, get_report, get_revision, list_reports,
};
pub use queries::workspace::report_workspace;
pub use repo::quality::IncidentSubject;
pub use stored::ContentIncident;
pub use views::*;

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "reporting",
        sql: include_str!("../migrations/0001_reporting.sql"),
    },
    Migration {
        version: 2,
        name: "incident_records",
        sql: include_str!("../migrations/0002_incident_records.sql"),
    },
    Migration {
        version: 3,
        name: "incident_closures",
        sql: include_str!("../migrations/0003_incident_closures.sql"),
    },
    Migration {
        version: 4,
        name: "delivery_indexes_and_withdrawal_staging",
        sql: include_str!("../migrations/0004_delivery_indexes_and_withdrawal_staging.sql"),
    },
    Migration {
        version: 5,
        name: "self_verification",
        sql: include_str!("../migrations/0005_self_verification.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "reporting",
    table_prefix: "rpt_",
    migrations: MIGRATIONS,
};

#[cfg(test)]
mod tests {
    #[test]
    fn schema_passes_lint() {
        assert_eq!(
            lis_store::lint_schema(&[
                lis_auth::SCHEMA,
                lis_identity::SCHEMA,
                lis_catalog::SCHEMA,
                lis_interfaces::SCHEMA,
                lis_orders::SCHEMA,
                lis_specimens::SCHEMA,
                lis_analytical::SCHEMA,
                lis_results::SCHEMA,
                super::SCHEMA,
            ])
            .unwrap(),
            Vec::new()
        );
    }
}
