//! UI-07/UI-10 native boundary regression using actual order, analytical,
//! review and released-report commands. Two dates are distinct billing groups;
//! the equal release timestamp must never collapse them into one task.
use super::*;
use lis_billing::{BillingWorkCursor, BillingWorkItem, BillingWorkTarget};

#[tokio::test]
async fn billing_pages_and_exact_target_preserve_same_patient_different_dates() {
    let desk = released_groups().await;
    let engine = desk.as_staff(Staff::Author).await;
    let page2 = assert_two_pages(engine).await;
    let exact = engine
        .list_billing_work(ListBillingWork {
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
            after: None,
            target: Some(BillingWorkTarget {
                subject_id: page2.subject_id,
                date_of_service: page2.date_of_service.clone(),
            }),
        })
        .await
        .unwrap();
    assert_eq!(exact, vec![page2.clone()]);
    let unknown = engine
        .list_billing_work(ListBillingWork {
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
            after: None,
            target: Some(BillingWorkTarget {
                subject_id: page2.subject_id,
                date_of_service: None,
            }),
        })
        .await
        .unwrap();
    assert!(
        unknown.is_empty(),
        "unknown collection date is not a wildcard or today's date"
    );
    assert_eq!(desk.count("SELECT count(*) FROM audit_events WHERE stream='access' AND action='billing.list_billing_work'"),4);
    // as_staff bypasses the journey role cache; restore it before testing a different actor.
    desk.resync().await;
    desk.lab.sign_in(Role::Accessioner).await;
    let denied = desk
        .engine()
        .list_billing_work(ListBillingWork {
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
            after: None,
            target: None,
        })
        .await
        .unwrap_err();
    assert_eq!(denied.code(), ErrorCode::Forbidden);
}

/// Actual imported, QC accepted, reviewed and finally released groups on two service dates.
async fn released_groups() -> Desk {
    let mut desk = Desk::start().await;
    let first = desk
        .add_sample(desk.lab.orderables.panel.1, late_evening(), true)
        .await;
    let second = desk
        .add_sample(
            desk.lab.orderables.panel.1,
            CollectionTimeInput::Date {
                date: "2026-10-12".to_owned(),
            },
            true,
        )
        .await;
    desk.lab.import(JOURNEY).await;
    for index in [first, second] {
        desk.import_drugs(index).await;
    }
    desk.lab.accept_qc().await;
    for index in [first, second] {
        desk.lab.approve_all(index).await.unwrap();
        desk.condition_not_met(index).await;
    }
    release_groups(&desk, [first, second]).await;
    desk
}

async fn release_groups(desk: &Desk, indices: [usize; 2]) {
    let engine = desk.as_staff(Staff::Releaser).await;
    // One session and fixed clock: both final releases have exactly the same time.
    for index in indices {
        let candidate = engine
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id: desk.lab.samples[index].specimen_id,
                amendment_reason: None,
            })
            .await
            .unwrap();
        assert_eq!(candidate.completeness, ReportCompleteness::Final);
        engine
            .release_report(ReleaseReportInput {
                candidate_id: candidate.candidate_id,
                expected_revision: candidate.revision,
                content_sha256: candidate.content_sha256,
                acknowledge_partial: false,
                password: secret(NEW_PASSWORD).into(),
                self_approval: false,
            })
            .await
            .unwrap();
    }
}

/// Equal timestamps retain both groups through a one-item boundary.
async fn assert_two_pages(engine: &Engine) -> BillingWorkItem {
    let started = std::time::Instant::now();
    let page1 = engine
        .list_billing_work(ListBillingWork {
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
            after: None,
            target: None,
        })
        .await
        .unwrap();
    println!(
        "synthetic two-final-group billing first page: {} microseconds; not capacity evidence",
        started.elapsed().as_micros()
    );
    assert_eq!(page1.len(), 1);
    assert_eq!(page1[0].has_more, Some(true));
    assert_eq!(page1[0].date_of_service.as_deref(), Some("2026-10-11"));
    let cursor = BillingWorkCursor {
        since: page1[0].since,
        subject_id: page1[0].subject_id,
        date_of_service: page1[0].date_of_service.clone(),
    };
    let page2 = engine
        .list_billing_work(ListBillingWork {
            filter: BillingWorkFilter::NeedsDerivation,
            limit: 1,
            after: Some(cursor),
            target: None,
        })
        .await
        .unwrap();
    assert_eq!(page2.len(), 1);
    assert_eq!(page2[0].has_more, Some(false));
    assert_eq!(page2[0].subject_id, page1[0].subject_id);
    assert_eq!(page2[0].since, page1[0].since);
    assert_eq!(page2[0].date_of_service.as_deref(), Some("2026-10-12"));
    page2.into_iter().next().unwrap()
}
