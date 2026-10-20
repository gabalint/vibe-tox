//! Collection revisions preserve source evidence and reopen clinical timing review.

use crate::{STAFF, fixture, last_event, raw, read, run};
use lis_core::{Capability, ErrorCode};
use lis_specimens::{
    CollectionTimeInput, CorrectCollectionTimeInput, ResolveCollectionCorrectionInput,
};

#[test]
fn collection_correction_is_versioned_and_gated_until_quality_review() {
    let f = fixture();
    let accession = f.accession(1);
    let id = accession.accession_id;
    let specimen = accession.specimens[0].specimen_id;
    let correction = run(&f.t, STAFF, move |ctx| {
        lis_specimens::correct_collection_time(
            ctx,
            CorrectCollectionTimeInput {
                accession_id: id,
                expected_revision: 1,
                time: CollectionTimeInput::Date {
                    date: "2025-12-31".to_owned(),
                },
                source_document_ref: "SYN-CORRECTION".to_owned(),
                reason: "Corrected supplied collection date".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(correction.revision, 2);
    assert_eq!(
        correction.original.collected_at,
        accession.collection.collected_at
    );
    assert_eq!(
        correction.current.collected_date.as_deref(),
        Some("2025-12-31")
    );
    let current = read(&f.t, move |conn| lis_specimens::get_accession(conn, id));
    assert_eq!(
        current.collection.collected_date,
        correction.current.collected_date
    );
    let gates = read(&f.t, move |conn| {
        lis_specimens::release_gate(conn, specimen)
    })
    .unwrap_err();
    assert!(
        gates
            .iter()
            .any(|gate| gate.code == "collection_time_review")
    );
    let correction_id = correction.corrections[0].correction_id;
    assert_eq!(
        last_event(&f.t),
        (
            "specimen.correct_collection_time".to_owned(),
            Some(correction_id.to_string()),
            Some("Corrected supplied collection date".to_owned()),
        )
    );
    review_correction(&f, correction_id);
    assert_immutable(&f);
    let gates = read(&f.t, move |conn| {
        lis_specimens::release_gate(conn, specimen)
    })
    .unwrap_err();
    assert!(
        !gates
            .iter()
            .any(|gate| gate.code == "collection_time_review")
    );
    let history = read(&f.t, move |conn| {
        lis_specimens::collection_corrections_for_specimen(conn, specimen)
    });
    assert_eq!(history.len(), 1);
    assert!(history[0].resolved_at.is_some());
}

#[test]
fn collection_correction_refuses_unauthorized_stale_unchanged_and_future_input() {
    let f = fixture();
    let accession = f.accession(1);
    let id = accession.accession_id;
    let input = CorrectCollectionTimeInput {
        accession_id: id,
        expected_revision: 1,
        time: CollectionTimeInput::Date {
            date: "2025-12-31".to_owned(),
        },
        source_document_ref: "SYN-CORRECTION".to_owned(),
        reason: "Source correction".to_owned(),
    };
    let denied = input.clone();
    assert_eq!(
        run(
            &f.t,
            &[],
            move |ctx| lis_specimens::correct_collection_time(ctx, denied)
        )
        .unwrap_err()
        .code(),
        ErrorCode::Forbidden
    );
    let accepted = input.clone();
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::correct_collection_time(ctx, accepted)
    })
    .unwrap();
    let stale = input.clone();
    assert_eq!(
        run(&f.t, STAFF, move |ctx| {
            lis_specimens::correct_collection_time(ctx, stale)
        })
        .unwrap_err()
        .code(),
        ErrorCode::Conflict
    );
    let mut unchanged = input.clone();
    unchanged.expected_revision = 2;
    assert_eq!(
        run(&f.t, STAFF, move |ctx| {
            lis_specimens::correct_collection_time(ctx, unchanged)
        })
        .unwrap_err()
        .code(),
        ErrorCode::Validation
    );
    let mut future = input;
    future.expected_revision = 2;
    future.time = CollectionTimeInput::Date {
        date: "2099-01-01".to_owned(),
    };
    assert_eq!(
        run(&f.t, STAFF, move |ctx| {
            lis_specimens::correct_collection_time(ctx, future)
        })
        .unwrap_err()
        .code(),
        ErrorCode::Validation
    );
    let history = read(&f.t, move |conn| {
        lis_specimens::get_collection_history(conn, id)
    });
    assert_eq!(history.revision, 2);
}

#[test]
fn same_day_and_unknown_corrections_preserve_original_and_all_prior_versions() {
    let f = fixture();
    let accession = f.accession(1);
    let id = accession.accession_id;
    let same_day = run(&f.t, STAFF, move |ctx| {
        lis_specimens::correct_collection_time(
            ctx,
            CorrectCollectionTimeInput {
                accession_id: id,
                expected_revision: 1,
                time: CollectionTimeInput::Datetime {
                    local: "2026-01-01T10:00".to_owned(),
                    utc_offset_minutes: -300,
                },
                source_document_ref: "SYN-TIME-CORRECTION".to_owned(),
                reason: "Corrected supplied time on the same date".to_owned(),
            },
        )
    })
    .unwrap();
    assert_ne!(
        same_day.current.collected_at,
        accession.collection.collected_at
    );
    assert_eq!(
        same_day.original.collected_at,
        accession.collection.collected_at
    );
    let unknown = run(&f.t, STAFF, move |ctx| {
        lis_specimens::correct_collection_time(
            ctx,
            CorrectCollectionTimeInput {
                accession_id: id,
                expected_revision: 2,
                time: CollectionTimeInput::Unknown,
                source_document_ref: "SYN-WITHDRAWN-TIME".to_owned(),
                reason: "Source could not verify collection timing".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(unknown.revision, 3);
    assert!(unknown.current.collected_at.is_none());
    assert!(unknown.current.collected_date.is_none());
    assert_eq!(unknown.corrections.len(), 2);
    let pending = read(&f.t, |conn| {
        lis_specimens::pending_collection_reviews(conn, 25)
    });
    assert_eq!(pending.len(), 1);
    assert_eq!(pending[0].accession_id, id);
    assert_eq!(unknown.subject.subject_id, f.subject_id);
    assert_eq!(unknown.accession_number, accession.accession_number);
    assert_eq!(
        unknown.original.collected_at,
        accession.collection.collected_at
    );
    assert_eq!(
        unknown.corrections[0].collected_at,
        same_day.current.collected_at
    );
}

fn review_correction(f: &crate::Fixture, correction_id: lis_specimens::CollectionCorrectionId) {
    assert_eq!(
        read(&f.t, |conn| lis_specimens::pending_collection_reviews(
            conn, 25
        ))
        .len(),
        1
    );
    let denied = run(&f.t, &[Capability::Accession], move |ctx| {
        lis_specimens::resolve_collection_correction(
            ctx,
            ResolveCollectionCorrectionInput {
                correction_id,
                reason: "Unauthorized review".to_owned(),
            },
        )
    })
    .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
    let resolved = run(&f.t, &[Capability::ManageQuality], move |ctx| {
        lis_specimens::resolve_collection_correction(
            ctx,
            ResolveCollectionCorrectionInput {
                correction_id,
                reason: "Timing and specimen stability reviewed".to_owned(),
            },
        )
    })
    .unwrap();
    assert!(resolved.corrections[0].resolved_at.is_some());
    assert!(resolved.corrections[0].resolved_by.is_some());
    assert_eq!(last_event(&f.t).0, "specimen.resolve_collection_correction");
    assert!(
        read(&f.t, |conn| lis_specimens::pending_collection_reviews(
            conn, 25
        ))
        .is_empty()
    );
    assert!(
        run(&f.t, &[Capability::ManageQuality], move |ctx| {
            lis_specimens::resolve_collection_correction(
                ctx,
                ResolveCollectionCorrectionInput {
                    correction_id,
                    reason: "Repeated review".to_owned(),
                },
            )
        })
        .is_err()
    );
}

fn assert_immutable(f: &crate::Fixture) {
    let conn = raw(f);
    assert!(
        conn.execute("UPDATE spc_collections SET collected_at = NULL", [])
            .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE spc_collection_corrections SET reason = 'rewritten'",
            []
        )
        .is_err()
    );
    assert!(
        conn.execute("DELETE FROM spc_collection_corrections", [])
            .is_err()
    );
    assert!(
        conn.execute(
            "UPDATE spc_collection_correction_reviews SET reason = 'rewritten'",
            []
        )
        .is_err()
    );
}
