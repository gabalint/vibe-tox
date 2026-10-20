//! Identifiers, coded enums and state transitions. Pure: no database.

use lis_core::Id;
use lis_core::text_enum;

use crate::error::SpecimenError;

/// Marker for [`AccessionId`].
pub enum Accession {}
/// One receipt of material for a subject's order, under one accession number.
pub type AccessionId = Id<Accession>;
/// Marker for [`CollectionId`].
pub enum Collection {}
/// One collection event.
pub type CollectionId = Id<Collection>;
/// Marker for [`CollectionObservationId`].
pub enum CollectionObservation {}
/// One collection-site observation.
pub type CollectionObservationId = Id<CollectionObservation>;
/// Marker for [`SpecimenId`].
pub enum Specimen {}
/// A specimen of one matrix from one collection.
pub type SpecimenId = Id<Specimen>;
/// Marker for [`ReceiptDecisionId`].
pub enum ReceiptDecisionRecord {}
/// One receipt decision about a specimen.
pub type ReceiptDecisionId = Id<ReceiptDecisionRecord>;
/// Marker for [`ContainerId`].
pub enum Container {}
/// A physical container: a primary tube or an aliquot.
pub type ContainerId = Id<Container>;
/// Marker for [`LocationId`].
pub enum Location {}
/// A storage location.
pub type LocationId = Id<Location>;
/// Marker for [`LocationEventId`].
pub(crate) enum LocationEvent {}
/// One container move.
pub(crate) type LocationEventId = Id<LocationEvent>;
/// Marker for [`HoldId`].
pub enum Hold {}
/// A hold on a container.
pub type HoldId = Id<Hold>;
/// Marker for [`ExcursionId`].
pub enum Excursion {}
/// A storage-condition excursion at a location.
pub type ExcursionId = Id<Excursion>;
/// Marker for [`PrinterId`].
pub enum Printer {}
/// A configured label printer.
pub type PrinterId = Id<Printer>;
/// Marker for [`LabelJobId`].
pub enum LabelJob {}
/// One label print or reprint request.
pub type LabelJobId = Id<LabelJob>;

text_enum! {
    /// Where a specimen stands after receipt (REQ-SPC-002).
    SpecimenAcceptance {
        /// Received; no decision yet.
        Received = "received",
        /// Accepted for testing.
        Accepted = "accepted",
        /// Accepted under a lab policy exception with a recorded scope.
        AcceptedConditionally = "accepted_conditionally",
        /// Held pending a decision.
        Held = "held",
        /// Rejected; final in M2.
        Rejected = "rejected",
    }
}

text_enum! {
    /// A receipt decision (REQ-SPC-002).
    ReceiptDecision {
        /// Accept.
        Accepted = "accepted",
        /// Accept under a policy reference and scope.
        AcceptedConditionally = "accepted_conditionally",
        /// Hold, with a reason.
        Held = "held",
        /// Reject, with a reason.
        Rejected = "rejected",
    }
}

text_enum! {
    /// A documented receipt discrepancy (REQ-SPC-002).
    ReceiptDiscrepancyCode {
        /// No label.
        Unlabeled = "unlabeled",
        /// Label does not match the requisition.
        LabelMismatch = "label_mismatch",
        /// Leaking container.
        Leaking = "leaking",
        /// Not enough material.
        InsufficientVolume = "insufficient_volume",
        /// Temperature or stability concern.
        TemperatureOrStability = "temperature_or_stability",
        /// Damaged container.
        ContainerDamaged = "container_damaged",
        /// Another discrepancy, described in a note.
        Other = "other",
    }
}

text_enum! {
    /// Primary container or aliquot.
    ContainerKind {
        /// The container the material arrived in.
        Primary = "primary",
        /// Material taken from a parent container.
        Aliquot = "aliquot",
    }
}

text_enum! {
    /// Container lifecycle (REQ-SPC-003, REQ-SPC-005).
    ContainerState {
        /// Holds usable material.
        Available = "available",
        /// No usable material left; kept for lineage.
        Depleted = "depleted",
        /// Physically disposed; final.
        Disposed = "disposed",
    }
}

text_enum! {
    /// Whether a volume was estimated or measured (REQ-SPC-003).
    VolumeKind {
        /// Estimated by eye or graduation.
        Estimated = "estimated",
        /// Measured.
        Measured = "measured",
        /// Not known; never depletes automatically.
        Unknown = "unknown",
    }
}

text_enum! {
    /// Why a container is held (REQ-SPC-005).
    ContainerHoldKind {
        /// Retention requirement.
        Retention = "retention",
        /// Legal hold.
        Legal = "legal",
        /// Quality investigation.
        Quality = "quality",
        /// Placed by an excursion assessment.
        Excursion = "excursion",
    }
}

text_enum! {
    /// Hold lifecycle.
    ContainerHoldState {
        /// Blocks disposal and aliquoting.
        Open = "open",
        /// Released with a reason.
        Released = "released",
    }
}

text_enum! {
    /// Excursion lifecycle (REQ-SPC-005).
    ExcursionState {
        /// Awaiting assessment.
        Open = "open",
        /// Assessed.
        Assessed = "assessed",
    }
}

text_enum! {
    /// The result of an excursion assessment.
    ExcursionOutcome {
        /// The material was judged unaffected.
        NoImpact = "no_impact",
        /// Excursion holds were placed on the listed containers.
        HoldsPlaced = "holds_placed",
    }
}

text_enum! {
    /// How label bytes reach a printer.
    PrinterTransport {
        /// Raw TCP to an IP address.
        Tcp = "tcp",
        /// A CUPS raw queue.
        Cups = "cups",
    }
}

text_enum! {
    /// First print or reprint of a container label (REQ-SPC-001).
    LabelJobKind {
        /// The first label request for the container.
        Print = "print",
        /// A later request; needs a reason.
        Reprint = "reprint",
    }
}

text_enum! {
    /// What happened to a label job.
    LabelOutcome {
        /// The bytes reached the printer or spooler; not proof a label printed.
        HandedOff = "handed_off",
        /// The label could not be rendered or handed over.
        Failed = "failed",
    }
}

text_enum! {
    /// How precisely the collection time is known (REQ-SPC-002).
    CollectionPrecision {
        /// Date, time and source offset.
        Datetime = "datetime",
        /// Date only.
        Date = "date",
        /// Not known.
        Unknown = "unknown",
    }
}

impl SpecimenAcceptance {
    /// The acceptance after `decision`. A decision is allowed from `received`
    /// and from `held` (but not `held` again); `accepted`, conditional
    /// acceptance and rejection are final in M2 (REQ-SPC-002).
    pub(crate) fn decide(self, decision: ReceiptDecision) -> Result<Self, SpecimenError> {
        let next = match decision {
            ReceiptDecision::Accepted => Self::Accepted,
            ReceiptDecision::AcceptedConditionally => Self::AcceptedConditionally,
            ReceiptDecision::Held => Self::Held,
            ReceiptDecision::Rejected => Self::Rejected,
        };
        match (self, next) {
            (Self::Received, _) => Ok(next),
            (Self::Held, Self::Held) => Err(SpecimenError::DecisionNotAllowed),
            (Self::Held, _) => Ok(next),
            (Self::Accepted | Self::AcceptedConditionally | Self::Rejected, _) => {
                Err(SpecimenError::DecisionNotAllowed)
            }
        }
    }

    /// Refuses use of the material of a held or rejected specimen.
    pub(crate) fn ensure_usable(self) -> Result<(), SpecimenError> {
        match self {
            Self::Held | Self::Rejected => Err(SpecimenError::SpecimenNotUsable(self.as_str())),
            Self::Received | Self::Accepted | Self::AcceptedConditionally => Ok(()),
        }
    }

    /// Whether a receipt decision can still be recorded.
    pub(crate) fn awaits_decision(self) -> bool {
        match self {
            Self::Received | Self::Held => true,
            Self::Accepted | Self::AcceptedConditionally | Self::Rejected => false,
        }
    }
}

impl ContainerState {
    /// Refuses anything but an available container.
    pub(crate) fn ensure_available(self) -> Result<(), SpecimenError> {
        match self {
            Self::Available => Ok(()),
            Self::Depleted | Self::Disposed => Err(SpecimenError::ContainerState(self.as_str())),
        }
    }

    /// Refuses a disposed container.
    pub(crate) fn ensure_not_disposed(self) -> Result<(), SpecimenError> {
        match self {
            Self::Available | Self::Depleted => Ok(()),
            Self::Disposed => Err(SpecimenError::ContainerState(self.as_str())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decisions_follow_the_acceptance_state_machine() {
        use ReceiptDecision as D;
        use SpecimenAcceptance as A;
        for decision in [D::Accepted, D::AcceptedConditionally, D::Held, D::Rejected] {
            assert!(A::Received.decide(decision).is_ok());
        }
        assert_eq!(A::Held.decide(D::Accepted).unwrap(), A::Accepted);
        assert_eq!(A::Held.decide(D::Rejected).unwrap(), A::Rejected);
        assert!(A::Held.decide(D::Held).is_err());
        for terminal in [A::Accepted, A::AcceptedConditionally, A::Rejected] {
            for decision in [D::Accepted, D::Held, D::Rejected] {
                assert!(terminal.decide(decision).is_err(), "{terminal:?}");
            }
        }
    }

    #[test]
    fn held_and_rejected_material_is_not_usable() {
        assert!(SpecimenAcceptance::Held.ensure_usable().is_err());
        assert!(SpecimenAcceptance::Rejected.ensure_usable().is_err());
        assert!(SpecimenAcceptance::Accepted.ensure_usable().is_ok());
    }

    #[test]
    fn only_available_containers_give_material() {
        assert!(ContainerState::Available.ensure_available().is_ok());
        assert!(ContainerState::Depleted.ensure_available().is_err());
        assert!(ContainerState::Disposed.ensure_not_disposed().is_err());
        assert!(ContainerState::Depleted.ensure_not_disposed().is_ok());
    }
}
