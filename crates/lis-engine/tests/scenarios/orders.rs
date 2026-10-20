//! Order-entry engine scenarios (REQ-ORD-001–003, 005–007; INV-13).

use lis_core::{CommandError, UserId};
use lis_engine::{
    AcceptOrderInput, AmendOrderEvidenceInput, AuthorityEvidenceInput, AuthorityEvidenceKind,
    BillingResponsibility, BillingReviewElement, CreateBillingReviewItemInput,
    DecideCancellationInput, DecideFollowOnInput, Decision, EvidenceInput, GetOrderInput, ItemKind,
    ListOrdersInput, ListStandingOrdersInput, OrderEvidenceId, OrderFilter, OrderId, OrderItemId,
    PlaceClarificationHoldInput, RecordTriggerNotMetInput, ReflexAuthorization, RegisterOrderInput,
    RegisterStandingOrderInput, RejectOrderInput, RequestCancellationInput, RequestFollowOnInput,
    ResolveClarificationHoldInput, SourceKind, StandingOrderId, TemplateUseInput, TriggerSource,
};

use lis_engine::Engine;

/// Plausible calls for the registry-wide negative authorization test. Domain
/// commands must refuse at `ctx.require` before observing these synthetic IDs.
pub(crate) async fn orders_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = order_record_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = follow_on_call(engine, name).await {
        return Some(result);
    }
    standing_call(engine, name).await
}

async fn order_record_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let order_id = OrderId::generate();
    Some(match name {
        "register_order" => engine.register_order(register_input()).await.map(drop),
        "get_order" => engine.get_order(GetOrderInput { order_id }).await.map(drop),
        "list_orders" => engine
            .list_orders(ListOrdersInput {
                filter: OrderFilter::All,
                subject_id: None,
                limit: 10,
                before_received_at: None,
                before_order_id: None,
            })
            .await
            .map(drop),
        "amend_order_evidence" => engine
            .amend_order_evidence(AmendOrderEvidenceInput {
                order_id,
                expected_revision: 1,
                add: evidence(),
                remove_evidence_ids: vec![OrderEvidenceId::generate()],
                reason: reason(),
            })
            .await
            .map(drop),
        "accept_order" => engine
            .accept_order(AcceptOrderInput {
                order_id,
                expected_revision: 1,
            })
            .await
            .map(drop),
        "reject_order" => engine
            .reject_order(RejectOrderInput {
                order_id,
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        "place_clarification_hold" => engine
            .place_clarification_hold(PlaceClarificationHoldInput {
                order_id,
                expected_revision: 1,
                deficiency: lis_engine::ClarificationDeficiency::OtherAuthority,
                note: None,
            })
            .await
            .map(drop),
        "resolve_clarification_hold" => engine
            .resolve_clarification_hold(ResolveClarificationHoldInput {
                hold_id: lis_orders::ClarificationHoldId::generate(),
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        "create_billing_review_item" => engine
            .create_billing_review_item(CreateBillingReviewItemInput {
                order_id,
                element: BillingReviewElement::Diagnosis,
                note: None,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn follow_on_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let order_id = OrderId::generate();
    let item_id = OrderItemId::generate();
    Some(match name {
        "request_follow_on" => engine
            .request_follow_on(RequestFollowOnInput {
                order_id,
                expected_revision: 1,
                kind: ItemKind::Reflex,
                orderable_version_id: lis_catalog::OrderableVersionId::generate(),
                sequence_role: lis_engine::SequenceRole::Confirmation,
                parent_item_id: Some(item_id),
                authority_reference: None,
                poc_result_id: Some(OrderEvidenceId::generate()),
                trigger_source: Some(TriggerSource::ClientPointOfCare),
                trigger_ref: None,
                reason: reason(),
            })
            .await
            .map(drop),
        "decide_follow_on" => engine
            .decide_follow_on(DecideFollowOnInput {
                item_id,
                expected_revision: 1,
                decision: Decision::Decline,
                reason: reason(),
            })
            .await
            .map(drop),
        "record_trigger_not_met" => engine
            .record_trigger_not_met(RecordTriggerNotMetInput {
                parent_item_id: item_id,
                orderable_id: lis_catalog::OrderableId::generate(),
                trigger_source: TriggerSource::ClientPointOfCare,
                trigger_ref: OrderEvidenceId::generate().to_string(),
                reason: reason(),
            })
            .await
            .map(drop),
        "request_cancellation" => engine
            .request_cancellation(RequestCancellationInput {
                item_id,
                requested_by: "Synthetic clinic".to_owned(),
                reason: reason(),
            })
            .await
            .map(drop),
        "decide_cancellation" => engine
            .decide_cancellation(DecideCancellationInput {
                request_id: lis_orders::CancellationRequestId::generate(),
                expected_revision: 1,
                decision: Decision::Decline,
                reason: reason(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn standing_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "register_standing_order" => engine
            .register_standing_order(RegisterStandingOrderInput {
                subject_id: lis_identity::SubjectId::generate(),
                practitioner_id: lis_identity::PractitionerId::generate(),
                scope_orderable_ids: vec![lis_catalog::OrderableId::generate()],
                valid_from: "2026-01-01".to_owned(),
                valid_until: "2026-12-31".to_owned(),
                visit_limit: Some(1),
                document_ref: "synthetic-document".to_owned(),
            })
            .await
            .map(drop),
        "list_standing_orders" => engine
            .list_standing_orders(ListStandingOrdersInput {
                subject_id: lis_identity::SubjectId::generate(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn reason() -> String {
    "Synthetic authorization test".to_owned()
}

fn register_input() -> RegisterOrderInput {
    RegisterOrderInput {
        subject_id: lis_identity::SubjectId::generate(),
        client_id: lis_identity::ClientId::generate(),
        client_location_id: None,
        practitioner_id: lis_identity::PractitionerId::generate(),
        placer_order_id: None,
        placer_authority: None,
        source_kind: SourceKind::PaperRequisition,
        source_document_ref: None,
        billing_responsibility: BillingResponsibility::NotSupplied,
        facility_setting: None,
        standing_order_id: None::<StandingOrderId>,
        template_use: TemplateUseInput::None,
        items: vec![lis_engine::OrderItemInput {
            orderable_version_id: lis_catalog::OrderableVersionId::generate(),
            sequence_role: lis_engine::SequenceRole::FirstLine,
            reflex_authorization: ReflexAuthorization::NotStated,
            reflex_scope_orderable_ids: Vec::new(),
        }],
        evidence: evidence(),
    }
}

fn evidence() -> EvidenceInput {
    EvidenceInput {
        authority: AuthorityEvidenceInput {
            kind: AuthorityEvidenceKind::NotSupplied,
            document_ref: None,
        },
        diagnoses: Vec::new(),
        medications: Vec::new(),
        point_of_care_results: Vec::new(),
    }
}
