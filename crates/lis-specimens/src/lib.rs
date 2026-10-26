//! Specimens module: accessions, collections, specimens, containers and
//! aliquots, receipt decisions, locations, holds, excursions, disposal and
//! label jobs (REQ-SPC-001–003, REQ-SPC-005, REQ-SPC-008 collection
//! observations, REQ-OPS-001). Table prefix `spc_`.
//! Design: docs/planning/m2-design-brief.md §2.3.
//!
//! - **Separate identities** (REQ-SPC-001). An accession fulfils accepted
//!   order items and takes its subject from the order. It has one collection
//!   event, one specimen per matrix and one or more primary containers per
//!   specimen; aliquots are containers with a parent. Each container gets a
//!   label code once; a trigger keeps the code, lineage and volume immutable,
//!   so a reprint always shows the same code. Numbering formats live in
//!   `numbering` and are lab-confirmable defaults (`A`+yy+6 digits,
//!   `C`+8 digits), allocated under the single writer above the larger of the
//!   highest issued number and the highest number floor. Floors only go up:
//!   a quality manager raises them ([`raise_number_floors`]), and recording a
//!   restore skips a fixed margin ([`raise_number_floors_after_restore`],
//!   REQ-OPS-002).
//! - **Receipt** (REQ-SPC-002). `received` → `accepted`,
//!   `accepted_conditionally`, `held` or `rejected`; `held` → any but `held`.
//!   Conditional acceptance needs a lab policy reference and scope; hold and
//!   rejection need a reason. Discrepancies are coded. Unknown and date-only
//!   collection times stay explicit; a datetime keeps its source offset.
//! - **Material** (REQ-SPC-003, REQ-SPC-005). Aliquoting is refused from a
//!   depleted, disposed or held container, from a held or rejected specimen,
//!   and when it would take more than the known remainder. Volume arithmetic
//!   uses exact decimals and identical unit text only; unknown volume never
//!   depletes. Moves record actor and time. Disposal is refused while a hold
//!   is open and leaves the container row with durable disposal evidence.
//!   Excursions are assessed; an assessment can place excursion holds.
//! - **Labels.** [`print_labels`] records a job and enqueues
//!   [`LABEL_PRINT_TOPIC`] in the same transaction. The worker renders with
//!   `lis-labels` and sends outside any transaction ([`label_request`],
//!   [`deliver_label`]), then records one outcome ([`record_label_outcome`]).
//!   `handed_off` means the bytes reached the printer, not that a label printed.
//! - **Gates for other modules.** [`testing_gate`] rechecks acceptance,
//!   container state, holds and matrix inside the caller's transaction;
//!   `spc_v_containers` is the published view. [`release_gate`] is the
//!   report-release recheck (REQ-SPC-005, NFR-01): acceptance plus open
//!   quality or excursion holds on any container; used-up material,
//!   retention and legal holds do not block a report.
//! - **Reads for reporting, billing and work queues** (M3).
//!   [`container_hold_history`] keeps released holds so a hold after a
//!   release still counts; [`specimens_for_subject`] gives collection times
//!   for the date of service; [`specimens_awaiting_decision`],
//!   [`failed_label_jobs`] and [`accepted_specimens_between`] feed the work
//!   queues and release reconciliation (REQ-OPS-004).
//!
//! Capabilities: accessioning, receipt, aliquots, depletion, moves, disposal,
//! excursion records and label requests `accession`; holds and excursion
//! assessments `manage_quality`; locations and printers
//! `manage_configuration`. Every command checks its capability first and
//! appends a clinical-stream audit event in its transaction; user-typed
//! reasons go to the event's `reason`, never to details. Queries perform no
//! authorization; the engine checks and audits reads. Views carry no
//! demographics except the scoped collection-history review, whose engine
//! read is authorized and audited, and the label worker, to print.

#![warn(clippy::wildcard_enum_match_arm)]

pub mod actions;
mod collection_history;
mod commands;
mod error;
mod input;
mod label_delivery;
mod material;
mod material_history;
mod model;
mod numbering;
mod order_accession_context;
mod queries;
mod queues;
mod repo;
mod standing_authority;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use collection_history::{
    CollectionCorrectionId, CollectionCorrectionView, CollectionHistory, CollectionReviewQueueItem,
    collection_corrections_for_specimen, get_collection_history, pending_collection_reviews,
};
pub use commands::accessioning::{
    CollectionInput, CollectionObservationInput, CreateAccessionInput, SpecimenInput,
    create_accession,
};
pub use commands::collection::{
    CorrectCollectionTimeInput, ResolveCollectionCorrectionInput, correct_collection_time,
    resolve_collection_correction,
};
pub use commands::configuration::{
    ConfigureLabelPrinterInput, ConfigureStorageLocationInput, configure_location,
    configure_printer,
};
pub use commands::containers::{
    CreateAliquotInput, DisposeContainerInput, MarkContainerDepletedInput, MoveContainerInput,
    create_aliquot, dispose_container, mark_container_depleted, move_container,
};
pub use commands::floors::{
    NumberFloor, NumberFloorId, NumberFloors, NumberPosition, RaiseNumberFloorsInput,
    number_floors, raise_number_floors, raise_number_floors_after_restore,
};
pub use commands::holds::{
    AssessExcursionInput, PlaceContainerHoldInput, RecordExcursionInput, ReleaseContainerHoldInput,
    assess_excursion, place_hold, record_excursion, release_hold,
};
pub use commands::join::join_accession;
pub use commands::labels::{PrintLabelsInput, print_labels, record_label_outcome};
pub use commands::receipt::{
    DiscrepancyInput, RecordReceiptDecisionInput, ResolveReceiptChronologyInput,
    record_receipt_decision, resolve_receipt_chronology,
};
pub use error::SpecimenError;
pub use input::CollectionTimeInput;
pub use label_delivery::{
    LABEL_PRINT_TOPIC, LabelDelivery, LabelRequest, deliver_label, label_job_id, label_request,
};
pub use material_history::{
    ExcursionPage, MaterialHistoryEntry, MaterialHistoryPage, MaterialHistorySnapshot,
    get_container_history, list_excursions,
};
pub use model::{
    AccessionId, CollectionId, CollectionObservationId, CollectionPrecision, ContainerHoldKind,
    ContainerHoldState, ContainerId, ContainerKind, ContainerState, ExcursionId, ExcursionOutcome,
    ExcursionState, HoldId, LabelJobId, LabelJobKind, LabelOutcome, LocationId, PrinterId,
    PrinterTransport, ReceiptDecision, ReceiptDecisionId, ReceiptDiscrepancyCode,
    SpecimenAcceptance, SpecimenId, VolumeKind,
};
pub use order_accession_context::{
    OrderAccessionContext, OrderAccessionItem, OrderAccessionSummary, get_order_accession_context,
};
pub use queries::{
    audit_record_label, container_hold_history, excursion, find_container, get_accession,
    list_locations, list_printers, order_item_accessioned, order_sample_barcodes, release_gate,
    specimen_accession, specimens_for_subject, testing_gate,
};
pub use queues::{
    FailedLabelJob, SpecimenAwaitingDecision, accepted_specimens_between, failed_label_jobs,
    specimens_awaiting_decision,
};
pub use views::{
    AccessionDetail, AccessionSpecimenView, CollectionObservationView, CollectionView,
    ContainerForTesting, ContainerHoldRecord, ContainerHoldView, ContainerLookup, ContainerView,
    ExcursionView, LabelJobView, LabelPrinterView, ReceiptDecisionView, ReceiptDiscrepancyView,
    StorageLocationRef, StorageLocationView, SubjectSpecimen,
};

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "specimens",
        sql: include_str!("../migrations/0001_specimens.sql"),
    },
    Migration {
        version: 2,
        name: "number_floors",
        sql: include_str!("../migrations/0002_number_floors.sql"),
    },
    Migration {
        version: 3,
        name: "collection_corrections",
        sql: include_str!("../migrations/0003_collection_corrections.sql"),
    },
    Migration {
        version: 4,
        name: "causal_order",
        sql: include_str!("../migrations/0004_causal_order.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "specimens",
    table_prefix: "spc_",
    migrations: MIGRATIONS,
};
