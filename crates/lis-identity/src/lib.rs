//! Identity and organizations (REQ-IDN-001, REQ-OPS-001, NFR-10; INV-02).
//! Table prefix `idn_`.
//!
//! - **Subjects** carry demographic assertions as provided (family name,
//!   given names, birth date, administrative sex, source, time and actor). A
//!   change adds a new assertion with a reason; earlier assertions are never
//!   changed (append-only triggers). REQ-IDN-002 builds corrections and merges
//!   on this later.
//! - **Identifiers** are scoped by assigning authority and identifier type.
//!   The same value from two authorities never links subjects. Comparison is
//!   byte-exact after trimming because authority-specific normalization is not
//!   yet defined. Deactivation keeps the row, and historical ownership blocks
//!   reassignment to another subject; deliberate transfer is not implemented.
//! - **Matching** is deterministic ([`find_subject_matches`]): an active
//!   identifier within the same authority and type is the same subject;
//!   otherwise subjects with the same NFC-normalized, lowercased family name,
//!   given names and birth date are listed as candidates, never linked.
//!   Registration and demographic correction need acknowledgement of exactly
//!   the current candidate set and open a possible-duplicate **identity
//!   hold**. M1 resolves holds as "confirmed distinct" with a reason, only
//!   while every participant remains at the revision reviewed.
//! - **Search** ([`search_subjects`]) browses patients with bounded pagination,
//!   or finds literal name/active identifier prefixes. Name matching ignores
//!   case and Unicode composition. Structured searches find the active owner
//!   of an identifier
//!   and subjects with any assertion whose normalized family name starts with
//!   the entered text (the whole family name plus a given-names prefix when
//!   given names are entered), optionally with the exact birth date. Results
//!   are [`SubjectSummary`] values: current demographics, active identifiers
//!   and the open hold count. Other modules show subjects through
//!   [`subject_summary`].
//! - **Client organizations**, their **locations** and **practitioners** with
//!   an NPI (assigning authority [`NPI_AUTHORITY`], check digit validated).
//! - **Laboratory profile** (M3; REQ-RPT-001, REQ-RPT-008, REQ-BIL-005): the
//!   performing laboratory's name, address, CLIA number, director, phone,
//!   IANA time zone (validated with `jiff`, stored canonical) and optional
//!   billing NPI and tax ID. [`set_laboratory_profile`] appends a version
//!   against the expected current version; every version after the first
//!   needs a reason, and versions are append-only and consecutive, so a
//!   report can name the exact version it froze.
//! - **History for impact review**: [`identity_holds_touching`] returns open
//!   and resolved holds with their times, so a hold opened after a report's
//!   release still counts once resolved.
//!
//! Capabilities: initial identity capture and identifier addition
//! `enter_orders`; demographic correction, identifier deactivation and hold
//! resolution `manage_quality` by a signed-in user; clients, locations and
//! practitioners `manage_configuration`; the laboratory profile
//! `manage_configuration` by a signed-in user. Every command checks its
//! capability first and appends a clinical-stream audit event in its
//! transaction. Queries perform no authorization; the engine checks and
//! audits reads (see [`queries`]).
//!
//! PHI (names, birth dates, sex, identifier values) is held in
//! [`lis_core::Phi`] in inputs and views and never reaches audit details,
//! error text or logs. IPC types derive `ts_rs::TS`; names other areas could
//! reuse carry an `Identity` or `Subject` prefix in TypeScript.

#![warn(clippy::wildcard_enum_match_arm)]

mod billing;
mod billing_model;
mod billing_validation;
mod commands;
mod directories;
mod error;
mod intake;
mod model;
pub mod queries;
mod repo;
mod search;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use billing::{get_patient_billing_profile, patient_billing_view, set_patient_billing_profile};
pub use billing_model::{
    BillingContact, BillingNoticeEvidence, BillingParty, BillingRelationship, FinancialEvidence,
    FinancialEvidenceKind, FinancialEvidenceStatus, GetPatientBillingProfileInput,
    PatientBillingContent, PatientBillingProfile, PatientBillingProfileView, PatientCoverage,
    SetPatientBillingProfileInput,
};
pub use billing_validation::{require_financial_access, validate_financial_evidence};
pub use commands::holds::{
    ConfirmDistinctSubjects, IdentityHoldResolved, confirm_distinct_subjects,
};
pub use commands::laboratory::{SetLaboratoryProfile, set_laboratory_profile};
pub use commands::organizations::{
    AddClientLocation, CreateClient, RegisterPractitioner, add_client_location, create_client,
    register_practitioner,
};
pub use commands::subjects::{
    AddSubjectIdentifier, AssertDemographics, DeactivateSubjectIdentifier, IdentifierAdded,
    RegisterSubject, SubjectRegistered, SubjectRevision, add_subject_identifier,
    assert_demographics, deactivate_subject_identifier, register_subject,
};
pub use error::IdentityError;
pub use model::{
    Client, ClientId, ClientLocation, ClientLocationId, DemographicsInput, IdentifierInput,
    IdentityAssertion, IdentityAssertionId, IdentityHold, IdentityHoldId, LaboratoryProfile,
    LaboratoryProfileId, NPI_AUTHORITY, Npi, Practitioner, PractitionerId, Subject, SubjectId,
    SubjectIdentifier, SubjectIdentifierId,
};
pub use queries::{
    SubjectMatchQuery, audit_record_label, current_laboratory_profile, find_subject_matches,
    get_client, get_laboratory_profile_by_id, get_practitioner, get_subject,
    identity_holds_touching, list_all_open_identity_holds, list_clients, list_laboratory_profiles,
    list_open_identity_holds, list_practitioners, match_subjects, subject_summaries,
    subject_summary,
};
pub use search::{SubjectSearch, search_subjects};
pub use views::{
    AssertionView, ClientLocationView, ClientView, HoldKind, IdentifierView, IdentityHoldRecord,
    IdentityHoldView, LaboratoryProfileView, PractitionerView, SubjectCandidates, SubjectMatches,
    SubjectSummary, SubjectView,
};

/// This module's migrations, in version order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "identity",
        sql: include_str!("../migrations/0001_identity.sql"),
    },
    Migration {
        version: 2,
        name: "laboratory_profile",
        sql: include_str!("../migrations/0002_laboratory_profile.sql"),
    },
    Migration {
        version: 3,
        name: "patient_billing",
        sql: include_str!("../migrations/0003_patient_billing.sql"),
    },
    Migration {
        version: 4,
        name: "directory_history",
        sql: include_str!("../migrations/0004_directory_history.sql"),
    },
    Migration {
        version: 5,
        name: "intake_deferrals",
        sql: include_str!("../migrations/0005_intake_deferrals.sql"),
    },
];

/// This module's schema descriptor.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "identity",
    table_prefix: "idn_",
    migrations: MIGRATIONS,
};

pub use intake::{
    BirthEvidenceKind, IntakeDeferralId, IntakeDeferralView, IntakeDisposition,
    ListIntakeDeferralsInput, RecordIntakeDeferralInput, ResolveIntakeDeferralInput,
    list_intake_deferrals, record_intake_deferral, resolve_intake_deferral,
};

pub use directories::{
    DirectoryHistoryEntry, DirectoryHistoryInput, DirectoryRecord, DirectorySnapshot, UpdateClient,
    UpdateClientLocation, UpdatePractitioner, assert_active_ordering_source, directory_history,
    list_active_clients, list_active_practitioners, update_client, update_client_location,
    update_practitioner,
};
