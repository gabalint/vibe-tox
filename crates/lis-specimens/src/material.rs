//! Rules for using a container's material: the remaining volume, the
//! testing gates and the actions a state allows. Pure: no database.

use lis_core::{Decimal, DecimalText, GateReason};

use crate::error::SpecimenError;
use crate::model::{ContainerHoldKind, ContainerState, SpecimenAcceptance};

/// A volume with its unit text.
pub(crate) type Amount = (DecimalText, String);

/// The parent's remaining volume after an aliquot takes `taken`, when it can
/// be computed: the parent's volume and the amount taken are known and use
/// the same unit text. Earlier aliquots in another unit or of unknown volume
/// are left out, so the result is an upper bound. No unit conversion
/// (REQ-SPC-003, risk R11). Refuses a negative remainder.
pub(crate) fn remaining_volume(
    parent: Option<&Amount>,
    earlier: &[Option<Amount>],
    taken: Option<&Amount>,
) -> Result<Option<Decimal>, SpecimenError> {
    let (Some((volume, unit)), Some((amount, taken_unit))) = (parent, taken) else {
        return Ok(None);
    };
    if taken_unit != unit {
        return Ok(None);
    }
    let mut remaining = volume.to_decimal();
    for (earlier_amount, _) in earlier.iter().flatten().filter(|(_, u)| u == unit) {
        remaining = remaining
            .checked_sub(earlier_amount.to_decimal())
            .ok_or(SpecimenError::VolumeExceeded)?;
    }
    let remaining = remaining
        .checked_sub(amount.to_decimal())
        .ok_or(SpecimenError::VolumeExceeded)?;
    if remaining < Decimal::ZERO {
        return Err(SpecimenError::VolumeExceeded);
    }
    Ok(Some(remaining))
}

/// The commands a container's state allows, for the UI's convenience; the
/// engine still checks every command.
pub(crate) fn container_actions(
    state: ContainerState,
    open_holds: usize,
    acceptance: SpecimenAcceptance,
) -> Vec<String> {
    let mut actions = Vec::new();
    let mut allow = |allowed: bool, action: &str| {
        if allowed {
            actions.push(action.to_owned());
        }
    };
    let available = state.ensure_available().is_ok();
    let present = state.ensure_not_disposed().is_ok();
    let usable = acceptance.ensure_usable().is_ok();
    allow(available && open_holds == 0 && usable, "create_aliquot");
    allow(available, "mark_container_depleted");
    allow(present, "move_container");
    allow(present, "print_labels");
    allow(present, "place_hold");
    allow(open_holds > 0, "release_hold");
    allow(present && open_holds == 0, "dispose_container");
    actions
}

/// The commands a specimen's acceptance allows.
pub(crate) fn specimen_actions(acceptance: SpecimenAcceptance) -> Vec<String> {
    if acceptance.awaits_decision() {
        vec!["record_receipt_decision".to_owned()]
    } else {
        Vec::new()
    }
}

fn gate(code: &str, message: &str) -> GateReason {
    GateReason {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// The gate a specimen's acceptance raises, shared by testing and release.
fn acceptance_gate(acceptance: SpecimenAcceptance) -> Option<GateReason> {
    match acceptance {
        SpecimenAcceptance::Accepted | SpecimenAcceptance::AcceptedConditionally => None,
        SpecimenAcceptance::Received => Some(gate(
            "specimen_not_accepted",
            "The specimen has no acceptance decision.",
        )),
        SpecimenAcceptance::Held => Some(gate("specimen_held", "The specimen is held.")),
        SpecimenAcceptance::Rejected => {
            Some(gate("specimen_rejected", "The specimen was rejected."))
        }
    }
}

/// The specimen-side gates that block analytical testing of a container.
pub(crate) fn testing_gates(
    acceptance: SpecimenAcceptance,
    state: ContainerState,
    on_hold: bool,
    matrix_matches: bool,
) -> Vec<GateReason> {
    let mut gates: Vec<GateReason> = acceptance_gate(acceptance).into_iter().collect();
    match state {
        ContainerState::Available => {}
        ContainerState::Depleted => {
            gates.push(gate("container_depleted", "The container is depleted."));
        }
        ContainerState::Disposed => {
            gates.push(gate("container_disposed", "The container was disposed."));
        }
    }
    if on_hold {
        gates.push(gate("container_on_hold", "The container has an open hold."));
    }
    if !matrix_matches {
        gates.push(gate(
            "matrix_mismatch",
            "The specimen type does not match the method.",
        ));
    }
    gates
}

/// The specimen-side gates that block releasing a report (REQ-SPC-005, L9):
/// acceptance, and open quality or excursion holds on any of its
/// containers. Retention and legal holds keep material, not results, so
/// they do not block; a used-up or disposed container does not either.
pub(crate) fn release_gates(
    acceptance: SpecimenAcceptance,
    open_hold_kinds: &[ContainerHoldKind],
) -> Vec<GateReason> {
    let mut gates: Vec<GateReason> = acceptance_gate(acceptance).into_iter().collect();
    let open = |wanted: ContainerHoldKind| open_hold_kinds.contains(&wanted);
    if open(ContainerHoldKind::Quality) {
        gates.push(gate(
            "container_quality_hold",
            "A container of the specimen has an open quality hold.",
        ));
    }
    if open(ContainerHoldKind::Excursion) {
        gates.push(gate(
            "container_excursion_hold",
            "A container of the specimen has an open excursion hold.",
        ));
    }
    gates
}

#[cfg(test)]
mod tests {
    use super::*;

    fn amount(value: &str, unit: &str) -> Amount {
        (DecimalText::parse(value).unwrap(), unit.to_owned())
    }

    #[test]
    fn remaining_volume_subtracts_same_unit_amounts_only() {
        let parent = amount("10.0", "mL");
        let earlier = [Some(amount("2.5", "mL")), Some(amount("1", "g")), None];
        let remaining =
            remaining_volume(Some(&parent), &earlier, Some(&amount("7.5", "mL"))).unwrap();
        assert_eq!(remaining, Some(Decimal::ZERO));
        assert_eq!(
            remaining_volume(Some(&parent), &earlier, Some(&amount("3", "uL"))).unwrap(),
            None
        );
        assert_eq!(
            remaining_volume(None, &earlier, Some(&parent)).unwrap(),
            None
        );
        assert_eq!(
            remaining_volume(Some(&parent), &earlier, None).unwrap(),
            None
        );
    }

    #[test]
    fn taking_more_than_remains_is_refused() {
        let parent = amount("10.0", "mL");
        let earlier = [Some(amount("6", "mL"))];
        assert!(matches!(
            remaining_volume(Some(&parent), &earlier, Some(&amount("4.01", "mL"))),
            Err(SpecimenError::VolumeExceeded)
        ));
    }

    #[test]
    fn release_gates_block_on_acceptance_and_quality_or_excursion_holds_only() {
        let codes = |acceptance, kinds: &[ContainerHoldKind]| -> Vec<String> {
            release_gates(acceptance, kinds)
                .into_iter()
                .map(|gate| gate.code)
                .collect()
        };
        let keeping = [ContainerHoldKind::Retention, ContainerHoldKind::Legal];
        assert!(codes(SpecimenAcceptance::Accepted, &keeping).is_empty());
        assert!(codes(SpecimenAcceptance::AcceptedConditionally, &[]).is_empty());
        assert_eq!(
            codes(
                SpecimenAcceptance::Accepted,
                &[ContainerHoldKind::Excursion, ContainerHoldKind::Quality]
            ),
            ["container_quality_hold", "container_excursion_hold"]
        );
        assert_eq!(
            codes(SpecimenAcceptance::Received, &[]),
            ["specimen_not_accepted"]
        );
        assert_eq!(codes(SpecimenAcceptance::Held, &[]), ["specimen_held"]);
        assert_eq!(
            codes(SpecimenAcceptance::Rejected, &keeping),
            ["specimen_rejected"]
        );
    }

    #[test]
    fn testing_gates_name_every_blocking_condition() {
        let codes = |gates: Vec<GateReason>| -> Vec<String> {
            gates.into_iter().map(|gate| gate.code).collect()
        };
        assert!(
            testing_gates(
                SpecimenAcceptance::AcceptedConditionally,
                ContainerState::Available,
                false,
                true
            )
            .is_empty()
        );
        assert_eq!(
            codes(testing_gates(
                SpecimenAcceptance::Held,
                ContainerState::Depleted,
                true,
                false
            )),
            [
                "specimen_held",
                "container_depleted",
                "container_on_hold",
                "matrix_mismatch"
            ]
        );
        assert_eq!(
            codes(testing_gates(
                SpecimenAcceptance::Received,
                ContainerState::Disposed,
                false,
                true
            )),
            ["specimen_not_accepted", "container_disposed"]
        );
    }
}
