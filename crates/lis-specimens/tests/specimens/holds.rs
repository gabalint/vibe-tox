//! Holds, excursions and the testing gate (REQ-SPC-005, REQ-SPC-003).

use lis_core::{Capability, ErrorCode, UtcTimestamp};
use lis_specimens::{
    AssessExcursionInput, ConfigureStorageLocationInput, ContainerHoldKind, ContainerHoldState,
    ExcursionOutcome, ExcursionState, PlaceContainerHoldInput, ReceiptDecision,
    RecordExcursionInput, RecordReceiptDecisionInput, ReleaseContainerHoldInput,
};

use crate::{STAFF, URINE, code_of, fixture, last_event, read, run};

const QUALITY: &[Capability] = &[Capability::ManageQuality];

#[test]
fn holds_need_a_reason_and_release_once() {
    let f = fixture();
    let container_id = f.accession(1).specimens[0].containers[0].container_id;
    let hold = |kind, reason: &str| PlaceContainerHoldInput {
        container_id,
        kind,
        reason: reason.to_owned(),
    };
    let blank = hold(ContainerHoldKind::Legal, " ");
    assert_eq!(
        code_of(run(&f.t, QUALITY, move |ctx| lis_specimens::place_hold(
            ctx, blank
        ))),
        ErrorCode::Validation
    );
    // Excursion holds come only from an excursion assessment.
    let excursion = hold(ContainerHoldKind::Excursion, "Synthetic");
    assert_eq!(
        code_of(run(&f.t, QUALITY, move |ctx| lis_specimens::place_hold(
            ctx, excursion
        ))),
        ErrorCode::Validation
    );
    let legal = hold(ContainerHoldKind::Legal, "Synthetic legal hold");
    let placed = run(&f.t, QUALITY, move |ctx| {
        lis_specimens::place_hold(ctx, legal)
    })
    .unwrap();
    assert_eq!(placed.state, ContainerHoldState::Open);
    let release = ReleaseContainerHoldInput {
        hold_id: placed.hold_id,
        expected_revision: 1,
        reason: "Synthetic release".to_owned(),
    };
    let released = run(&f.t, QUALITY, {
        let release = release.clone();
        move |ctx| lis_specimens::release_hold(ctx, release)
    })
    .unwrap();
    assert_eq!(
        (released.state, released.revision),
        (ContainerHoldState::Released, 2)
    );
    assert_eq!(last_event(&f.t).0, "specimen.release_hold");
    let mut again = release;
    again.expected_revision = 2;
    assert_eq!(
        code_of(run(&f.t, QUALITY, move |ctx| lis_specimens::release_hold(
            ctx, again
        ))),
        ErrorCode::Conflict
    );
}

#[test]
fn an_excursion_assessment_places_excursion_holds() {
    let f = fixture();
    let detail = f.accession(2);
    let containers: Vec<_> = detail.specimens[0]
        .containers
        .iter()
        .map(|container| container.container_id)
        .collect();
    let location = ConfigureStorageLocationInput {
        location_id: None,
        expected_revision: None,
        code: "fridge-1".to_owned(),
        name: "Synthetic fridge".to_owned(),
        storage_condition: None,
        active: true,
    };
    let location_id = run(&f.t, &[Capability::ManageConfiguration], move |ctx| {
        lis_specimens::configure_location(ctx, location)
    })
    .unwrap()
    .location_id;
    let record = RecordExcursionInput {
        location_id,
        observed_from: UtcTimestamp::parse("2026-01-01T22:00:00.000000000Z").unwrap(),
        observed_to: Some(UtcTimestamp::parse("2026-01-02T01:00:00.000000000Z").unwrap()),
        description: "Door left open overnight".to_owned(),
    };
    let excursion = run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_excursion(ctx, record)
    })
    .unwrap();
    assert_eq!(excursion.state, ExcursionState::Open);
    let assess = |outcome, container_ids| AssessExcursionInput {
        excursion_id: excursion.excursion_id,
        expected_revision: 1,
        outcome,
        container_ids,
        reason: "Synthetic assessment".to_owned(),
    };
    let inconsistent = assess(ExcursionOutcome::NoImpact, containers.clone());
    assert_eq!(
        code_of(run(&f.t, QUALITY, move |ctx| {
            lis_specimens::assess_excursion(ctx, inconsistent)
        })),
        ErrorCode::Validation
    );
    let input = assess(ExcursionOutcome::HoldsPlaced, containers);
    let assessed = run(&f.t, QUALITY, move |ctx| {
        lis_specimens::assess_excursion(ctx, input)
    })
    .unwrap();
    assert_eq!(assessed.state, ExcursionState::Assessed);
    assert_eq!(assessed.holds.len(), 2);
    assert!(
        assessed
            .holds
            .iter()
            .all(|hold| hold.kind == ContainerHoldKind::Excursion
                && hold.excursion_id == Some(excursion.excursion_id))
    );
    assert_eq!(last_event(&f.t).0, "specimen.assess_excursion");
    let twice = assess(ExcursionOutcome::NoImpact, Vec::new());
    assert_eq!(
        code_of(run(&f.t, QUALITY, move |ctx| {
            lis_specimens::assess_excursion(ctx, twice)
        })),
        ErrorCode::Conflict
    );
}

#[test]
fn testing_gate_names_every_blocking_condition() {
    let f = fixture();
    let detail = f.accession(1);
    let container_id = detail.specimens[0].containers[0].container_id;
    let gate = |matrix: &'static str| {
        read(&f.t, move |conn| {
            Ok(lis_specimens::testing_gate(conn, container_id, matrix)?
                .map_err(|gates| gates.into_iter().map(|g| g.code).collect::<Vec<_>>()))
        })
    };
    assert_eq!(gate(URINE).unwrap_err(), ["specimen_not_accepted"]);
    let specimen_id = detail.specimens[0].specimen_id;
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(
            ctx,
            RecordReceiptDecisionInput {
                specimen_id,
                expected_revision: 1,
                decision: ReceiptDecision::Accepted,
                discrepancies: Vec::new(),
                policy_reference: None,
                scope: None,
                reason: None,
            },
        )
    })
    .unwrap();
    let ready = gate(URINE).unwrap();
    assert_eq!(ready.label_code, "C00000001");
    assert_eq!(ready.subject_id, f.subject_id);
    assert_eq!(gate("syn-other").unwrap_err(), ["matrix_mismatch"]);
    run(&f.t, QUALITY, move |ctx| {
        lis_specimens::place_hold(
            ctx,
            PlaceContainerHoldInput {
                container_id,
                kind: ContainerHoldKind::Quality,
                reason: "Synthetic".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(gate(URINE).unwrap_err(), ["container_on_hold"]);
}
