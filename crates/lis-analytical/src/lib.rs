//! Analytical module: instrument readiness, analyzer source registrations, lots,
//! preparation batches and steps, runs, positions and executions, QC
//! observations and assessments, and deviations (REQ-ANA-001, REQ-ANA-003
//! registration and re-export parts, REQ-ANA-004–007, REQ-OPS-001). Table
//! prefix `ana_`. Design: docs/planning/m2-design-brief.md §2.4.
//!
//! - **Readiness** (REQ-ANA-007). Instrument status is an append-only event
//!   log; no event means not ready. The latest event is chosen by a
//!   per-instrument sequence assigned in the writing transaction, never by
//!   the host clock (a trigger refuses any other number). Lots move `active` → `withdrawn`; expiry
//!   is derived from `expires_on` (usable through that UTC date).
//!   [`create_run`] rechecks, in its transaction, that the instrument is in
//!   service, the pinned method version remains approved and every lot
//!   is active and unexpired.
//! - **Source registrations** (REQ-ANA-003). `draft` → `active` → `retired`,
//!   one active per instrument, approved by a signed-in user other than the
//!   author unless the laboratory's separation-of-duties policy lets the
//!   author approve and they confirm it (ADR 0015; a trigger reads
//!   `auth_v_separation_policy`); approval retires the previous one. The
//!   test-code map, the sample ID field, the revision indicator and whether
//!   the source reports final concentrations are immutable once approved
//!   (triggers). A result-file
//!   source ([`register_file_source`], result files brief F2) is a
//!   registration with a [`FileLayout`], fixed the same way; the layout's
//!   structure has one check, [`FileLayout::validate`].
//!   AU sources carry an immutable [`AuSourceProfile`] with exact wire settings
//!   and declared numeric units. The connection must match at registration and
//!   approval; the importer compares the receipt's frozen settings again.
//! - **Measured, not ordered** (brief F17). [`attach_served_component`]
//!   attaches an accepted add-on component to the active execution that
//!   already measured it, inside the caller's audited command.
//! - **Runs and positions** (REQ-ANA-001, REQ-ANA-006). [`add_patient_position`]
//!   rechecks `lis_specimens::testing_gate` against the method's specimen
//!   type and `lis_orders::testing_gate` for each component, creates the
//!   execution and links it to the order components the method serves, and
//!   copies the container's printed label as the sample label. Reruns,
//!   re-extractions and dilutions need a related execution on the same
//!   specimen and a reason; a dilution needs a source that reports final
//!   concentration. Control, calibrator and blank positions carry no
//!   execution (a CHECK), so their data can never become a patient
//!   observation; QC positions report under the lot's sample label.
//! - **Preparation batches** (REQ-ANA-001). `open` → `completed`. Each step
//!   raises the batch revision; [`complete_prep_batch`] needs at least one
//!   step and the revision shown, and a completed batch takes no more steps
//!   (triggers). Only a completed batch can be accepted; its assessment
//!   records the revision assessed, and cites no QC observations (none
//!   belongs to a batch).
//! - **Imports.** [`position_by_sample_label`] matches only open runs of the
//!   sending instrument (none, one or ambiguous); [`patient_intake_gate`]
//!   refuses new patient evidence for a closed run or a failed execution.
//!   [`record_qc_observations`] stores QC rows only in `ana_qc_observations`
//!   (a trigger refuses patient positions) and applies [`reexport::decide`];
//!   a changed row it cannot classify becomes a QC conflict against the
//!   exact observation it contradicts, resolved by a person with
//!   [`resolve_qc_conflict`] (accept as a correction, or reject).
//! - **QC** (REQ-ANA-004, REQ-ANA-005). Assessments per run × analyte, run
//!   and preparation batch are append-only and made by a signed-in user; no
//!   row means pending. A new assessment supersedes the current one with a
//!   reason; a rejection is never superseded by an acceptance (also a
//!   trigger), and there is no waiver path. A run × analyte assessment cites
//!   exactly the current observations, and is current only while they are
//!   still exactly the current observations of that run × analyte: a new,
//!   superseded or remapped observation makes it stale, and an open QC
//!   conflict on the analyte blocks. Deviations scoped to an instrument,
//!   lot, batch, run or position block dependent results while open; each
//!   takes a database-wide sequence, and the dependency snapshot carries the
//!   highest applicable one, so a deviation opened after a review keeps the
//!   review stale after it closes. A failed execution blocks its results.
//!   [`dependency_status`] is the one gate the result review calls.
//! - **Reads for billing and work queues** (M3). [`component_executions`]
//!   lists an order component's executions and their states (billing's
//!   failed-attempt flag); [`qc_work_queue`] lists run × analyte QC that is
//!   pending, stale, or rejected on an open run, by the same status rule as
//!   the review gate; [`open_deviations`] lists open deviations
//!   (REQ-OPS-004).
//!
//! Capabilities: readiness, lots, QC assessments and deviations
//! `manage_quality`; registrations `manage_configuration`, approval
//! `approve_configuration`; batches, runs, positions and failures
//! `perform_analysis`; QC conflicts `manage_quality`; QC intake
//! `instrument_intake`. Every command checks its capability first and
//! appends a clinical-stream audit event in its transaction; user-typed
//! reasons go to the event's `reason`, never to details. Queries perform no
//! authorization; run detail includes patient context and the engine audits reads
//! that disclose sample labels (worklist, runs).

#![warn(clippy::wildcard_enum_match_arm)]

pub mod actions;
mod au_profile;
mod commands;
mod dependency;
mod deviation_targets;
mod error;
mod file_layout;
mod model;
mod queries;
pub mod reexport;
mod repo;
mod served;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use au_profile::{AuSourceProfile, AuTestDefinition};

pub use commands::components::attach_served_component;
pub use commands::conflicts::{ResolveQcConflictInput, resolve_qc_conflict};
pub use commands::deviations::{
    CloseDeviationInput, DeviationTarget, OpenDeviationInput, close_deviation, open_deviation,
};
pub use commands::file_sources::{RegisterFileSourceInput, register_file_source};
pub use commands::lots::{
    CompletePrepBatchInput, CreatePrepBatchInput, RecordPrepStepInput, RegisterLotInput,
    WithdrawLotInput, complete_prep_batch, create_prep_batch, record_prep_step, register_lot,
    withdraw_lot,
};
pub use commands::patient::{AddPatientPositionInput, add_patient_position};
pub use commands::performing_site::{RecordRunPerformingSiteInput, record_run_performing_site};
pub use commands::qc::{AssessQcInput, QcRow, assess_qc, record_qc_observations};
pub use commands::readiness::{
    ApproveSourceInput, RegisterSourceInput, SetInstrumentStatusInput, SourceTestCodeInput,
    approve_source, register_source, set_instrument_status,
};
pub use commands::runs::{
    AddQcPositionInput, CloseRunInput, CreateRunInput, FailExecutionInput, add_qc_position,
    close_run, create_run, fail_execution,
};
pub use dependency::{DependencyRead, dependency_status, patient_intake_gate};
pub use error::AnalyticalError;
pub use file_layout::{
    EndOfData, FileColumns, FileDelimiter, FileEncoding, FileLayout, FileShape, MAX_FIELD_BYTES,
    MAX_FILE_COLUMNS, MAX_FILE_ROWS, SampleTypeWord, ValueColumn, ValueWord, ValueWordMeaning,
};
pub use model::{
    AttemptKind, DeviationId, DeviationScope, DeviationState, ExecutionId, ExecutionState,
    InstrumentStatus, LotId, LotState, MaterialKind, PositionId, PositionRole, PrepBatchId,
    PrepBatchState, PrepStepId, QcAssessmentId, QcConflictId, QcConflictResolution,
    QcConflictState, QcObservationId, QcOutcome, QcRole, QcScope, QcStatus, RegistrationState,
    RevisionIndicator, RunId, RunState, SampleIdField, SourceRegistrationId,
};
pub use queries::preparation::{ListPrepBatchesInput, PrepBatchSummary, list_prep_batches};
pub use queries::qc::get_run_qc;
pub use queries::queues::{component_executions, get_deviation, open_deviations, qc_work_queue};
pub use queries::runs::{ListRunsPageInput, get_run, list_runs, list_runs_page, run_created_at};
pub use queries::{
    active_registration, execution_components, execution_dilution_factor,
    execution_laboratory_profile, list_instrument_readiness, list_lots, list_source_registrations,
    list_worklist, position_by_sample_label, prep_batch, run_laboratory_profile,
    source_registration,
};
pub use views::{
    ComponentExecution, DependencySnapshot, DeviationView, ExecutionView, InstrumentReadinessView,
    LotView, PositionMatch, PositionRef, PrepBatchQcView, PrepBatchView, PrepStepView,
    QcAssessmentHistory, QcAssessmentView, QcConflictView, QcObservationView, QcWorkItem,
    RegistrationSummary, RunDetail, RunPositionView, RunQcAnalyteView, RunQcView, RunSummary,
    SourceRegistrationView, SourceTestCodeView, WorklistEntry,
};

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "analytical",
        sql: include_str!("../migrations/0001_analytical.sql"),
    },
    Migration {
        version: 2,
        name: "review_dependencies",
        sql: include_str!("../migrations/0002_review_dependencies.sql"),
    },
    Migration {
        version: 3,
        name: "worklist_indexes",
        sql: include_str!("../migrations/0003_worklist_indexes.sql"),
    },
    Migration {
        version: 4,
        name: "source_file_layouts",
        sql: include_str!("../migrations/0004_source_file_layouts.sql"),
    },
    Migration {
        version: 5,
        name: "self_approval",
        sql: include_str!("../migrations/0005_self_approval.sql"),
    },
    Migration {
        version: 6,
        name: "source_au_profiles",
        sql: include_str!("../migrations/0006_source_au_profiles.sql"),
    },
    Migration {
        version: 7,
        name: "performing_profile",
        sql: include_str!("../migrations/0007_performing_profile.sql"),
    },
    Migration {
        version: 8,
        name: "qc_scope_lookup",
        sql: include_str!("../migrations/0008_qc_scope_lookup.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "analytical",
    table_prefix: "ana_",
    migrations: MIGRATIONS,
};
