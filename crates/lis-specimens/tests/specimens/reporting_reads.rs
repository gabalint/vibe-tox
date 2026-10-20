//! Reads that reporting, billing and the work queues call (M3): the release
//! gate (REQ-SPC-005, NFR-01), hold history, a subject's specimens and the
//! receipt queue (REQ-OPS-004).

use std::time::Duration;

use lis_core::{Capability, UtcTimestamp};
use lis_specimens::{
    CollectionPrecision, ContainerHoldKind, ContainerId, PlaceContainerHoldInput, ReceiptDecision,
    RecordReceiptDecisionInput, ReleaseContainerHoldInput, SpecimenAcceptance, SpecimenId,
};

use crate::{Fixture, STAFF, fixture, read, run};

/// Moves the fixed test clock forward by `minutes`; returns the new now.
fn advance(f: &Fixture, minutes: u64) -> UtcTimestamp {
    let now = lis_store::testing::test_start()
        .checked_add(Duration::from_secs(minutes * 60))
        .unwrap();
    f.t.clock.set(now);
    now
}

const QUALITY: &[Capability] = &[Capability::ManageQuality];

fn decide(f: &Fixture, specimen_id: SpecimenId, expected_revision: i64, decision: ReceiptDecision) {
    let reason = matches!(decision, ReceiptDecision::Held | ReceiptDecision::Rejected)
        .then(|| "Synthetic reason".to_owned());
    let conditional = decision == ReceiptDecision::AcceptedConditionally;
    let input = RecordReceiptDecisionInput {
        specimen_id,
        expected_revision,
        decision,
        discrepancies: Vec::new(),
        policy_reference: conditional.then(|| "SYN-POLICY-1".to_owned()),
        scope: conditional.then(|| "Synthetic scope".to_owned()),
        reason,
    };
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(ctx, input)
    })
    .unwrap();
}

fn hold(f: &Fixture, container_id: ContainerId, kind: ContainerHoldKind) -> lis_specimens::HoldId {
    let input = PlaceContainerHoldInput {
        container_id,
        kind,
        reason: "Synthetic hold".to_owned(),
    };
    run(&f.t, QUALITY, move |ctx| {
        lis_specimens::place_hold(ctx, input)
    })
    .unwrap()
    .hold_id
}

fn release_gate(f: &Fixture, specimen_id: SpecimenId) -> Result<(), Vec<String>> {
    read(&f.t, move |conn| {
        Ok(lis_specimens::release_gate(conn, specimen_id)?
            .map_err(|gates| gates.into_iter().map(|gate| gate.code).collect()))
    })
}

#[test]
fn release_gate_refuses_unaccepted_held_and_rejected_specimens() {
    let f = fixture();
    let received = f.accession(1).specimens[0].specimen_id;
    assert_eq!(
        release_gate(&f, received).unwrap_err(),
        ["specimen_not_accepted"]
    );
    decide(&f, received, 1, ReceiptDecision::Held);
    assert_eq!(release_gate(&f, received).unwrap_err(), ["specimen_held"]);
    decide(&f, received, 2, ReceiptDecision::Rejected);
    assert_eq!(
        release_gate(&f, received).unwrap_err(),
        ["specimen_rejected"]
    );
}

#[test]
fn release_gate_blocks_on_quality_and_excursion_holds_but_not_on_used_material() {
    let f = fixture();
    let detail = f.accession(2);
    let specimen_id = detail.specimens[0].specimen_id;
    let first = detail.specimens[0].containers[0].container_id;
    let second = detail.specimens[0].containers[1].container_id;
    decide(&f, specimen_id, 1, ReceiptDecision::Accepted);
    assert_eq!(release_gate(&f, specimen_id), Ok(()));

    // Used-up material blocks testing, never the report of finished work.
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::mark_container_depleted(
            ctx,
            lis_specimens::MarkContainerDepletedInput {
                container_id: first,
                expected_revision: 1,
                reason: "Synthetic: used up".to_owned(),
            },
        )
    })
    .unwrap();
    hold(&f, first, ContainerHoldKind::Legal);
    hold(&f, second, ContainerHoldKind::Retention);
    assert_eq!(release_gate(&f, specimen_id), Ok(()));

    let quality = hold(&f, second, ContainerHoldKind::Quality);
    assert_eq!(
        release_gate(&f, specimen_id).unwrap_err(),
        ["container_quality_hold"]
    );
    let release = ReleaseContainerHoldInput {
        hold_id: quality,
        expected_revision: 1,
        reason: "Synthetic release".to_owned(),
    };
    run(&f.t, QUALITY, move |ctx| {
        lis_specimens::release_hold(ctx, release)
    })
    .unwrap();
    assert_eq!(release_gate(&f, specimen_id), Ok(()));
}

#[test]
fn hold_history_keeps_released_holds_with_their_release_time() {
    let f = fixture();
    let detail = f.accession(1);
    let specimen_id = detail.specimens[0].specimen_id;
    let container_id = detail.specimens[0].containers[0].container_id;
    let quality = hold(&f, container_id, ContainerHoldKind::Quality);
    let release = ReleaseContainerHoldInput {
        hold_id: quality,
        expected_revision: 1,
        reason: "Synthetic release".to_owned(),
    };
    run(&f.t, QUALITY, move |ctx| {
        lis_specimens::release_hold(ctx, release)
    })
    .unwrap();
    let legal = hold(&f, container_id, ContainerHoldKind::Legal);
    let history = read(&f.t, move |conn| {
        lis_specimens::container_hold_history(conn, specimen_id)
    });
    let summary: Vec<_> = history
        .iter()
        .map(|record| (record.hold.hold_id, record.released_at.is_some()))
        .collect();
    assert_eq!(summary, [(quality, true), (legal, false)]);
    assert!(history[0].released_at.unwrap() >= history[0].hold.placed_at);
}

#[test]
fn a_subjects_specimens_carry_the_collection_time_as_recorded() {
    let f = fixture();
    let first = f.accession(1);
    let second = f.accession(1);
    let subject_id = f.subject_id;
    let specimens = read(&f.t, move |conn| {
        lis_specimens::specimens_for_subject(conn, subject_id)
    });
    let ids: Vec<_> = specimens.iter().map(|s| s.specimen_id).collect();
    assert_eq!(
        ids,
        [
            first.specimens[0].specimen_id,
            second.specimens[0].specimen_id
        ]
    );
    let specimen = &specimens[0];
    assert_eq!(specimen.accession_number, first.accession_number);
    assert_eq!(specimen.order_id, first.order_id);
    assert_eq!(specimen.precision, CollectionPrecision::Datetime);
    assert_eq!(
        specimen.collected_at,
        Some(UtcTimestamp::parse("2026-01-01T14:30:00.000000000Z").unwrap())
    );
    assert_eq!(specimen.utc_offset_minutes, Some(-300));
    assert_eq!(specimen.collected_date, None);
}

#[test]
fn receipt_queue_lists_received_and_held_specimens_oldest_first() {
    let f = fixture();
    let received = f.accession(1).specimens[0].specimen_id;
    let held = f.accession(1).specimens[0].specimen_id;
    let accepted = f.accession(1).specimens[0].specimen_id;
    advance(&f, 5);
    decide(&f, held, 1, ReceiptDecision::Held);
    decide(&f, accepted, 1, ReceiptDecision::Accepted);
    let queue = read(&f.t, |conn| {
        lis_specimens::specimens_awaiting_decision(conn, 10)
    });
    let listed: Vec<_> = queue
        .iter()
        .map(|item| (item.specimen_id, item.acceptance))
        .collect();
    // The held specimen waits since its decision, after the other receipt.
    assert_eq!(
        listed,
        [
            (received, SpecimenAcceptance::Received),
            (held, SpecimenAcceptance::Held)
        ]
    );
    assert!(queue[1].since > queue[0].since);
    let limited = read(&f.t, |conn| {
        lis_specimens::specimens_awaiting_decision(conn, 1)
    });
    assert_eq!(limited.len(), 1);
}

#[test]
fn reconciliation_counts_specimens_by_the_time_of_acceptance() {
    let f = fixture();
    let early = f.accession(1).specimens[0].specimen_id;
    let accepted = f.accession(1).specimens[0].specimen_id;
    let rejected = f.accession(1).specimens[0].specimen_id;
    f.accession(1);
    decide(&f, early, 1, ReceiptDecision::Accepted);
    let from = advance(&f, 10);
    decide(&f, accepted, 1, ReceiptDecision::AcceptedConditionally);
    decide(&f, rejected, 1, ReceiptDecision::Rejected);
    let to = advance(&f, 20);
    let between = |from, to| {
        read(&f.t, move |conn| {
            lis_specimens::accepted_specimens_between(conn, from, to)
        })
    };
    assert_eq!(between(from, to), [accepted]);
    assert!(between(to, to).is_empty());
}
