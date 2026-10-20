//! Analytes outside the ordered scope (result files brief F17, Q-41): a
//! file row that passes every check is kept as a measurement with no result
//! line, a row that fails one is set aside with the reason, re-exports
//! follow F12, and an accepted add-on can use the kept measurement.

use lis_analytical::AttemptKind;
use lis_core::Comparator;
use lis_orders::{DecideFollowOnInput, Decision, ItemKind, OrderComponentId, RequestFollowOnInput};
use lis_results::{
    MeasurementForm, QuarantineReason, QuarantineResolution, ResolveQuarantineItemInput,
    ResultCause, ResultOutcome, SetAsideKind, UseMeasuredResultInput,
};

use crate::rows::{current, quarantined, row};
use crate::*;

fn open_quarantine(f: &Fixture) -> i64 {
    f.read(|conn| {
        Ok(conn.query_row(
            "SELECT count(*) FROM res_quarantine_items WHERE state = 'open'",
            [],
            |row| row.get(0),
        )?)
    })
}

/// Requests and accepts the fentanyl test as an add-on of `sample`'s order;
/// returns its component.
pub(crate) fn add_fentanyl(f: &Fixture, sample: &Sample) -> OrderComponentId {
    let request = RequestFollowOnInput {
        order_id: sample.order_id,
        expected_revision: sample.order_revision,
        kind: ItemKind::AddOn,
        orderable_version_id: f.fentanyl,
        sequence_role: SequenceRole::FirstLine,
        parent_item_id: None,
        authority_reference: Some("SYN-ADDON-1".to_owned()),
        poc_result_id: None,
        trigger_source: None,
        trigger_ref: None,
        reason: "Synthetic clinician add-on".to_owned(),
    };
    let order = f.setup(|ctx| lis_orders::request_follow_on(ctx, request));
    let item = order
        .items
        .iter()
        .find(|item| item.kind == ItemKind::AddOn)
        .unwrap();
    let decide = DecideFollowOnInput {
        item_id: item.item_id,
        expected_revision: item.revision,
        decision: Decision::Accept,
        reason: "Synthetic acceptance".to_owned(),
    };
    let order = f.setup(|ctx| lis_orders::decide_follow_on(ctx, decide));
    order
        .items
        .iter()
        .find(|item| item.kind == ItemKind::AddOn)
        .unwrap()
        .components[0]
        .component_id
}

#[test]
fn an_analyte_outside_the_ordered_scope_is_kept_without_a_line_or_queue_item() {
    let f = fixture();
    let s = f.sample();
    let attempt = f.import(&[row(&s.label, "Unknown", "Fentanyl", "0.8", UNIT, "1")]);
    assert_eq!(
        (
            attempt.rows_retained,
            attempt.rows_new,
            attempt.rows_quarantined
        ),
        (1, 0, 0)
    );
    assert_eq!(open_quarantine(&f), 0, "no Not ordered queue item");
    let results = f.results(&s);
    assert!(results.lines.is_empty());
    let kept = &results.measured_not_ordered;
    assert_eq!(kept.len(), 1);
    assert_eq!(
        (
            kept[0].analyte_code.as_str(),
            kept[0].value_text.as_deref(),
            kept[0].units_text.as_deref(),
            kept[0].form,
            kept[0].execution_id,
            kept[0].run_id,
        ),
        (
            "SYN-FEN",
            Some("0.8"),
            Some(UNIT),
            MeasurementForm::Quantity,
            s.execution_id,
            f.run
        )
    );
    assert!(kept[0].add_on_components.is_empty(), "nothing was added on");
    assert_eq!(results.allowed_actions, ["submit_technical_review"]);
    // How the run list names the run: instrument and creation time.
    let run = f.run;
    let created = f.read(move |conn| Ok(lis_analytical::run_created_at(conn, run)?));
    assert_eq!(
        (kept[0].instrument_name.as_str(), kept[0].run_created_at),
        ("Synthetic LC-MS/MS files", created)
    );
}

/// The positions the results-expected queue lists.
fn awaiting(f: &Fixture) -> Vec<lis_analytical::PositionId> {
    f.read(|conn| {
        Ok(lis_results::awaiting_results(conn, 100)?
            .into_iter()
            .map(|row| row.position_id)
            .collect())
    })
}

/// The patient position of `sample`'s execution on the fixture's run.
fn position_of(f: &Fixture, sample: &Sample) -> lis_analytical::PositionId {
    let (run, execution) = (f.run, sample.execution_id);
    f.read(move |conn| {
        Ok(lis_analytical::get_run(conn, run, test_start())?
            .positions
            .into_iter()
            .find(|position| position.execution.as_ref().map(|e| e.execution_id) == Some(execution))
            .unwrap()
            .position_id)
    })
}

// M1 of the wave-10 review: a file of unordered analytes cannot hide a missing
// ordered result: 39 kept rows leave the position in the results-expected queue
// until every ordered analyte is measured or explicitly reconciled.
#[test]
fn kept_measurements_do_not_end_the_wait_for_ordered_results() {
    let f = fixture();
    let mut codes: Vec<(String, AnalyteId)> = (1..=39)
        .map(|n| (format!("Fentanyl-{n:02}"), f.analytes.fen))
        .collect();
    codes.push(("Morphine".to_owned(), f.analytes.mor));
    codes.push(("Oxycodone".to_owned(), f.analytes.oxy));
    f.replace_source(layout(), codes);
    let s = f.sample();
    let position = position_of(&f, &s);
    let rows: Vec<String> = (1..=39)
        .map(|n| {
            row(
                &s.label,
                "Unknown",
                &format!("Fentanyl-{n:02}"),
                "0.8",
                UNIT,
                "1",
            )
        })
        .collect();
    assert_eq!(f.import(&rows).rows_retained, 39);
    assert!(awaiting(&f).contains(&position), "still awaiting");
    f.import(&[row(&s.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    assert!(
        awaiting(&f).contains(&position),
        "one ordered result leaves the second analyte pending"
    );
    f.import(&[row(&s.label, "Unknown", "Oxycodone", "15", UNIT, "1")]);
    assert!(
        !awaiting(&f).contains(&position),
        "all ordered analytes arrived"
    );
}

/// Joins the add-on item of `component` to `sample`'s accession directly, as an
/// accession made with it would hold it.
fn join(f: &Fixture, sample: &Sample, component: OrderComponentId) {
    let (order, accession) = (sample.order_id, sample.accession_id);
    let item = f.read(move |conn| {
        Ok(lis_orders::get_order(conn, order)?
            .items
            .into_iter()
            .find(|item| item.components.iter().any(|c| c.component_id == component))
            .unwrap()
            .item_id)
    });
    f.setup(move |ctx| lis_specimens::join_accession(ctx, accession, item));
}

fn use_kept(
    f: &Fixture,
    sample: &Sample,
    component: OrderComponentId,
) -> Result<SpecimenResults, ResultError> {
    let input = UseMeasuredResultInput {
        order_component_id: component,
        execution_id: sample.execution_id,
        reason: "Synthetic add-on uses the measured result".to_owned(),
    };
    f.run_as(principal(STAFF), move |ctx| {
        lis_results::use_measured_result(ctx, input)
    })
}

fn gate_codes(result: Result<SpecimenResults, ResultError>) -> Vec<String> {
    result
        .unwrap_err()
        .gate_reasons()
        .iter()
        .map(|gate| gate.code.clone())
        .collect()
}

// L1 of the wave-10 review: the gates patient placement runs apply also when the
// add-on's item is already in the accession and is not joined again.
#[test]
fn using_a_kept_measurement_rechecks_the_placement_gates() {
    let f = fixture();
    let s = f.sample();
    f.import(&[row(&s.label, "Unknown", "Fentanyl", "0.8", UNIT, "1")]);
    let component = add_fentanyl(&f, &s);
    join(&f, &s, component);
    let order = s.order_id;
    let revision = f.read(move |conn| Ok(lis_orders::get_order(conn, order)?.revision));
    let hold = lis_orders::PlaceClarificationHoldInput {
        order_id: order,
        expected_revision: revision,
        deficiency: lis_orders::ClarificationDeficiency::OtherAuthority,
        note: None,
    };
    f.setup(|ctx| lis_orders::place_clarification_hold(ctx, hold));
    assert!(
        gate_codes(use_kept(&f, &s, component)).contains(&"clarification_hold_open".to_owned()),
        "order gate"
    );

    let t = f.sample();
    f.import(&[row(&t.label, "Unknown", "Fentanyl", "0.9", UNIT, "1")]);
    let component = add_fentanyl(&f, &t);
    join(&f, &t, component);
    let label = t.label.clone();
    let container = f.read(move |conn| Ok(lis_specimens::find_container(conn, &label)?.container));
    let hold = lis_specimens::PlaceContainerHoldInput {
        container_id: container.container_id,
        kind: lis_specimens::ContainerHoldKind::Quality,
        reason: "Synthetic quality hold".to_owned(),
    };
    f.setup(|ctx| lis_specimens::place_hold(ctx, hold));
    assert_eq!(
        gate_codes(use_kept(&f, &t, component)),
        ["container_on_hold"]
    );
    assert_eq!(
        f.results(&t).measured_not_ordered.len(),
        1,
        "nothing written"
    );
}

// L8 of the wave-10 review: a line left by an execution that failed advances under
// its current revision, and the change is audited with the previous revision.
#[test]
fn a_remaining_line_advances_under_its_revision_and_is_audited() {
    let f = fixture();
    let s = f.sample();
    f.import(&[row(&s.label, "Unknown", "Fentanyl", "0.8", UNIT, "1")]);
    let component = add_fentanyl(&f, &s);
    join(&f, &s, component);
    f.close_run(f.run);
    let second = f.open_run(false);
    let first_execution = s.execution_id;
    // The add-on alone is placed again, with the analyst's stability statement.
    let placement = lis_analytical::AddPatientPositionInput {
        run_id: second,
        position_label: "P-add-on".to_owned(),
        container_label: s.label.clone(),
        attempt_kind: AttemptKind::Initial,
        related_execution_id: None,
        dilution_factor: None,
        prep_batch_id: None,
        reason: Some("Synthetic stability SOP checked".to_owned()),
    };
    let run = f.setup(|ctx| lis_analytical::add_patient_position(ctx, placement));
    let retest = Sample {
        execution_id: run.positions[0].execution.as_ref().unwrap().execution_id,
        ..s
    };
    f.import(&[row(&retest.label, "Unknown", "Fentanyl", "0.9", UNIT, "1")]);
    let line_before = f
        .results(&retest)
        .lines
        .into_iter()
        .find(|line| line.analyte_code == "SYN-FEN")
        .unwrap();
    let failed = retest.execution_id;
    let revision = f.read(move |conn| {
        Ok(lis_analytical::get_run(conn, second, test_start())?
            .positions
            .into_iter()
            .find_map(|p| p.execution.filter(|e| e.execution_id == failed))
            .unwrap()
            .revision)
    });
    let fail = lis_analytical::FailExecutionInput {
        execution_id: failed,
        expected_revision: revision,
        reason: "Synthetic injection failure".to_owned(),
    };
    f.setup(|ctx| lis_analytical::fail_execution(ctx, fail));
    let used = Sample {
        execution_id: first_execution,
        ..retest
    };
    let results = use_kept(&f, &used, component).unwrap();
    let line = results
        .lines
        .iter()
        .find(|line| line.analyte_code == "SYN-FEN")
        .unwrap();
    assert_eq!(line.result_id, line_before.result_id);
    assert_eq!(line.revision, line_before.revision + 1);
    let now = line.current.as_ref().unwrap();
    assert_eq!(
        (now.cause, now.magnitude.as_deref()),
        (ResultCause::Selected, Some("0.8"))
    );
    let result_id = line.result_id.to_string();
    let audited: (i64, i64) = f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT old_revision, new_revision FROM audit_events \
             WHERE action = 'result.use_measured_result' AND record_type = 'result' \
               AND record_id = ?1",
            [result_id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    assert_eq!(audited, (line_before.revision, line_before.revision + 1));
}

#[test]
fn a_kept_row_that_fails_a_check_is_set_aside_with_the_reason() {
    let f = fixture();
    let (s, t) = (f.sample(), f.sample());
    let attempt = f.import(&[
        row(&s.label, "Unknown", "Fentanyl", "0.8", "ug/L", "1"),
        row(&s.label, "Unknown", "Fentanyl", "", UNIT, "1"),
        row(&s.label, "Unknown", "Fentanyl", "ND", UNIT, "1"),
        row(&s.label, "Unknown", "Fentanyl", "0.9", UNIT, "10"),
        row(&t.label, "Unknown", "Fentanyl", "N/F", UNIT, "1"),
    ]);
    assert_eq!(
        (
            attempt.rows_set_aside,
            attempt.rows_retained,
            attempt.rows_quarantined
        ),
        (4, 1, 0)
    );
    let reasons: Vec<_> = f
        .detail(&attempt)
        .set_aside
        .iter()
        .map(|row| (row.kind, row.reason))
        .collect();
    let unusable = |reason| (SetAsideKind::NotOrderedUnusable, Some(reason));
    assert_eq!(
        reasons,
        [
            unusable(QuarantineReason::IncompatibleUnit),
            unusable(QuarantineReason::MalformedValue),
            unusable(QuarantineReason::MalformedValue),
            unusable(QuarantineReason::DilutionMismatch),
        ]
    );
    assert!(f.measurements(&s, f.analytes.fen).is_empty());
    assert_eq!(
        f.measurements(&t, f.analytes.fen),
        [(
            "word".to_owned(),
            Some("N/F".to_owned()),
            Some(UNIT.to_owned())
        )]
    );
}

// F12 for kept rows: identical rows are duplicates; a changed row waits as
// `conflicting_duplicate` until accepted as reprocessing, which keeps the
// new value, still without a result line.
#[test]
fn a_re_exported_kept_row_is_a_duplicate_or_a_conflict_until_accepted() {
    let f = fixture();
    let s = f.sample();
    let fentanyl = |value: &str| row(&s.label, "Unknown", "Fentanyl", value, UNIT, "1");
    f.import(&[fentanyl("0.8")]);
    let again = f.import(&[
        fentanyl("0.8"),
        row(&s.label, "Unknown", "Morphine", "30", UNIT, "1"),
    ]);
    assert_eq!(
        (again.rows_duplicate, again.rows_new, again.rows_retained),
        (1, 1, 0)
    );
    let changed = f.import(&[fentanyl("0.9")]);
    let detail = f.detail(&changed);
    assert_eq!(
        quarantined(&detail),
        [(QuarantineReason::ConflictingDuplicate, "L2".to_owned())]
    );
    let kept = |f: &Fixture| -> Vec<Option<String>> {
        f.results(&s)
            .measured_not_ordered
            .into_iter()
            .map(|kept| kept.value_text)
            .collect()
    };
    assert_eq!(kept(&f), [Some("0.8".to_owned())]);
    let item = &detail.quarantine[0];
    let resolve = ResolveQuarantineItemInput {
        item_id: item.item_id,
        expected_revision: item.revision,
        resolution: QuarantineResolution::AcceptAsReprocessing,
        reason: "Synthetic reintegration confirmed".to_owned(),
    };
    f.setup(|ctx| lis_results::resolve_quarantine_item(ctx, resolve));
    assert_eq!(kept(&f), [Some("0.9".to_owned())]);
    assert_eq!(f.measurements(&s, f.analytes.fen).len(), 2);
    assert!(
        f.results(&s)
            .lines
            .iter()
            .all(|line| line.analyte_code != "SYN-FEN")
    );
}

// F17: an accepted add-on pulls the kept word in; the revision is selected,
// derived from the word under its rule, and goes to technical review.
#[test]
fn an_accepted_add_on_uses_a_kept_word_as_a_selected_censored_result() {
    let f = fixture();
    let s = f.sample();
    f.import(&[row(&s.label, "Unknown", "Fentanyl", "N/F", UNIT, "1")]);
    let component = add_fentanyl(&f, &s);
    let offered = f.results(&s);
    assert_eq!(
        offered.measured_not_ordered[0]
            .add_on_components
            .iter()
            .map(|item| (item.component_id, item.orderable_code.as_str()))
            .collect::<Vec<_>>(),
        [(component, "syn-fen")]
    );
    assert!(
        offered
            .allowed_actions
            .contains(&"use_measured_result".to_owned())
    );
    let input = UseMeasuredResultInput {
        order_component_id: component,
        execution_id: s.execution_id,
        reason: "Synthetic add-on uses the measured result".to_owned(),
    };
    let results = f.setup(|ctx| lis_results::use_measured_result(ctx, input));
    assert!(results.measured_not_ordered.is_empty());
    let used = current(&results, "SYN-FEN");
    assert_eq!(
        (
            used.cause,
            used.outcome,
            used.comparator,
            used.magnitude.as_deref(),
            used.reason.is_some()
        ),
        (
            ResultCause::Selected,
            ResultOutcome::Censored,
            Some(Comparator::Lt),
            Some("0.5"),
            true
        )
    );
    let accession_items = f.read({
        let accession = s.accession_id;
        move |conn| {
            Ok(lis_specimens::get_accession(conn, accession)?
                .order_item_ids
                .len())
        }
    });
    assert_eq!(accession_items, 2, "the add-on joined the accession");
}
