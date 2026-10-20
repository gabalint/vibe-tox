//! Runs, QC positions, run closure and preparation batches (REQ-ANA-001,
//! REQ-ANA-007, REQ-OPS-001).

use lis_analytical::{
    AddQcPositionInput, CloseRunInput, CreatePrepBatchInput, CreateRunInput, InstrumentStatus,
    LotId, MaterialKind, PositionRole, QcRole, RecordPrepStepInput, RunState, WithdrawLotInput,
};
use lis_catalog::{MethodVersionId, RetireMethodVersion};
use lis_core::ErrorCode;

use crate::{Fixture, LATER, STAFF, code_of, counts, fixture, gate_codes, last_event, read, run};

fn create(
    f: &Fixture,
    method: MethodVersionId,
    lots: Vec<LotId>,
) -> Result<lis_analytical::RunDetail, lis_analytical::AnalyticalError> {
    let input = CreateRunInput {
        instrument_id: f.instrument,
        method_version_id: method,
        lot_ids: lots,
    };
    run(f, STAFF, move |ctx| lis_analytical::create_run(ctx, input))
}

/// A retired method version of the urine specimen type, used by no orderable.
fn retired_method(f: &Fixture) -> MethodVersionId {
    let urine = f.urine.method;
    let specimen_type = read(f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, urine)?.specimen_type_id)
    });
    let method = crate::approved_method(&f.db, specimen_type, "syn-ur-old", f.urine.analytes);
    let revision = read(f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, method)?
            .lifecycle
            .revision)
    });
    f.setup(move |ctx| {
        lis_catalog::retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: method,
                expected_revision: revision,
                reason: "Synthetic retirement".to_owned(),
            },
        )
    });
    method
}

#[test]
fn a_run_needs_an_instrument_in_service() {
    let f = fixture();
    let before = counts(&f);
    let never = create(&f, f.urine.method, Vec::new());
    assert_eq!(gate_codes(never), ["instrument_not_in_service"]);
    f.set_status(f.instrument, InstrumentStatus::OutOfService);
    let before_out = counts(&f);
    let out = create(&f, f.urine.method, Vec::new());
    assert_eq!(gate_codes(out), ["instrument_not_in_service"]);
    assert_eq!(counts(&f), before_out);
    assert_ne!(before, before_out);
}

#[test]
fn a_run_needs_an_approved_method_version_and_usable_lots() {
    let f = fixture();
    f.ready();
    let retired = retired_method(&f);
    assert_eq!(
        gate_codes(create(&f, retired, Vec::new())),
        ["method_not_current"]
    );
    let expired = f.lot(MaterialKind::Reagent, "R-OLD", "2026-01-01");
    let withdrawn = f.lot(MaterialKind::Reagent, "R-WD", LATER);
    let input = WithdrawLotInput {
        lot_id: withdrawn,
        expected_revision: 1,
        reason: "Synthetic recall".to_owned(),
    };
    run(&f, STAFF, move |ctx| {
        lis_analytical::withdraw_lot(ctx, input)
    })
    .unwrap();
    let mut codes = gate_codes(create(&f, retired, vec![expired, withdrawn]));
    codes.sort();
    assert_eq!(
        codes,
        ["lot_expired", "lot_withdrawn", "method_not_current"]
    );
    assert_eq!(
        code_of(create(&f, f.urine.method, vec![LotId::generate()])),
        ErrorCode::NotFound
    );
}

#[test]
fn a_created_run_is_open_with_its_lots_and_audited() {
    let f = fixture();
    let run_detail = f.open_run();
    assert_eq!(run_detail.state, RunState::Open);
    assert_eq!(run_detail.lots.len(), 1);
    assert_eq!(run_detail.revision, 1);
    assert_eq!(
        run_detail.allowed_actions,
        ["add_patient_position", "add_qc_position", "close_run"]
    );
    let (action, record, _) = last_event(&f);
    assert_eq!(action, "analytical.create_run");
    assert_eq!(record, Some(run_detail.run_id.to_string()));
}

fn qc(
    run_id: lis_analytical::RunId,
    label: &str,
    role: QcRole,
    lot: Option<LotId>,
    sample: Option<&str>,
) -> AddQcPositionInput {
    AddQcPositionInput {
        run_id,
        position_label: label.to_owned(),
        role,
        lot_id: lot,
        sample_label: sample.map(str::to_owned),
    }
}

#[test]
fn qc_positions_take_their_lot_label_and_never_an_execution() {
    let f = fixture();
    let run_id = f.open_run().run_id;
    let (detail, lot) = f.control(run_id, "P1");
    let position = &detail.positions[0];
    assert_eq!(position.role, PositionRole::Control);
    assert_eq!(position.lot_id, Some(lot));
    assert!(position.sample_label.starts_with("QC-C-P1"));
    assert_eq!(position.execution, None);
    assert_eq!(last_event(&f).0, "analytical.add_qc_position");
    let input = qc(run_id, "P2", QcRole::Blank, None, Some("BLANK-1"));
    let blank = run(&f, STAFF, move |ctx| {
        lis_analytical::add_qc_position(ctx, input)
    })
    .unwrap();
    assert_eq!(blank.positions[1].sample_label, "BLANK-1");
    // The same control lot twice on one run would be ambiguous to match.
    let twice = qc(run_id, "P3", QcRole::Control, Some(lot), None);
    let duplicate = run(&f, STAFF, move |ctx| {
        lis_analytical::add_qc_position(ctx, twice)
    });
    assert_eq!(code_of(duplicate), ErrorCode::Conflict);
    let same_label = qc(run_id, "P1", QcRole::Blank, None, Some("BLANK-2"));
    let taken = run(&f, STAFF, move |ctx| {
        lis_analytical::add_qc_position(ctx, same_label)
    });
    assert_eq!(code_of(taken), ErrorCode::Conflict);
}

#[test]
fn qc_positions_refuse_mismatched_or_unusable_lots() {
    let f = fixture();
    let run_id = f.open_run().run_id;
    let calibrator = f.lot(MaterialKind::Calibrator, "CAL-1", LATER);
    let expired = f.lot(MaterialKind::Control, "CTL-OLD", "2026-01-01");
    let before = counts(&f);
    for input in [
        qc(run_id, "P1", QcRole::Control, Some(calibrator), None),
        qc(run_id, "P1", QcRole::Control, None, None),
        qc(
            run_id,
            "P1",
            QcRole::Calibrator,
            Some(calibrator),
            Some("OWN-LABEL"),
        ),
        qc(
            run_id,
            "P1",
            QcRole::Blank,
            Some(calibrator),
            Some("BLANK-1"),
        ),
        qc(run_id, "P1", QcRole::Blank, None, None),
    ] {
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::add_qc_position(ctx, input)
        });
        assert_eq!(code_of(result), ErrorCode::Validation);
    }
    let input = qc(run_id, "P1", QcRole::Control, Some(expired), None);
    let result = run(&f, STAFF, move |ctx| {
        lis_analytical::add_qc_position(ctx, input)
    });
    assert_eq!(gate_codes(result), ["lot_expired"]);
    assert_eq!(counts(&f), before);
}

#[test]
fn a_closed_run_takes_no_positions() {
    let f = fixture();
    let detail = f.open_run();
    let run_id = detail.run_id;
    let stale = CloseRunInput {
        run_id,
        expected_revision: 9,
        reason: None,
    };
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::close_run(
            ctx, stale
        ))),
        ErrorCode::Conflict
    );
    let close = CloseRunInput {
        run_id,
        expected_revision: 1,
        reason: Some("Synthetic end of shift".to_owned()),
    };
    let closed = run(&f, STAFF, move |ctx| lis_analytical::close_run(ctx, close)).unwrap();
    assert_eq!((closed.state, closed.revision), (RunState::Closed, 2));
    assert!(closed.allowed_actions.is_empty());
    assert_eq!(last_event(&f).2.as_deref(), Some("Synthetic end of shift"));
    let input = qc(run_id, "P1", QcRole::Blank, None, Some("BLANK-1"));
    let late = run(&f, STAFF, move |ctx| {
        lis_analytical::add_qc_position(ctx, input)
    });
    assert_eq!(code_of(late), ErrorCode::Conflict);
    let again = CloseRunInput {
        run_id,
        expected_revision: 2,
        reason: None,
    };
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::close_run(
            ctx, again
        ))),
        ErrorCode::Conflict
    );
    let raw = f.raw().execute(
        "INSERT INTO ana_run_positions (id, run_id, position_label, role, sample_label, \
         created_by, created_at) VALUES ('x', ?1, 'P9', 'blank', 'B9', 'u', 't')",
        [run_id.to_string()],
    );
    assert!(
        raw.is_err(),
        "the trigger refuses positions on a closed run"
    );
    assert!(
        f.raw()
            .execute("UPDATE ana_runs SET state = 'open'", [])
            .is_err()
    );
}

#[test]
fn preparation_batches_record_ordered_steps_with_usable_lots() {
    let f = fixture();
    let method = f.urine.method;
    let batch = run(&f, STAFF, move |ctx| {
        lis_analytical::create_prep_batch(
            ctx,
            CreatePrepBatchInput {
                method_version_id: method,
            },
        )
    })
    .unwrap();
    let standard = f.lot(MaterialKind::InternalStandard, "IS-1", LATER);
    let expired = f.lot(MaterialKind::Reagent, "R-OLD", "2026-01-01");
    let step = |code: &str, lot| RecordPrepStepInput {
        prep_batch_id: batch.prep_batch_id,
        step_code: code.to_owned(),
        lot_id: lot,
        note: None,
    };
    let first = step("hydrolysis", None);
    run(&f, STAFF, move |ctx| {
        lis_analytical::record_prep_step(ctx, first)
    })
    .unwrap();
    let second = step("spike-is", Some(standard));
    let detail = run(&f, STAFF, move |ctx| {
        lis_analytical::record_prep_step(ctx, second)
    })
    .unwrap();
    let seqs: Vec<i64> = detail.steps.iter().map(|s| s.seq).collect();
    assert_eq!(seqs, [1, 2]);
    assert_eq!(last_event(&f).0, "analytical.record_prep_step");
    let bad = step("extract", Some(expired));
    let refused = run(&f, STAFF, move |ctx| {
        lis_analytical::record_prep_step(ctx, bad)
    });
    assert_eq!(gate_codes(refused), ["lot_expired"]);
    let retired = retired_method(&f);
    let stale_method = run(&f, STAFF, move |ctx| {
        lis_analytical::create_prep_batch(
            ctx,
            CreatePrepBatchInput {
                method_version_id: retired,
            },
        )
    });
    assert_eq!(gate_codes(stale_method), ["method_not_current"]);
    assert!(f.raw().execute("DELETE FROM ana_prep_steps", []).is_err());
}

#[test]
fn run_cursor_reaches_equal_timestamp_records_without_duplicates() {
    let f = fixture();
    let mut expected = [
        f.open_run().run_id,
        f.open_run().run_id,
        f.open_run().run_id,
    ];
    expected.sort_by_key(|id| std::cmp::Reverse(id.to_string()));
    let method = f.urine.method;
    let instrument = f.instrument;
    let page = move |after| lis_analytical::ListRunsPageInput {
        state: Some(RunState::Open),
        instrument_id: Some(instrument),
        method_version_id: Some(method),
        after_run_id: after,
        limit: 2,
    };
    let first = read(&f, move |conn| {
        lis_analytical::list_runs_page(conn, page(None))
    });
    assert_eq!(
        first.iter().map(|r| r.run_id).collect::<Vec<_>>(),
        expected[..2]
    );
    let cursor = first[1].run_id;
    let last = read(&f, move |conn| {
        lis_analytical::list_runs_page(conn, page(Some(cursor)))
    });
    assert_eq!(
        last.iter().map(|r| r.run_id).collect::<Vec<_>>(),
        expected[2..]
    );
}
