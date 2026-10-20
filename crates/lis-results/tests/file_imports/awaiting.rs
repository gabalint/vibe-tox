//! WFREV-01: expected work is durable per frozen analyte, not first result or run page.
use crate::rows::row;
use crate::*;
use lis_results::{RecordResultDispositionInput, ResultOutcome};

fn pending(f: &Fixture, execution: ExecutionId) -> bool {
    f.read(move |conn| {
        Ok(lis_results::awaiting_results(conn, 500)?
            .iter()
            .any(|row| row.execution_id == execution))
    })
}

#[test]
fn partial_duplicate_restart_and_closed_run_preserve_unresolved_analytes() {
    let f = fixture();
    let sample = f.sample();
    let partial = row(&sample.label, "Unknown", "Morphine", "30.00", UNIT, "1");
    assert_eq!(f.import(std::slice::from_ref(&partial)).rows_new, 1);
    assert!(pending(&f, sample.execution_id));
    assert_eq!(f.import(&[partial]).rows_duplicate, 1);
    assert!(pending(&f, sample.execution_id));
    let f = Fixture {
        t: f.t.reopen().unwrap(),
        ..f
    };
    assert!(
        pending(&f, sample.execution_id),
        "restart reconstructs the task"
    );
    f.close_run(f.run);
    assert!(
        pending(&f, sample.execution_id),
        "run closure is not a disposition"
    );
    assert_eq!(
        f.measurements(&sample, f.analytes.mor)[0].1.as_deref(),
        Some("30.00")
    );
}

#[test]
fn missing_is_unresolved_until_a_reasoned_terminal_disposition() {
    let f = fixture();
    let sample = f.sample();
    f.import(&[row(&sample.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    let specimen = sample.specimen_id;
    let pending_analyte = f
        .results(&sample)
        .pending_analytes
        .into_iter()
        .find(|a| a.analyte_id == f.analytes.oxy)
        .unwrap();
    let missing = RecordResultDispositionInput {
        result_id: None,
        expected_revision: 0,
        component_id: Some(pending_analyte.component_id),
        analyte_id: Some(pending_analyte.analyte_id),
        specimen_id: Some(specimen),
        outcome: ResultOutcome::Missing,
        reason: "Synthetic missing observation investigated".to_owned(),
    };
    f.setup(|ctx| lis_results::record_result_disposition(ctx, missing));
    assert!(
        pending(&f, sample.execution_id),
        "missing retains the intake task"
    );
    let line = f
        .results(&sample)
        .lines
        .into_iter()
        .find(|line| line.analyte_id == f.analytes.oxy)
        .unwrap();
    let not_performed = RecordResultDispositionInput {
        result_id: Some(line.result_id),
        expected_revision: line.revision,
        component_id: None,
        analyte_id: None,
        specimen_id: None,
        outcome: ResultOutcome::NotPerformed,
        reason: "Synthetic authorized terminal nonperformance".to_owned(),
    };
    f.setup(|ctx| lis_results::record_result_disposition(ctx, not_performed));
    assert!(!pending(&f, sample.execution_id));
    assert_eq!(f.measurements(&sample, f.analytes.mor).len(), 1);
}

#[test]
fn failed_execution_ends_its_task_and_repeat_creates_its_own_expected_work() {
    let f = fixture();
    let sample = f.sample();
    f.import(&[row(&sample.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    let failed = sample.execution_id;
    let input = lis_analytical::FailExecutionInput {
        execution_id: failed,
        expected_revision: 1,
        reason: "Synthetic interrupted injection".to_owned(),
    };
    f.setup(|ctx| lis_analytical::fail_execution(ctx, input));
    assert!(!pending(&f, failed));
    f.close_run(f.run);
    let run = f.open_run(false);
    let repeat = f.place(sample, run, AttemptKind::Rerun, None);
    assert!(pending(&f, repeat.execution_id));
    assert_eq!(
        f.import(&[
            row(&repeat.label, "Unknown", "Morphine", "31", UNIT, "1"),
            row(&repeat.label, "Unknown", "Oxycodone", "15", UNIT, "1"),
        ])
        .rows_new,
        2
    );
    assert!(!pending(&f, repeat.execution_id));
    assert_eq!(
        f.measurements(&repeat, f.analytes.mor).len(),
        2,
        "original observation remains"
    );
}

#[test]
fn pending_beyond_five_hundred_newer_runs_is_still_reachable() {
    let f = fixture();
    let sample = f.sample();
    f.close_run(f.run);
    for _ in 0..501 {
        let run = f.open_run(false);
        f.close_run(run);
    }
    assert!(pending(&f, sample.execution_id));
}
