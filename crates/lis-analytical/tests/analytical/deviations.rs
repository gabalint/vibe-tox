//! Deviations (REQ-ANA-004, REQ-ANA-005, REQ-OPS-001).

use lis_analytical::{
    CloseDeviationInput, DeviationScope, DeviationState, DeviationTarget, LotId,
    OpenDeviationInput, RunId,
};
use lis_core::{Clock, ErrorCode};

use crate::{STAFF, code_of, counts, fixture, last_event, read, run};

fn open(scope: DeviationTarget) -> OpenDeviationInput {
    OpenDeviationInput {
        scope,
        analyte_id: None,
        description: "Synthetic pipette drift".to_owned(),
        reason: "Synthetic maintenance finding".to_owned(),
    }
}

#[test]
fn a_deviation_needs_an_existing_target_and_text() {
    let f = fixture();
    let before = counts(&f);
    for scope in [
        DeviationTarget::Run(RunId::generate()),
        DeviationTarget::Lot(LotId::generate()),
        DeviationTarget::Instrument(lis_core::InstrumentId::generate()),
    ] {
        let input = open(scope);
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::open_deviation(ctx, input)
        });
        assert_eq!(code_of(result), ErrorCode::NotFound);
    }
    let mut unknown_analyte = open(DeviationTarget::Instrument(f.instrument));
    unknown_analyte.analyte_id = Some(lis_catalog::AnalyteId::generate());
    let result = run(&f, STAFF, move |ctx| {
        lis_analytical::open_deviation(ctx, unknown_analyte)
    });
    assert_eq!(code_of(result), ErrorCode::NotFound);
    let mut blank = open(DeviationTarget::Instrument(f.instrument));
    blank.description = " ".to_owned();
    let result = run(&f, STAFF, move |ctx| {
        lis_analytical::open_deviation(ctx, blank)
    });
    assert_eq!(code_of(result), ErrorCode::Validation);
    assert_eq!(counts(&f), before);
}

#[test]
fn a_deviation_closes_once_with_a_disposition_and_stays_on_record() {
    let f = fixture();
    let detail = f.open_run();
    let input = open(DeviationTarget::Run(detail.run_id));
    let opened = run(&f, STAFF, move |ctx| {
        lis_analytical::open_deviation(ctx, input)
    })
    .unwrap();
    assert_eq!(
        (opened.scope, opened.state),
        (DeviationScope::Run, DeviationState::Open)
    );
    let (action, record, reason) = last_event(&f);
    assert_eq!(action, "analytical.open_deviation");
    assert_eq!(record, Some(opened.deviation_id.to_string()));
    assert_eq!(reason.as_deref(), Some("Synthetic maintenance finding"));
    let close = |revision| CloseDeviationInput {
        deviation_id: opened.deviation_id,
        expected_revision: revision,
        disposition: "Synthetic: recalibrated".to_owned(),
        reason: "Synthetic verification passed".to_owned(),
    };
    let stale = close(3);
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::close_deviation(
            ctx, stale
        ))),
        ErrorCode::Conflict
    );
    let ok = close(1);
    let closed = run(&f, STAFF, move |ctx| {
        lis_analytical::close_deviation(ctx, ok)
    })
    .unwrap();
    assert_eq!(closed.state, DeviationState::Closed);
    assert_eq!(
        closed.disposition.as_deref(),
        Some("Synthetic: recalibrated")
    );
    let twice = close(2);
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::close_deviation(
            ctx, twice
        ))),
        ErrorCode::Conflict
    );
    let run_id = detail.run_id;
    let qc = read(&f, move |conn| lis_analytical::get_run_qc(conn, run_id));
    assert_eq!(qc.deviations.len(), 1);
    assert!(
        f.raw()
            .execute("UPDATE ana_deviations SET description = 'x'", [])
            .is_err()
    );
    assert!(f.raw().execute("DELETE FROM ana_deviations", []).is_err());
}

#[test]
fn open_deviations_are_listed_oldest_first_until_closed() {
    let f = fixture();
    let first_run = f.open_run().run_id;
    let second_run = f.open_run().run_id;
    let mut opened = Vec::new();
    for run_id in [first_run, second_run] {
        let input = open(DeviationTarget::Run(run_id));
        opened.push(
            run(&f, STAFF, move |ctx| {
                lis_analytical::open_deviation(ctx, input)
            })
            .unwrap()
            .deviation_id,
        );
        f.clock.set(
            f.clock
                .now()
                .checked_add(std::time::Duration::from_secs(60))
                .unwrap(),
        );
    }
    let listed = |limit| {
        read(&f, move |conn| lis_analytical::open_deviations(conn, limit))
            .into_iter()
            .map(|deviation| deviation.deviation_id)
            .collect::<Vec<_>>()
    };
    assert_eq!(listed(10), opened);
    assert_eq!(listed(1), opened[..1]);
    let close = CloseDeviationInput {
        deviation_id: opened[0],
        expected_revision: 1,
        disposition: "Synthetic: recalibrated".to_owned(),
        reason: "Synthetic verification passed".to_owned(),
    };
    run(&f, STAFF, move |ctx| {
        lis_analytical::close_deviation(ctx, close)
    })
    .unwrap();
    assert_eq!(listed(10), opened[1..]);
}
