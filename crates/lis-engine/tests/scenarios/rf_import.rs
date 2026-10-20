//! Result files through the engine's real path (result files brief §5, §10,
//! F7, F12; REQ-INT-005, REQ-ANA-003, REQ-RES-001): a file loaded with
//! `load_result_file` is received, imported by the outbox worker, reviewed
//! and released; a header the template does not match is retried on the
//! original receipt after a new template is approved; a re-export after
//! reintegration waits as a conflict until accepted as reprocessing.
//! Expected outcomes come from the brief. Synthetic data only.

use lis_core::{Comparator, ErrorCode};
use lis_engine::{LoadOutcome, QuarantineResolution, ResolveQuarantineItemInput, RetryImportInput};
use lis_interfaces::FileOrigin;
use lis_results::{ImportFailureCode, ImportOutcome, QuarantineReason, ResultCause, ResultOutcome};

use crate::m2_lab::Role;
use crate::m2_steps::line;
use crate::rf_lab::{FileLab, HEADER, control_rows, row};

/// `(result revision, analyte)` of every released report line.
fn released_lines(f: &FileLab) -> Vec<(String, String)> {
    f.lab
        .t()
        .conn()
        .prepare(
            "SELECT result_revision_id, analyte_id FROM rpt_v_released_lines \
             ORDER BY analyte_id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// `(registration, word)` the revision cites (F7).
fn value_rule(f: &FileLab, revision: lis_results::ResultRevisionId) -> (String, String) {
    f.lab
        .t()
        .conn()
        .query_row(
            "SELECT value_rule_registration_id, value_rule_word FROM res_result_revisions \
             WHERE id = ?1",
            [revision.to_string()],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap()
}

// §10: load → technical review → release, including a word-derived censored
// result with its rule reference (F7).
#[tokio::test]
async fn a_loaded_file_is_imported_reviewed_and_released_with_a_word_derived_result() {
    let mut f = FileLab::start().await;
    let s = f.add_sample(f.tests.both).await;
    let label = s.label_code.as_str();
    let mut rows = control_rows();
    rows.extend([
        row(label, "Unknown", "Morphine", "152.4"),
        row(label, "Unknown", "Morphine-D6", "98.0"),
        row(label, "Unknown", "Oxycodone", "N/F"),
        row("CAL-1", "Standard", "Morphine", "10.2"),
    ]);
    let attempt = f.import("batch-001.csv", &rows).await;
    assert_eq!(attempt.outcome, ImportOutcome::Processed);
    assert_eq!(
        (
            attempt.rows_total,
            attempt.rows_qc,
            attempt.rows_new,
            attempt.rows_not_result,
            attempt.rows_set_aside
        ),
        (8, 4, 2, 1, 1)
    );
    let detail = f.detail(&attempt).await;
    let file = detail.file.clone().unwrap();
    assert_eq!(
        (file.origin, file.file_name.expose().as_str()),
        (FileOrigin::Upload, "batch-001.csv")
    );
    assert_eq!(f.username(file.loaded_by.unwrap()), "an1");
    assert_eq!(file.loaded_by_display.as_deref(), Some("Test an1"));
    assert_eq!(attempt.origin, Some(FileOrigin::Upload));

    f.lab.accept_qc().await;
    let results = f.lab.results(0).await;
    let oxy = line(&results, "SYN-OXY").current.clone().unwrap();
    assert_eq!(
        (
            oxy.cause,
            oxy.outcome,
            oxy.comparator,
            oxy.magnitude.as_deref(),
            oxy.units.as_deref()
        ),
        (
            ResultCause::Initial,
            ResultOutcome::Censored,
            Some(Comparator::Lt),
            Some("2"),
            Some("ng/mL")
        )
    );
    assert_eq!(
        value_rule(&f, oxy.revision_id),
        (
            detail.registration_id.unwrap().to_string(),
            "N/F".to_owned()
        )
    );
    f.lab.approve_all(0).await.unwrap();
    let released = f.release(&s).await;
    assert_eq!(released.revision_no, 1);
    let lines = released_lines(&f);
    assert_eq!(lines.len(), 2);
    assert!(lines.contains(&(oxy.revision_id.to_string(), f.lab.analytes.oxy.to_string())));
}

// §5 and F12: a changed header fails the whole file and keeps its bytes;
// loading it again is a duplicate receipt; the recovery is Retry import on
// the original receipt with the template approved since.
#[tokio::test]
async fn a_layout_mismatch_is_retried_on_the_original_receipt_after_a_new_template() {
    let mut f = FileLab::start().await;
    let s = f.add_sample(f.tests.both).await;
    let renamed = HEADER.replace("Final Conc", "Final Concentration");
    let path = f.file(
        "renamed.csv",
        &renamed,
        &[row(&s.label_code, "Unknown", "Morphine", "61.0")],
    );
    let loaded = f.load(path.clone()).await;
    let first = f.attempt(loaded.receipt_id, 1).await;
    assert_eq!(first.failure_code, Some(ImportFailureCode::LayoutMismatch));
    assert_eq!((first.rows_total, first.rows_new), (0, 0));
    assert_eq!(f.load(path).await.outcome, LoadOutcome::Duplicate);

    let fixed = f.register_template(&renamed).await;
    f.lab.sign_in(Role::Reviewer).await;
    let early = f
        .engine()
        .retry_import(RetryImportInput {
            receipt_id: loaded.receipt_id,
            reason: " ".to_owned(),
        })
        .await;
    assert_eq!(early.unwrap_err().code(), ErrorCode::Validation);
    f.approve_template().await;
    f.lab.sign_in(Role::Reviewer).await;
    let attempt_no = f
        .engine()
        .retry_import(RetryImportInput {
            receipt_id: loaded.receipt_id,
            reason: "Synthetic template corrected".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(attempt_no, 2);
    let second = f.attempt(loaded.receipt_id, 2).await;
    assert_eq!((second.failure_code, second.rows_new), (None, 1));
    assert_eq!(f.detail(&second).await.registration_id, Some(fixed));
    let results = f.lab.results(0).await;
    let mor = line(&results, "SYN-MOR").current.clone().unwrap();
    assert_eq!(mor.magnitude.as_deref(), Some("61.0"));
}

// F12: a re-export after reintegration gives a changed row, quarantined as
// `conflicting_duplicate` until a reviewer accepts it as reprocessing.
#[tokio::test]
async fn a_reintegrated_re_export_waits_as_a_conflict_until_accepted() {
    let mut f = FileLab::start().await;
    let s = f.add_sample(f.tests.both).await;
    let label = s.label_code.as_str();
    let mut rows = control_rows();
    rows.push(row(label, "Unknown", "Morphine", "152.4"));
    f.import("batch-001.csv", &rows).await;
    f.lab.accept_qc().await;
    f.lab.approve_all(0).await.unwrap();

    let attempt = f
        .import(
            "batch-001-reintegrated.csv",
            &[
                row(label, "Unknown", "Morphine", "148.9"),
                row(label, "Unknown", "Oxycodone", "12.0"),
            ],
        )
        .await;
    assert_eq!((attempt.rows_quarantined, attempt.rows_new), (1, 1));
    let detail = f.detail(&attempt).await;
    let item = &detail.quarantine[0];
    assert_eq!(item.reason, QuarantineReason::ConflictingDuplicate);
    let results = f.lab.results(0).await;
    let mor = line(&results, "SYN-MOR");
    assert_eq!(
        mor.current.as_ref().unwrap().magnitude.as_deref(),
        Some("152.4")
    );
    assert!(mor.gates.iter().any(|gate| gate.code == "quarantine_open"));

    f.engine()
        .resolve_quarantine_item(ResolveQuarantineItemInput {
            item_id: item.item_id,
            expected_revision: item.revision,
            resolution: QuarantineResolution::AcceptAsReprocessing,
            reason: "Synthetic reintegration confirmed".to_owned(),
        })
        .await
        .unwrap();
    let results = f.lab.results(0).await;
    let current = line(&results, "SYN-MOR").current.clone().unwrap();
    assert_eq!(
        (current.cause, current.magnitude.as_deref()),
        (ResultCause::Reprocessed, Some("148.9"))
    );
}
