//! Interfaces (ticket #4555, step 2): a released report's PDF written to a Report PDF
//! connection's folder byte for byte; its correction sent first to the connection that had
//! the earlier revision, whatever the rules; its withdrawal notice after that; and a
//! revision held while its impact review is open, then superseded by its correction
//! (REQ-RPT-003, REQ-RPT-011, REQ-RPT-012, REQ-RPT-013, L13). Synthetic data only.

use std::fs;

use lis_engine::{
    CreatePartnerConnectionInput, ListPartnerMessages, PartnerConnectionDetail,
    PartnerConnectionInput, RecordPartnerDestinationCheck, ReportDetail, RevisionSummary,
    UpdatePartnerConnectionInput, WithdrawReportInput,
};
use lis_interfaces::partner::{
    DeliveryReason, MessageSource, MessageView, PartnerConnectionSettings, PartnerMessageRow,
    PartnerMessageType, PartnerProtocol, RoutingRule, RuleCondition, RuleField, Schedule,
    TransportOutcome, WaitReason,
};
use tempfile::TempDir;

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_lab::Role;
use crate::m3_changes::Change;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::m3_release_races::verified_artifact;
use crate::partner_contract::{approve_and_test, draft_contract};

const PATTERN: &str = "{report}_r{revision}.pdf";
pub(super) const AMENDMENT: &str = "Synthetic correction: integration error";

fn settings(rules: Vec<RoutingRule>) -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "Report PDFs - all clients".to_owned(),
        partner: "Synthetic clinic".to_owned(),
        message_type: PartnerMessageType::ReportPdf,
        protocol: PartnerProtocol::Folder,
        rules,
        schedule: Schedule::Immediately,
        file_name_pattern: PATTERN.to_owned(),
        sftp: None,
        intake: None,
        mllp: None,
        mllp_listener: None,
    }
}

/// The bench with a started connection, no rules, checked by a second administrator, and
/// its folder.
pub(super) struct Pdfs {
    pub(super) bench: Bench,
    pub(super) folder: TempDir,
    pub(super) connection: PartnerConnectionDetail,
}

impl Pdfs {
    async fn start() -> Self {
        Self::start_with(settings(Vec::new())).await
    }

    pub(super) async fn start_with(settings: PartnerConnectionSettings) -> Self {
        let bench = Bench::ready(Variant::Panel).await;
        let folder = tempfile::tempdir().unwrap();
        bench.sign_in(Staff::Desk(Role::Admin)).await;
        let engine = bench.lab().engine();
        // Only a folder connection has a folder to choose; an MLLP one sends to an address.
        let folder_choice = if settings.protocol == PartnerProtocol::Folder {
            let chosen = engine
                .choose_partner_folder(Some(folder.path().to_owned()))
                .await
                .unwrap()
                .unwrap();
            Some(chosen.folder_choice)
        } else {
            None
        };
        let created = engine
            .create_partner_connection(CreatePartnerConnectionInput {
                settings,
                folder_choice,
                processed_folder_choice: None,
                failed_folder_choice: None,
                password: None,
                active: true,
            })
            .await
            .unwrap();
        bench.sign_in(Staff::Racer).await;
        let id = created.connection.id;
        bench
            .lab()
            .engine()
            .record_partner_destination_check(RecordPartnerDestinationCheck {
                connection_id: id,
                expected_revision: created.connection.revision,
                method: "Synthetic call-back to the clinic's IT".to_owned(),
                self_approval: false,
            })
            .await
            .unwrap();
        // An HL7 connection's tests pass only once its code mappings are approved.
        bench.sign_in(Staff::Desk(Role::Admin)).await;
        draft_contract(bench.lab().engine(), id).await;
        bench.sign_in(Staff::Racer).await;
        approve_and_test(bench.lab().engine(), id).await;
        let connection = bench
            .lab()
            .engine()
            .get_partner_connection(PartnerConnectionInput { connection_id: id })
            .await
            .unwrap();
        Self {
            bench,
            folder,
            connection,
        }
    }

    pub(super) async fn release(&self, amendment: Option<&str>) -> RevisionSummary {
        self.bench.sign_in(Staff::Releaser).await;
        let candidate = self.bench.prepare(amendment).await.unwrap();
        self.bench.release(&candidate).await.unwrap()
    }

    /// A reviewer corrects a reported result: the released revision's impact opens.
    pub(super) async fn correct_result(&self) {
        let corrected = Change::ResultRevision
            .prepare(&self.bench, Staff::Releaser)
            .await;
        self.bench.sign_in(Staff::Desk(Role::Reviewer)).await;
        corrected.commit(self.bench.lab().engine()).await.unwrap();
    }

    pub(super) async fn approve_correction(&self) {
        self.bench
            .desk()
            .await
            .approve_all(self.bench.index)
            .await
            .unwrap();
    }

    pub(super) async fn deliver(&self) {
        self.bench.lab().engine().deliver_partner_messages().await;
    }

    pub(super) fn files(&self) -> Vec<String> {
        let mut names: Vec<String> = fs::read_dir(self.folder.path())
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        names.sort();
        names
    }

    pub(super) async fn rows(&self) -> Vec<PartnerMessageRow> {
        self.bench.sign_in(Staff::Desk(Role::Admin)).await;
        let input = ListPartnerMessages {
            since: None,
            connection_id: None,
            view: MessageView::All,
            search: None,
            hide_tests: false,
            row_ids: Vec::new(),
        };
        let mut rows = self
            .bench
            .lab()
            .engine()
            .list_partner_messages(input)
            .await
            .unwrap()
            .rows;
        rows.sort_by_key(|row| (row.source.as_str(), row.source_number));
        rows
    }

    pub(super) async fn report(&self) -> ReportDetail {
        self.bench.report(Staff::Releaser).await
    }
}

#[tokio::test]
async fn a_report_pdf_is_written_and_its_correction_and_withdrawal_follow() {
    let pdfs = Pdfs::start().await;
    let first = pdfs.release(None).await;
    pdfs.deliver().await;
    let number = pdfs.report().await.report_number;
    let name = format!("{number}_r1.pdf");
    assert_eq!(pdfs.files(), std::slice::from_ref(&name));
    // The issued bytes, never regenerated (NFR-07).
    assert_eq!(
        fs::read(pdfs.folder.path().join(&name)).unwrap(),
        verified_artifact(&pdfs.bench, &first.artifact_sha256)
    );

    // The rules no longer match the report; its correction still goes first to the
    // connection that had revision 1 (REQ-RPT-013).
    only_another_client(&pdfs).await;
    pdfs.correct_result().await;
    pdfs.approve_correction().await;
    let second = pdfs.release(Some(AMENDMENT)).await;
    pdfs.deliver().await;
    let corrected = format!("{number}_r2.pdf");
    assert_eq!(pdfs.files(), [name.clone(), corrected.clone()]);
    assert_eq!(
        fs::read(pdfs.folder.path().join(&corrected)).unwrap(),
        verified_artifact(&pdfs.bench, &second.artifact_sha256)
    );
    let rows = pdfs.rows().await;
    assert_eq!(rows[1].reason, Some(DeliveryReason::FollowUp));
    assert_eq!(rows[1].source_label.as_deref(), Some(number.as_str()));
    assert!(rows.iter().all(|row| row.outcome_unknown));

    withdraw(&pdfs).await;
    pdfs.deliver().await;
    assert!(pdfs.files().contains(&format!("{number}_r2-withdrawn.pdf")));
    let rows = pdfs.rows().await;
    let notice = rows
        .iter()
        .find(|row| row.source == MessageSource::ReportWithdrawal)
        .unwrap();
    assert_eq!(notice.reason, Some(DeliveryReason::FollowUp));
    assert_eq!(notice.transport, TransportOutcome::Written);
}

/// Changes the connection's rule so the report's client no longer matches.
async fn only_another_client(pdfs: &Pdfs) {
    pdfs.bench.sign_in(Staff::Desk(Role::Admin)).await;
    let engine = pdfs.bench.lab().engine();
    let connection_id = pdfs.connection.connection.id;
    let current = engine
        .get_partner_connection(PartnerConnectionInput { connection_id })
        .await
        .unwrap();
    engine
        .update_partner_connection(UpdatePartnerConnectionInput {
            connection_id,
            expected_revision: current.connection.revision,
            settings: settings(vec![RoutingRule {
                field: RuleField::Client,
                condition: RuleCondition::Equals,
                value: "SYN-OTHER".to_owned(),
            }]),
            folder_choice: None,
            processed_folder_choice: None,
            failed_folder_choice: None,
            password: None,
            reason: "Synthetic: only another client".to_owned(),
        })
        .await
        .unwrap();
}

async fn withdraw(pdfs: &Pdfs) {
    let report = pdfs.report().await;
    pdfs.bench
        .lab()
        .engine()
        .withdraw_report(WithdrawReportInput {
            report_id: report.report_id,
            expected_revision_no: 2,
            reason: "Synthetic wrong patient".to_owned(),
            password: secret(NEW_PASSWORD).into(),
        })
        .await
        .unwrap();
}

/// L13: a revision whose impact review is open waits; its correction supersedes it, so
/// only the correction is written.
#[tokio::test]
async fn an_unwritten_revision_waits_on_its_impact_and_is_superseded_by_its_correction() {
    let pdfs = Pdfs::start().await;
    pdfs.release(None).await;
    pdfs.correct_result().await;
    pdfs.deliver().await;
    assert!(pdfs.files().is_empty());
    let rows = pdfs.rows().await;
    assert_eq!(rows[0].transport, TransportOutcome::NotSent);
    assert_eq!(rows[0].waiting, Some(WaitReason::ImpactReview));

    pdfs.approve_correction().await;
    pdfs.release(Some(AMENDMENT)).await;
    pdfs.deliver().await;
    let number = pdfs.report().await.report_number;
    assert_eq!(pdfs.files(), [format!("{number}_r2.pdf")]);
    let rows = pdfs.rows().await;
    assert!(rows[0].superseded);
    assert_eq!(rows[0].transport, TransportOutcome::NotSent);
    assert_eq!(rows[1].reason, Some(DeliveryReason::FollowUp));
    assert_eq!(rows[1].transport, TransportOutcome::Written);
    assert_eq!(
        pdfs.bench.count(
            "audit_events WHERE action = 'interfaces.write_message' AND outcome = 'succeeded'"
        ),
        1
    );
}
