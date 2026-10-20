//! Catalog and orders module: orders and items, frozen component manifests,
//! order evidence recorded as supplied, clarification holds, follow-on requests
//! and cancellations (REQ-ORD-001–003, REQ-ORD-005–007). Table prefix `ord_`.
//! Design: docs/planning/m2-design-brief.md §2.1.

#![warn(clippy::wildcard_enum_match_arm)]

mod accession_readiness;
mod billing_context;
mod billing_model;
mod billing_readiness;
mod billing_reviews;
mod commands;
mod error;
mod inputs;
mod model;
pub mod queries;
mod query_detail;
mod queues;
mod standing_authority;
mod views;

use lis_core::{Migration, ModuleSchema};

pub use accession_readiness::{AccessionItemReadiness, accession_item_readiness};
pub use billing_context::{get_order_billing_context, set_order_billing_context};
pub use billing_model::{
    BillingReviewRecord, BillingReviewStatus, ListBillingReviewItemsInput, OrderBillingContext,
    OrderBillingSelection, OrderBillingView, SetOrderBillingContextInput,
    UpdateBillingReviewItemInput,
};
pub use billing_readiness::{billing_context_readiness, order_billing_view};
pub use billing_reviews::{
    billing_review_items, list_billing_review_items, update_billing_review_item,
};

pub use commands::{
    accept_order, amend_order_evidence, create_billing_review_item, decide_cancellation,
    decide_follow_on, place_clarification_hold, record_trigger_not_met,
    record_trigger_not_met_from_result, register_order, register_standing_order, reject_order,
    request_cancellation, request_follow_on, request_follow_on_from_result,
    resolve_clarification_hold,
};
pub use error::OrderError;
pub use inputs::{
    AcceptOrderInput, AmendOrderEvidenceInput, AuthorityEvidenceInput,
    CreateBillingReviewItemInput, DecideCancellationInput, DecideFollowOnInput, DiagnosisInput,
    EvidenceInput, GetOrderInput, LaboratoryResultFollowOnInput,
    LaboratoryResultTriggerNotMetInput, ListOrdersInput, ListStandingOrdersInput, MedicationInput,
    OrderItemInput, PlaceClarificationHoldInput, PointOfCareResultInput, RecordTriggerNotMetInput,
    RegisterOrderInput, RegisterStandingOrderInput, RejectOrderInput, RequestCancellationInput,
    RequestFollowOnInput, ResolveClarificationHoldInput, TemplateUseInput,
};
pub use model::{
    AuthorityEvidenceKind, BillingResponsibility, BillingReviewElement, BillingReviewItemId,
    CancellationRequestId, ClarificationDeficiency, ClarificationHoldId, ComponentState, Decision,
    ItemKind, ItemState, MedicationSource, OrderComponentId, OrderEvidenceId, OrderFilter, OrderId,
    OrderItemId, OrderStatus, ReflexAuthorization, ResultRevisionId, SequenceRole, SourceKind,
    StandingOrderId, TriggerAssessmentId, TriggerSource,
};
pub use queries::{
    ComponentForTesting, ItemReadyForAccession, components_for_order, get_order,
    items_ready_for_accession, list_orders, list_standing_orders, open_holds, testing_gate,
};
pub use queues::{OrderQueueItem, orders_awaiting_decision, orders_with_open_hold};
pub use standing_authority::standing_collection_gate;
pub use views::{
    AuthorityEvidenceView, BillingReviewItemView, CancellationView, ClarificationHoldView,
    DiagnosisView, MedicationView, OrderComponentView, OrderDetail, OrderEvidenceView,
    OrderItemView, OrderSummary, PointOfCareResultView, StandingOrderView, TriggerAssessmentView,
};

/// Forward-only migrations, numbered from `0001` (coding standards).
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "orders",
        sql: include_str!("../migrations/0001_orders.sql"),
    },
    Migration {
        version: 2,
        name: "order_integrity",
        sql: include_str!("../migrations/0002_order_integrity.sql"),
    },
    Migration {
        version: 3,
        name: "explicit_sequence_role",
        sql: include_str!("../migrations/0003_explicit_sequence_role.sql"),
    },
    Migration {
        version: 4,
        name: "order_list_indexes",
        sql: include_str!("../migrations/0004_order_list_indexes.sql"),
    },
    Migration {
        version: 5,
        name: "billing_context",
        sql: include_str!("../migrations/0005_billing_context.sql"),
    },
];

/// This module's schema; the engine applies it in crate-map order.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "orders",
    table_prefix: "ord_",
    migrations: MIGRATIONS,
};
