//! Duplicates and the M3 exit evidence (implementation plan, milestone M3;
//! `docs/planning/m3-design-brief.md` §3.6, §4.4): one revision per
//! candidate, an amendment that preserves the prior version byte for byte,
//! VAL-05, and no bare "delivered" (REQ-RPT-001/002/005/011, NFR-07).
//! Synthetic data only.

use std::collections::BTreeSet;

use lis_catalog::{MethodAnalyte, MethodVersionContent};
use lis_core::{DecimalText, ErrorCode};
use lis_engine::{
    ApproveMethodVersion, DraftMethodVersion, ExportReportPdfInput, GetMethodVersionInput,
    GetOrderableVersionInput, GetReportRevisionInput, ListDeliveryDestinationsInput,
    RecordDeliveryAttemptInput, RedirectDeliveryObligationInput, ReplaceDeliveryDestinationInput,
    ReportDetail, RetireOrderableVersion, VerifyDeliveryDestinationInput,
};
use lis_reporting::{
    DeliveryEvidence, DeliveryFailureCode, DestinationId, ExportPurpose, ObligationId,
    ObligationStatus, ReportRelationship, ReportRevisionId, RevisionAvailability,
};

use crate::m2_lab::Role;
use crate::m3_changes::Change;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::m3_release_races::{released_events, verified_artifact};
use crate::m3_report::open_keys;

const AMENDMENT: &str = "Synthetic correction: integration error";

#[tokio::test]
async fn releasing_the_same_candidate_twice_gives_one_revision() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    bench.release(&candidate).await.unwrap();
    let again = bench.release(&candidate).await.unwrap_err();
    assert_eq!(again.code(), ErrorCode::Conflict);

    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let (first, second) = tokio::join!(bench.release(&candidate), bench.release(&candidate));
    assert!(first.is_ok() != second.is_ok(), "{first:?} {second:?}");
    assert_eq!(bench.count("rpt_revisions"), 1);
    assert_eq!(released_events(&bench).len(), 1);
}

#[tokio::test]
async fn releasing_a_superseded_candidate_is_a_conflict() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let superseded = bench.prepare(None).await.unwrap();
    let current = bench.prepare(None).await.unwrap();
    let refused = bench.release(&superseded).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    assert_eq!(bench.count("rpt_revisions"), 0);
    assert_eq!(bench.release(&current).await.unwrap().revision_no, 1);
}

/// M3 exit, "amendment preserves the prior version": revision 1's row and
/// bytes are identical after a correction; revision 1 reads `superseded`;
/// revision 2 is a `correction` of 1 with its reason; the destination of
/// revision 1 has a follow-up obligation and the stale one is closed.
#[tokio::test]
async fn a_correction_preserves_the_prior_revision_byte_for_byte() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let first = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let row = bench.revision_row(first.revision_id);
    let bytes = verified_artifact(&bench, &first.artifact_sha256);

    let corrected = Change::ResultRevision
        .prepare(&bench, Staff::Releaser)
        .await;
    bench.sign_in(Staff::Desk(Role::Reviewer)).await;
    corrected.commit(bench.lab().engine()).await.unwrap();
    bench.desk().await.approve_all(bench.index).await.unwrap();
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(Some(AMENDMENT)).await.unwrap();
    assert_eq!(candidate.relationship, ReportRelationship::Correction);
    assert_eq!(candidate.supersedes_revision_no, Some(1));
    let second = bench.release(&candidate).await.unwrap();

    assert_eq!(bench.revision_row(first.revision_id), row);
    assert_eq!(verified_artifact(&bench, &first.artifact_sha256), bytes);
    let report = bench.report(Staff::Releaser).await;
    let revision = |no: i64| {
        report
            .revisions
            .iter()
            .find(|r| r.revision_no == no)
            .unwrap()
    };
    assert_eq!(revision(1).availability, RevisionAvailability::Superseded);
    assert_eq!(revision(2).relationship, ReportRelationship::Correction);
    assert_eq!(revision(2).amendment_reason.as_deref(), Some(AMENDMENT));
    assert_eq!(second.revision_no, 2);
    let old = report
        .obligations
        .iter()
        .find(|o| o.revision_no == 1)
        .unwrap();
    let follow_up = report
        .obligations
        .iter()
        .find(|o| o.revision_no == 2)
        .unwrap();
    assert_eq!(old.status.code, ObligationStatus::Closed);
    assert_eq!(
        follow_up.destination.destination_id,
        old.destination.destination_id
    );
}

/// VAL-05: a new method version (another cutoff) after release leaves the
/// released content and its export byte-identical.
#[tokio::test]
async fn a_new_method_version_leaves_the_released_export_byte_identical() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let released = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let before = released_copy(&bench, released.revision_id).await;

    change_cutoff(&bench).await;
    assert_eq!(released_copy(&bench, released.revision_id).await, before);
}

/// The fixture's Q account explicitly holds Administrator as well as QualityManager.
/// That second administrator retires dependent orderables and approves A's new
/// method version with cutoff 60, preserving separation of duties.
async fn change_cutoff(bench: &Bench) {
    let lab = bench.desk().await;
    lab.sign_in(Role::Quality).await;
    let e = lab.engine();
    let o = lab.orderables;
    for orderable_version_id in [o.panel.1, o.confirmation.1, o.with_validity.1] {
        let version = e.get_orderable_version(GetOrderableVersionInput {
            orderable_version_id,
        });
        e.retire_orderable_version(RetireOrderableVersion {
            orderable_version_id,
            expected_revision: version.await.unwrap().lifecycle.revision,
            reason: "Synthetic: the method changes".to_owned(),
        })
        .await
        .unwrap();
        let retired = e.get_orderable_version(GetOrderableVersionInput {
            orderable_version_id,
        });
        assert_eq!(
            retired
                .await
                .unwrap()
                .lifecycle
                .retired_by_display
                .as_deref(),
            Some("Test qm1")
        );
    }
    lab.sign_in(Role::Admin).await;
    let method_version_id = lab.methods.drugs;
    let view = e.get_method_version(GetMethodVersionInput { method_version_id });
    let view = view.await.unwrap();
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: view.specimen_type_id,
        certainty: view.certainty,
        output_scale: view.output_scale,
        threshold_rule: view.threshold_rule,
        threshold_rounding: view.threshold_rounding,
        analytes: view
            .analytes
            .into_iter()
            .map(|analyte| MethodAnalyte {
                cutoff: analyte
                    .limits
                    .cutoff
                    .map(|_| DecimalText::parse("60").unwrap()),
                ..analyte.limits
            })
            .collect(),
    };
    let draft = e.draft_method_version(DraftMethodVersion {
        method_id: view.method_id,
        content,
    });
    let method_version_id = draft.await.unwrap().id;
    lab.sign_in(Role::Quality).await;
    e.approve_method_version(ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
}

/// The printed content (as JSON) and an exported PDF of a revision, as R.
async fn released_copy(bench: &Bench, revision_id: ReportRevisionId) -> (String, Vec<u8>) {
    bench.sign_in(Staff::Releaser).await;
    let e = bench.lab().engine();
    let revision = e.get_report_revision(GetReportRevisionInput { revision_id });
    let content = serde_json::to_string(&revision.await.unwrap().content).unwrap();
    let downloads = tempfile::tempdir().unwrap();
    let exported = e
        .export_report_pdf(
            ExportReportPdfInput {
                revision_id,
                purpose: ExportPurpose::Records,
                obligation_id: None,
            },
            Some(downloads.path().to_owned()),
        )
        .await
        .unwrap();
    let pdf = std::fs::read(downloads.path().join(exported.file_name)).unwrap();
    (content, pdf)
}

/// M3 exit, "no screen or export shows a bare delivered": every status an
/// obligation passes through has an engine label naming the channel, and
/// none says "delivered"; the printed content carries no delivery status.
#[tokio::test]
async fn no_delivery_label_or_printed_content_says_delivered() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let released = bench
        .release(&bench.prepare(None).await.unwrap())
        .await
        .unwrap();
    let mut seen = Vec::new();
    let first = bench.report(Staff::Releaser).await.obligations[0].obligation_id;
    record(&bench, &mut seen).await;
    for (evidence, failure_code) in [
        (
            DeliveryEvidence::Failed,
            Some(DeliveryFailureCode::NoAnswerOrBusy),
        ),
        (DeliveryEvidence::OutcomeUnknown, None),
    ] {
        attempt(&bench, first, evidence, failure_code).await;
        record(&bench, &mut seen).await;
    }
    let hold = Change::ContainerHold.prepare(&bench, Staff::Releaser).await;
    bench.sign_in(Staff::Desk(Role::Quality)).await;
    hold.commit(bench.lab().engine()).await.unwrap();
    let keys = open_keys(&record(&bench, &mut seen).await);
    bench.assess_no_impact(released.revision_id, keys).await;
    let replacement = replace_destination(&bench).await;
    record(&bench, &mut seen).await;
    bench.sign_in(Staff::Releaser).await;
    let redirected = bench
        .lab()
        .engine()
        .redirect_delivery_obligation(RedirectDeliveryObligationInput {
            obligation_id: first,
            destination_id: replacement,
            reason: "Synthetic: the clinic changed its fax line".to_owned(),
        })
        .await
        .unwrap();
    attempt(
        &bench,
        redirected.obligation_id,
        DeliveryEvidence::HandedOff,
        None,
    )
    .await;
    record(&bench, &mut seen).await;

    let codes: BTreeSet<_> = seen.iter().map(|(code, _)| code.as_str()).collect();
    let expected = [
        "attention",
        "awaiting",
        "blocked",
        "closed",
        "handed_off",
        "paused",
    ];
    assert_eq!(codes, BTreeSet::from(expected));
    for (_, label) in &seen {
        assert!(label.starts_with("Fax by staff"), "{label}");
        assert!(!label.to_lowercase().contains("delivered"), "{label}");
    }
    let (content, _) = released_copy(&bench, released.revision_id).await;
    assert!(!content.to_lowercase().contains("deliver"), "{content}");
    let exports = bench.report(Staff::Releaser).await.exports;
    assert!(
        exports[0].label.ends_with("(not a delivery)"),
        "{}",
        exports[0].label
    );
}

/// Adds every obligation's `(status code, label)` to `seen`; returns the report.
async fn record(bench: &Bench, seen: &mut Vec<(String, String)>) -> ReportDetail {
    let report = bench.report(Staff::Releaser).await;
    for obligation in &report.obligations {
        let status = &obligation.status;
        seen.push((status.code.as_str().to_owned(), status.label.clone()));
    }
    report
}

async fn attempt(
    bench: &Bench,
    obligation_id: ObligationId,
    evidence: DeliveryEvidence,
    failure_code: Option<DeliveryFailureCode>,
) {
    bench.sign_in(Staff::Releaser).await;
    bench
        .lab()
        .engine()
        .record_delivery_attempt(RecordDeliveryAttemptInput {
            obligation_id,
            evidence,
            observed_at: None,
            failure_code,
            note: None,
        })
        .await
        .unwrap();
}

/// A replaces the fax destination (retiring it) and the race actor verifies
/// the replacement; returns the replacement's ID.
async fn replace_destination(bench: &Bench) -> DestinationId {
    let lab = bench.desk().await;
    lab.sign_in(Role::Admin).await;
    let e = lab.engine();
    let destinations = e.list_delivery_destinations(ListDeliveryDestinationsInput {
        client_id: None,
        state: None,
    });
    let old = destinations.await.unwrap().remove(0);
    let replacement = e
        .replace_delivery_destination(ReplaceDeliveryDestinationInput {
            destination_id: old.destination_id,
            expected_revision: old.revision,
            address: Some("555-0198".to_owned()),
            label: "Synthetic Clinic new fax".to_owned(),
            reason: "Synthetic: the clinic changed its fax line".to_owned(),
        })
        .await
        .unwrap();
    bench.sign_in(Staff::Racer).await;
    bench
        .lab()
        .engine()
        .verify_delivery_destination(VerifyDeliveryDestinationInput {
            destination_id: replacement.destination_id,
            expected_revision: replacement.revision,
            method: "Synthetic test fax confirmed by phone".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap();
    replacement.destination_id
}
