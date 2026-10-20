//! M2 exit evidence, the full journey (implementation plan, milestone M2;
//! REQ-INT-009, REQ-ANA-003/004/005, REQ-RES-005, NFR-01, NFR-02):
//! configure, order, accession, label, analyze, import through the real TCP
//! listener, assess QC and review. Expected outcomes come from
//! `docs/planning/m2-design-brief.md` §1 and §3.2–§3.6 and from
//! `fixtures/astm/m2/README.md`, never from the implementation.

use std::io::Read;
use std::net::{Ipv4Addr, TcpListener};
use std::thread;
use std::time::Duration;

use lis_analytical::RevisionIndicator;
use lis_core::Comparator;
use lis_engine::{
    ConfigureLabelPrinterInput, GetAccessionInput, ListReviewQueueInput, PrintLabelsInput,
    RetryImportInput, ReviewQueueFilter,
};
use lis_results::{ImportOutcome, ResultCause, ResultLineView, ResultOutcome, ReviewStatus};
use lis_specimens::{LabelJobKind, LabelOutcome, PrinterTransport};

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{CORRECTION, JOURNEY, JOURNEY_DUPLICATE, PARTIAL, gate_codes, line};

/// `(sample index, analyte code, comparator, magnitude, classification)`
/// from the README table for `journey.astm` with cutoff 50 `at_or_above`.
const JOURNEY_LINES: [(usize, &str, Option<Comparator>, &str, &str); 6] = [
    (0, "SYN-MOR", None, "152.40", "meets_cutoff"),
    (
        0,
        "SYN-OXY",
        Some(Comparator::Lt),
        "5.0",
        "does_not_meet_cutoff",
    ),
    (1, "SYN-MOR", None, "50.0", "meets_cutoff"),
    (1, "SYN-OXY", None, "49.9", "does_not_meet_cutoff"),
    (2, "SYN-MOR", Some(Comparator::Gt), "1000", "meets_cutoff"),
    (2, "SYN-OXY", None, "0.00", "does_not_meet_cutoff"),
];

#[tokio::test]
async fn m2_journey_runs_from_configuration_to_technical_review() {
    let lab = Lab::start(RevisionIndicator::None).await;
    print_first_label(&lab).await;

    let attempt = lab.import(JOURNEY).await;
    assert_eq!(attempt.outcome, ImportOutcome::Processed);
    let counts = (
        attempt.rows_total,
        attempt.rows_qc,
        attempt.rows_new,
        attempt.rows_duplicate,
        attempt.rows_quarantined,
    );
    assert_eq!(counts, (10, 4, 6, 0, 0));
    assert_qc_rows_stay_out_of_patient_results(&lab).await;
    assert_journey_lines(&lab).await;

    // REQ-RES-005: no line can be approved before its QC is accepted.
    let refused = lab.approve_all(0).await.unwrap_err();
    assert!(gate_codes(&refused).contains(&"qc_pending".to_owned()));
    lab.accept_qc().await;
    lab.sign_in(Role::Reviewer).await;
    let first_page = lab
        .engine()
        .list_review_queue(ListReviewQueueInput {
            after: None,
            filter: ReviewQueueFilter::Ready,
            limit: 1,
        })
        .await
        .unwrap();
    assert_eq!(
        first_page.items.len(),
        1,
        "the public page respects its requested item limit"
    );
    assert!(
        first_page.next.is_some(),
        "the explicit cursor preserves more review work"
    );
    let review = lab.approve_all(0).await.unwrap();
    let results = lab.results(0).await;
    for analyte in ["SYN-MOR", "SYN-OXY"] {
        assert_eq!(
            line(&results, analyte).review_status,
            ReviewStatus::Current {
                review_id: review.review_id
            }
        );
    }
    assert_journey_audit_trail(&lab);
    lab.engine().shutdown().await;
}

/// Prints the first container's label on a TCP test printer through the
/// `spc.label.print` worker.
async fn print_first_label(lab: &Lab) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let printer = thread::spawn(move || {
        let (mut connection, _) = listener.accept().unwrap();
        let mut received = Vec::new();
        connection.read_to_end(&mut received).unwrap();
        received
    });
    lab.sign_in(Role::Admin).await;
    let printer_id = lab
        .engine()
        .configure_printer(ConfigureLabelPrinterInput {
            printer_id: None,
            expected_revision: None,
            code: "bench-m2".to_owned(),
            name: "Synthetic bench printer".to_owned(),
            transport: PrinterTransport::Tcp,
            ip_address: Some("127.0.0.1".to_owned()),
            port: Some(port),
            queue: None,
            active: true,
        })
        .await
        .unwrap()
        .printer_id;
    lab.sign_in(Role::Accessioner).await;
    let sample = &lab.samples[0];
    lab.engine()
        .print_labels(PrintLabelsInput {
            container_ids: vec![sample.container_id],
            printer_id,
            kind: LabelJobKind::Print,
            reason: None,
        })
        .await
        .unwrap();
    // The worker runs on this runtime: wait for its outcome before joining.
    assert_eq!(wait_for_label_outcome(lab).await, LabelOutcome::HandedOff);
    let label = String::from_utf8(printer.join().unwrap()).unwrap();
    assert!(label.contains(&sample.label_code), "{label}");
}

async fn wait_for_label_outcome(lab: &Lab) -> LabelOutcome {
    let accession = lab
        .engine()
        .find_container(lis_engine::FindContainerInput {
            label_code: lab.samples[0].label_code.clone(),
        })
        .await
        .unwrap()
        .accession_id;
    for _ in 0..400 {
        let detail = lab
            .engine()
            .get_accession(GetAccessionInput {
                accession_id: accession,
            })
            .await
            .unwrap();
        if let Some(outcome) = detail.specimens[0].containers[0].label_jobs[0].outcome {
            return outcome;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the label worker recorded no outcome");
}

/// Brief §3.4: control rows are stored only as QC observations.
async fn assert_qc_rows_stay_out_of_patient_results(lab: &Lab) {
    lab.sign_in(Role::Quality).await;
    let qc = lab
        .engine()
        .get_run_qc(lis_engine::GetRunQcInput { run_id: lab.run })
        .await
        .unwrap();
    for analyte in [lab.analytes.mor, lab.analytes.oxy] {
        let entry = qc
            .analytes
            .iter()
            .find(|a| a.analyte_id == analyte)
            .unwrap();
        let labels: Vec<&str> = entry
            .observations
            .iter()
            .map(|o| o.position_label.as_str())
            .collect();
        assert_eq!(labels.len(), 2, "{labels:?}");
        assert!(
            labels.contains(&"Q1") && labels.contains(&"Q2"),
            "{labels:?}"
        );
        assert!(entry.assessments.current.is_none());
    }
}

/// README: six patient lines with the source text kept exactly; the two
/// samples not in the message stay pending, with no fabricated line.
async fn assert_journey_lines(lab: &Lab) {
    for (index, code, comparator, magnitude, classification) in JOURNEY_LINES {
        let results = lab.results(index).await;
        assert_eq!(
            (results.lines.len(), results.pending_analytes.len()),
            (2, 0)
        );
        let current = line(&results, code).current.clone().unwrap();
        let outcome = if comparator.is_some() {
            ResultOutcome::Censored
        } else {
            ResultOutcome::Quantity
        };
        assert_eq!(
            (
                current.cause,
                current.revision_no,
                current.outcome,
                current.comparator,
                current.magnitude.as_deref(),
                current.units.as_deref(),
                current.classification.as_str(),
                current.evidence.len(),
            ),
            (
                ResultCause::Initial,
                1,
                outcome,
                comparator,
                Some(magnitude),
                Some("ng/mL"),
                classification,
                1,
            ),
            "{code} of sample {index}"
        );
    }
    for index in [3, 4] {
        let results = lab.results(index).await;
        assert_eq!(
            (results.lines.len(), results.pending_analytes.len()),
            (0, 2)
        );
    }
}

/// Every step left its audit event on the stream the security standard names.
fn assert_journey_audit_trail(lab: &Lab) {
    let expected = [
        ("security", "interface.register_instrument"),
        ("clinical", "catalog.create_specimen_type"),
        ("clinical", "catalog.create_analyte"),
        ("clinical", "catalog.draft_method_version"),
        ("clinical", "catalog.approve_method_version"),
        ("clinical", "catalog.draft_orderable_version"),
        ("clinical", "catalog.approve_orderable_version"),
        ("clinical", "catalog.approve_validity_policy_version"),
        ("clinical", "analytical.register_source"),
        ("clinical", "analytical.approve_source"),
        ("clinical", "analytical.set_instrument_status"),
        ("clinical", "analytical.register_lot"),
        ("clinical", "identity.register_subject"),
        ("clinical", "order.register"),
        ("clinical", "order.accept"),
        ("clinical", "specimen.create_accession"),
        ("clinical", "specimen.record_receipt_decision"),
        ("clinical", "specimen.print_label"),
        ("clinical", "specimen.record_label_outcome"),
        ("clinical", "analytical.create_run"),
        ("clinical", "analytical.add_qc_position"),
        ("clinical", "analytical.add_patient_position"),
        ("clinical", "interface.complete_receipt"),
        ("clinical", "analytical.record_qc_observations"),
        ("clinical", "result.import"),
        ("clinical", "analytical.assess_qc"),
        ("clinical", "result.submit_review"),
        ("access", "result.get_specimen_results"),
    ];
    let mut missing = Vec::new();
    for (stream, action) in expected {
        if !lab.t().events(stream).iter().any(|(a, _)| a == action) {
            missing.push(format!("{stream}/{action}"));
        }
    }
    assert!(missing.is_empty(), "missing audit events: {missing:?}");
    let imports = lab
        .t()
        .events("clinical")
        .into_iter()
        .filter(|(action, _)| action == "result.import")
        .count();
    assert_eq!(imports, 1, "one result.import event per attempt");
}

/// The result lines of the three samples in `journey.astm`.
async fn journey_lines(lab: &Lab) -> Vec<Vec<ResultLineView>> {
    let mut lines = Vec::new();
    for index in 0..3 {
        lines.push(lab.results(index).await.lines);
    }
    lines
}

/// Brief §3.5, NFR-02: a byte-identical message is a duplicate receipt with
/// no import attempt; a reviewer's retry of a processed receipt is a new
/// attempt that adds no evidence.
#[tokio::test]
async fn a_byte_identical_message_creates_no_import_attempt_or_evidence() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let first = lab.import(JOURNEY).await;
    let before = journey_lines(&lab).await;

    let duplicate = lab.deliver(JOURNEY_DUPLICATE).await;
    assert_eq!(duplicate.duplicate_of, Some(first.receipt_id));
    let queued: i64 = lab
        .t()
        .conn()
        .query_row(
            "SELECT count(*) FROM store_outbox WHERE dedup_key = ?1",
            [duplicate.receipt_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(queued, 0, "a duplicate receipt queues no import");
    assert_eq!(lab.attempts().await.len(), 1);
    assert_eq!(journey_lines(&lab).await, before);

    let attempt_no = lab
        .engine()
        .retry_import(RetryImportInput {
            receipt_id: first.receipt_id,
            reason: "Synthetic retry".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(attempt_no, 2);
    let retry = lab.attempt(first.receipt_id, 2).await;
    assert_eq!(
        (
            retry.rows_new,
            retry.rows_reprocessed,
            retry.rows_quarantined
        ),
        (0, 0, 0)
    );
    assert_eq!(journey_lines(&lab).await, before);
}

/// Brief §3.5: a partial re-export processes only its rows; omission never
/// deletes, cancels or means a negative result.
#[tokio::test]
async fn a_partial_re_export_leaves_omitted_results_untouched() {
    let lab = Lab::start(RevisionIndicator::None).await;
    lab.import(JOURNEY).await;
    let before = journey_lines(&lab).await;
    let attempt = lab.import(PARTIAL).await;
    assert_eq!(attempt.outcome, ImportOutcome::Processed);
    assert_eq!(
        (attempt.rows_total, attempt.rows_duplicate, attempt.rows_new),
        (2, 2, 0)
    );
    assert_eq!(journey_lines(&lab).await, before);
}

/// Brief §3.5 with indicator `result_status_correction`: a changed row with
/// status `C` creates processing revision 2, supersedes the measurement and
/// writes a `reprocessed` revision; the review of the old revision goes stale.
#[tokio::test]
async fn a_declared_correction_reprocesses_the_line_and_stales_its_review() {
    let lab = Lab::start(RevisionIndicator::ResultStatusCorrection).await;
    lab.import(JOURNEY).await;
    lab.accept_qc().await;
    let review = lab.approve_all(0).await.unwrap();
    let reviewed = line(&lab.results(0).await, "SYN-MOR")
        .current
        .clone()
        .unwrap();

    let attempt = lab.import(CORRECTION).await;
    assert_eq!(
        (
            attempt.rows_total,
            attempt.rows_reprocessed,
            attempt.rows_quarantined
        ),
        (1, 1, 0)
    );
    let results = lab.results(0).await;
    let mor = line(&results, "SYN-MOR");
    let current = mor.current.clone().unwrap();
    assert_eq!(current.cause, ResultCause::Reprocessed);
    assert_eq!(current.revision_no, 2);
    assert_eq!(current.magnitude.as_deref(), Some("148.90"));
    assert_eq!(current.evidence[0].processing_revision_seq, 2);
    assert_eq!(current.evidence[0].status_text.as_deref(), Some("C"));
    // The earlier revision and its measurement are kept, superseded.
    let kept = mor
        .history
        .iter()
        .find(|r| r.revision_id == reviewed.revision_id)
        .unwrap();
    assert_eq!(kept.magnitude.as_deref(), Some("152.40"));
    assert!(kept.evidence[0].superseded);
    // The untouched analyte keeps its review.
    assert_eq!(
        line(&results, "SYN-OXY").review_status,
        ReviewStatus::Current {
            review_id: review.review_id
        }
    );
    let reprocess_events = lab
        .t()
        .events("clinical")
        .into_iter()
        .filter(|(action, _)| action == "result.reprocess")
        .count();
    assert_eq!(reprocess_events, 1);
    let ReviewStatus::Stale { reasons } = &mor.review_status else {
        panic!("the reviewed line is not stale: {:?}", mor.review_status);
    };
    assert!(
        reasons.iter().any(|r| r.code == "result_revision_changed"),
        "{reasons:?}"
    );
}
