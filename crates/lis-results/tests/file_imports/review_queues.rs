//! Operational and detailed review queues share predicates across page boundaries.
use crate::rows::{current, row};
use crate::*;
use lis_results::{
    ReviewDecision, ReviewQueueCursor, ReviewQueueFilter, SubmitTechnicalReviewInput,
};

fn assert_matching_pages(
    f: &Fixture,
    filter: ReviewQueueFilter,
) -> Vec<lis_results::ReviewQueueEntry> {
    f.read(move |conn| {
        let mut after = None;
        let mut seen = Vec::new();
        loop {
            let detailed = lis_results::review_queue_page(conn, filter, 1, after)?;
            let operational = lis_results::review_work_queue_page(conn, filter, 1, after)?;
            assert_eq!(detailed.len(), operational.len());
            for (detail, task) in detailed.iter().zip(&operational) {
                assert_eq!(
                    (detail.specimen_id, detail.since),
                    (task.specimen_id, task.since)
                );
                assert_eq!(detail.accession_number, task.accession_number);
                assert_eq!(detail.specimen_type_code, task.specimen_type_code);
                assert_eq!(
                    task.state,
                    match filter {
                        ReviewQueueFilter::Stale => "stale",
                        ReviewQueueFilter::Blocked => "qc_pending",
                        _ => panic!("unexpected test filter"),
                    }
                );
            }
            let Some(first) = detailed.first() else { break };
            after = Some(ReviewQueueCursor {
                since: first.since,
                specimen_id: first.specimen_id,
            });
            seen.push(first.clone());
            if detailed.len() == 1 {
                break;
            }
        }
        Ok(seen)
    })
}

#[test]
fn filtered_queue_pages_keep_counts_and_current_gates_for_reviewed_history() {
    let f = fixture();
    let samples = [f.sample(), f.sample(), f.sample()];
    for sample in &samples {
        f.import(&[row(
            &sample.label,
            "Unknown",
            "Morphine",
            "50.00",
            UNIT,
            "1",
        )]);
    }
    let blocked = assert_matching_pages(&f, ReviewQueueFilter::Blocked);
    assert_eq!(blocked.len(), 3);
    assert!(
        blocked
            .iter()
            .all(|entry| entry.blocked == 1 && entry.stale == 0 && entry.pending == 1)
    );
    assert!(assert_matching_pages(&f, ReviewQueueFilter::Stale).is_empty());
    let revision = current(&f.results(&samples[1]), "SYN-MOR").revision_id;
    let specimen_id = samples[1].specimen_id;
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2026-10-12T08:00:00.000000000Z").unwrap());
    f.setup(move |ctx| {
        lis_results::submit_technical_review(
            ctx,
            SubmitTechnicalReviewInput {
                specimen_id,
                result_revision_ids: vec![revision],
                decision: ReviewDecision::Rejected,
                reason: Some("Synthetic quality rejection retains outstanding QC gate".to_owned()),
            },
        )
    });
    let stale = assert_matching_pages(&f, ReviewQueueFilter::Stale);
    assert_eq!(stale.len(), 1);
    assert_eq!(
        (stale[0].specimen_id, stale[0].stale, stale[0].blocked),
        (specimen_id, 1, 1)
    );
    let blocked = assert_matching_pages(&f, ReviewQueueFilter::Blocked);
    assert_eq!(blocked.len(), 3);
    assert_eq!(blocked.iter().filter(|entry| entry.stale == 1).count(), 1);
    f.read(|conn| {
        assert!(
            lis_results::review_work_queue_page(conn, ReviewQueueFilter::Blocked, 0, None).is_err()
        );
        assert!(
            lis_results::review_work_queue_page(conn, ReviewQueueFilter::Blocked, 501, None)
                .is_err()
        );
        Ok(())
    });
}

#[test]
fn a_new_queue_read_observes_reviews_committed_during_an_older_wal_snapshot() {
    let f = fixture();
    let sample = f.sample();
    f.import(&[row(
        &sample.label,
        "Unknown",
        "Morphine",
        "50.00",
        UNIT,
        "1",
    )]);
    let revision = current(&f.results(&sample), "SYN-MOR").revision_id;
    let specimen_id = sample.specimen_id;
    let mut conn = Connection::open_with_flags(
        f.t.data_dir().join(lis_store::DATABASE_FILE),
        rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY,
    )
    .unwrap();
    let snapshot = conn.transaction().unwrap();
    assert!(
        lis_results::review_stale_queue(&snapshot, 50)
            .unwrap()
            .is_empty()
    );
    f.t.clock
        .set(lis_core::UtcTimestamp::parse("2026-10-12T08:00:00.000000000Z").unwrap());
    f.setup(move |ctx| {
        lis_results::submit_technical_review(
            ctx,
            SubmitTechnicalReviewInput {
                specimen_id,
                result_revision_ids: vec![revision],
                decision: ReviewDecision::Rejected,
                reason: Some(
                    "Synthetic rejection committed while an earlier queue snapshot stays open"
                        .to_owned(),
                ),
            },
        )
    });
    assert!(
        lis_results::review_stale_queue(&snapshot, 50)
            .unwrap()
            .is_empty()
    );
    snapshot.rollback().unwrap();
    let changed = lis_results::review_stale_queue(&conn, 50).unwrap();
    assert_eq!(changed.len(), 1);
    assert_eq!(changed[0].specimen_id, specimen_id);
    assert_eq!(changed[0].state, "stale");
}
