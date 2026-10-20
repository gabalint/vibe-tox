//! Reuse is limited to one read-only snapshot; later reads recheck clinical changes.
use crate::qc::{Setup, setup};
use lis_analytical::{DependencyRead, DependencySnapshot, InstrumentStatus, QcOutcome};
use lis_core::Clock;
use rusqlite::{Connection, OpenFlags};

fn readonly(s: &Setup) -> Connection {
    Connection::open_with_flags(
        s.f.dir.path().join(lis_store::DATABASE_FILE),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap()
}

fn assert_current_read(s: &Setup) -> Vec<String> {
    let mut conn = readonly(s);
    let tx = conn.transaction().unwrap();
    let mut reader = DependencyRead::new(&tx);
    let expected = lis_analytical::dependency_status(&tx, s.patient, s.analyte).unwrap();
    assert_eq!(reader.status(s.patient, s.analyte).unwrap(), expected);
    assert_eq!(reader.status(s.patient, s.analyte).unwrap(), expected);
    assert_eq!(
        reader.causal_records(s.patient, s.analyte).unwrap(),
        DependencySnapshot::causal_records(&tx, s.patient, s.analyte).unwrap()
    );
    expected
        .err()
        .unwrap_or_default()
        .into_iter()
        .map(|gate| gate.code)
        .collect()
}

#[test]
fn dependency_read_reuses_only_its_snapshot_and_next_read_sees_changed_qc() {
    let s = setup();
    s.observe("48", "F");
    let accepted = s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let mut conn = readonly(&s);
    let tx = conn.transaction().unwrap();
    let mut reader = DependencyRead::new(&tx);
    let before = reader.status(s.patient, s.analyte).unwrap();
    assert!(before.is_ok());
    let causal = reader.causal_records(s.patient, s.analyte).unwrap();
    // A real command commits on the writer while this WAL read snapshot remains open.
    s.observe("50", "C");
    assert_eq!(reader.status(s.patient, s.analyte).unwrap(), before);
    assert_eq!(reader.causal_records(s.patient, s.analyte).unwrap(), causal);
    drop(reader);
    tx.rollback().unwrap();
    assert_eq!(assert_current_read(&s), ["qc_stale"]);
    s.assess(s.input(QcOutcome::Accepted, Some(accepted.assessment_id)))
        .unwrap();
    assert!(assert_current_read(&s).is_empty());
    let tx = conn.transaction().unwrap();
    let records = DependencyRead::new(&tx)
        .causal_records(s.patient, s.analyte)
        .unwrap();
    assert_eq!(
        records
            .iter()
            .filter(|(code, _)| *code == "qc_superseded")
            .count(),
        2
    );
}

#[test]
fn fresh_dependency_reads_observe_instrument_and_method_safety_changes() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert!(assert_current_read(&s).is_empty());
    s.f.set_status(s.f.instrument, InstrumentStatus::OutOfService);
    assert_eq!(assert_current_read(&s), ["instrument_not_in_service"]);
    s.f.ready();
    assert!(assert_current_read(&s).is_empty());
    let orderable = s.f.urine.test;
    let version = crate::read(&s.f, move |conn| {
        Ok(lis_catalog::get_orderable_version(conn, orderable)?)
    });
    s.f.setup(move |ctx| {
        lis_catalog::retire_orderable_version(
            ctx,
            lis_catalog::RetireOrderableVersion {
                orderable_version_id: orderable,
                expected_revision: version.lifecycle.revision,
                reason: "Synthetic stop new orders before method safety suspension".to_owned(),
            },
        )
    });
    let method = s.f.urine.method;
    let version = crate::read(&s.f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, method)?)
    });
    s.f.setup(move |ctx| {
        lis_catalog::retire_method_version(
            ctx,
            lis_catalog::RetireMethodVersion {
                method_version_id: method,
                expected_revision: version.lifecycle.revision,
                reason: "Synthetic explicit safety suspension".to_owned(),
            },
        )
    });
    assert_eq!(assert_current_read(&s), ["method_not_current"]);
}

#[test]
fn position_memo_is_scoped_to_its_id_and_snapshot_before_execution_failure() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let (run_id, now) = (s.run_id, s.f.clock.now());
    let execution = crate::read(&s.f, move |conn| lis_analytical::get_run(conn, run_id, now))
        .positions
        .into_iter()
        .find(|position| position.position_id == s.patient)
        .unwrap()
        .execution
        .unwrap();
    let mut conn = readonly(&s);
    let tx = conn.transaction().unwrap();
    let mut reader = DependencyRead::new(&tx);
    // Alternate positions, then revisit the patient across distinct public reads.
    for position in [s.patient, s.control, s.patient] {
        assert_eq!(
            reader.status(position, s.analyte).unwrap(),
            lis_analytical::dependency_status(&tx, position, s.analyte).unwrap()
        );
        assert_eq!(
            reader.causal_records(position, s.analyte).unwrap(),
            DependencySnapshot::causal_records(&tx, position, s.analyte).unwrap()
        );
    }
    let before = reader.status(s.patient, s.analyte).unwrap();
    assert!(before.is_ok());
    crate::run(&s.f, crate::STAFF, move |ctx| {
        lis_analytical::fail_execution(
            ctx,
            lis_analytical::FailExecutionInput {
                execution_id: execution.execution_id,
                expected_revision: execution.revision,
                reason: "Synthetic failed execution after the read snapshot".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(reader.status(s.patient, s.analyte).unwrap(), before);
    drop(reader);
    tx.rollback().unwrap();
    assert_eq!(assert_current_read(&s), ["execution_failed"]);
}
