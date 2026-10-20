//! Aliquots, depletion, moves and disposal (REQ-SPC-001, REQ-SPC-003, REQ-SPC-005).

use lis_core::{Capability, DecimalText, ErrorCode};
use lis_specimens::{
    AccessionDetail, ConfigureStorageLocationInput, ContainerHoldKind, ContainerId, ContainerState,
    CreateAliquotInput, DisposeContainerInput, LocationId, MarkContainerDepletedInput,
    MoveContainerInput, PlaceContainerHoldInput, ReceiptDecision, RecordReceiptDecisionInput,
    ReleaseContainerHoldInput, SpecimenError, VolumeKind,
};

use crate::{Fixture, STAFF, code_of, counts, fixture, last_event, read, run};

fn aliquot(
    parent: ContainerId,
    expected_revision: i64,
    volume: Option<&str>,
) -> CreateAliquotInput {
    CreateAliquotInput {
        parent_container_id: parent,
        expected_revision,
        volume_taken: volume.map(|v| DecimalText::parse(v).unwrap()),
        volume_unit: volume.map(|_| "mL".to_owned()),
        volume_kind: if volume.is_some() {
            VolumeKind::Measured
        } else {
            VolumeKind::Unknown
        },
    }
}

fn take(f: &Fixture, input: CreateAliquotInput) -> Result<AccessionDetail, SpecimenError> {
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::create_aliquot(ctx, input)
    })
}

/// The container `id` in `detail`.
fn container(detail: &AccessionDetail, id: ContainerId) -> &lis_specimens::ContainerView {
    detail
        .specimens
        .iter()
        .flat_map(|specimen| &specimen.containers)
        .find(|container| container.container_id == id)
        .unwrap()
}

/// The refusal code of a command, after checking that nothing was written.
fn refused<T: std::fmt::Debug>(
    f: &Fixture,
    command: impl FnOnce() -> Result<T, SpecimenError>,
) -> ErrorCode {
    let before = counts(&f.t);
    let code = code_of(command());
    assert_eq!(counts(&f.t), before);
    code
}

fn place_hold(f: &Fixture, container_id: ContainerId) -> lis_specimens::ContainerHoldView {
    run(&f.t, &[Capability::ManageQuality], move |ctx| {
        lis_specimens::place_hold(
            ctx,
            PlaceContainerHoldInput {
                container_id,
                kind: ContainerHoldKind::Quality,
                reason: "Synthetic investigation".to_owned(),
            },
        )
    })
    .unwrap()
}

#[test]
fn aliquots_take_volume_and_deplete_the_parent_at_zero() {
    let f = fixture();
    let detail = f.accession(1);
    let parent = detail.specimens[0].containers[0].container_id;
    let after = take(&f, aliquot(parent, 1, Some("4.0"))).unwrap();
    let first = container(&after, parent);
    assert_eq!(
        (first.state, first.revision),
        (ContainerState::Available, 2)
    );
    let child = after.specimens[0]
        .containers
        .iter()
        .find(|c| c.parent_container_id == Some(parent))
        .unwrap();
    assert_eq!(child.label_code, "C00000002");
    assert_eq!(child.volume.as_ref().unwrap().as_str(), "4.0");
    let (action, record, _) = last_event(&f.t);
    assert_eq!(action, "specimen.create_aliquot");
    assert_eq!(record, Some(child.container_id.to_string()));
    // 10.0 − 4.0 − 6.00 = 0: the parent is depleted, explicitly computed.
    let after = take(&f, aliquot(parent, 2, Some("6.00"))).unwrap();
    assert_eq!(container(&after, parent).state, ContainerState::Depleted);
}

#[test]
fn aliquot_volume_may_not_exceed_the_known_remainder() {
    let f = fixture();
    let parent = f.accession(1).specimens[0].containers[0].container_id;
    assert_eq!(
        refused(&f, || take(&f, aliquot(parent, 1, Some("10.01")))),
        ErrorCode::Validation
    );
    assert_eq!(
        refused(&f, || take(&f, aliquot(parent, 1, Some("-1")))),
        ErrorCode::Validation
    );
    // Unknown volume never depletes and needs no arithmetic.
    let after = take(&f, aliquot(parent, 1, None)).unwrap();
    assert_eq!(container(&after, parent).state, ContainerState::Available);
}

#[test]
fn aliquot_is_refused_from_a_depleted_parent() {
    let f = fixture();
    let parent = f.accession(1).specimens[0].containers[0].container_id;
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::mark_container_depleted(
            ctx,
            MarkContainerDepletedInput {
                container_id: parent,
                expected_revision: 1,
                reason: "Spilled".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(last_event(&f.t).2.as_deref(), Some("Spilled"));
    assert_eq!(
        refused(&f, || take(&f, aliquot(parent, 2, None))),
        ErrorCode::Conflict
    );
}

#[test]
fn aliquot_is_refused_from_a_held_container_or_specimen() {
    let f = fixture();
    let detail = f.accession(2);
    let held_container = detail.specimens[0].containers[0].container_id;
    place_hold(&f, held_container);
    assert_eq!(
        refused(&f, || take(&f, aliquot(held_container, 1, None))),
        ErrorCode::GateBlocked
    );
    let specimen_id = detail.specimens[0].specimen_id;
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::record_receipt_decision(
            ctx,
            RecordReceiptDecisionInput {
                specimen_id,
                expected_revision: 1,
                decision: ReceiptDecision::Held,
                discrepancies: Vec::new(),
                policy_reference: None,
                scope: None,
                reason: Some("Pending clinic call".to_owned()),
            },
        )
    })
    .unwrap();
    let other = detail.specimens[0].containers[1].container_id;
    assert_eq!(
        refused(&f, || take(&f, aliquot(other, 1, None))),
        ErrorCode::GateBlocked
    );
}

#[test]
fn a_stale_revision_is_a_conflict() {
    let f = fixture();
    let parent = f.accession(1).specimens[0].containers[0].container_id;
    take(&f, aliquot(parent, 1, None)).unwrap();
    assert_eq!(
        refused(&f, || take(&f, aliquot(parent, 1, None))),
        ErrorCode::Conflict
    );
}

fn location(f: &Fixture, code: &str, active: bool) -> LocationId {
    let input = ConfigureStorageLocationInput {
        location_id: None,
        expected_revision: None,
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        storage_condition: Some("2-8 C".to_owned()),
        active,
    };
    run(&f.t, &[Capability::ManageConfiguration], move |ctx| {
        lis_specimens::configure_location(ctx, input)
    })
    .unwrap()
    .location_id
}

fn move_to(
    f: &Fixture,
    container_id: ContainerId,
    expected_revision: i64,
    location_id: LocationId,
) -> Result<AccessionDetail, SpecimenError> {
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::move_container(
            ctx,
            MoveContainerInput {
                container_id,
                expected_revision,
                location_id,
                reason: None,
            },
        )
    })
}

#[test]
fn moves_need_an_active_location_and_show_the_latest() {
    let f = fixture();
    let id = f.accession(1).specimens[0].containers[0].container_id;
    let fridge = location(&f, "fridge-1", true);
    let retired = location(&f, "fridge-0", false);
    assert_eq!(
        refused(&f, || move_to(&f, id, 1, retired)),
        ErrorCode::Conflict
    );
    let after = move_to(&f, id, 1, fridge).unwrap();
    let moved = container(&after, id);
    assert_eq!(moved.location.as_ref().unwrap().code, "fridge-1");
    assert_eq!(moved.revision, 2);
    assert_eq!(last_event(&f.t).0, "specimen.move_container");
}

fn dispose(
    f: &Fixture,
    container_id: ContainerId,
    expected_revision: i64,
) -> Result<AccessionDetail, SpecimenError> {
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::dispose_container(
            ctx,
            DisposeContainerInput {
                container_id,
                expected_revision,
                reason: "Retention period ended".to_owned(),
            },
        )
    })
}

#[test]
fn disposal_is_refused_while_a_hold_is_open() {
    let f = fixture();
    let id = f.accession(1).specimens[0].containers[0].container_id;
    let hold = place_hold(&f, id);
    assert_eq!(refused(&f, || dispose(&f, id, 1)), ErrorCode::GateBlocked);
    run(&f.t, &[Capability::ManageQuality], move |ctx| {
        lis_specimens::release_hold(
            ctx,
            ReleaseContainerHoldInput {
                hold_id: hold.hold_id,
                expected_revision: 1,
                reason: "Investigation closed".to_owned(),
            },
        )
    })
    .unwrap();
    let after = dispose(&f, id, 1).unwrap();
    let disposed = container(&after, id);
    assert_eq!(disposed.state, ContainerState::Disposed);
    assert!(disposed.allowed_actions.is_empty());
    let evidence: String = read(&f.t, move |conn| {
        Ok(conn.query_row(
            "SELECT reason FROM spc_disposals WHERE container_id = ?1",
            [id],
            |row| row.get(0),
        )?)
    });
    assert_eq!(evidence, "Retention period ended");
    // Disposed is final.
    assert_eq!(refused(&f, || dispose(&f, id, 2)), ErrorCode::Conflict);
    assert_eq!(
        refused(&f, || take(&f, aliquot(id, 2, None))),
        ErrorCode::Conflict
    );
}

#[test]
fn label_code_and_lineage_cannot_be_rewritten() {
    let f = fixture();
    let id = f.accession(1).specimens[0].containers[0].container_id;
    // Tamper attempts below the application, on a writable connection.
    let conn = crate::raw(&f);
    let rewrite = conn
        .execute(
            "UPDATE spc_containers SET label_code = 'C99999999' WHERE id = ?1",
            [id],
        )
        .unwrap_err();
    assert!(rewrite.to_string().contains("immutable"), "{rewrite}");
    let delete = conn
        .execute("DELETE FROM spc_containers WHERE id = ?1", [id])
        .unwrap_err();
    assert!(delete.to_string().contains("never deleted"), "{delete}");
}

#[test]
fn current_location_follows_committed_moves_with_equal_and_rollback_clocks() {
    let f = fixture();
    let id = f.accession(1).specimens[0].containers[0].container_id;
    let first = location(&f, "first", true);
    let second = location(&f, "second", true);
    move_to(&f, id, 1, first).unwrap();
    let after = move_to(&f, id, 2, second).unwrap();
    assert_eq!(
        container(&after, id).location.as_ref().unwrap().location_id,
        second
    );
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2020-01-01T00:00:00.000000000Z").unwrap());
    let after = move_to(&f, id, 3, first).unwrap();
    assert_eq!(
        container(&after, id).location.as_ref().unwrap().location_id,
        first
    );
    assert!(!container(&after, id).location_chronology_unresolved);
}

#[test]
fn material_history_pages_committed_moves_and_retains_terminal_attribution() {
    let f = fixture();
    let id = f.accession(1).specimens[0].containers[0].container_id;
    let first = location(&f, "history-a", true);
    let second = location(&f, "history-b", true);
    for revision in 1..=51 {
        move_to(
            &f,
            id,
            revision,
            if revision % 2 == 0 { first } else { second },
        )
        .unwrap();
    }
    let head = read(&f.t, move |conn| {
        lis_specimens::get_container_history(conn, id, 0, None)
    });
    assert_eq!(head.moves.len(), 50);
    assert!(head.has_more);
    assert_eq!(head.moves[0].causal_sequence, Some(52));
    move_to(&f, id, 52, first).unwrap();
    let snapshot = head.snapshot;
    let tail = read(&f.t, move |conn| {
        lis_specimens::get_container_history(conn, id, 50, Some(snapshot))
    });
    assert_eq!(tail.moves.len(), 1);
    assert!(!tail.has_more);
    assert_eq!(tail.moves[0].causal_sequence, Some(2));
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::mark_container_depleted(
            ctx,
            MarkContainerDepletedInput {
                container_id: id,
                expected_revision: 53,
                reason: "Synthetic material exhausted".into(),
            },
        )
    })
    .unwrap();
    let history = read(&f.t, move |conn| {
        lis_specimens::get_container_history(conn, id, 0, None)
    });
    let depleted = history.depletion.unwrap();
    assert_eq!(
        depleted.reason.as_deref(),
        Some("Synthetic material exhausted")
    );
    assert!(!depleted.actor.is_empty());
}
