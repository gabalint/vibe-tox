//! Patient positions, executions and the worklist (REQ-ANA-001,
//! REQ-ANA-006, REQ-SPC-003, REQ-ORD-002, REQ-OPS-001).

use lis_analytical::{
    AnalyticalError, AttemptKind, ExecutionState, FailExecutionInput, PositionMatch, PositionRole,
    RevisionIndicator, RunDetail,
};
use lis_core::{Clock, DecimalText, ErrorCode};
use lis_orders::{
    ClarificationDeficiency, DecideFollowOnInput, Decision, ItemKind, PlaceClarificationHoldInput,
    RequestFollowOnInput, SequenceRole,
};
use lis_specimens::{ContainerHoldKind, PlaceContainerHoldInput};

use crate::{
    Fixture, Material, ORAL_FLUID, STAFF, URINE, code_of, counts, fixture, gate_codes, last_event,
    patient_input, read, run,
};

fn rerun(
    f: &Fixture,
    detail: &RunDetail,
    kind: AttemptKind,
    label: &str,
    position: &str,
) -> Result<RunDetail, AnalyticalError> {
    let related = detail.positions[0].execution.as_ref().unwrap().execution_id;
    let mut input = patient_input(detail.run_id, position, label);
    input.attempt_kind = kind;
    input.related_execution_id = Some(related);
    input.reason = Some("Synthetic internal standard failure".to_owned());
    if kind == AttemptKind::Dilution {
        input.dilution_factor = Some(DecimalText::parse("10").unwrap());
    }
    run(f, STAFF, move |ctx| {
        lis_analytical::add_patient_position(ctx, input)
    })
}

#[test]
fn a_patient_position_creates_an_execution_for_the_served_components() {
    let f = fixture();
    let material = f.urine_material();
    let run_id = f.open_run().run_id;
    let detail = f.place(run_id, "S1", &material.label).unwrap();
    let position = &detail.positions[0];
    assert_eq!(position.role, PositionRole::Patient);
    assert_eq!(
        position.sample_label, material.label,
        "the printed label is copied"
    );
    let execution = position.execution.as_ref().unwrap();
    assert_eq!(execution.container_id, material.container_id);
    assert_eq!(execution.state, ExecutionState::Active);
    assert_eq!(execution.component_ids.len(), 1);
    assert_eq!(execution.expected_analytes, 2);
    assert_eq!(execution.allowed_actions, ["fail_execution"]);
    let (action, record, _) = last_event(&f);
    assert_eq!(action, "analytical.add_patient_position");
    assert_eq!(record, Some(position.position_id.to_string()));
    let execution_id = execution.execution_id;
    let components = read(&f, move |conn| {
        lis_analytical::execution_components(conn, execution_id)
    });
    assert_eq!(components, execution.component_ids);
}

#[test]
fn material_that_is_not_accepted_or_on_hold_is_refused() {
    let f = fixture();
    let run_id = f.open_run().run_id;
    let received = f.material(f.urine.test, URINE, false);
    let before = counts(&f);
    assert_eq!(
        gate_codes(f.place(run_id, "S1", &received.label)),
        ["specimen_not_accepted"]
    );
    let held = f.urine_material();
    let hold = PlaceContainerHoldInput {
        container_id: held.container_id,
        kind: ContainerHoldKind::Quality,
        reason: "Synthetic quality hold".to_owned(),
    };
    f.setup(move |ctx| lis_specimens::place_hold(ctx, hold));
    let before_held = counts(&f);
    assert_eq!(
        gate_codes(f.place(run_id, "S1", &held.label)),
        ["container_on_hold"]
    );
    assert_eq!(counts(&f), before_held);
    assert_ne!(before, before_held);
}

#[test]
fn material_of_another_matrix_is_refused() {
    let f = fixture();
    let oral = f.material(f.oral_fluid.test, ORAL_FLUID, true);
    let run_id = f.open_run().run_id;
    assert_eq!(
        gate_codes(f.place(run_id, "S1", &oral.label)),
        ["matrix_mismatch"]
    );
}

#[test]
fn an_order_on_hold_blocks_its_material() {
    let f = fixture();
    let material = f.urine_material();
    let hold = PlaceClarificationHoldInput {
        order_id: material.order_id,
        expected_revision: material.order_revision,
        deficiency: ClarificationDeficiency::SignatureOrIntentRequired,
        note: None,
    };
    f.setup(move |ctx| lis_orders::place_clarification_hold(ctx, hold));
    let run_id = f.open_run().run_id;
    assert_eq!(
        gate_codes(f.place(run_id, "S1", &material.label)),
        ["clarification_hold_open"]
    );
}

#[test]
fn unknown_material_and_unserved_methods_are_refused() {
    let f = fixture();
    let run_id = f.open_run().run_id;
    assert_eq!(
        code_of(f.place(run_id, "S1", "C99999999")),
        ErrorCode::NotFound
    );
    // Urine material on a run whose method none of its tests uses.
    let material = f.urine_material();
    let urine = f.urine.method;
    let specimen_type = read(&f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, urine)?.specimen_type_id)
    });
    let other = crate::approved_method(&f.db, specimen_type, "syn-ur-x", f.urine.analytes);
    let input = lis_analytical::CreateRunInput {
        instrument_id: f.instrument,
        method_version_id: other,
        lot_ids: Vec::new(),
    };
    let other_run = run(&f, STAFF, move |ctx| lis_analytical::create_run(ctx, input)).unwrap();
    let result = f.place(other_run.run_id, "S1", &material.label);
    assert!(
        matches!(result, Err(AnalyticalError::NothingToTest)),
        "{result:?}"
    );
}

#[test]
fn an_initial_attempt_is_refused_while_an_execution_is_active() {
    let f = fixture();
    let material = f.urine_material();
    let first = f.open_run();
    f.place(first.run_id, "S1", &material.label).unwrap();
    let second = f.open_run();
    let again = f.place(second.run_id, "S1", &material.label);
    assert!(
        matches!(again, Err(AnalyticalError::AlreadyInProgress)),
        "{again:?}"
    );
}

#[test]
fn follow_on_attempts_need_lineage_on_the_same_specimen_and_a_reason() {
    let f = fixture();
    let material = f.urine_material();
    let first = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let mut no_reason = patient_input(first.run_id, "S2", &material.label);
    no_reason.attempt_kind = AttemptKind::Rerun;
    no_reason.related_execution_id = first.positions[0]
        .execution
        .as_ref()
        .map(|e| e.execution_id);
    let refused = run(&f, STAFF, move |ctx| {
        lis_analytical::add_patient_position(ctx, no_reason)
    });
    assert_eq!(code_of(refused), ErrorCode::Validation);
    let mut no_lineage = patient_input(first.run_id, "S2", &material.label);
    no_lineage.attempt_kind = AttemptKind::Reextraction;
    no_lineage.reason = Some("Synthetic".to_owned());
    let refused = run(&f, STAFF, move |ctx| {
        lis_analytical::add_patient_position(ctx, no_lineage)
    });
    assert_eq!(code_of(refused), ErrorCode::Validation);

    let other = f.urine_material();
    let wrong_specimen = rerun(&f, &first, AttemptKind::Rerun, &other.label, "S2");
    assert_eq!(code_of(wrong_specimen), ErrorCode::Validation);

    let second_run = f.open_run();
    let mut shaped = first.clone();
    shaped.run_id = second_run.run_id;
    let rerun_detail = rerun(&f, &shaped, AttemptKind::Rerun, &material.label, "S1").unwrap();
    let execution = rerun_detail.positions[0].execution.as_ref().unwrap();
    let original = first.positions[0].execution.as_ref().unwrap();
    assert_eq!(execution.attempt_kind, AttemptKind::Rerun);
    assert_eq!(execution.related_execution_id, Some(original.execution_id));
    assert_eq!(execution.component_ids, original.component_ids);
    assert_eq!(
        last_event(&f).2.as_deref(),
        Some("Synthetic internal standard failure")
    );
}

#[test]
fn a_dilution_needs_a_source_reporting_final_concentration() {
    let f = fixture();
    let material = f.urine_material();
    let first = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let mut no_factor = patient_input(first.run_id, "S2", &material.label);
    no_factor.attempt_kind = AttemptKind::Dilution;
    no_factor.related_execution_id = first.positions[0]
        .execution
        .as_ref()
        .map(|e| e.execution_id);
    no_factor.reason = Some("Synthetic over range".to_owned());
    let mut zero = no_factor.clone();
    zero.dilution_factor = Some(DecimalText::parse("0").unwrap());
    for input in [no_factor, zero] {
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::add_patient_position(ctx, input)
        });
        assert_eq!(code_of(result), ErrorCode::Validation);
    }
    let unregistered = rerun(&f, &first, AttemptKind::Dilution, &material.label, "S2");
    assert_eq!(gate_codes(unregistered), ["dilution_not_supported"]);
    f.register_source(RevisionIndicator::None, false);
    let not_final = rerun(&f, &first, AttemptKind::Dilution, &material.label, "S2");
    assert_eq!(gate_codes(not_final), ["dilution_not_supported"]);
    f.register_source(RevisionIndicator::None, true);
    let second_run = f.open_run();
    let mut shaped = first;
    shaped.run_id = second_run.run_id;
    let diluted = rerun(&f, &shaped, AttemptKind::Dilution, &material.label, "S1").unwrap();
    let execution = diluted.positions[0].execution.as_ref().unwrap();
    assert_eq!(
        execution.dilution_factor.as_ref().map(DecimalText::as_str),
        Some("10")
    );
}

#[test]
fn failing_an_execution_keeps_it_with_its_reason() {
    let f = fixture();
    let material = f.urine_material();
    let detail = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let execution = detail.positions[0].execution.clone().unwrap();
    let fail = |revision| FailExecutionInput {
        execution_id: execution.execution_id,
        expected_revision: revision,
        reason: "Synthetic clot".to_owned(),
    };
    let stale = fail(4);
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::fail_execution(
            ctx, stale
        ))),
        ErrorCode::Conflict
    );
    let ok = fail(1);
    let failed = run(&f, STAFF, move |ctx| {
        lis_analytical::fail_execution(ctx, ok)
    })
    .unwrap();
    let after = failed.positions[0].execution.as_ref().unwrap();
    assert_eq!(after.state, ExecutionState::Failed);
    assert_eq!(after.state_reason.as_deref(), Some("Synthetic clot"));
    assert!(after.allowed_actions.is_empty());
    let twice = fail(2);
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::fail_execution(
            ctx, twice
        ))),
        ErrorCode::Conflict
    );
    // A failed initial attempt frees the components for a new initial attempt.
    let next = f.open_run();
    f.place(next.run_id, "S1", &material.label).unwrap();
    assert!(
        f.raw()
            .execute("UPDATE ana_executions SET state = 'active'", [])
            .is_err()
    );
}

#[test]
fn an_add_on_needs_a_stability_attestation() {
    let f = fixture();
    let base = f.urine_material();
    let request = RequestFollowOnInput {
        order_id: base.order_id,
        expected_revision: base.order_revision,
        kind: ItemKind::AddOn,
        orderable_version_id: f.urine.test,
        sequence_role: SequenceRole::FirstLine,
        parent_item_id: None,
        authority_reference: Some("SYN-ADDON-1".to_owned()),
        poc_result_id: None,
        trigger_source: None,
        trigger_ref: None,
        reason: "Synthetic clinic call".to_owned(),
    };
    let requested = f.setup(move |ctx| lis_orders::request_follow_on(ctx, request));
    let item = requested
        .items
        .iter()
        .find(|item| item.kind == ItemKind::AddOn)
        .unwrap()
        .clone();
    let decide = DecideFollowOnInput {
        item_id: item.item_id,
        expected_revision: item.revision,
        decision: Decision::Accept,
        reason: "Synthetic authority on file".to_owned(),
    };
    f.setup(move |ctx| lis_orders::decide_follow_on(ctx, decide));
    let input = crate::accession_input(base.order_id, vec![item.item_id], URINE);
    let accession = f.setup(move |ctx| lis_specimens::create_accession(ctx, input));
    let add_on = accept(&f, accession);
    let run_id = f.open_run().run_id;
    let missing = f.place(run_id, "S1", &add_on.label);
    assert_eq!(code_of(missing), ErrorCode::Validation);
    let mut attested = patient_input(run_id, "S1", &add_on.label);
    attested.reason = Some("Stability checked under the synthetic SOP".to_owned());
    run(&f, STAFF, move |ctx| {
        lis_analytical::add_patient_position(ctx, attested)
    })
    .unwrap();
}

/// Accepts the first specimen of `accession`.
fn accept(f: &Fixture, accession: lis_specimens::AccessionDetail) -> Material {
    let specimen = accession.specimens[0].clone();
    let decision = lis_specimens::RecordReceiptDecisionInput {
        specimen_id: specimen.specimen_id,
        expected_revision: specimen.revision,
        decision: lis_specimens::ReceiptDecision::Accepted,
        discrepancies: Vec::new(),
        policy_reference: None,
        scope: None,
        reason: None,
    };
    let accession = f.setup(move |ctx| lis_specimens::record_receipt_decision(ctx, decision));
    let container = &accession.specimens[0].containers[0];
    Material {
        order_id: accession.order_id,
        order_revision: 0,
        container_id: container.container_id,
        label: container.label_code.clone(),
    }
}

#[test]
fn sample_labels_match_only_open_runs_of_the_sending_instrument() {
    let f = fixture();
    let material = f.urine_material();
    let instrument = f.instrument;
    let lookup = |label: String| {
        read(&f, move |conn| {
            lis_analytical::position_by_sample_label(conn, instrument, &label)
        })
    };
    assert_eq!(lookup(material.label.clone()), PositionMatch::None);
    let first = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let PositionMatch::One(found) = lookup(material.label.clone()) else {
        panic!("one position expected");
    };
    assert_eq!(found.position_id, first.positions[0].position_id);
    assert_eq!(found.role, PositionRole::Patient);
    let second = f.open_run();
    let mut shaped = first.clone();
    shaped.run_id = second.run_id;
    rerun(&f, &shaped, AttemptKind::Rerun, &material.label, "S1").unwrap();
    assert_eq!(lookup(material.label.clone()), PositionMatch::Ambiguous);
    let close = lis_analytical::CloseRunInput {
        run_id: first.run_id,
        expected_revision: 1,
        reason: None,
    };
    run(&f, STAFF, move |ctx| lis_analytical::close_run(ctx, close)).unwrap();
    assert!(matches!(
        lookup(material.label.clone()),
        PositionMatch::One(_)
    ));
    let other = crate::register_instrument(&f.db, "syn-other", 1);
    let elsewhere = read(&f, move |conn| {
        lis_analytical::position_by_sample_label(conn, other, &material.label)
    });
    assert_eq!(elsewhere, PositionMatch::None);
}

#[test]
fn the_worklist_lists_accepted_material_with_pending_tests() {
    let f = fixture();
    let pending = f.urine_material();
    let received = f.material(f.urine.test, URINE, false);
    let method = f.urine.method;
    let list = || read(&f, move |conn| lis_analytical::list_worklist(conn, method));
    let entries = list();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].label_code, pending.label);
    assert_eq!(entries[0].specimen_type_code, URINE);
    assert_eq!(entries[0].orderables, ["syn-ur-t"]);
    assert_eq!(entries[0].component_ids.len(), 1);
    assert_ne!(entries[0].label_code, received.label);
    let placed = f.place(f.open_run().run_id, "S1", &pending.label).unwrap();
    assert!(list().is_empty(), "placed material leaves the worklist");
    let execution = placed.positions[0].execution.as_ref().unwrap();
    let fail = FailExecutionInput {
        execution_id: execution.execution_id,
        expected_revision: execution.revision,
        reason: "Synthetic failed preparation".to_owned(),
    };
    run(&f, STAFF, move |ctx| {
        lis_analytical::fail_execution(ctx, fail)
    })
    .unwrap();
    assert_eq!(list().len(), 1, "failed work returns to the worklist");
}

#[test]
fn the_worklist_prefilter_uses_the_active_execution_index() {
    let f = fixture();
    let plan: Vec<String> = read(&f, |conn| {
        Ok(conn
            .prepare(
                "EXPLAIN QUERY PLAN SELECT 1 FROM ana_executions execution \
                 WHERE execution.container_id = 'synthetic-container' \
                 AND execution.method_version_id = 'synthetic-method' \
                 AND execution.state = 'active'",
            )?
            .query_map([], |row| row.get(3))?
            .collect::<Result<_, _>>()?)
    });
    assert!(
        plan.iter()
            .any(|step| step.contains("ana_active_executions_method_container")),
        "active execution index missing from query plan: {plan:?}"
    );
}

#[test]
fn component_executions_list_every_attempt_with_its_state() {
    let f = fixture();
    let material = f.urine_material();
    let detail = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let first = detail.positions[0].execution.clone().unwrap();
    let component = first.component_ids[0];
    let fail = FailExecutionInput {
        execution_id: first.execution_id,
        expected_revision: 1,
        reason: "Synthetic clot".to_owned(),
    };
    run(&f, STAFF, move |ctx| {
        lis_analytical::fail_execution(ctx, fail)
    })
    .unwrap();
    f.clock.set(
        f.clock
            .now()
            .checked_add(std::time::Duration::from_secs(60))
            .unwrap(),
    );
    let next = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let second = next.positions[0].execution.clone().unwrap();
    let listed: Vec<_> = read(&f, move |conn| {
        lis_analytical::component_executions(conn, component)
    })
    .into_iter()
    .map(|execution| {
        (
            execution.execution_id,
            execution.attempt_kind,
            execution.state,
        )
    })
    .collect();
    assert_eq!(
        listed,
        [
            (
                first.execution_id,
                AttemptKind::Initial,
                ExecutionState::Failed
            ),
            (
                second.execution_id,
                AttemptKind::Initial,
                ExecutionState::Active
            ),
        ]
    );
    let unknown = lis_orders::OrderComponentId::generate();
    assert!(
        read(&f, move |conn| lis_analytical::component_executions(
            conn, unknown
        ))
        .is_empty()
    );
}
