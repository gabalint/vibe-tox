//! Typed identifiers and closed order-state codes.

use lis_core::Id;
use lis_core::text_enum;

macro_rules! id_type {
    ($(#[$meta:meta])* $marker:ident, $alias:ident) => {
        $(#[$meta])*
        pub enum $marker {}
        #[doc = concat!("Identifier for `", stringify!($marker), "`.")]
        pub type $alias = Id<$marker>;
    };
}

id_type!(/// An order aggregate.
    Order, OrderId);
id_type!(/// An initial, add-on or reflex item.
    OrderItem, OrderItemId);
id_type!(/// One frozen testable component.
    OrderComponent, OrderComponentId);
id_type!(/// One supplied evidence row.
    OrderEvidence, OrderEvidenceId);
id_type!(/// An order clarification hold.
    ClarificationHold, ClarificationHoldId);
id_type!(/// A non-blocking billing review item.
    BillingReviewItem, BillingReviewItemId);
id_type!(/// A patient-specific standing order.
    StandingOrder, StandingOrderId);
id_type!(/// A cancellation request.
    CancellationRequest, CancellationRequestId);
id_type!(/// An explicit trigger decision.
    TriggerAssessment, TriggerAssessmentId);
id_type!(/// A laboratory result revision used only as verified trigger provenance.
    ResultRevision, ResultRevisionId);

text_enum!(/// Lifecycle state of an order.
    OrderStatus { Received = "received", Accepted = "accepted", Rejected = "rejected", });
text_enum!(/// How the order reached the LIS in M2a.
    SourceKind { PaperRequisition = "paper_requisition", Fax = "fax", });
text_enum!(/// Who is expected to pay, exactly as supplied.
    BillingResponsibility { ThirdParty = "third_party", PatientSelfPay = "patient_self_pay", Client = "client", NotSupplied = "not_supplied", });
text_enum!(/// Kind of an order item.
    ItemKind { Initial = "initial", AddOn = "add_on", Reflex = "reflex", });
text_enum!(/// Lifecycle state of an item.
    ItemState { Requested = "requested", Accepted = "accepted", Declined = "declined", Cancelled = "cancelled", });
text_enum!(/// Lifecycle state of a frozen component.
    ComponentState { Active = "active", Cancelled = "cancelled", });
text_enum!(/// Role of the component in the testing sequence.
    SequenceRole { FirstLine = "first_line", Confirmation = "confirmation", });
text_enum!(/// Recorded reflex authority.
    ReflexAuthorization { Authorized = "authorized", NotAuthorized = "not_authorized", NotStated = "not_stated", });
text_enum!(/// Kind of order-authority evidence supplied.
    AuthorityEvidenceKind { WetSignature = "wet_signature", ElectronicSignature = "electronic_signature", IntentAttestation = "intent_attestation", NotSupplied = "not_supplied", });
text_enum!(/// Source of a medication statement.
    MedicationSource { OrderingClinic = "ordering_clinic", Patient = "patient", Other = "other", });
text_enum!(/// A missing payer-documentation element; never a testing hold.
    BillingReviewElement { Diagnosis = "diagnosis", LcdRationale = "lcd_rationale", PayerSignature = "payer_signature", EnrollmentStatus = "enrollment_status", Other = "other", });
text_enum!(/// Why order authority needs clarification.
ClarificationDeficiency {
    RequestorMissing = "requestor_missing",
    TestSelectionMissing = "test_selection_missing",
    SignatureOrIntentRequired = "signature_or_intent_required",
    StandingOrderExpired = "standing_order_expired",
    StandingOrderExhausted = "standing_order_exhausted",
    StandingOrderOutOfScope = "standing_order_out_of_scope",
    ReflexAuthorityMissing = "reflex_authority_missing",
    OtherAuthority = "other_authority",
});
text_enum!(/// Source of a trigger assessment.
    TriggerSource { LaboratoryResult = "laboratory_result", ClientPointOfCare = "client_point_of_care", });
text_enum!(/// A request decision.
    Decision { Accept = "accept", Decline = "decline", });
text_enum!(/// Filter for the order-entry worklist.
    OrderFilter { NeedsDecision = "needs_decision", OnHold = "on_hold", Accepted = "accepted", All = "all", });
