//! Report layouts through the engine (REQ-RPT-007): an administrator drafts a
//! layout, quality approves it, and only candidates prepared afterwards render
//! with it; a candidate prepared before keeps the layout it recorded, and the
//! released revision names its layout. Previews render synthetic data only.

use lis_core::{CommandError, ErrorCode};
use lis_engine::{
    ApproveReportLayoutInput, DraftReportLayoutInput, Engine, GetReportRevisionInput,
    PreviewReportLayoutInput, RejectReportLayoutInput,
};
use lis_reporting::LayoutVersionId;
use lis_reporting::layouts::ReportLayout;

use super::{Desk, code};
use crate::m2_lab::Role;

/// Calls the layout command `name` with plausible input aimed at random IDs.
pub(super) async fn layout_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let version_id = LayoutVersionId::generate();
    Some(match name {
        "get_report_layouts" => engine.get_report_layouts().await.map(drop),
        "preview_report_layout" => engine
            .preview_report_layout(PreviewReportLayoutInput {
                layout: ReportLayout::Standard,
            })
            .await
            .map(drop),
        "draft_report_layout" => engine
            .draft_report_layout(DraftReportLayoutInput {
                layout: ReportLayout::Standard,
            })
            .await
            .map(drop),
        "approve_report_layout" => engine
            .approve_report_layout(ApproveReportLayoutInput {
                version_id,
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "reject_report_layout" => engine
            .reject_report_layout(RejectReportLayoutInput {
                version_id,
                expected_revision: 1,
                reason: "Synthetic rejection".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn candidate_templates(desk: &Desk) -> Vec<(String, i64)> {
    let conn = desk.lab.t().conn();
    let mut statement = conn
        .prepare("SELECT template_id, template_version FROM rpt_candidates ORDER BY prepared_at")
        .unwrap();
    statement
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<rusqlite::Result<_>>()
        .unwrap()
}

#[tokio::test]
async fn an_approved_layout_applies_to_candidates_prepared_afterwards() {
    let desk = Desk::start().await;
    let before = desk.prepare(None).await.unwrap();

    desk.lab.sign_in(Role::Admin).await;
    let preview = desk
        .engine()
        .preview_report_layout(PreviewReportLayoutInput {
            layout: ReportLayout::Standard,
        })
        .await
        .unwrap();
    assert!(preview.starts_with(b"%PDF"));
    let drafted = desk
        .engine()
        .draft_report_layout(DraftReportLayoutInput {
            layout: ReportLayout::Standard,
        })
        .await
        .unwrap();
    let draft = drafted.draft.unwrap();
    // Configuration staff may not decide; quality staff do.
    let refused = desk
        .engine()
        .approve_report_layout(ApproveReportLayoutInput {
            version_id: draft.version_id,
            expected_revision: draft.revision,
            self_approval: false,
        })
        .await;
    assert_eq!(code(refused).0, ErrorCode::Forbidden);
    desk.lab.sign_in(Role::Quality).await;
    let approved = desk
        .engine()
        .approve_report_layout(ApproveReportLayoutInput {
            version_id: draft.version_id,
            expected_revision: draft.revision,
            self_approval: false,
        })
        .await
        .unwrap();
    assert_eq!(approved.in_force.layout, ReportLayout::Standard);

    let after = desk.prepare(None).await.unwrap();
    assert_ne!(before.candidate_id, after.candidate_id);
    assert_eq!(
        candidate_templates(&desk),
        [
            ("tox-report".to_owned(), 3),
            ("tox-report-standard".to_owned(), 1)
        ]
    );
    let revision = desk.release(&after).await.unwrap();
    desk.as_releaser().await;
    let detail = desk
        .engine()
        .get_report_revision(GetReportRevisionInput {
            revision_id: revision.revision_id,
        })
        .await
        .unwrap();
    assert_eq!(detail.provenance.layout_name.as_deref(), Some("Standard"));
    assert_eq!(detail.provenance.template_id, "tox-report-standard");
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events WHERE action IN \
             ('report.draft_layout', 'report.approve_layout') AND outcome = 'succeeded'"
        ),
        2
    );
}
