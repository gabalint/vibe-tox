//! A file identical to an earlier one from the same instrument (brief F12,
//! NFR-02): its attempt is listed, imports nothing and names the earlier
//! receipt's latest attempt, which holds the rows.

use lis_interfaces::ReceiptId;
use lis_results::{ImportAttemptFilter, ImportOutcome, ImportSource, RetryImportInput};

use crate::rows::row;
use crate::*;

/// Rows an attempt wrote: measurements, quarantine and set-aside rows.
fn written_by(f: &Fixture, attempt: &ImportAttemptSummary) -> i64 {
    let id = attempt.import_attempt_id;
    f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT (SELECT count(*) FROM res_measurements WHERE import_attempt_id = ?1) \
             + (SELECT count(*) FROM res_quarantine_items WHERE import_attempt_id = ?1) \
             + (SELECT count(*) FROM res_set_aside_rows WHERE import_attempt_id = ?1)",
            [id],
            |row| row.get(0),
        )?)
    })
}

fn listed(f: &Fixture, hide_duplicates: bool) -> Vec<ImportAttemptSummary> {
    let filter = ImportAttemptFilter {
        hide_duplicates,
        ..ImportAttemptFilter::default()
    };
    f.read(move |conn| lis_results::list_import_attempts_page(conn, &filter, 10, None, None))
}

fn duplicate_of(f: &Fixture, receipt: ReceiptId) -> Option<ReceiptId> {
    f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT duplicate_of FROM ifc_receipts WHERE id = ?1",
            [receipt],
            |row| row.get(0),
        )?)
    })
}

#[test]
fn a_duplicate_file_imports_nothing_and_links_the_earlier_attempt() {
    let f = fixture();
    let s = f.sample();
    let bytes = csv(&[row(&s.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    let first = f.receive(&bytes);
    let again = f.receive(&bytes);
    assert_eq!(duplicate_of(&f, again), Some(first));

    // Listed before the earlier receipt is imported: no link yet.
    let duplicate = f.import_source(again, 1, ImportSource::Duplicate(first));
    assert_eq!(duplicate.outcome, ImportOutcome::Duplicate);
    assert_eq!((duplicate.rows_total, duplicate.failure_code), (0, None));
    assert_eq!(duplicate.duplicate_of_attempt_id, None);

    let layout = f.registration().unwrap().file_layout.unwrap();
    let earlier = f.import_source(first, 1, ImportSource::File(extract_file(&bytes, &layout)));
    assert_eq!(
        (earlier.outcome, earlier.rows_new),
        (ImportOutcome::Processed, 1)
    );
    assert_eq!(written_by(&f, &duplicate), 0);
    let detail = f.detail(&duplicate);
    assert_eq!(
        detail.summary.duplicate_of_attempt_id,
        Some(earlier.import_attempt_id)
    );
    assert_eq!(detail.registration_id, None);
    assert!(detail.quarantine.is_empty() && detail.set_aside.is_empty());

    // Repeated delivery of the outbox item: the same attempt, no second event.
    let repeated = f.import_source(again, 1, ImportSource::Duplicate(first));
    assert_eq!(repeated.import_attempt_id, duplicate.import_attempt_id);
    assert_eq!(import_events(&f), 2);
    assert_listed(&f, &duplicate, &earlier);
    assert_retry_refused(&f, again);
}

fn import_events(f: &Fixture) -> i64 {
    f.read(|conn| {
        Ok(conn.query_row(
            "SELECT count(*) FROM audit_events WHERE action = 'result.import'",
            [],
            |row| row.get(0),
        )?)
    })
}

/// Listed with its link to the earlier attempt, and left out by Hide duplicates.
fn assert_listed(f: &Fixture, duplicate: &ImportAttemptSummary, earlier: &ImportAttemptSummary) {
    let all = listed(f, false);
    assert_eq!(all.len(), 2);
    assert!(
        all.iter()
            .any(|row| row.import_attempt_id == duplicate.import_attempt_id
                && row.duplicate_of_attempt_id == Some(earlier.import_attempt_id))
    );
    let shown = listed(f, true);
    assert_eq!(
        shown
            .iter()
            .map(|row| row.import_attempt_id)
            .collect::<Vec<_>>(),
        [earlier.import_attempt_id]
    );
}

/// A duplicate's rows are the earlier receipt's: it is the one retried.
fn assert_retry_refused(f: &Fixture, again: ReceiptId) {
    let retry = f.run_as(principal(&[Capability::ReviewResults]), move |ctx| {
        lis_results::retry_import(
            ctx,
            RetryImportInput {
                receipt_id: again,
                reason: "Synthetic retry".to_owned(),
            },
        )
    });
    assert!(matches!(
        retry,
        Err(ResultError::WrongState("duplicate receipt"))
    ));
}
