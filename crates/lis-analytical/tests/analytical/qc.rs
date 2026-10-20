//! QC assessments and the dependency gate (REQ-ANA-004, REQ-ANA-005,
//! REQ-ANA-007): every gate code of `dependency_status`, supersession with
//! history, and no waiver path.

use std::time::Duration;

use lis_analytical::{
    AnalyticalError, AssessQcInput, CloseRunInput, CompletePrepBatchInput, CreatePrepBatchInput,
    DeviationTarget, InstrumentStatus, MaterialKind, OpenDeviationInput, PositionId, PrepBatchId,
    PrepBatchView, QcAssessmentId, QcObservationId, QcOutcome, QcScope, QcStatus,
    RecordPrepStepInput, RevisionIndicator, RunId,
};
use lis_catalog::AnalyteId;
use lis_core::{Capability, Clock, ErrorCode, Principal};
use lis_interfaces::ReceiptId;

use crate::{Fixture, STAFF, code_of, fixture, observe, patient_input, read, run, run_as};

/// A run with a control at P1 and urine material at S1, a registration
/// with corrections, and a receipt for QC rows.
pub(crate) struct Setup {
    pub(crate) f: Fixture,
    pub(crate) run_id: RunId,
    pub(crate) control: PositionId,
    pub(crate) patient: PositionId,
    pub(crate) receipt: ReceiptId,
    pub(crate) analyte: AnalyteId,
}

pub(crate) fn setup() -> Setup {
    let f = fixture();
    f.register_source(RevisionIndicator::ResultStatusCorrection, false);
    let material = f.urine_material();
    let run_id = f.open_run().run_id;
    f.control(run_id, "P1");
    let detail = f.place(run_id, "S1", &material.label).unwrap();
    let receipt = f.receipt("qc");
    Setup {
        analyte: f.urine.analytes[0],
        control: detail.positions[0].position_id,
        patient: detail.positions[1].position_id,
        f,
        run_id,
        receipt,
    }
}

impl Setup {
    pub(crate) fn observe(&self, value: &str, status: &str) {
        observe(&self.f, self.control, self.receipt, "A1", value, status).unwrap();
    }

    pub(crate) fn current_observations(&self) -> Vec<QcObservationId> {
        let run_id = self.run_id;
        let analyte = self.analyte;
        read(&self.f, move |conn| {
            lis_analytical::get_run_qc(conn, run_id)
        })
        .analytes
        .into_iter()
        .find(|a| a.analyte_id == analyte)
        .unwrap()
        .observations
        .into_iter()
        .filter(|o| !o.superseded)
        .map(|o| o.observation_id)
        .collect()
    }

    pub(crate) fn input(
        &self,
        outcome: QcOutcome,
        supersedes: Option<QcAssessmentId>,
    ) -> AssessQcInput {
        AssessQcInput {
            scope: QcScope::RunAnalyte,
            run_id: Some(self.run_id),
            analyte_id: Some(self.analyte),
            prep_batch_id: None,
            outcome,
            evidence_observation_ids: self.current_observations(),
            supersedes_assessment_id: supersedes,
            reason: supersedes.map(|_| "Synthetic reassessment".to_owned()),
        }
    }

    pub(crate) fn assess(
        &self,
        input: AssessQcInput,
    ) -> Result<lis_analytical::QcAssessmentView, AnalyticalError> {
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::assess_qc(ctx, input)
        })
    }

    /// The QC work-queue status of this run × analyte, if it is queued.
    pub(crate) fn queued(&self) -> Option<QcStatus> {
        let (run_id, analyte) = (self.run_id, self.analyte);
        read(&self.f, |conn| lis_analytical::qc_work_queue(conn, 50))
            .into_iter()
            .find(|item| item.run_id == run_id && item.analyte_id == analyte)
            .map(|item| item.status)
    }

    pub(crate) fn gates(&self, position: PositionId) -> Vec<String> {
        let analyte = self.analyte;
        match read(&self.f, move |conn| {
            lis_analytical::dependency_status(conn, position, analyte)
        }) {
            Ok(_) => Vec::new(),
            Err(gates) => gates.into_iter().map(|g| g.code).collect(),
        }
    }
}

#[test]
fn qc_is_pending_until_assessed_and_the_snapshot_names_the_assessment() {
    let s = setup();
    assert_eq!(s.gates(s.patient), ["qc_pending"]);
    s.observe("48", "F");
    let accepted = s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert_eq!(accepted.evidence_observation_ids.len(), 1);
    let (patient, analyte) = (s.patient, s.analyte);
    let snapshot = read(&s.f, move |conn| {
        lis_analytical::dependency_status(conn, patient, analyte)
    })
    .unwrap();
    assert_eq!(snapshot.qc_assessment_id, accepted.assessment_id);
    assert_eq!(snapshot.prep_assessment_id, None);
}

#[test]
fn acceptance_needs_received_observations_and_exactly_the_current_ones() {
    let s = setup();
    let empty = s.assess(s.input(QcOutcome::Accepted, None));
    assert_eq!(code_of(empty), ErrorCode::Validation);
    s.observe("48", "F");
    let mut partial = s.input(QcOutcome::Accepted, None);
    partial.evidence_observation_ids.clear();
    assert!(matches!(
        s.assess(partial),
        Err(AnalyticalError::EvidenceChanged)
    ));
    let mut wrong_scope = s.input(QcOutcome::Accepted, None);
    wrong_scope.prep_batch_id = Some(lis_analytical::PrepBatchId::generate());
    assert_eq!(code_of(s.assess(wrong_scope)), ErrorCode::Validation);
    let system = Principal::system("test-worker", [Capability::ManageQuality]);
    let input = s.input(QcOutcome::Accepted, None);
    let by_system = run_as(&s.f, system, move |ctx| {
        lis_analytical::assess_qc(ctx, input)
    });
    assert!(matches!(by_system, Err(AnalyticalError::UserRequired)));
}

#[test]
fn a_new_observation_makes_the_assessment_stale_until_reassessed() {
    let s = setup();
    s.observe("48", "F");
    let first = s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    s.observe("50", "C");
    assert_eq!(s.gates(s.patient), ["qc_stale"]);
    let not_current = s.assess(s.input(QcOutcome::Accepted, None));
    assert_eq!(code_of(not_current), ErrorCode::Conflict);
    let mut no_reason = s.input(QcOutcome::Accepted, Some(first.assessment_id));
    no_reason.reason = None;
    assert_eq!(code_of(s.assess(no_reason)), ErrorCode::Validation);
    let second = s
        .assess(s.input(QcOutcome::Accepted, Some(first.assessment_id)))
        .unwrap();
    assert!(s.gates(s.patient).is_empty());
    let (run_id, analyte) = (s.run_id, s.analyte);
    let qc = read(&s.f, move |conn| lis_analytical::get_run_qc(conn, run_id));
    let history = &qc
        .analytes
        .iter()
        .find(|a| a.analyte_id == analyte)
        .unwrap()
        .assessments;
    assert_eq!(
        history.current.as_ref().unwrap().assessment_id,
        second.assessment_id
    );
    let ids: Vec<QcAssessmentId> = history.history.iter().map(|a| a.assessment_id).collect();
    assert_eq!(ids, [second.assessment_id, first.assessment_id]);
    assert_eq!(history.history[0].supersedes_id, Some(first.assessment_id));
}

#[test]
fn a_rejection_blocks_and_is_never_relabeled_as_accepted() {
    let s = setup();
    s.observe("48", "F");
    let rejected = s.assess(s.input(QcOutcome::Rejected, None)).unwrap();
    assert_eq!(s.gates(s.patient), ["qc_rejected"]);
    let waiver = s.assess(s.input(QcOutcome::Accepted, Some(rejected.assessment_id)));
    assert!(
        matches!(waiver, Err(AnalyticalError::RejectionFinal)),
        "{waiver:?}"
    );
    // The trigger refuses the same waiver below the command.
    let raw = s.f.raw().execute(
        "INSERT INTO ana_qc_assessments (id, scope, run_id, analyte_id, outcome, supersedes_id, \
         reason, assessed_by, assessed_at) SELECT 'x', scope, run_id, analyte_id, 'accepted', id, \
         'r', 'u', 't' FROM ana_qc_assessments",
        [],
    );
    assert!(raw.is_err());
    assert!(
        s.f.raw()
            .execute("UPDATE ana_qc_assessments SET outcome = 'accepted'", [])
            .is_err()
    );
    assert!(
        s.f.raw()
            .execute("DELETE FROM ana_qc_assessments", [])
            .is_err()
    );
    assert_eq!(s.gates(s.patient), ["qc_rejected"]);
}

#[test]
fn a_rejected_run_blocks_every_analyte() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let input = AssessQcInput {
        scope: QcScope::Run,
        run_id: Some(s.run_id),
        analyte_id: None,
        prep_batch_id: None,
        outcome: QcOutcome::Rejected,
        evidence_observation_ids: Vec::new(),
        supersedes_assessment_id: None,
        reason: Some("Synthetic calibration failure".to_owned()),
    };
    s.assess(input).unwrap();
    assert_eq!(s.gates(s.patient), ["qc_rejected"]);
}

#[test]
fn a_preparation_batch_is_pending_until_accepted() {
    let s = setup();
    let batch = s.prep_batch();
    let position = s.place_prepared(batch, "S2");
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert_eq!(s.gates(position), ["prep_not_completed", "prep_pending"]);
    s.complete(batch);
    assert_eq!(s.gates(position), ["prep_pending"]);
    let rejected = s.assess(prep_input(batch, QcOutcome::Rejected)).unwrap();
    assert_eq!(s.gates(position), ["prep_rejected"]);
    assert!(
        s.gates(s.patient).is_empty(),
        "other material is unaffected"
    );
    let mut accept = prep_input(batch, QcOutcome::Accepted);
    accept.supersedes_assessment_id = Some(rejected.assessment_id);
    accept.reason = Some("Synthetic".to_owned());
    assert!(matches!(
        s.assess(accept),
        Err(AnalyticalError::RejectionFinal)
    ));
}

/// A preparation-batch assessment without cited observations.
pub(crate) fn prep_input(batch: PrepBatchId, outcome: QcOutcome) -> AssessQcInput {
    AssessQcInput {
        scope: QcScope::PrepBatch,
        run_id: None,
        analyte_id: None,
        prep_batch_id: Some(batch),
        outcome,
        evidence_observation_ids: Vec::new(),
        supersedes_assessment_id: None,
        reason: None,
    }
}

impl Setup {
    /// An open preparation batch for the urine method.
    pub(crate) fn prep_batch(&self) -> PrepBatchId {
        let method = self.f.urine.method;
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::create_prep_batch(
                ctx,
                CreatePrepBatchInput {
                    method_version_id: method,
                },
            )
        })
        .unwrap()
        .prep_batch_id
    }

    /// Records one step on `batch` and completes it.
    pub(crate) fn complete(&self, batch: PrepBatchId) -> PrepBatchView {
        let step = RecordPrepStepInput {
            prep_batch_id: batch,
            step_code: "extract".to_owned(),
            lot_id: None,
            note: None,
        };
        let open = run(&self.f, STAFF, move |ctx| {
            lis_analytical::record_prep_step(ctx, step)
        })
        .unwrap();
        let input = CompletePrepBatchInput {
            prep_batch_id: batch,
            expected_revision: open.revision,
        };
        run(&self.f, STAFF, move |ctx| {
            lis_analytical::complete_prep_batch(ctx, input)
        })
        .unwrap()
    }

    /// Places new urine material prepared in `batch` at `label`; returns the position.
    pub(crate) fn place_prepared(&self, batch: PrepBatchId, label: &str) -> PositionId {
        let material = self.f.urine_material();
        let mut input = patient_input(self.run_id, label, &material.label);
        input.prep_batch_id = Some(batch);
        let detail = run(&self.f, STAFF, move |ctx| {
            lis_analytical::add_patient_position(ctx, input)
        })
        .unwrap();
        detail
            .positions
            .iter()
            .find(|position| position.label == label)
            .unwrap()
            .position_id
    }
}

#[test]
fn an_open_deviation_blocks_its_scope_until_closed() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    let open = |scope, analyte| OpenDeviationInput {
        scope,
        analyte_id: analyte,
        description: "Synthetic temperature drift".to_owned(),
        reason: "Synthetic investigation".to_owned(),
    };
    let other_analyte = open(DeviationTarget::Run(s.run_id), Some(s.f.urine.analytes[1]));
    run(&s.f, STAFF, move |ctx| {
        lis_analytical::open_deviation(ctx, other_analyte)
    })
    .unwrap();
    assert!(
        s.gates(s.patient).is_empty(),
        "a deviation for another analyte does not block"
    );
    let instrument = open(DeviationTarget::Instrument(s.f.instrument), None);
    let deviation = run(&s.f, STAFF, move |ctx| {
        lis_analytical::open_deviation(ctx, instrument)
    })
    .unwrap();
    assert_eq!(s.gates(s.patient), ["deviation_open"]);
    let close = lis_analytical::CloseDeviationInput {
        deviation_id: deviation.deviation_id,
        expected_revision: deviation.revision,
        disposition: "Synthetic: no impact".to_owned(),
        reason: "Synthetic review complete".to_owned(),
    };
    run(&s.f, STAFF, move |ctx| {
        lis_analytical::close_deviation(ctx, close)
    })
    .unwrap();
    assert!(s.gates(s.patient).is_empty());
}

#[test]
fn readiness_and_method_status_are_rechecked_at_review() {
    let s = setup();
    s.observe("48", "F");
    s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    s.f.set_status(s.f.instrument, InstrumentStatus::OutOfService);
    assert_eq!(s.gates(s.patient), ["instrument_not_in_service"]);
    s.f.set_status(s.f.instrument, InstrumentStatus::InService);
    assert!(s.gates(s.patient).is_empty());
}

#[test]
fn a_retired_method_is_not_current_at_review() {
    let s = setup();
    let urine = s.f.urine.method;
    let specimen_type = read(&s.f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, urine)?.specimen_type_id)
    });
    let method = crate::approved_method(&s.f.db, specimen_type, "syn-ur-r", s.f.urine.analytes);
    let input = lis_analytical::CreateRunInput {
        instrument_id: s.f.instrument,
        method_version_id: method,
        lot_ids: Vec::new(),
    };
    let run_id = run(&s.f, STAFF, move |ctx| {
        lis_analytical::create_run(ctx, input)
    })
    .unwrap()
    .run_id;
    let control = s.f.control(run_id, "P1").0.positions[0].position_id;
    let revision = read(&s.f, move |conn| {
        Ok(lis_catalog::get_method_version(conn, method)?
            .lifecycle
            .revision)
    });
    s.f.setup(move |ctx| {
        lis_catalog::retire_method_version(
            ctx,
            lis_catalog::RetireMethodVersion {
                method_version_id: method,
                expected_revision: revision,
                reason: "Synthetic retirement".to_owned(),
            },
        )
    });
    assert_eq!(s.gates(control), ["qc_pending", "method_not_current"]);
}

#[test]
fn a_lot_that_expired_before_the_material_was_placed_blocks() {
    let f = fixture();
    f.ready();
    // The fixed clock starts on 2026-01-02; this lot is usable through that day.
    let lot = f.lot(MaterialKind::Reagent, "R-TODAY", "2026-01-02");
    let input = lis_analytical::CreateRunInput {
        instrument_id: f.instrument,
        method_version_id: f.urine.method,
        lot_ids: vec![lot],
    };
    let run_id = run(&f, STAFF, move |ctx| lis_analytical::create_run(ctx, input))
        .unwrap()
        .run_id;
    let on_time = f.place(run_id, "S1", &f.urine_material().label).unwrap();
    f.clock.set(
        f.clock
            .now()
            .checked_add(Duration::from_secs(86_400))
            .unwrap(),
    );
    let late = f.place(run_id, "S2", &f.urine_material().label).unwrap();
    let analyte = f.urine.analytes[0];
    let gates = |position: PositionId| match read(&f, move |conn| {
        lis_analytical::dependency_status(conn, position, analyte)
    }) {
        Ok(_) => Vec::new(),
        Err(gates) => gates.into_iter().map(|g| g.code).collect::<Vec<_>>(),
    };
    assert_eq!(gates(on_time.positions[0].position_id), ["qc_pending"]);
    assert_eq!(
        gates(late.positions[1].position_id),
        ["qc_pending", "lot_expired"]
    );
}

#[test]
fn the_qc_work_queue_follows_the_review_gate_status() {
    let s = setup();
    // No observation yet: nothing to assess.
    assert_eq!(s.queued(), None);
    s.observe("48", "F");
    assert_eq!(s.queued(), Some(QcStatus::Pending));
    let first = s.assess(s.input(QcOutcome::Accepted, None)).unwrap();
    assert_eq!(s.queued(), None);
    s.observe("50", "C");
    assert_eq!(s.queued(), Some(QcStatus::Stale));
    assert_eq!(s.gates(s.patient), ["qc_stale"]);
    let rejected = s
        .assess(s.input(QcOutcome::Rejected, Some(first.assessment_id)))
        .unwrap();
    assert_eq!(s.queued(), Some(QcStatus::Rejected));
    let since = read(&s.f, |conn| lis_analytical::qc_work_queue(conn, 1))[0].since;
    assert_eq!(since, rejected.assessed_at);
    // A rejection leaves the queue once its run is closed.
    let close = CloseRunInput {
        run_id: s.run_id,
        expected_revision: 1,
        reason: None,
    };
    run(&s.f, STAFF, move |ctx| {
        lis_analytical::close_run(ctx, close)
    })
    .unwrap();
    assert_eq!(s.queued(), None);
}

#[test]
fn the_qc_work_queue_is_oldest_first_and_limited() {
    let s = setup();
    s.observe("48", "F");
    let later = s.f.open_run().run_id;
    s.f.clock.set(
        s.f.clock
            .now()
            .checked_add(Duration::from_secs(60))
            .unwrap(),
    );
    let (control, _) = s.f.control(later, "P1");
    let control = control.positions[0].position_id;
    observe(&s.f, control, s.receipt, "A1", "47", "F").unwrap();
    let queue = read(&s.f, |conn| lis_analytical::qc_work_queue(conn, 10));
    let runs: Vec<RunId> = queue.iter().map(|item| item.run_id).collect();
    assert_eq!(runs, [s.run_id, later]);
    assert!(queue[0].since < queue[1].since);
    assert_eq!(
        read(&s.f, |conn| lis_analytical::qc_work_queue(conn, 1)).len(),
        1
    );
}
