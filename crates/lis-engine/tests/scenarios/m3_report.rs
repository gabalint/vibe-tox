//! Reporting steps on the [`Bench`], through the engine's public reporting
//! commands (`docs/planning/m3-design-brief.md` §7), and the stored-state
//! checks the §3.6 expectations name. Steps run as the signed-in user unless
//! they say otherwise. Synthetic data only.

use std::path::PathBuf;

use lis_core::CommandError;
use lis_engine::{
    CandidateView, CreateDeliveryDestinationInput, GetOrderInput, GetReportCandidateInput,
    GetReportInput, GetReportWorkspaceInput, ListWorkQueueItemsInput, PrepareReportCandidateInput,
    RecordDeliveryAttemptInput, RecordImpactAssessmentInput, ReleaseReportInput, ReportDetail,
    RevisionSummary, VerifyDeliveryDestinationInput,
};
use lis_reporting::{
    DeliveryChannel, DeliveryEvidence, ImpactDecision, ObligationId, ReportCompleteness, ReportId,
    ReportRevisionId,
};

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};

impl Bench {
    /// The bench with its client's fax destination, created by A and
    /// verified by a second `manage_configuration` user, the race actor.
    pub(crate) async fn ready(variant: Variant) -> Self {
        Self::ready_as(variant, lis_engine::InstallationKind::Laboratory).await
    }

    /// The same bench in an installation of `kind` (proposed REQ-OPS-016).
    pub(crate) async fn ready_as(variant: Variant, kind: lis_engine::InstallationKind) -> Self {
        let bench = Self::start_as(variant, kind).await;
        let lab = bench.desk().await;
        lab.sign_in(Role::Accessioner).await;
        let order_id = bench.sample().order_id;
        let client_id = lab
            .engine()
            .get_order(GetOrderInput { order_id })
            .await
            .unwrap()
            .client_id;
        lab.sign_in(Role::Admin).await;
        let destination = lab
            .engine()
            .create_delivery_destination(CreateDeliveryDestinationInput {
                client_id,
                client_location_id: None,
                channel: DeliveryChannel::Fax,
                address: Some("555-0199".to_owned()),
                label: "Synthetic Clinic fax".to_owned(),
            })
            .await
            .unwrap();
        bench.sign_in(Staff::Racer).await;
        bench
            .lab()
            .engine()
            .verify_delivery_destination(VerifyDeliveryDestinationInput {
                destination_id: destination.destination_id,
                expected_revision: destination.revision,
                method: "Synthetic test fax confirmed by phone".to_owned(),
                self_approval: false,
            })
            .await
            .unwrap();
        bench
    }

    /// Prepares a candidate for the reported specimen.
    pub(crate) async fn prepare(
        &self,
        amendment_reason: Option<&str>,
    ) -> Result<CandidateView, CommandError> {
        self.lab()
            .engine()
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id: self.sample().specimen_id,
                amendment_reason: amendment_reason.map(str::to_owned),
            })
            .await
    }

    /// Releases `candidate` as shown, with the signed-in user's password,
    /// acknowledging a partial report.
    pub(crate) async fn release(
        &self,
        candidate: &CandidateView,
    ) -> Result<RevisionSummary, CommandError> {
        self.lab()
            .engine()
            .release_report(ReleaseReportInput {
                candidate_id: candidate.candidate_id,
                expected_revision: candidate.revision,
                content_sha256: candidate.content_sha256.clone(),
                acknowledge_partial: candidate.completeness == ReportCompleteness::Partial,
                password: secret(NEW_PASSWORD).into(),
                self_approval: false,
            })
            .await
    }

    /// The candidate as `reader` reads it (staleness recomputed).
    pub(crate) async fn candidate(
        &self,
        reader: Staff,
        candidate: &CandidateView,
    ) -> CandidateView {
        self.sign_in(reader).await;
        self.lab()
            .engine()
            .get_report_candidate(GetReportCandidateInput {
                candidate_id: candidate.candidate_id,
            })
            .await
            .unwrap()
    }

    /// The specimen's report, as `reader` reads it.
    pub(crate) async fn report(&self, reader: Staff) -> ReportDetail {
        self.sign_in(reader).await;
        let e = self.lab().engine();
        let workspace = e
            .get_report_workspace(GetReportWorkspaceInput {
                specimen_id: self.sample().specimen_id,
            })
            .await
            .unwrap();
        let report_id: ReportId = workspace.report.unwrap().report_id;
        e.get_report(GetReportInput { report_id }).await.unwrap()
    }

    /// Records a staff-attested hand-off on `obligation_id`.
    pub(crate) async fn hand_off(&self, obligation_id: ObligationId) -> Result<(), CommandError> {
        self.lab()
            .engine()
            .record_delivery_attempt(RecordDeliveryAttemptInput {
                obligation_id,
                evidence: DeliveryEvidence::HandedOff,
                observed_at: None,
                failure_code: None,
                note: None,
            })
            .await
            .map(drop)
    }

    /// Q assesses `keys` of `revision_id` as having no impact.
    pub(crate) async fn assess_no_impact(&self, revision_id: ReportRevisionId, keys: Vec<String>) {
        self.desk().await.sign_in(Role::Quality).await;
        self.lab()
            .engine()
            .record_impact_assessment(RecordImpactAssessmentInput {
                revision_id,
                keys,
                decision: ImpactDecision::NoImpact,
                reason: "Synthetic: released content unaffected".to_owned(),
            })
            .await
            .unwrap();
    }

    /// Report revision IDs in the impact-review work queue, as Q sees it.
    pub(crate) async fn impact_queue(&self) -> Vec<String> {
        self.desk().await.sign_in(Role::Quality).await;
        let items = self
            .lab()
            .engine()
            .list_work_queue_items(ListWorkQueueItemsInput {
                after: None,
                queue: "impact_review".to_owned(),
                limit: 100,
            })
            .await
            .unwrap()
            .items;
        items.into_iter().map(|item| item.item_id).collect()
    }

    /// Rows of a reporting table, read directly as the §3.6 checks name them.
    pub(crate) fn count(&self, table: &str) -> i64 {
        self.lab()
            .t()
            .conn()
            .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    /// The stored `rpt_revisions` row of `revision_id`, as text.
    pub(crate) fn revision_row(&self, revision_id: ReportRevisionId) -> Vec<String> {
        let row = format!("rpt_revisions: [Text(\"{revision_id}\")");
        self.lab()
            .t()
            .fingerprint()
            .into_iter()
            .filter(|line| line.starts_with(&row))
            .collect()
    }

    /// The stored file of an artifact digest.
    pub(crate) fn artifact_path(&self, sha256: &str) -> PathBuf {
        let conn = self.lab().t().conn();
        let database = PathBuf::from(conn.path().unwrap());
        database
            .parent()
            .unwrap()
            .join(lis_store::ARTIFACTS_DIR)
            .join(sha256)
    }

    /// `(seq, correlation ID)` of every event with `action` and `outcome` on `stream`.
    pub(crate) fn events_of(
        &self,
        stream: &str,
        action: &str,
        outcome: &str,
    ) -> Vec<(i64, String)> {
        self.lab()
            .t()
            .conn()
            .prepare(
                "SELECT seq, correlation_id FROM audit_events \
                 WHERE stream = ?1 AND action = ?2 AND outcome = ?3 ORDER BY seq",
            )
            .unwrap()
            .query_map([stream, action, outcome], |row| {
                Ok((row.get(0)?, row.get(1)?))
            })
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// The sequence number on `stream` of the first event of `correlation_id`.
    pub(crate) fn seq_of(&self, stream: &str, correlation_id: &str) -> Option<i64> {
        self.lab()
            .t()
            .conn()
            .query_row(
                "SELECT min(seq) FROM audit_events WHERE stream = ?1 AND correlation_id = ?2",
                [stream, correlation_id],
                |row| row.get(0),
            )
            .unwrap()
    }
}

/// The open impact keys of a report.
pub(crate) fn open_keys(report: &ReportDetail) -> Vec<String> {
    report
        .impact
        .as_ref()
        .map(|impact| {
            impact
                .open
                .iter()
                .map(|reason| reason.key.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// The audit action of the IPC command `name`.
pub(crate) fn action_of(name: &str) -> &'static str {
    lis_engine::registry::COMMANDS
        .iter()
        .find(|spec| spec.name == name)
        .unwrap_or_else(|| panic!("no command {name}"))
        .action
}
