//! Bounded candidate scans must not hide a sparse match after an empty page.
use crate::rows::row;
use crate::*;
use lis_results::{RecordResultDispositionInput, ResultOutcome, ReviewQueueFilter};

#[test]
fn an_empty_incomplete_page_continues_to_a_later_blocked_specimen() {
    let f = fixture();
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2026-10-15T10:00:00.000000000Z").unwrap());
    for _ in 0..500 {
        record_nonperformance(&f);
    }
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2026-10-15T11:00:00.000000000Z").unwrap());
    let blocked = f.sample();
    f.import(&[row(
        &blocked.label,
        "Unknown",
        "Morphine",
        "50.00",
        UNIT,
        "1",
    )]);
    let expected_id = blocked.specimen_id;
    f.read(move |conn| {
        let first =
            lis_results::scan_review_work_queue(conn, ReviewQueueFilter::Blocked, 50, None)?;
        assert!(first.items.is_empty());
        assert_eq!(first.scanned, 500);
        let cursor = first.next.expect("empty does not establish exhaustion");
        let last = lis_results::scan_review_work_queue(
            conn,
            ReviewQueueFilter::Blocked,
            50,
            Some(cursor),
        )?;
        assert_eq!(last.scanned, 1);
        assert_eq!(last.items.len(), 1);
        assert_eq!(last.items[0].specimen_id, expected_id);
        assert_eq!(last.items[0].state, "qc_pending");
        assert!(last.next.is_none());
        let detailed =
            lis_results::scan_review_queue(conn, ReviewQueueFilter::Blocked, 50, Some(cursor))?;
        assert_eq!(detailed.items[0].specimen_id, expected_id);
        assert_eq!(detailed.items[0].blocked, 1);
        assert!(detailed.next.is_none());
        Ok(())
    });
}

fn record_nonperformance(f: &Fixture) {
    let sample = f.sample();
    let pending = f
        .results(&sample)
        .pending_analytes
        .into_iter()
        .find(|line| line.analyte_id == f.analytes.mor)
        .unwrap();
    let input = RecordResultDispositionInput {
        result_id: None,
        expected_revision: 0,
        component_id: Some(pending.component_id),
        analyte_id: Some(pending.analyte_id),
        specimen_id: Some(sample.specimen_id),
        outcome: ResultOutcome::Missing,
        reason: "Synthetic missing result investigated before explicit nonperformance".to_owned(),
    };
    f.setup(move |ctx| lis_results::record_result_disposition(ctx, input));
    let line = f
        .results(&sample)
        .lines
        .into_iter()
        .find(|line| line.analyte_id == f.analytes.mor)
        .unwrap();
    let input = RecordResultDispositionInput {
        result_id: Some(line.result_id),
        expected_revision: line.revision,
        component_id: None,
        analyte_id: None,
        specimen_id: None,
        outcome: ResultOutcome::NotPerformed,
        reason: "Synthetic authorized terminal nonperformance for sparse queue history".to_owned(),
    };
    f.setup(move |ctx| lis_results::record_result_disposition(ctx, input));
}
