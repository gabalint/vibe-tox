//! Regression tests for the M2b review of the dependency gate (REQ-ANA-001,
//! REQ-ANA-003–007): exact QC evidence, failed executions, deviations opened
//! after a review, the preparation-batch lifecycle, the instrument status
//! order under a backward clock, and QC re-export conflicts.

use std::time::Duration;

use lis_analytical::reexport::RowDecision;
use lis_analytical::{
    AnalyticalError, ApproveSourceInput, CloseDeviationInput, CloseRunInput,
    CompletePrepBatchInput, DependencySnapshot, DeviationTarget, FailExecutionInput,
    InstrumentStatus, OpenDeviationInput, PositionId, PrepBatchState, QcConflictResolution,
    QcConflictState, QcConflictView, QcOutcome, QcStatus, RecordPrepStepInput, RegisterSourceInput,
    ResolveQcConflictInput, RevisionIndicator, SampleIdField, SourceTestCodeInput,
};
use lis_core::{Capability, Clock, ErrorCode, Principal, UtcTimestamp};

use crate::qc::{Setup, prep_input, setup};
use crate::{STAFF, code_of, gate_codes, observe, read, run, run_as};

fn later(at: UtcTimestamp, seconds: u64) -> UtcTimestamp {
    at.checked_add(Duration::from_secs(seconds)).unwrap()
}

impl Setup {
    fn snapshot(&self, position: PositionId) -> DependencySnapshot {
        let analyte = self.analyte;
        read(&self.f, move |conn| {
            lis_analytical::dependency_status(conn, position, analyte)
        })
        .unwrap()
    }

    fn intake(&self, position: PositionId) -> Result<Vec<String>, AnalyticalError> {
        let found = self
            .f
            .db
            .read_blocking(move |conn| lis_analytical::patient_intake_gate(conn, position))?;
        Ok(found
            .err()
            .unwrap_or_default()
            .into_iter()
            .map(|g| g.code)
            .collect())
    }

    /// Opens a deviation and closes it again.
    fn deviation(&self, scope: DeviationTarget, analyte: Option<lis_catalog::AnalyteId>) {
        let input = OpenDeviationInput {
            scope,
            analyte_id: analyte,
            description: "Synthetic temperature drift".to_owned(),
            reason: "Synthetic investigation".to_owned(),
        };
        let opened = run(&self.f, STAFF, move |ctx| {
            lis_analytical::open_deviation(ctx, input)
        })
        .unwrap();
        let close = CloseDeviationInput {
            deviation_id: opened.deviation_id,
            expected_revision: opened.revision,
            disposition: "Synthetic: no impact".to_owned(),
            reason: "Synthetic review complete".to_owned(),
        };
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::close_deviation(ctx, close)
        })
        .unwrap();
    }

    /// Approves a new registration mapping `A1` to the second urine analyte
    /// and `B1` to the first, with the correction indicator.
    fn remap(&self) {
        let input = RegisterSourceInput {
            au_profile: None,
            instrument_id: self.f.instrument,
            sender_name: "SYNTHETIC".to_owned(),
            sender_version: "1.1".to_owned(),
            sample_id_field: SampleIdField::SpecimenId,
            revision_indicator: RevisionIndicator::ResultStatusCorrection,
            reports_final_concentration: false,
            test_codes: [("A1", 1), ("B1", 0)]
                .map(|(code, analyte)| SourceTestCodeInput {
                    source_test_code: code.to_owned(),
                    analyte_id: self.f.urine.analytes[analyte],
                })
                .to_vec(),
        };
        let draft = run(&self.f, STAFF, move |ctx| {
            lis_analytical::register_source(ctx, input)
        })
        .unwrap();
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::approve_source(
                ctx,
                ApproveSourceInput {
                    registration_id: draft.registration_id,
                    expected_revision: draft.revision,
                    self_approval: false,
                },
            )
        })
        .unwrap();
    }

    fn conflicts(&self) -> Vec<QcConflictView> {
        let run_id = self.run_id;
        read(&self.f, move |conn| {
            lis_analytical::get_run_qc(conn, run_id)
        })
        .conflicts
    }

    fn resolve(
        &self,
        conflict: &QcConflictView,
        resolution: QcConflictResolution,
    ) -> Result<QcConflictView, AnalyticalError> {
        let input = ResolveQcConflictInput {
            conflict_id: conflict.conflict_id,
            expected_revision: conflict.revision,
            resolution,
            reason: "Synthetic analyzer log reviewed".to_owned(),
        };
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::resolve_qc_conflict(ctx, input)
        })
    }
}

/// #5: the assessment is current only while its cited observations are
/// exactly the current ones of the run × analyte.
#[test]
fn a_cited_observation_corrected_into_another_analyte_makes_the_assessment_stale() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert!(s.gates(s.patient).is_empty());
    // A1 now maps to the other analyte; the corrected A1 row supersedes the
    // cited observation and leaves this analyte with no current observation.
    s.remap();
    s.observe("50", "C");
    assert!(s.current_observations().is_empty());
    assert_eq!(s.gates(s.patient), ["qc_stale"]);
    assert_eq!(s.queued(), Some(QcStatus::Stale));
}

/// #6: a failed execution blocks review and refuses new patient evidence.
#[test]
fn a_failed_execution_blocks_review_and_refuses_new_patient_evidence() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert!(s.gates(s.patient).is_empty());
    assert_eq!(s.intake(s.patient).unwrap(), Vec::<String>::new());
    let (run_id, now) = (s.run_id, s.f.clock.now());
    let detail = read(&s.f, move |conn| lis_analytical::get_run(conn, run_id, now));
    let execution = detail.positions[1].execution.clone().unwrap();
    let input = FailExecutionInput {
        execution_id: execution.execution_id,
        expected_revision: execution.revision,
        reason: "Synthetic injection failure".to_owned(),
    };
    run(&s.f, STAFF, move |ctx| {
        lis_analytical::fail_execution(ctx, input)
    })
    .unwrap();
    assert_eq!(s.gates(s.patient), ["execution_failed"]);
    assert_eq!(s.intake(s.patient).unwrap(), ["execution_failed"]);
    let close = CloseRunInput {
        run_id,
        expected_revision: 1,
        reason: None,
    };
    run(&s.f, STAFF, move |ctx| {
        lis_analytical::close_run(ctx, close)
    })
    .unwrap();
    assert_eq!(
        s.intake(s.patient).unwrap(),
        ["run_closed", "execution_failed"]
    );
    assert!(matches!(
        s.intake(s.control),
        Err(AnalyticalError::WrongState("position"))
    ));
}

/// #7: the snapshot's deviation watermark rises with every applicable
/// deviation, open or closed, whatever the host clock says.
#[test]
fn a_deviation_opened_after_a_review_raises_the_watermark_for_good() {
    let s = setup();
    let start = s.f.clock.now();
    s.f.clock.set(later(start, 86_400));
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let reviewed = s.snapshot(s.patient);
    assert_eq!(reviewed.deviation_watermark, 0);
    // A deviation limited to another analyte does not apply.
    s.deviation(DeviationTarget::Run(s.run_id), Some(s.f.urine.analytes[1]));
    assert_eq!(s.snapshot(s.patient), reviewed);
    s.deviation(DeviationTarget::Instrument(s.f.instrument), None);
    let after = s.snapshot(s.patient);
    assert_eq!(after.qc_assessment_id, reviewed.qc_assessment_id);
    assert!(after.deviation_watermark > reviewed.deviation_watermark);
    // Opened with the host clock set back: still later in sequence.
    s.f.clock.set(start);
    s.deviation(DeviationTarget::Position(s.patient), None);
    assert!(s.snapshot(s.patient).deviation_watermark > after.deviation_watermark);
}

/// #9: a preparation batch is completed once, with steps, and then frozen.
#[test]
fn a_preparation_batch_is_completed_once_and_then_frozen() {
    let s = setup();
    let batch = s.prep_batch();
    let early = CompletePrepBatchInput {
        prep_batch_id: batch,
        expected_revision: 1,
    };
    let empty = run(&s.f, STAFF, move |ctx| {
        lis_analytical::complete_prep_batch(ctx, early)
    });
    assert_eq!(gate_codes(empty), ["prep_no_steps"]);
    let completed = s.complete(batch);
    assert_eq!(completed.state, PrepBatchState::Completed);
    assert_eq!((completed.revision, completed.steps.len()), (3, 1));
    let late = RecordPrepStepInput {
        prep_batch_id: batch,
        step_code: "spike".to_owned(),
        lot_id: None,
        note: None,
    };
    let appended = run(&s.f, STAFF, move |ctx| {
        lis_analytical::record_prep_step(ctx, late)
    });
    assert!(matches!(
        appended,
        Err(AnalyticalError::WrongState("preparation batch"))
    ));
    // The triggers refuse the same below the commands.
    let raw = s.f.raw();
    assert!(
        raw.execute(
            "INSERT INTO ana_prep_steps (id, batch_id, seq, step_code, performed_by, \
             performed_at) VALUES ('x', ?1, 9, 's', 'u', 't')",
            [batch.to_string()],
        )
        .is_err()
    );
    assert!(
        raw.execute("UPDATE ana_prep_batches SET revision = revision + 1", [])
            .is_err()
    );
}

/// #9: only a completed batch is accepted, on its steps, and the
/// assessment records the revision assessed.
#[test]
fn only_a_completed_batch_is_accepted_and_its_revision_is_recorded() {
    let s = setup();
    let batch = s.prep_batch();
    let position = s.place_prepared(batch, "S2");
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let open = s.assess(prep_input(batch, QcOutcome::Accepted));
    assert_eq!(gate_codes(open), ["prep_not_completed"]);
    let completed = s.complete(batch);
    let mut citing = prep_input(batch, QcOutcome::Accepted);
    citing.evidence_observation_ids = s.current_observations();
    assert_eq!(code_of(s.assess(citing)), ErrorCode::Validation);
    let accepted = s.assess(prep_input(batch, QcOutcome::Accepted)).unwrap();
    assert!(s.gates(position).is_empty());
    assert_eq!(
        s.snapshot(position).prep_assessment_id,
        Some(accepted.assessment_id)
    );
    let recorded: i64 =
        s.f.raw()
            .query_row(
                "SELECT prep_batch_revision FROM ana_qc_assessments WHERE id = ?1",
                [accepted.assessment_id.to_string()],
                |row| row.get(0),
            )
            .unwrap();
    assert_eq!(recorded, completed.revision);
}

/// #10: the latest status is the last one recorded, even when the host
/// clock moved backward in between.
#[test]
fn a_backward_clock_change_never_hides_the_latest_status_event() {
    let s = setup();
    let start = s.f.clock.now();
    s.f.clock.set(later(start, 3_600));
    s.f.set_status(s.f.instrument, InstrumentStatus::InService);
    s.f.clock.set(start);
    s.f.set_status(s.f.instrument, InstrumentStatus::OutOfService);
    let listed = read(&s.f, lis_analytical::list_instrument_readiness);
    assert_eq!(listed[0].status, Some(InstrumentStatus::OutOfService));
    assert!(
        s.gates(s.patient)
            .contains(&"instrument_not_in_service".to_owned())
    );
    // The trigger refuses a sequence number other than the next one.
    let raw = s.f.raw().execute(
        "INSERT INTO ana_instrument_status_events (id, instrument_id, status, reason, \
         recorded_by, recorded_at, seq) SELECT 'x', instrument_id, 'in_service', 'r', 'u', \
         't', 1 FROM ana_instrument_status_events LIMIT 1",
        [],
    );
    assert!(raw.is_err());
}

/// #4: a changed QC row without a declared correction is kept against its
/// run × analyte, blocks until a person decides, and an acceptance appends
/// evidence that makes the assessment stale.
#[test]
fn a_conflicting_qc_row_blocks_until_resolved() {
    let s = setup();
    s.observe("48", "F");
    let first = s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let changed = observe(&s.f, s.control, s.receipt, "A1", "50", "F").unwrap();
    assert_eq!(changed, RowDecision::Conflict);
    assert_eq!(s.gates(s.patient), ["qc_conflict"]);
    assert_eq!(s.queued(), Some(QcStatus::Stale));
    // The same row again is not recorded twice.
    observe(&s.f, s.control, s.receipt, "A1", "50", "F").unwrap();
    let conflicts = s.conflicts();
    assert_eq!(conflicts.len(), 1);
    // Resolution is a person's decision.
    let system = Principal::system("test-worker", [Capability::ManageQuality]);
    let input = ResolveQcConflictInput {
        conflict_id: conflicts[0].conflict_id,
        expected_revision: 1,
        resolution: QcConflictResolution::Reject,
        reason: "Synthetic".to_owned(),
    };
    let by_system = run_as(&s.f, system, move |ctx| {
        lis_analytical::resolve_qc_conflict(ctx, input)
    });
    assert!(matches!(by_system, Err(AnalyticalError::UserRequired)));
    let rejected = s
        .resolve(&conflicts[0], QcConflictResolution::Reject)
        .unwrap();
    assert_eq!(rejected.state, QcConflictState::Rejected);
    assert!(
        s.gates(s.patient).is_empty(),
        "the stored observation stays"
    );
    // A second changed row, accepted as a correction.
    observe(&s.f, s.control, s.receipt, "A1", "51", "F").unwrap();
    let open = s.conflicts().remove(0);
    assert_eq!(open.state, QcConflictState::Open);
    let accepted = s
        .resolve(&open, QcConflictResolution::AcceptAsCorrection)
        .unwrap();
    assert_eq!(accepted.state, QcConflictState::Accepted);
    assert_eq!(s.current_observations(), [accepted.observation_id.unwrap()]);
    assert_eq!(s.gates(s.patient), ["qc_stale"]);
    s.assess(s.input(QcOutcome::Accepted, Some(first.assessment_id)))
        .unwrap();
    assert!(s.gates(s.patient).is_empty());
    assert_eq!(
        code_of(s.resolve(&accepted, QcConflictResolution::Reject)),
        ErrorCode::Conflict
    );
}

/// #4: accepting a conflict whose contradicted observation is no longer
/// current is refused; the person must look again.
#[test]
fn a_conflict_is_accepted_only_against_the_observation_it_contradicted() {
    let s = setup();
    s.observe("48", "F");
    observe(&s.f, s.control, s.receipt, "A1", "50", "F").unwrap();
    observe(&s.f, s.control, s.receipt, "A1", "52", "F").unwrap();
    let conflicts = s.conflicts();
    assert_eq!(conflicts.len(), 2);
    s.resolve(&conflicts[0], QcConflictResolution::AcceptAsCorrection)
        .unwrap();
    assert!(matches!(
        s.resolve(&conflicts[1], QcConflictResolution::AcceptAsCorrection),
        Err(AnalyticalError::EvidenceChanged)
    ));
    s.resolve(&conflicts[1], QcConflictResolution::Reject)
        .unwrap();
    assert!(
        s.conflicts()
            .iter()
            .all(|conflict| conflict.state != QcConflictState::Open)
    );
}
