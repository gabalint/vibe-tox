//! QC rows from imports: re-export decisions, supersession and the
//! structural separation from patient observations (REQ-ANA-001,
//! REQ-ANA-003, brief §3.4–3.5). Receipts come from the real TCP listener.

use lis_analytical::reexport::RowDecision;
use lis_analytical::{AnalyticalError, PositionId, RevisionIndicator};
use lis_core::{Capability, ErrorCode};

use crate::{Fixture, STAFF, code_of, fixture, last_event, observe, read, run};

/// A run with a control at P1 and urine material at S1.
fn run_with_control(f: &Fixture) -> (lis_analytical::RunDetail, PositionId, PositionId) {
    let material = f.urine_material();
    let run_id = f.open_run().run_id;
    let (_, _) = f.control(run_id, "P1");
    let detail = f.place(run_id, "S1", &material.label).unwrap();
    let control = detail.positions[0].position_id;
    let patient = detail.positions[1].position_id;
    (detail, control, patient)
}

#[test]
fn qc_rows_follow_the_re_export_table_and_keep_history() {
    let f = fixture();
    f.register_source(RevisionIndicator::ResultStatusCorrection, false);
    let (detail, control, _) = run_with_control(&f);
    let receipt = f.receipt("one");
    assert_eq!(
        observe(&f, control, receipt, "A1", "48", "F").unwrap(),
        RowDecision::New
    );
    let (action, ..) = last_event(&f);
    assert_eq!(action, "analytical.record_qc_observations");
    assert_eq!(
        observe(&f, control, receipt, "A1", "48", "F").unwrap(),
        RowDecision::Duplicate
    );
    assert_eq!(
        observe(&f, control, receipt, "A1", "51", "F").unwrap(),
        RowDecision::Conflict
    );
    assert_eq!(
        observe(&f, control, receipt, "A1", "51", "C").unwrap(),
        RowDecision::Correction
    );
    let run_id = detail.run_id;
    let qc = read(&f, move |conn| lis_analytical::get_run_qc(conn, run_id));
    let observations = &qc.analytes[0].observations;
    let values: Vec<(Option<&str>, bool)> = observations
        .iter()
        .map(|o| (o.value_text.as_deref(), o.superseded))
        .collect();
    assert_eq!(values, [(Some("48"), true), (Some("51"), false)]);
    assert_eq!(qc.analytes[0].analyte_id, f.urine.analytes[0]);
    assert!(
        f.raw()
            .execute("UPDATE ana_qc_observations SET value_text = '1'", [])
            .is_err()
    );
    assert!(
        f.raw()
            .execute("DELETE FROM ana_qc_observations", [])
            .is_err()
    );
}

#[test]
fn without_the_correction_indicator_every_changed_row_is_a_conflict() {
    let f = fixture();
    f.register_source(RevisionIndicator::None, false);
    let (_, control, _) = run_with_control(&f);
    let receipt = f.receipt("two");
    observe(&f, control, receipt, "A1", "48", "F").unwrap();
    assert_eq!(
        observe(&f, control, receipt, "A1", "51", "C").unwrap(),
        RowDecision::Conflict
    );
}

#[test]
fn unmapped_test_codes_are_kept_without_an_analyte() {
    let f = fixture();
    f.register_source(RevisionIndicator::None, false);
    let (detail, control, _) = run_with_control(&f);
    let receipt = f.receipt("three");
    observe(&f, control, receipt, "ZZ9", "7", "F").unwrap();
    let run_id = detail.run_id;
    let qc = read(&f, move |conn| lis_analytical::get_run_qc(conn, run_id));
    assert_eq!(qc.unmapped_observations.len(), 1);
    assert_eq!(qc.unmapped_observations[0].analyte_id, None);
    assert!(qc.analytes.iter().all(|a| a.observations.is_empty()));
}

#[test]
fn qc_rows_never_reach_patient_positions() {
    let f = fixture();
    let (detail, control, patient) = run_with_control(&f);
    let receipt = f.receipt("four");
    let refused = observe(&f, patient, receipt, "A1", "48", "F");
    assert!(
        matches!(refused, Err(AnalyticalError::WrongState("position"))),
        "{refused:?}"
    );
    // Structural guards, bypassing the command: a trigger and two CHECKs.
    let raw = f.raw();
    let insert = raw.execute(
        "INSERT INTO ana_qc_observations (id, position_id, receipt_id, record_path, \
         source_test_code, recorded_at) VALUES ('x', ?1, ?2, 'P1/O1/R1', 'A1', 't')",
        [patient.to_string(), receipt.to_string()],
    );
    assert!(
        insert.is_err(),
        "the trigger refuses QC rows on patient positions"
    );
    let execution = detail.positions[1]
        .execution
        .as_ref()
        .unwrap()
        .execution_id
        .to_string();
    for (role, execution) in [("control", Some(execution)), ("patient", None)] {
        let inserted = raw.execute(
            "INSERT INTO ana_run_positions (id, run_id, position_label, role, execution_id, \
             sample_label, created_by, created_at) VALUES ('y', ?1, 'P9', ?2, ?3, 'Q9', 'u', 't')",
            rusqlite::params![detail.run_id.to_string(), role, execution],
        );
        assert!(inserted.is_err(), "{role}");
    }
    assert!(observe(&f, control, receipt, "A1", "48", "F").is_ok());
}

#[test]
fn qc_rows_need_an_open_run_and_the_intake_capability() {
    let f = fixture();
    let (detail, control, _) = run_with_control(&f);
    let receipt = f.receipt("five");
    let row = lis_analytical::QcRow {
        position_id: control,
        receipt_id: receipt,
        record_path: "P1/O1/R1".to_owned(),
        source_test_code: "A1".to_owned(),
        row: lis_analytical::reexport::SourceRow {
            value_text: Some("1".to_owned()),
            units_text: None,
            flags_text: None,
            status_text: None,
        },
    };
    let forbidden = run(&f, STAFF, move |ctx| {
        lis_analytical::record_qc_observations(ctx, &[row])
    });
    assert_eq!(code_of(forbidden), ErrorCode::Forbidden);
    assert!(!STAFF.contains(&Capability::InstrumentIntake));
    let close = lis_analytical::CloseRunInput {
        run_id: detail.run_id,
        expected_revision: 1,
        reason: None,
    };
    run(&f, STAFF, move |ctx| lis_analytical::close_run(ctx, close)).unwrap();
    let closed = observe(&f, control, receipt, "A1", "48", "F");
    assert!(
        matches!(closed, Err(AnalyticalError::WrongState("run"))),
        "{closed:?}"
    );
}
