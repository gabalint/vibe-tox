//! M2 import quarantine through the engine (REQ-ANA-003, REQ-RES-001,
//! REQ-RES-005, NFR-01): every row the importer cannot place is kept as a
//! quarantine item with its reason and the received text, never as patient
//! evidence, and the rest of the message still imports. Expected outcomes
//! from `docs/planning/m2-design-brief.md` §3.2–§3.5 and
//! `fixtures/astm/m2/README.md`. Synthetic data only.

use lis_analytical::{AttemptKind, RevisionIndicator};
use lis_core::ErrorCode;
use lis_engine::{
    AddPatientPositionInput, CreateRunInput, GetImportAttemptInput, GetRunInput,
    ImportAttemptDetail, ImportAttemptSummary, ListImportAttemptsInput, QuarantineResolution,
    ResolveQuarantineItemInput,
};
use lis_results::{ImportOutcome, QuarantineReason, QuarantineState, ResultCause};

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{
    CORRECTION, JOURNEY, MALFORMED_VALUE, UNIT_MISMATCH, UNKNOWN_SAMPLE, gate_codes, line, message,
};

impl Lab {
    /// The audited attempt detail with its quarantined rows.
    async fn detail(&self, attempt: &ImportAttemptSummary) -> ImportAttemptDetail {
        self.sign_in(Role::Reviewer).await;
        self.engine()
            .get_import_attempt(GetImportAttemptInput {
                import_attempt_id: attempt.import_attempt_id,
            })
            .await
            .unwrap()
    }

    /// The analytes of sample `index` still waiting for a result, by code.
    async fn pending(&self, index: usize) -> Vec<String> {
        let results = self.results(index).await;
        results
            .pending_analytes
            .into_iter()
            .map(|p| p.analyte_code)
            .collect()
    }
}

/// The only quarantined row of `detail`: `(reason, sample label, value text,
/// units text)` exactly as received.
fn quarantined(detail: &ImportAttemptDetail) -> (QuarantineReason, String, String, String) {
    assert_eq!(detail.quarantine.len(), 1, "{:?}", detail.quarantine);
    let item = &detail.quarantine[0];
    assert_eq!(item.state, QuarantineState::Open);
    (
        item.reason,
        item.sample_label.clone().unwrap_or_default(),
        item.value_text.clone().unwrap_or_default(),
        item.units_text.clone().unwrap_or_default(),
    )
}

/// README: `unknown_sample`, `incompatible_unit` and `malformed_value`.
#[tokio::test]
async fn fixture_rows_that_cannot_be_placed_are_quarantined_with_their_reason() {
    let lab = Lab::start(RevisionIndicator::None).await;

    let unknown = lab.import(UNKNOWN_SAMPLE).await;
    assert_eq!(unknown.outcome, ImportOutcome::Quarantined);
    assert_eq!(
        quarantined(&lab.detail(&unknown).await),
        (
            QuarantineReason::UnknownSample,
            "C99999999".to_owned(),
            "75.0".to_owned(),
            "ng/mL".to_owned()
        )
    );

    // No unit conversion: the `ug/L` row waits; the `ng/mL` row imports.
    let units = lab.import(UNIT_MISMATCH).await;
    assert_eq!(units.outcome, ImportOutcome::ProcessedWithQuarantine);
    assert_eq!((units.rows_new, units.rows_quarantined), (1, 1));
    let (reason, label, value, unit) = quarantined(&lab.detail(&units).await);
    assert_eq!(
        (reason, label.as_str(), value.as_str(), unit.as_str()),
        (
            QuarantineReason::IncompatibleUnit,
            "C00000004",
            "88.0",
            "ug/L"
        )
    );
    assert_eq!(lab.pending(3).await, ["SYN-MOR"]);
    let oxy = line(&lab.results(3).await, "SYN-OXY")
        .current
        .clone()
        .unwrap();
    assert_eq!(oxy.magnitude.as_deref(), Some("12.0"));

    // Nothing is trimmed: `< 5.0` is not `<5.0`.
    let malformed = lab.import(MALFORMED_VALUE).await;
    assert_eq!((malformed.rows_new, malformed.rows_quarantined), (1, 1));
    let (reason, label, value, _) = quarantined(&lab.detail(&malformed).await);
    assert_eq!(
        (reason, label.as_str(), value.as_str()),
        (QuarantineReason::MalformedValue, "C00000005", "< 5.0")
    );
    assert_eq!(lab.pending(4).await, ["SYN-MOR"]);

    let first_page = lab
        .engine()
        .list_import_attempts(ListImportAttemptsInput {
            after_import_attempt_id: None,
            instrument_id: Some(lab.instrument),
            limit: 1,
            before: None,
        })
        .await
        .unwrap();
    assert_eq!(
        first_page.len(),
        2,
        "the extra row proves more import attempts exist"
    );
}

/// Brief §3.2 step 5.2 and §3.3: an analyte no component of the execution
/// expects is `not_in_ordered_scope`; a label on two open runs of the
/// sending instrument is `ambiguous_sample` (row order is never used).
#[tokio::test]
async fn rows_outside_the_order_or_matching_two_positions_are_quarantined() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let confirmation = lab.add_sample(lab.orderables.confirmation.1).await;
    lab.place(&confirmation).await;
    let label = &confirmation.label_code;
    let scope = lab
        .import(&message(&[
            "P|1",
            &format!("O|1|{label}|SYN-POS-10||R"),
            "R|1|^^^SYN-MOR|60.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
            "R|2|^^^SYN-OXY|70.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!((scope.rows_new, scope.rows_quarantined), (1, 1));
    let (reason, _, value, _) = quarantined(&lab.detail(&scope).await);
    assert_eq!(
        (reason, value.as_str()),
        (QuarantineReason::NotInOrderedScope, "70.0")
    );

    place_rerun_on_second_run(&lab, 1).await;
    let ambiguous = lab
        .import(&message(&[
            "P|1",
            "O|1|C00000002|SYN-POS-03||R",
            "R|1|^^^SYN-MOR|51.0|ng/mL||H||F||SYN-TECH-01||20261012135500|SYNTH-LCMS-01",
        ]))
        .await;
    assert_eq!(ambiguous.outcome, ImportOutcome::Quarantined);
    let (reason, label, _, _) = quarantined(&lab.detail(&ambiguous).await);
    assert_eq!(
        (reason, label.as_str()),
        (QuarantineReason::AmbiguousSample, "C00000002")
    );
    assert!(lab.results(1).await.lines.is_empty());
}

/// Puts a rerun of sample `index` on a second open run of the same analyzer.
async fn place_rerun_on_second_run(lab: &Lab, index: usize) {
    lab.sign_in(Role::Analyst).await;
    let e = lab.engine();
    let label = &lab.samples[index].label_code;
    let first = e.get_run(GetRunInput { run_id: lab.run }).await.unwrap();
    let execution = first
        .positions
        .iter()
        .find(|p| &p.sample_label == label)
        .and_then(|p| p.execution.as_ref())
        .unwrap()
        .execution_id;
    let second = e
        .create_run(CreateRunInput {
            instrument_id: lab.instrument,
            method_version_id: lab.methods.drugs,
            lot_ids: Vec::new(),
        })
        .await
        .unwrap();
    e.add_patient_position(AddPatientPositionInput {
        run_id: second.run_id,
        position_label: "R1".to_owned(),
        container_label: label.clone(),
        attempt_kind: AttemptKind::Rerun,
        related_execution_id: Some(execution),
        dilution_factor: None,
        prep_batch_id: None,
        reason: Some("Synthetic rerun".to_owned()),
    })
    .await
    .unwrap();
}

/// README and brief §3.5: with indicator `none` (the default) a changed row
/// is `conflicting_duplicate`; the old measurement stays current and the
/// open item blocks review until a reviewer accepts it as reprocessing.
#[tokio::test]
async fn a_correction_without_the_registration_indicator_waits_in_quarantine() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let attempt = lab.import(CORRECTION).await;
    assert_eq!(attempt.outcome, ImportOutcome::Quarantined);
    let detail = lab.detail(&attempt).await;
    let (reason, label, value, _) = quarantined(&detail);
    assert_eq!(
        (reason, label.as_str(), value.as_str()),
        (
            QuarantineReason::ConflictingDuplicate,
            "C00000001",
            "148.90"
        )
    );
    let results = lab.results(0).await;
    let mor = line(&results, "SYN-MOR");
    let current = mor.current.clone().unwrap();
    assert_eq!(
        (current.revision_no, current.magnitude.as_deref()),
        (1, Some("152.40"))
    );
    assert!(mor.gates.iter().any(|g| g.code == "quarantine_open"));
    let refused = lab.approve(0, vec![current.revision_id]).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert!(gate_codes(&refused).contains(&"quarantine_open".to_owned()));

    let item = &detail.quarantine[0];
    let resolve = |reason: &str| ResolveQuarantineItemInput {
        item_id: item.item_id,
        expected_revision: item.revision,
        resolution: QuarantineResolution::AcceptAsReprocessing,
        reason: reason.to_owned(),
    };
    let unexplained = lab.engine().resolve_quarantine_item(resolve(" ")).await;
    assert_eq!(unexplained.unwrap_err().code(), ErrorCode::Validation);
    lab.engine()
        .resolve_quarantine_item(resolve("Synthetic confirmed correction"))
        .await
        .unwrap();
    let results = lab.results(0).await;
    let mor = line(&results, "SYN-MOR");
    let current = mor.current.clone().unwrap();
    assert_eq!(current.cause, ResultCause::Reprocessed);
    assert_eq!(current.magnitude.as_deref(), Some("148.90"));
    assert!(mor.gates.is_empty(), "{:?}", mor.gates);
    lab.approve(0, vec![current.revision_id]).await.unwrap();
}
