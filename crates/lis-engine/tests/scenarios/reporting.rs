//! Reporting scenarios on the M2 journey laboratory (REQ-RPT-001/002/003/
//! 005/011/012/013, REQ-IDN-004, NFR-01, NFR-07): prepare, release with
//! fresh authentication, supplement and correction, withdrawal, impact,
//! destinations, delivery and export. Expected outcomes come from
//! `docs/planning/m3-design-brief.md` §2.1, §3 and §4, never from the
//! implementation. Synthetic data only.

mod condition_review;

use std::path::PathBuf;

use lis_analytical::{QcOutcome, RevisionIndicator};
use lis_auth::{
    DEFAULT_SEPARATION_POLICY, RoleCode, SeparationPolicy, SetSeparationPolicyInput,
    SetUserRolesInput,
};
use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::{
    AddDeliveryObligationInput, AmendOrderEvidenceInput, AuthorityEvidenceInput,
    AuthorityEvidenceKind, CandidateView, ClosePreservationIncidentInput,
    CreateDeliveryDestinationInput, DeliveryQueueFilter, Engine, EvidenceInput,
    ExportReportPdfInput, GetOrderInput, GetReportCandidateInput, GetReportInput,
    GetReportRevisionInput, GetReportWorkspaceInput, ListDeliveryDestinationsInput,
    ListDeliveryObligationsInput, ListReportsInput, ListWorkQueueItemsInput,
    PointOfCareResultInput, PrepareReportCandidateInput, RecordDeliveryAttemptInput,
    RecordImpactAssessmentInput, RecordTriggerNotMetInput, RedirectDeliveryObligationInput,
    ReleaseReportInput, ReplaceDeliveryDestinationInput, ReportDetail, ReportListFilter,
    RetireDeliveryDestinationInput, RevisionSummary, SetLaboratoryProfile, TriggerSource,
    VerifyDeliveryDestinationInput, WithdrawReportInput, WorkQueueItem,
};
use lis_identity::ClientId;
use lis_reporting::{
    CandidateId, DeliveryChannel, DeliveryEvidence, DeliveryFailureCode, DestinationId,
    ExportPurpose, ImpactDecision, IncidentResolution, ObligationId, ObligationKind,
    ObligationStatus, PreservationIncidentId, ReleaseQueue, ReportCompleteness, ReportId,
    ReportRelationship, ReportRevisionId, RevisionAvailability, StoredRecord,
};
use lis_results::ResultOutcome;
use lis_specimens::{ContainerHoldKind, PlaceContainerHoldInput, ReleaseContainerHoldInput};

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{JOURNEY, current_revisions, gate_codes, line, reapprove};

/// Calls the reporting command `name` with plausible input aimed at random IDs.
pub(crate) async fn reporting_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = report_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = quality_call(engine, name).await {
        return Some(result);
    }
    delivery_call(engine, name).await
}

async fn quality_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let revision_id = ReportRevisionId::generate();
    Some(match name {
        "record_impact_assessment" => engine
            .record_impact_assessment(RecordImpactAssessmentInput {
                revision_id,
                keys: vec!["identity_changed:2".to_owned()],
                decision: ImpactDecision::NoImpact,
                reason: "Synthetic assessment".to_owned(),
            })
            .await
            .map(drop),
        "close_preservation_incident" => engine
            .close_preservation_incident(close(
                PreservationIncidentId::generate(),
                IncidentResolution::AcceptedAsLost,
            ))
            .await
            .map(drop),
        _ => return None,
    })
}

async fn report_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let specimen_id = lis_specimens::SpecimenId::generate();
    let candidate_id = CandidateId::generate();
    let report_id = ReportId::generate();
    let revision_id = ReportRevisionId::generate();
    Some(match name {
        "get_report_workspace" => engine
            .get_report_workspace(GetReportWorkspaceInput { specimen_id })
            .await
            .map(drop),
        "prepare_report_candidate" => engine
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id,
                amendment_reason: None,
            })
            .await
            .map(drop),
        "get_report_candidate" => engine
            .get_report_candidate(GetReportCandidateInput { candidate_id })
            .await
            .map(drop),
        "release_report" => engine
            .release_report(release_input(candidate_id, 1, "0"))
            .await
            .map(drop),
        "withdraw_report" => engine
            .withdraw_report(WithdrawReportInput {
                report_id,
                expected_revision_no: 1,
                reason: "Synthetic withdrawal".to_owned(),
                password: secret(NEW_PASSWORD).into(),
            })
            .await
            .map(drop),
        "list_reports" => engine
            .list_reports(ListReportsInput {
                before_report_id: None,
                filter: ReportListFilter::All,
                search: None,
                limit: 10,
                before_released_at: None,
            })
            .await
            .map(drop),
        "get_report" => engine
            .get_report(GetReportInput { report_id })
            .await
            .map(drop),
        "get_report_revision" => engine
            .get_report_revision(GetReportRevisionInput { revision_id })
            .await
            .map(drop),
        "export_report_pdf" => engine
            .export_report_pdf(
                ExportReportPdfInput {
                    revision_id,
                    purpose: ExportPurpose::Records,
                    obligation_id: None,
                },
                Some(std::env::temp_dir()),
            )
            .await
            .map(drop),
        _ => return None,
    })
}

async fn delivery_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let obligation_id = ObligationId::generate();
    let destination_id = DestinationId::generate();
    let reason = || "Synthetic reason".to_owned();
    Some(match name {
        "list_delivery_obligations" => engine
            .list_delivery_obligations(ListDeliveryObligationsInput {
                filter: DeliveryQueueFilter::All,
                limit: 10,
                after: None,
            })
            .await
            .map(drop),
        "record_delivery_attempt" => engine
            .record_delivery_attempt(attempt(obligation_id, DeliveryEvidence::HandedOff, None))
            .await
            .map(drop),
        "add_delivery_obligation" => engine
            .add_delivery_obligation(AddDeliveryObligationInput {
                revision_id: ReportRevisionId::generate(),
                destination_id,
                reason: reason(),
            })
            .await
            .map(drop),
        "redirect_delivery_obligation" => engine
            .redirect_delivery_obligation(RedirectDeliveryObligationInput {
                obligation_id,
                destination_id,
                reason: reason(),
            })
            .await
            .map(drop),
        "list_delivery_destinations" => engine
            .list_delivery_destinations(ListDeliveryDestinationsInput {
                client_id: None,
                state: None,
            })
            .await
            .map(drop),
        "create_delivery_destination" => engine
            .create_delivery_destination(fax(ClientId::generate(), "555-0100"))
            .await
            .map(drop),
        "verify_delivery_destination" => engine
            .verify_delivery_destination(verify(destination_id))
            .await
            .map(drop),
        "replace_delivery_destination" => engine
            .replace_delivery_destination(ReplaceDeliveryDestinationInput {
                destination_id,
                expected_revision: 1,
                address: Some("555-0199".to_owned()),
                label: "Synthetic fax".to_owned(),
                reason: reason(),
            })
            .await
            .map(drop),
        "retire_delivery_destination" => engine
            .retire_delivery_destination(RetireDeliveryDestinationInput {
                destination_id,
                expected_revision: 1,
                reason: reason(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

fn close(
    incident_id: PreservationIncidentId,
    resolution: IncidentResolution,
) -> ClosePreservationIncidentInput {
    ClosePreservationIncidentInput {
        incident_id,
        expected_revision: Some(1),
        resolution,
        reason: "Synthetic investigation finished".to_owned(),
    }
}

fn release_input(candidate_id: CandidateId, revision: i64, sha: &str) -> ReleaseReportInput {
    ReleaseReportInput {
        candidate_id,
        expected_revision: revision,
        content_sha256: sha.to_owned(),
        acknowledge_partial: true,
        password: secret(NEW_PASSWORD).into(),
        self_approval: false,
    }
}

fn fax(client_id: ClientId, number: &str) -> CreateDeliveryDestinationInput {
    CreateDeliveryDestinationInput {
        client_id,
        client_location_id: None,
        channel: DeliveryChannel::Fax,
        address: Some(number.to_owned()),
        label: format!("Synthetic clinic fax {number}"),
    }
}

fn verify(destination_id: DestinationId) -> VerifyDeliveryDestinationInput {
    VerifyDeliveryDestinationInput {
        destination_id,
        expected_revision: 1,
        method: "Synthetic test fax confirmed by phone".to_owned(),
        self_approval: false,
    }
}

fn attempt(
    obligation_id: ObligationId,
    evidence: DeliveryEvidence,
    failure_code: Option<DeliveryFailureCode>,
) -> RecordDeliveryAttemptInput {
    RecordDeliveryAttemptInput {
        obligation_id,
        evidence,
        observed_at: None,
        failure_code,
        note: None,
    }
}

fn code(result: Result<impl std::fmt::Debug, CommandError>) -> (ErrorCode, Vec<String>) {
    let error = result.unwrap_err();
    (error.code(), gate_codes(&error))
}

/// The M2 laboratory with sample 0 imported, QC accepted and reviewed, a
/// laboratory profile, a releaser, a second administrator and one verified
/// fax destination of the ordering client.
struct Desk {
    lab: Lab,
    releaser: String,
    configurer: String,
    client: ClientId,
    fax: DestinationId,
}

impl Desk {
    async fn start() -> Self {
        let lab = Lab::start_as_with_profile(
            RevisionIndicator::None,
            lis_engine::InstallationKind::Laboratory,
            Some(laboratory_profile()),
        )
        .await;
        lab.import(JOURNEY).await;
        lab.accept_qc().await;
        lab.approve_all(0).await.unwrap();
        lab.sign_in(Role::Admin).await;
        lab.t().create_user("rel1", &[RoleCode::Releaser]).await;
        lab.t()
            .create_user("cfg2", &[RoleCode::Administrator])
            .await;
        let releaser = lab.t().first_sign_in("rel1").await.1;
        let configurer = lab.t().first_sign_in("cfg2").await.1;
        resync(&lab).await;
        let client = ordering_client(&lab).await;
        let mut desk = Self {
            lab,
            releaser,
            configurer,
            client,
            fax: DestinationId::generate(),
        };
        desk.fax = desk.verified_fax("555-0101").await;
        desk
    }

    fn engine(&self) -> &Engine {
        self.lab.engine()
    }

    /// Creates a fax destination as the administrator and verifies it as the second one.
    async fn verified_fax(&self, number: &str) -> DestinationId {
        self.lab.sign_in(Role::Admin).await;
        let created = self
            .engine()
            .create_delivery_destination(fax(self.client, number))
            .await
            .unwrap();
        self.as_configurer().await;
        self.engine()
            .verify_delivery_destination(verify(created.destination_id))
            .await
            .unwrap();
        resync(&self.lab).await;
        created.destination_id
    }

    async fn as_releaser(&self) {
        self.lab
            .t()
            .sign_in_with("rel1", NEW_PASSWORD, &self.releaser)
            .await;
    }

    async fn as_configurer(&self) {
        self.lab
            .t()
            .sign_in_with("cfg2", NEW_PASSWORD, &self.configurer)
            .await;
    }

    async fn prepare(&self, amendment_reason: Option<&str>) -> Result<CandidateView, CommandError> {
        self.as_releaser().await;
        let source = self
            .engine()
            .get_report_workspace(GetReportWorkspaceInput {
                specimen_id: self.lab.samples[0].specimen_id,
            })
            .await?
            .condition_review_source;
        let condition_review =
            (!source.conditions.is_empty()).then(|| lis_reporting::ConditionReportReview {
                source_sha256: source.source_sha256,
                decisions: source
                    .conditions
                    .iter()
                    .map(|condition| lis_reporting::ConditionReportDecision {
                        decision_id: condition.decision_id,
                        code: condition.code,
                        include_in_report: true,
                    })
                    .collect(),
                reason: "Synthetic fixture explicitly retains each supplied condition".to_owned(),
            });
        let prepared = self
            .engine()
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review,
                specimen_id: self.lab.samples[0].specimen_id,
                amendment_reason: amendment_reason.map(str::to_owned),
            })
            .await;
        resync(&self.lab).await;
        prepared
    }

    async fn release(&self, candidate: &CandidateView) -> Result<RevisionSummary, CommandError> {
        self.as_releaser().await;
        let released = self
            .engine()
            .release_report(release_input(
                candidate.candidate_id,
                candidate.revision,
                &candidate.content_sha256,
            ))
            .await;
        resync(&self.lab).await;
        released
    }

    /// Prepares and releases the next revision.
    async fn issue(&self, amendment_reason: Option<&str>) -> RevisionSummary {
        let candidate = self.prepare(amendment_reason).await.unwrap();
        self.release(&candidate).await.unwrap()
    }

    async fn report(&self, report_id: ReportId) -> ReportDetail {
        self.as_releaser().await;
        let detail = self
            .engine()
            .get_report(GetReportInput { report_id })
            .await
            .unwrap();
        resync(&self.lab).await;
        detail
    }

    fn count(&self, sql: &str) -> i64 {
        self.lab
            .t()
            .conn()
            .query_row(sql, [], |row| row.get(0))
            .unwrap()
    }

    fn artifacts_dir(&self) -> PathBuf {
        let database: String = self
            .lab
            .t()
            .conn()
            .query_row("PRAGMA database_list", [], |row| row.get(2))
            .unwrap();
        PathBuf::from(database).parent().unwrap().join("artifacts")
    }
}

fn artifact_count(desk: &Desk) -> usize {
    std::fs::read_dir(desk.artifacts_dir()).unwrap().count()
}

fn remove_artifact(desk: &Desk, sha256: &str) {
    std::fs::remove_file(desk.artifacts_dir().join(sha256)).unwrap();
}

// Establish performing-site provenance before the fixture creates a run.
fn laboratory_profile() -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version: None,
        name: "Synthetic Toxicology Laboratory".to_owned(),
        address_lines: vec!["100 Example Way".to_owned()],
        clia_id: "00D0000000".to_owned(),
        director: "Dr. Example Director".to_owned(),
        phone: "555-0100".to_owned(),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}

async fn ordering_client(lab: &Lab) -> ClientId {
    lab.sign_in(Role::Accessioner).await;
    lab.engine()
        .get_order(GetOrderInput {
            order_id: lab.samples[0].order_id,
        })
        .await
        .unwrap()
        .client_id
}

/// After a sign-in outside the lab's roles, makes the signed-in user and
/// the lab's record of it agree again (the quality manager).
async fn resync(lab: &Lab) {
    lab.sign_in(Role::Analyst).await;
    lab.sign_in(Role::Quality).await;
}

/// Brief §3.1–§3.5, REQ-RPT-005, REQ-IDN-004: the unassessed reflex makes
/// the report partial; release needs acknowledgement and the password; the
/// revision freezes exact content and routes to the verified destination.
#[tokio::test]
async fn release_issues_a_partial_revision_bound_to_fresh_authentication() {
    let desk = Desk::start().await;
    let candidate = partial_candidate(&desk).await;
    refuse_without_acknowledgement_or_password(&desk, &candidate).await;
    let revision = desk.release(&candidate).await.unwrap();
    assert_eq!(
        (revision.revision_no, revision.availability),
        (1, RevisionAvailability::Current)
    );
    assert_release_records(&desk, &candidate, &revision).await;
    // Brief §3.6: releasing the same candidate twice gives one revision.
    assert_eq!(
        code(desk.release(&candidate).await),
        (ErrorCode::Conflict, vec!["candidate_not_ready".to_owned()])
    );
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 1);
}

/// The workspace hint, then the prepared candidate's exact printed strings.
async fn partial_candidate(desk: &Desk) -> CandidateView {
    assert_workspace_hint(desk).await;
    let candidate = desk.prepare(None).await.unwrap();
    assert_eq!(
        (candidate.relationship, candidate.completeness),
        (ReportRelationship::Initial, ReportCompleteness::Partial)
    );
    assert!(candidate.gates.is_empty() && !candidate.stale);
    assert_eq!(candidate.allowed_actions, ["release_report"]);
    assert!(candidate.content.report.issued_at.ends_with("CDT"));
    let rows = &candidate.content.result_groups[0].rows;
    let comparisons: Vec<&str> = rows.iter().map(|row| row.comparison.as_str()).collect();
    assert_eq!(comparisons, ["Meets cutoff", "Does not meet cutoff"]);
    let printed: Vec<(&str, &str)> = rows
        .iter()
        .map(|row| (row.result.as_str(), row.limits.as_str()))
        .collect();
    assert_eq!(
        printed,
        [("152.40", "Cutoff 50 ng/mL"), ("<5.0", "Cutoff 50 ng/mL")]
    );
    assert_eq!(candidate.content.other_tests.len(), 1);
    candidate
}

/// What a candidate prepared now would contain: partial, with no gate.
async fn assert_workspace_hint(desk: &Desk) {
    desk.as_releaser().await;
    let workspace = desk
        .engine()
        .get_report_workspace(GetReportWorkspaceInput {
            specimen_id: desk.lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    resync(&desk.lab).await;
    assert!(
        workspace.preview.gates.is_empty(),
        "{:?}",
        workspace.preview.gates
    );
    assert_eq!(
        workspace.preview.completeness,
        Some(ReportCompleteness::Partial)
    );
    assert!(workspace.report.is_none() && workspace.open_candidate.is_none());
}

/// L3 and REQ-IDN-004: no acknowledgement, or a wrong password (a committed
/// rejection), releases nothing.
async fn refuse_without_acknowledgement_or_password(desk: &Desk, candidate: &CandidateView) {
    let input = || {
        release_input(
            candidate.candidate_id,
            candidate.revision,
            &candidate.content_sha256,
        )
    };
    desk.as_releaser().await;
    let mut unacknowledged = input();
    unacknowledged.acknowledge_partial = false;
    let refused = desk.engine().release_report(unacknowledged).await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Validation);
    let mut wrong = input();
    wrong.password = secret("not the password").into();
    let refused = desk.engine().release_report(wrong).await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Validation);
    resync(&desk.lab).await;
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events WHERE action = 'auth.reauthenticate' \
             AND outcome = 'failed'"
        ),
        1
    );
}

/// The report list holds identifiers only; content reads leave access events.
async fn assert_listed_and_audited(desk: &Desk) {
    let listed = desk
        .engine()
        .list_reports(ListReportsInput {
            before_report_id: None,
            filter: ReportListFilter::Partial,
            search: Some("A".to_owned()),
            limit: 10,
            before_released_at: None,
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert!(
        listed[0]
            .report_number
            .starts_with(&listed[0].accession_number)
    );
    desk.engine()
        .list_delivery_obligations(ListDeliveryObligationsInput {
            filter: DeliveryQueueFilter::All,
            limit: 10,
            after: None,
        })
        .await
        .unwrap();
    desk.engine()
        .list_delivery_destinations(ListDeliveryDestinationsInput {
            client_id: Some(desk.client),
            state: None,
        })
        .await
        .unwrap();
    let access = desk.lab.t().events("access");
    for action in [
        "report.list_reports",
        "report.list_obligations",
        "report.list_destinations",
        "report.get_workspace",
        "report.prepare_candidate",
        "report.get_revision",
    ] {
        assert!(access.iter().any(|(a, _)| a == action), "{action}");
    }
}

/// The release manifest, released lines, routing, exact content and access events.
async fn assert_release_records(
    desk: &Desk,
    candidate: &CandidateView,
    revision: &RevisionSummary,
) {
    // The release manifest and the fresh authentication share one correlation ID.
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events r JOIN audit_events a \
             ON a.correlation_id = r.correlation_id \
             WHERE r.action = 'report.release' AND a.action = 'auth.reauthenticate' \
             AND a.outcome = 'succeeded' AND a.stream = 'security'"
        ),
        1
    );
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_v_released_lines WHERE disposition = 'reported'"),
        2
    );
    let report = desk.report(candidate.report_id).await;
    assert_eq!(report.obligations.len(), 1);
    assert_eq!(
        report.obligations[0].status.code,
        ObligationStatus::Awaiting
    );
    assert_eq!(
        report.obligations[0].status.label,
        "Fax by staff: awaiting delivery"
    );
    desk.lab.sign_in(Role::Accessioner).await;
    let detail = desk
        .engine()
        .get_report_revision(GetReportRevisionInput {
            revision_id: revision.revision_id,
        })
        .await
        .unwrap();
    assert_eq!(detail.content, candidate.content);
    assert_eq!(detail.provenance.content_sha256, candidate.content_sha256);
    assert_listed_and_audited(desk).await;
}

/// REQ-RPT-002, REQ-RPT-013, NFR-07: a supplement and a correction are new
/// revisions; earlier rows and bytes never change; stale obligations close
/// and earlier destinations get follow-ups.
#[tokio::test]
async fn supplements_and_corrections_are_new_revisions_that_keep_earlier_bytes() {
    let desk = Desk::start().await;
    let first = desk.issue(None).await;
    let export_dir = tempfile::tempdir().unwrap();
    let first_bytes = export(&desk, first.revision_id, &export_dir).await;
    assert_eq!(
        desk.count(&format!(
            "SELECT count(*) FROM rpt_exports WHERE artifact_sha256 = '{}'",
            first.artifact_sha256
        )),
        1
    );
    assert_eq!(
        code(desk.prepare(None).await),
        (ErrorCode::Conflict, vec!["nothing_changed".to_owned()])
    );

    // The client's point-of-care result means the reflex condition is not
    // met: the report becomes final.
    condition_not_met(&desk.lab, 0).await;
    let supplement = desk.prepare(None).await.unwrap();
    assert_eq!(
        (supplement.relationship, supplement.completeness),
        (ReportRelationship::Supplement, ReportCompleteness::Final)
    );
    let second = desk.release(&supplement).await.unwrap();
    assert_eq!(second.revision_no, 2);

    // A reported result changes: a correction needs a reason.
    revise_morphine(&desk).await;
    assert_eq!(
        desk.prepare(None).await.unwrap_err().code(),
        ErrorCode::Validation
    );
    let correction = desk
        .prepare(Some("Synthetic corrected result"))
        .await
        .unwrap();
    assert_eq!(correction.relationship, ReportRelationship::Correction);
    assert_eq!(correction.supersedes_revision_no, Some(2));
    assert_eq!(
        correction.content.result_groups[0].rows[0].result,
        "Invalid"
    );
    desk.release(&correction).await.unwrap();

    let report = desk.report(supplement.report_id).await;
    let states: Vec<(i64, RevisionAvailability)> = report
        .revisions
        .iter()
        .map(|r| (r.revision_no, r.availability))
        .collect();
    assert_eq!(
        states,
        [
            (1, RevisionAvailability::Superseded),
            (2, RevisionAvailability::Superseded),
            (3, RevisionAvailability::Current)
        ]
    );
    let open: Vec<i64> = report
        .obligations
        .iter()
        .filter(|o| o.status.code == ObligationStatus::Awaiting)
        .map(|o| o.revision_no)
        .collect();
    assert_eq!(open, [3], "earlier obligations close as newer_revision");
    // VAL-05: the first revision's bytes are unchanged after the corrections.
    assert_eq!(
        export(&desk, first.revision_id, &export_dir).await,
        first_bytes
    );
    // Findings 10 and B2: each export's intent precedes its record.
    assert_eq!(
        export_events(&desk),
        [
            "export_intent",
            "report_export",
            "export_intent",
            "report_export"
        ]
    );
    assert_billing_work(&desk).await;
}

/// Request 9: the current final revision is billing work, titled by its
/// accession number.
/// The record types of the report export events, in order.
fn export_events(desk: &Desk) -> Vec<String> {
    desk.lab
        .t()
        .conn()
        .prepare(
            "SELECT record_type FROM audit_events \
             WHERE action = 'report.export_pdf' AND stream = 'security' ORDER BY seq",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

async fn assert_billing_work(desk: &Desk) {
    desk.lab.sign_in(Role::Admin).await;
    desk.lab.t().create_user("bil1", &[RoleCode::Billing]).await;
    let secret = desk.lab.t().first_sign_in("bil1").await.1;
    desk.lab
        .t()
        .sign_in_with("bil1", NEW_PASSWORD, &secret)
        .await;
    let work = queue_items(desk, "billing_work").await;
    resync(&desk.lab).await;
    assert_eq!(work.len(), 1, "{work:?}");
    assert_eq!(
        (work[0].state.as_str(), work[0].next_action.as_str()),
        ("needs_derivation", "Derive services")
    );
    assert!(work[0].title.starts_with('A'), "{}", work[0].title);
}

/// A work queue's items as the signed-in user sees them.
async fn queue_items(desk: &Desk, queue: &str) -> Vec<WorkQueueItem> {
    desk.engine()
        .list_work_queue_items(ListWorkQueueItemsInput {
            after: None,
            queue: queue.to_owned(),
            limit: 10,
        })
        .await
        .unwrap()
        .items
}

/// Review finding 4 (REQ-OPS-004): ready and blocked specimens are told
/// apart before the limit, so an older blocked specimen never hides a
/// ready one.
#[tokio::test]
async fn the_release_queues_filter_before_the_limit() {
    let desk = Desk::start().await;
    desk.lab.sign_in(Role::Quality).await;
    desk.engine()
        .place_hold(PlaceContainerHoldInput {
            container_id: desk.lab.samples[0].container_id,
            kind: ContainerHoldKind::Quality,
            reason: "Synthetic quality investigation".to_owned(),
        })
        .await
        .unwrap();
    desk.lab.approve_all(1).await.unwrap();
    desk.lab.approve_all(2).await.unwrap();
    let specimen = |index: usize| desk.lab.samples[index].specimen_id;
    let conn = desk.lab.t().conn();
    for (queue, index, has_more) in [
        (ReleaseQueue::Ready, 1, true),
        (ReleaseQueue::Blocked, 0, false),
    ] {
        let owner = lis_reporting::release_queue(&conn, queue, 1).unwrap();
        assert_eq!(owner.len(), 1, "{queue:?}");
        assert_eq!(owner[0].specimen_id, specimen(index), "{queue:?}");
        assert!(owner[0].report_number.is_none());
        assert!(
            owner[0]
                .accession_number
                .as_deref()
                .is_some_and(|number| number.starts_with('A'))
        );
        assert_eq!(owner[0].has_more, Some(has_more));
    }
    desk.as_releaser().await;
    for (queue, index) in [("release_ready", 1), ("release_blocked", 0)] {
        let items = desk
            .engine()
            .list_work_queue_items(ListWorkQueueItemsInput {
                after: None,
                queue: queue.to_owned(),
                limit: 1,
            })
            .await
            .unwrap()
            .items;
        assert_eq!(items.len(), 1, "{queue}");
        assert_eq!(items[0].item_id, specimen(index).to_string(), "{queue}");
    }
    resync(&desk.lab).await;
}

/// Adds a synthetic point-of-care result to sample `index`'s order and records
/// that the confirmation reflex condition is not met on its basis.
pub(crate) async fn condition_not_met(lab: &Lab, index: usize) {
    lab.sign_in(Role::Accessioner).await;
    let order_id = lab.samples[index].order_id;
    let order = lab
        .engine()
        .get_order(GetOrderInput { order_id })
        .await
        .unwrap();
    let amended = lab
        .engine()
        .amend_order_evidence(AmendOrderEvidenceInput {
            order_id,
            expected_revision: order.revision,
            add: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::WetSignature,
                    document_ref: Some("SYN-REQ-M2".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: vec![PointOfCareResultInput {
                    device_or_test: "Synthetic cup".to_owned(),
                    analyte_text: "Synthetic opiates".to_owned(),
                    result_text: "Synthetic not detected".to_owned(),
                    performed_at_text: None,
                    source: "Synthetic clinic".to_owned(),
                }],
            },
            remove_evidence_ids: Vec::new(),
            reason: "Synthetic point-of-care result received".to_owned(),
        })
        .await
        .unwrap();
    let poc = amended.evidence.point_of_care_results[0].evidence_id;
    lab.engine()
        .record_trigger_not_met(RecordTriggerNotMetInput {
            parent_item_id: lab.samples[index].item_id,
            orderable_id: lab.orderables.confirmation.0,
            trigger_source: TriggerSource::ClientPointOfCare,
            trigger_ref: poc.to_string(),
            reason: "Synthetic condition not met".to_owned(),
        })
        .await
        .unwrap();
}

/// Records a new `invalid` revision of sample 0's morphine result and reviews it.
async fn revise_morphine(desk: &Desk) {
    let results = desk.lab.results(0).await;
    let morphine = line(&results, "SYN-MOR");
    desk.engine()
        .record_result_disposition(lis_engine::RecordResultDispositionInput {
            result_id: Some(morphine.result_id),
            expected_revision: morphine.revision,
            component_id: None,
            analyte_id: None,
            specimen_id: None,
            outcome: ResultOutcome::Invalid,
            reason: "Synthetic invalid result".to_owned(),
        })
        .await
        .unwrap();
    desk.lab.approve_all(0).await.unwrap();
}

async fn export(desk: &Desk, revision_id: ReportRevisionId, dir: &tempfile::TempDir) -> Vec<u8> {
    desk.lab.sign_in(Role::Accessioner).await;
    let exported = desk
        .engine()
        .export_report_pdf(
            ExportReportPdfInput {
                revision_id,
                purpose: ExportPurpose::Records,
                obligation_id: None,
            },
            Some(dir.path().to_owned()),
        )
        .await
        .unwrap();
    assert_eq!(exported.folder, "downloads");
    std::fs::read(dir.path().join(exported.file_name)).unwrap()
}

/// Brief §4.2, L12: withdrawal is terminal, needs the password, notifies
/// every destination and closes open report obligations.
#[tokio::test]
async fn withdrawal_is_terminal_and_notifies_every_destination() {
    let desk = Desk::start().await;
    let candidate = desk.prepare(None).await.unwrap();
    let revision = desk.release(&candidate).await.unwrap();
    let withdraw = |password: &str, expected_revision_no| WithdrawReportInput {
        report_id: candidate.report_id,
        expected_revision_no,
        reason: "Synthetic wrong patient".to_owned(),
        password: secret(password).into(),
    };
    desk.as_releaser().await;
    let artifacts_before = artifact_count(&desk);
    let wrong = desk
        .engine()
        .withdraw_report(withdraw("wrong password", 1))
        .await;
    assert_eq!(wrong.unwrap_err().code(), ErrorCode::Validation);
    assert_eq!(artifact_count(&desk), artifacts_before);
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_withdrawal_notice_artifacts"),
        0
    );
    let stale = desk
        .engine()
        .withdraw_report(withdraw(NEW_PASSWORD, 2))
        .await;
    assert_eq!(code(stale).0, ErrorCode::Conflict);
    assert_eq!(artifact_count(&desk), artifacts_before);
    let withdrawn = desk
        .engine()
        .withdraw_report(withdraw(NEW_PASSWORD, 1))
        .await
        .unwrap();
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_withdrawal_notice_artifacts"),
        1
    );
    resync(&desk.lab).await;
    assert_eq!(
        withdrawn.availability,
        Some(RevisionAvailability::Withdrawn)
    );
    let kinds: Vec<(ObligationKind, ObligationStatus)> = withdrawn
        .obligations
        .iter()
        .map(|o| (o.kind, o.status.code))
        .collect();
    assert_eq!(
        kinds,
        [
            (ObligationKind::WithdrawalNotice, ObligationStatus::Awaiting),
            (ObligationKind::Report, ObligationStatus::Closed)
        ]
    );
    assert_eq!(
        code(desk.prepare(None).await),
        (ErrorCode::Conflict, vec!["report_withdrawn".to_owned()])
    );
    // The notice is exported through its obligation, never the withdrawn PDF for delivery.
    desk.lab.sign_in(Role::Accessioner).await;
    let dir = tempfile::tempdir().unwrap();
    let notice = desk
        .engine()
        .export_report_pdf(
            ExportReportPdfInput {
                revision_id: revision.revision_id,
                purpose: ExportPurpose::Delivery,
                obligation_id: Some(withdrawn.obligations[0].obligation_id),
            },
            Some(dir.path().to_owned()),
        )
        .await
        .unwrap();
    assert!(
        notice.file_name.ends_with("-withdrawal.pdf"),
        "{}",
        notice.file_name
    );
}

/// H5, M4 brief §3.8: while a recovery reconciliation is open a delivery export is
/// refused before any file is reserved or written, and no export intent is recorded.
#[tokio::test]
async fn a_delivery_export_writes_no_file_while_recovery_is_open() {
    let desk = Desk::start().await;
    let candidate = desk.prepare(None).await.unwrap();
    let revision = desk.release(&candidate).await.unwrap();
    let report = desk.report(candidate.report_id).await;
    let obligation = report
        .obligations
        .iter()
        .find(|o| o.kind == ObligationKind::Report)
        .unwrap()
        .obligation_id;
    open_recovery_reconciliation(&desk);
    desk.lab.sign_in(Role::Accessioner).await;
    let dir = tempfile::tempdir().unwrap();
    let refused = desk
        .engine()
        .export_report_pdf(
            ExportReportPdfInput {
                revision_id: revision.revision_id,
                purpose: ExportPurpose::Delivery,
                obligation_id: Some(obligation),
            },
            Some(dir.path().to_owned()),
        )
        .await;
    assert_eq!(
        code(refused),
        (
            ErrorCode::GateBlocked,
            vec![lis_ops::recovery::GATE_CODE.to_owned()]
        )
    );
    assert_eq!(std::fs::read_dir(dir.path()).unwrap().count(), 0);
    assert_eq!(
        desk.count("SELECT count(*) FROM audit_events WHERE record_type = 'export_intent'"),
        0
    );
}

/// A restore record without a closure: the recovery reconciliation is open.
fn open_recovery_reconciliation(desk: &Desk) {
    let digest = "a".repeat(64);
    desk.lab
        .t()
        .conn()
        .execute(
            "INSERT INTO ops_restores (id, set_id, set_created_at, set_kind, snapshot_sha256, \
             backup_app_version, staged_at, applied_at, recorded_at, anchors_compared, \
             anchors_matched, anchors_lost_interval, anchors_unverified, anchors_mismatched, \
             anchors_missing, anchor_conflicts, anchor_gaps, anchors_unanchored, \
             applied_event_seq, applied_event_hash, accession_floor, container_floor) VALUES \
             ('01890000-0000-7000-8000-000000000001', 'synthetic-set', ?1, 'manual', ?2, \
             '0.0.0', ?1, ?1, ?1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, ?2, 'A26000001', 'C00000001')",
            rusqlite::params!["2026-10-13T00:00:00.000000000Z", digest],
        )
        .unwrap();
}

/// Brief §3.4 rows 4, 6 and 13: a missing file, an expired candidate,
/// another signatory and the technical reviewer are each refused, and
/// nothing is released.
#[tokio::test]
async fn release_refuses_missing_files_expiry_other_signatories_and_self_release() {
    let desk = Desk::start().await;
    let missing = desk.prepare(None).await.unwrap();
    remove_artifact(&desk, missing.artifact_sha256.as_deref().unwrap());
    assert_eq!(
        code(desk.release(&missing).await),
        (ErrorCode::Conflict, vec!["artifact_missing".to_owned()])
    );
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_preservation_incidents"),
        1
    );
    // Review finding 5: quality staff find the incident in their queue.
    desk.lab.sign_in(Role::Quality).await;
    let incidents = queue_items(&desk, "preservation_incidents").await;
    assert_eq!(
        (incidents.len(), incidents[0].title.as_str()),
        (1, missing.report_number.as_str())
    );
    assert_eq!(incidents[0].state, "missing");
    assert!(
        incidents[0]
            .item_id
            .starts_with(&missing.report_id.to_string())
    );

    let expired = desk.prepare(None).await.unwrap();
    desk.lab.t().advance(61 * 60);
    assert_eq!(
        code(desk.release(&expired).await),
        (ErrorCode::Conflict, vec!["candidate_expired".to_owned()])
    );

    // The technical reviewer also becomes a releaser: a clinical role, granted by the
    // quality manager with a fresh password.
    desk.lab.sign_in(Role::Quality).await;
    let reviewer = desk
        .engine()
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|u| u.username == "rev1")
        .unwrap();
    desk.engine()
        .set_user_roles(SetUserRolesInput {
            user_id: reviewer.user_id,
            roles: vec![RoleCode::TechnicalReviewer, RoleCode::Releaser],
            expected_revision: reviewer.revision,
            reason: "Synthetic second role".to_owned(),
            password: secret(NEW_PASSWORD).into(),
        })
        .await
        .unwrap();
    let others = desk.prepare(None).await.unwrap();
    desk.lab.sign_in(Role::Reviewer).await;
    let release_as_reviewer =
        |c: &CandidateView| release_input(c.candidate_id, c.revision, &c.content_sha256);
    assert_eq!(
        desk.engine()
            .release_report(release_as_reviewer(&others))
            .await
            .unwrap_err()
            .code(),
        ErrorCode::Forbidden
    );
    let own = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: desk.lab.samples[0].specimen_id,
            amendment_reason: None,
        })
        .await
        .unwrap();
    assert_eq!(
        code(
            desk.engine()
                .release_report(release_as_reviewer(&own))
                .await
        ),
        (ErrorCode::GateBlocked, vec!["self_release".to_owned()])
    );
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);
}

/// ADR 0015 (REQ-RES-005): where the laboratory's separation-of-duties policy allows
/// it, the technical reviewer releases their own report, but only after confirming
/// the self-release, which the audit trail marks.
#[tokio::test]
async fn the_reviewer_releases_their_own_report_under_the_policy_after_confirming() {
    let desk = Desk::start().await;
    desk.lab.sign_in(Role::Quality).await;
    let reviewer = desk
        .engine()
        .list_users()
        .await
        .unwrap()
        .into_iter()
        .find(|u| u.username == "rev1")
        .unwrap();
    desk.engine()
        .set_user_roles(SetUserRolesInput {
            user_id: reviewer.user_id,
            roles: vec![RoleCode::TechnicalReviewer, RoleCode::Releaser],
            expected_revision: reviewer.revision,
            reason: "Synthetic second role".to_owned(),
            password: secret(NEW_PASSWORD).into(),
        })
        .await
        .unwrap();
    desk.engine()
        .set_separation_policy(SetSeparationPolicyInput {
            expected_version: None,
            values: SeparationPolicy {
                self_release_reports: true,
                ..DEFAULT_SEPARATION_POLICY
            },
            reason: "Synthetic single-reviewer shift".to_owned(),
            password: secret(NEW_PASSWORD).into(),
        })
        .await
        .unwrap();
    desk.lab.sign_in(Role::Reviewer).await;
    let own = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: desk.lab.samples[0].specimen_id,
            amendment_reason: None,
        })
        .await
        .unwrap();
    let input = || release_input(own.candidate_id, own.revision, &own.content_sha256);
    let refused = desk.engine().release_report(input()).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    let fields = serde_json::to_value(&refused).unwrap()["fieldErrors"].to_string();
    assert!(fields.contains("selfApproval"), "{fields}");
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);

    let mut confirmed = input();
    confirmed.self_approval = true;
    desk.engine().release_report(confirmed).await.unwrap();
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 1);
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM audit_events WHERE action = 'report.release' \
             AND json_extract(details, '$.self_approval') IS 1"
        ),
        1
    );
}

/// NFR-01, brief §3.6 "X first": a container quality hold placed after
/// preparation blocks release; a QC rejection makes the candidate stale.
#[tokio::test]
async fn release_rechecks_holds_and_qc_committed_after_preparation() {
    let desk = Desk::start().await;
    let candidate = desk.prepare(None).await.unwrap();
    desk.lab.sign_in(Role::Quality).await;
    let hold = desk
        .engine()
        .place_hold(PlaceContainerHoldInput {
            container_id: desk.lab.samples[0].container_id,
            kind: ContainerHoldKind::Quality,
            reason: "Synthetic quality investigation".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(
        code(desk.release(&candidate).await),
        (
            ErrorCode::GateBlocked,
            vec!["container_quality_hold".to_owned()]
        )
    );
    desk.engine()
        .release_hold(ReleaseContainerHoldInput {
            hold_id: hold.hold_id,
            expected_revision: hold.revision,
            reason: "Synthetic investigation closed".to_owned(),
        })
        .await
        .unwrap();
    desk.lab
        .assess_qc(desk.lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    let (error, reasons) = code(desk.release(&candidate).await);
    assert_eq!(error, ErrorCode::Conflict);
    assert!(
        reasons.contains(&"candidate_stale".to_owned()),
        "{reasons:?}"
    );
    assert!(
        reasons.contains(&"review_stale:qc_rejected".to_owned()),
        "{reasons:?}"
    );
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);
    assert_eq!(
        desk.count("SELECT count(*) FROM audit_events WHERE action = 'report.release'"),
        0
    );
}

/// Brief §4.3, L13 and "release first": a QC rejection after release opens
/// impact; delivery pauses until the quality manager assesses it.
#[tokio::test]
async fn impact_after_release_pauses_delivery_until_assessed() {
    let desk = Desk::start().await;
    let candidate = desk.prepare(None).await.unwrap();
    let revision = desk.release(&candidate).await.unwrap();
    desk.lab
        .assess_qc(desk.lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    let report = desk.report(candidate.report_id).await;
    let impact = report.impact.unwrap();
    let keys: Vec<String> = impact.open.iter().map(|r| r.key.clone()).collect();
    assert!(
        keys.iter()
            .any(|k| k.starts_with("review_stale:") && k.contains(":qc_rejected:")),
        "{keys:?}"
    );
    let obligation = report.obligations[0].obligation_id;
    assert_eq!(report.obligations[0].status.code, ObligationStatus::Paused);
    desk.lab.sign_in(Role::Accessioner).await;
    assert_eq!(
        code(
            desk.engine()
                .record_delivery_attempt(attempt(obligation, DeliveryEvidence::HandedOff, None))
                .await
        ),
        (ErrorCode::Conflict, vec!["revision_impact_open".to_owned()])
    );
    desk.lab.sign_in(Role::Quality).await;
    let assessed = desk
        .engine()
        .record_impact_assessment(RecordImpactAssessmentInput {
            revision_id: revision.revision_id,
            keys,
            decision: ImpactDecision::NoImpact,
            reason: "Synthetic: other controls in range".to_owned(),
        })
        .await
        .unwrap();
    assert!(assessed.open.is_empty() && !assessed.correction_due);
    desk.lab.sign_in(Role::Accessioner).await;
    let handed = desk
        .engine()
        .record_delivery_attempt(attempt(obligation, DeliveryEvidence::HandedOff, None))
        .await
        .unwrap();
    assert_eq!(handed.status.code, ObligationStatus::HandedOff);
    assert!(
        handed
            .status
            .label
            .starts_with("Fax by staff: handed off (attested by Test acc1, "),
        "{}",
        handed.status.label
    );
}

/// REQ-RPT-012, REQ-RPT-011: a destination is verified by a second person;
/// unverified or retired destinations block attempts; a failed attempt
/// needs its code; an open obligation is redirected, never re-created.
#[tokio::test]
async fn destinations_need_a_second_person_and_attempts_recheck_them() {
    let desk = Desk::start().await;
    let pending = unverified_fax(&desk).await;
    let revision = desk.issue(None).await;
    let report = desk
        .report(revision_report(&desk, revision.revision_id).await)
        .await;
    let obligation_to = |destination| {
        report
            .obligations
            .iter()
            .find(|o| o.destination.destination_id == destination)
            .unwrap()
    };
    let blocked = obligation_to(pending);
    assert_eq!(
        blocked.status.label,
        "Fax by staff: blocked (destination not verified)"
    );
    desk.lab.sign_in(Role::Accessioner).await;
    let handed_off = attempt(blocked.obligation_id, DeliveryEvidence::HandedOff, None);
    assert_eq!(
        code(desk.engine().record_delivery_attempt(handed_off).await),
        (
            ErrorCode::GateBlocked,
            vec!["destination_not_verified".to_owned()]
        )
    );
    let main = obligation_to(desk.fax).obligation_id;
    let uncoded = attempt(main, DeliveryEvidence::Failed, None);
    let refused = desk.engine().record_delivery_attempt(uncoded).await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Validation);
    let busy = attempt(
        main,
        DeliveryEvidence::Failed,
        Some(DeliveryFailureCode::NoAnswerOrBusy),
    );
    let failed = desk.engine().record_delivery_attempt(busy).await.unwrap();
    assert_eq!(
        failed.status.label,
        "Fax by staff: failed (busy or no answer)"
    );
    redirect_from_retired(&desk, main).await;
}

/// A destination its creator may not verify.
async fn unverified_fax(desk: &Desk) -> DestinationId {
    desk.lab.sign_in(Role::Admin).await;
    let pending = desk
        .engine()
        .create_delivery_destination(fax(desk.client, "555-0102"))
        .await
        .unwrap();
    let own = desk
        .engine()
        .verify_delivery_destination(verify(pending.destination_id))
        .await;
    assert_eq!(
        code(own),
        (
            ErrorCode::GateBlocked,
            vec!["second_person_required".to_owned()]
        )
    );
    pending.destination_id
}

/// The main fax is retired; its open obligation is redirected to a new,
/// verified destination and the old one closes.
async fn redirect_from_retired(desk: &Desk, main: ObligationId) {
    desk.lab.sign_in(Role::Admin).await;
    desk.engine()
        .retire_delivery_destination(RetireDeliveryDestinationInput {
            destination_id: desk.fax,
            expected_revision: 1,
            reason: "Synthetic number changed".to_owned(),
        })
        .await
        .unwrap();
    let replacement = desk.verified_fax("555-0103").await;
    desk.lab.sign_in(Role::Accessioner).await;
    let redirected = desk
        .engine()
        .redirect_delivery_obligation(RedirectDeliveryObligationInput {
            obligation_id: main,
            destination_id: replacement,
            reason: "Synthetic new number".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(redirected.status.code, ObligationStatus::Awaiting);
    let queue = desk
        .engine()
        .list_delivery_obligations(ListDeliveryObligationsInput {
            filter: DeliveryQueueFilter::All,
            limit: 50,
            after: None,
        })
        .await
        .unwrap();
    assert!(
        queue.items.iter().all(|o| o.obligation_id != main),
        "the old obligation is closed"
    );
    assert!(
        queue
            .items
            .iter()
            .all(|o| !o.status.label.to_lowercase().contains("delivered"))
    );
}

async fn revision_report(desk: &Desk, revision_id: ReportRevisionId) -> ReportId {
    desk.lab.sign_in(Role::Accessioner).await;
    desk.engine()
        .get_report_revision(GetReportRevisionInput { revision_id })
        .await
        .unwrap()
        .report_id
}

/// Review finding 1 (REQ-RPT-012): an attempt that finds the address wrong
/// blocks every attempt and new obligation to that destination, and the
/// destination waits in the configuration queue, until it is replaced and
/// the obligation redirected.
#[tokio::test]
async fn a_wrong_destination_blocks_further_attempts_until_replaced_and_redirected() {
    let desk = Desk::start().await;
    let revision = desk.issue(None).await;
    let report = desk
        .report(revision_report(&desk, revision.revision_id).await)
        .await;
    let main = report.obligations[0].obligation_id;
    desk.lab.sign_in(Role::Accessioner).await;
    let wrong = attempt(
        main,
        DeliveryEvidence::Failed,
        Some(DeliveryFailureCode::WrongDestination),
    );
    let blocked = desk.engine().record_delivery_attempt(wrong).await.unwrap();
    assert_eq!(
        (blocked.status.code, blocked.status.label.as_str()),
        (
            ObligationStatus::Blocked,
            "Fax by staff: blocked (destination reported wrong)"
        )
    );
    assert_eq!(blocked.allowed_actions, ["redirect_delivery_obligation"]);
    let wrong_gate = (
        ErrorCode::GateBlocked,
        vec!["destination_reported_wrong".to_owned()],
    );
    let retry = attempt(main, DeliveryEvidence::HandedOff, None);
    assert_eq!(
        code(desk.engine().record_delivery_attempt(retry).await),
        wrong_gate
    );
    assert_eq!(
        code(
            desk.engine()
                .add_delivery_obligation(AddDeliveryObligationInput {
                    revision_id: revision.revision_id,
                    destination_id: desk.fax,
                    reason: "Synthetic second copy".to_owned(),
                })
                .await
        ),
        wrong_gate
    );
    assert_eq!(
        destination_queue(&desk).await,
        [(desk.fax, "reported_wrong")]
    );
    replace_and_redirect(&desk, main).await;
}

/// The unverified-destination queue as configuration staff see it.
async fn destination_queue(desk: &Desk) -> Vec<(DestinationId, &'static str)> {
    desk.lab.sign_in(Role::Admin).await;
    let items = desk
        .engine()
        .list_work_queue_items(ListWorkQueueItemsInput {
            after: None,
            queue: "unverified_destinations".to_owned(),
            limit: 10,
        })
        .await
        .unwrap()
        .items;
    items
        .iter()
        .map(|item| {
            let state = match item.state.as_str() {
                "reported_wrong" => "reported_wrong",
                "unverified" => "unverified",
                other => panic!("unexpected destination state {other}"),
            };
            (item.item_id.parse().unwrap(), state)
        })
        .collect()
}

/// Replacing the wrong fax empties the queue once the replacement is
/// verified; the redirected obligation can then be handed off.
async fn replace_and_redirect(desk: &Desk, main: ObligationId) {
    desk.lab.sign_in(Role::Admin).await;
    let replacement = desk
        .engine()
        .replace_delivery_destination(ReplaceDeliveryDestinationInput {
            destination_id: desk.fax,
            expected_revision: 1,
            address: Some("555-0104".to_owned()),
            label: "Synthetic clinic fax 555-0104".to_owned(),
            reason: "Synthetic: the old number reached another office".to_owned(),
        })
        .await
        .unwrap();
    assert_eq!(
        destination_queue(desk).await,
        [(replacement.destination_id, "unverified")]
    );
    desk.as_configurer().await;
    desk.engine()
        .verify_delivery_destination(verify(replacement.destination_id))
        .await
        .unwrap();
    resync(&desk.lab).await;
    assert!(destination_queue(desk).await.is_empty());
    desk.lab.sign_in(Role::Accessioner).await;
    let redirected = desk
        .engine()
        .redirect_delivery_obligation(RedirectDeliveryObligationInput {
            obligation_id: main,
            destination_id: replacement.destination_id,
            reason: "Synthetic corrected number".to_owned(),
        })
        .await
        .unwrap();
    let handed = desk
        .engine()
        .record_delivery_attempt(attempt(
            redirected.obligation_id,
            DeliveryEvidence::HandedOff,
            None,
        ))
        .await
        .unwrap();
    assert_eq!(handed.status.code, ObligationStatus::HandedOff);
}

/// Review finding 2 (L4, REQ-RPT-001): R reviews and prepares, T reviews
/// the unchanged revision again, and R's release is refused as stale: the
/// candidate names R's review, so the live two-person check must not judge
/// T's instead.
#[tokio::test]
async fn release_refuses_a_candidate_whose_reviews_were_replaced() {
    let desk = Desk::start().await;
    let reviser = reviewing_releaser(&desk).await;
    let revisions = current_revisions(&desk.lab.results(0).await);
    reviser.sign_in(&desk).await;
    reapprove(desk.engine(), desk.lab.samples[0].specimen_id, revisions).await;
    let candidate = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: desk.lab.samples[0].specimen_id,
            amendment_reason: None,
        })
        .await
        .unwrap();
    resync(&desk.lab).await;
    desk.lab.sign_in(Role::Reviewer).await;
    let revisions = current_revisions(&desk.lab.results(0).await);
    reapprove(desk.engine(), desk.lab.samples[0].specimen_id, revisions).await;
    reviser.sign_in(&desk).await;
    let released = desk
        .engine()
        .release_report(release_input(
            candidate.candidate_id,
            candidate.revision,
            &candidate.content_sha256,
        ))
        .await;
    resync(&desk.lab).await;
    let (error, gates) = code(released);
    assert_eq!(error, ErrorCode::Conflict);
    assert!(
        gates.contains(&"review_stale:review_replaced".to_owned()),
        "{gates:?}"
    );
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);
}

/// A second account holding both the technical reviewer and releaser roles.
struct Account {
    username: &'static str,
    secret: String,
}

impl Account {
    async fn sign_in(&self, desk: &Desk) {
        desk.lab
            .t()
            .sign_in_with(self.username, NEW_PASSWORD, &self.secret)
            .await;
    }
}

async fn reviewing_releaser(desk: &Desk) -> Account {
    desk.lab.sign_in(Role::Admin).await;
    desk.lab
        .t()
        .create_user("revrel", &[RoleCode::TechnicalReviewer, RoleCode::Releaser])
        .await;
    let secret = desk.lab.t().first_sign_in("revrel").await.1;
    resync(&desk.lab).await;
    Account {
        username: "revrel",
        secret,
    }
}

async fn assert_damaged_workspace_recovery(desk: &Desk, candidate_id: CandidateId) {
    desk.as_releaser().await;
    let workspace = desk
        .engine()
        .get_report_workspace(GetReportWorkspaceInput {
            specimen_id: desk.lab.samples[0].specimen_id,
        })
        .await
        .unwrap();
    assert_eq!(workspace.unavailable_candidate_id, Some(candidate_id));
    assert!(workspace.open_candidate.is_none());
    assert!(workspace.preview.content.is_some());
    assert!(
        workspace
            .allowed_actions
            .iter()
            .any(|a| a == "prepare_report_candidate")
    );
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM rpt_preservation_incidents WHERE candidate_id IS NOT NULL"
        ),
        1
    );
    resync(&desk.lab).await;
}

/// Review finding 3 (NFR-07): stored candidate or revision JSON that fails
/// its digest is never shown or released; its detection records one
/// preservation incident naming the record, which later detections reuse
/// while it is open. The report itself stays readable, without impact.
#[tokio::test]
async fn tampered_report_json_is_an_incident_and_never_shown_or_released() {
    let desk = Desk::start().await;
    let candidate = desk.prepare(None).await.unwrap();
    tamper(&desk, "data_json", &candidate.candidate_id.to_string());
    assert_damaged_workspace_recovery(&desk, candidate.candidate_id).await;
    let mismatch = (
        ErrorCode::Conflict,
        vec!["stored_content_mismatch".to_owned()],
    );
    desk.as_releaser().await;
    let shown = desk
        .engine()
        .get_report_candidate(GetReportCandidateInput {
            candidate_id: candidate.candidate_id,
        })
        .await;
    assert_eq!(code(shown), mismatch);
    assert_eq!(code(desk.release(&candidate).await), mismatch);
    assert_eq!(desk.count("SELECT count(*) FROM rpt_revisions"), 0);
    let revision = desk.issue(None).await;
    let report_id = revision_report(&desk, revision.revision_id).await;
    let released_candidate: String = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT candidate_id FROM rpt_revisions WHERE id = ?1",
            [revision.revision_id.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    tamper(&desk, "content_json", &released_candidate);
    desk.lab.sign_in(Role::Accessioner).await;
    let read = desk
        .engine()
        .get_report_revision(GetReportRevisionInput {
            revision_id: revision.revision_id,
        })
        .await;
    assert_eq!(code(read), mismatch);
    let detail = desk
        .engine()
        .get_report(GetReportInput { report_id })
        .await
        .unwrap();
    assert_eq!(detail.impact, None);
    let listed: Vec<_> = detail
        .incidents
        .iter()
        .map(|i| (i.revision_id, i.record, i.closure.is_none()))
        .collect();
    assert_eq!(
        listed,
        [
            (None, StoredRecord::DataJson, true),
            (Some(revision.revision_id), StoredRecord::ContentJson, true)
        ]
    );
    assert_eq!(
        desk.count(
            "SELECT count(*) FROM rpt_preservation_incidents WHERE kind = 'corrupt' \
             AND stored_record = 'data_json' AND candidate_id IS NOT NULL"
        ),
        1
    );
    assert_eq!(
        desk.count(&format!(
            "SELECT count(*) FROM rpt_preservation_incidents WHERE stored_record = 'content_json' \
             AND revision_id = '{}'",
            revision.revision_id
        )),
        1
    );
}

/// Changes a candidate's stored JSON behind the application's back, as a
/// privileged writer could (the candidate trigger refuses it otherwise).
fn tamper(desk: &Desk, column: &str, candidate_id: &str) {
    let conn = desk.lab.t().conn();
    conn.execute_batch("DROP TRIGGER IF EXISTS rpt_candidates_transition")
        .unwrap();
    let changed = conn
        .execute(
            &format!(
                "UPDATE rpt_candidates SET {column} = replace({column}, '152.40', '999.99') \
                 WHERE id = ?1"
            ),
            [candidate_id],
        )
        .unwrap();
    assert_eq!(changed, 1);
}

/// Request 8: a quality-only user opens the reports they assess, with only
/// the actions open to them; a user with neither delivery nor quality
/// authority is refused and the denial audited.
#[tokio::test]
async fn report_history_is_open_to_quality_and_actions_follow_the_caller() {
    let desk = Desk::start().await;
    let revision = desk.issue(None).await;
    let report_id = revision_report(&desk, revision.revision_id).await;
    desk.lab
        .assess_qc(desk.lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    desk.lab.sign_in(Role::Quality).await;
    let listed = desk
        .engine()
        .list_reports(ListReportsInput {
            before_report_id: None,
            filter: ReportListFilter::All,
            search: None,
            limit: 10,
            before_released_at: None,
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    let quality = desk
        .engine()
        .get_report(GetReportInput { report_id })
        .await
        .unwrap();
    assert_eq!(quality.allowed_actions, ["record_impact_assessment"]);
    assert!(
        quality
            .obligations
            .iter()
            .all(|o| o.allowed_actions.is_empty())
    );
    let order_client = quality.client_id;
    assert_eq!(order_client, desk.client);
    assert!(
        quality
            .obligations
            .iter()
            .all(|o| o.client_id == desk.client)
    );
    desk.engine()
        .get_report_revision(GetReportRevisionInput {
            revision_id: revision.revision_id,
        })
        .await
        .unwrap();
    desk.lab.sign_in(Role::Accessioner).await;
    let delivery = desk
        .engine()
        .get_report(GetReportInput { report_id })
        .await
        .unwrap();
    assert_eq!(delivery.allowed_actions, ["add_delivery_obligation"]);
    let releaser = desk.report(report_id).await;
    // The releaser role also delivers reports.
    assert_eq!(
        releaser.allowed_actions,
        [
            "prepare_report_candidate",
            "withdraw_report",
            "add_delivery_obligation"
        ]
    );
    desk.lab.sign_in(Role::Analyst).await;
    let refused = desk.engine().get_report(GetReportInput { report_id }).await;
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Forbidden);
    let (action, outcome): (String, String) = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT action, outcome FROM audit_events WHERE stream = 'security' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (action.as_str(), outcome.as_str()),
        ("report.get_report", "denied")
    );
}

#[tokio::test]
async fn report_history_marks_a_capped_answer() {
    let desk = Desk::start().await;
    desk.issue(None).await;
    issue_sample(&desk, 1).await;
    desk.as_releaser().await;
    let listed = desk
        .engine()
        .list_reports(ListReportsInput {
            before_report_id: None,
            filter: ReportListFilter::All,
            search: None,
            limit: 1,
            before_released_at: None,
        })
        .await
        .unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].has_more, Some(true));
}

/// Prepares and releases the first revision of sample `index` after
/// approving its results.
async fn issue_sample(desk: &Desk, index: usize) -> RevisionSummary {
    desk.lab.approve_all(index).await.unwrap();
    desk.as_releaser().await;
    let candidate = desk
        .engine()
        .prepare_report_candidate(PrepareReportCandidateInput {
            condition_review: None,
            specimen_id: desk.lab.samples[index].specimen_id,
            amendment_reason: None,
        })
        .await
        .unwrap();
    let released = desk.release(&candidate).await.unwrap();
    resync(&desk.lab).await;
    released
}

/// The candidate a revision was released from.
fn released_candidate(desk: &Desk, revision_id: ReportRevisionId) -> String {
    desk.lab
        .t()
        .conn()
        .query_row(
            "SELECT candidate_id FROM rpt_revisions WHERE id = ?1",
            [revision_id.to_string()],
            |row| row.get(0),
        )
        .unwrap()
}

/// NFR-07, REQ-OPS-004: a revision whose stored content fails its digest
/// is left out of the impact and delivery queues, which keep working for
/// every other revision; the incidents queue lists it, recorded or not.
#[tokio::test]
async fn damaged_content_leaves_the_other_queue_items_working() {
    let desk = Desk::start().await;
    let damaged = desk.issue(None).await;
    let intact = issue_sample(&desk, 1).await;
    desk.lab
        .assess_qc(desk.lab.analytes.mor, QcOutcome::Rejected)
        .await
        .unwrap();
    desk.lab.sign_in(Role::Quality).await;
    assert_eq!(queue_items(&desk, "impact_review").await.len(), 2);
    let report_id = revision_report(&desk, damaged.revision_id).await;
    desk.lab.sign_in(Role::Quality).await;
    tamper(
        &desk,
        "content_json",
        &released_candidate(&desk, damaged.revision_id),
    );

    let impact = queue_items(&desk, "impact_review").await;
    assert_eq!(
        impact.iter().map(|i| i.item_id.clone()).collect::<Vec<_>>(),
        [intact.revision_id.to_string()]
    );
    let incidents = queue_items(&desk, "preservation_incidents").await;
    assert_eq!(
        incidents
            .iter()
            .map(|i| (i.item_id.clone(), i.state.as_str()))
            .collect::<Vec<_>>(),
        [(
            format!("{report_id}:revision:{}", damaged.revision_id),
            "corrupt"
        )]
    );
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_preservation_incidents"),
        0
    );

    desk.lab.sign_in(Role::Accessioner).await;
    let delivery = desk
        .engine()
        .list_delivery_obligations(ListDeliveryObligationsInput {
            filter: DeliveryQueueFilter::All,
            limit: 50,
            after: None,
        })
        .await
        .unwrap();
    assert!(!delivery.items.is_empty());
    assert!(
        delivery
            .items
            .iter()
            .all(|o| o.revision_id == intact.revision_id)
    );
    assert!(!queue_items(&desk, "delivery_work").await.is_empty());
}

/// NFR-07, REQ-OPS-004: opening the report records the damaged content as
/// one incident; quality staff close it with a resolution and a reason,
/// once; the closed incident stays readable and leaves the queue, and a
/// loss accepted is not recorded again.
#[tokio::test]
async fn a_preservation_incident_is_closed_once_and_stays_readable() {
    let desk = Desk::start().await;
    let revision = desk.issue(None).await;
    let report_id = revision_report(&desk, revision.revision_id).await;
    tamper(
        &desk,
        "data_json",
        &released_candidate(&desk, revision.revision_id),
    );
    desk.lab.sign_in(Role::Quality).await;
    let detail = desk
        .engine()
        .get_report(GetReportInput { report_id })
        .await
        .unwrap();
    let open = detail.incidents[0].clone();
    assert_eq!(
        (
            detail.incidents.len(),
            open.revision,
            open.allowed_actions.as_slice()
        ),
        (1, 1, ["close_preservation_incident".to_owned()].as_slice())
    );
    let item = &queue_items(&desk, "preservation_incidents").await[0];
    assert_eq!(item.item_id, format!("{report_id}:{}", open.incident_id));

    refuse_bad_closures(&desk, open.incident_id).await;
    desk.lab.sign_in(Role::Quality).await;
    let closed = desk
        .engine()
        .close_preservation_incident(close(open.incident_id, IncidentResolution::AcceptedAsLost))
        .await
        .unwrap();
    let closure = closed.closure.clone().unwrap();
    assert_eq!(
        (closed.revision, closure.resolution, closure.reason.as_str()),
        (
            2,
            IncidentResolution::AcceptedAsLost,
            "Synthetic investigation finished"
        )
    );
    assert!(closed.allowed_actions.is_empty());
    let again = desk
        .engine()
        .close_preservation_incident(ClosePreservationIncidentInput {
            expected_revision: None,
            ..close(open.incident_id, IncidentResolution::RevisionReissued)
        })
        .await;
    assert_eq!(again.unwrap_err().code(), ErrorCode::Conflict);
    assert!(
        queue_items(&desk, "preservation_incidents")
            .await
            .is_empty()
    );
    let reread = desk
        .engine()
        .get_report(GetReportInput { report_id })
        .await
        .unwrap();
    assert_eq!(reread.incidents, [closed]);
    assert_eq!(
        desk.count("SELECT count(*) FROM rpt_preservation_incidents"),
        1
    );
    let (stream, reason): (String, String) = desk
        .lab
        .t()
        .conn()
        .query_row(
            "SELECT stream, reason FROM audit_events \
             WHERE action = 'report.close_preservation_incident' AND outcome = 'succeeded'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(
        (stream.as_str(), reason.as_str()),
        ("clinical", "Synthetic investigation finished")
    );
}

/// Closing needs `manage_quality`, a reason and the incident revision
/// shown; each refusal writes nothing.
async fn refuse_bad_closures(desk: &Desk, incident_id: PreservationIncidentId) {
    let closures = "SELECT count(*) FROM rpt_preservation_incident_closures";
    desk.lab.sign_in(Role::Accessioner).await;
    let forbidden = desk
        .engine()
        .close_preservation_incident(close(incident_id, IncidentResolution::AcceptedAsLost))
        .await;
    assert_eq!(forbidden.unwrap_err().code(), ErrorCode::Forbidden);
    desk.lab.sign_in(Role::Quality).await;
    let blank = desk
        .engine()
        .close_preservation_incident(ClosePreservationIncidentInput {
            reason: "  ".to_owned(),
            ..close(incident_id, IncidentResolution::AcceptedAsLost)
        })
        .await;
    assert_eq!(blank.unwrap_err().code(), ErrorCode::Validation);
    let stale = desk
        .engine()
        .close_preservation_incident(ClosePreservationIncidentInput {
            expected_revision: Some(2),
            ..close(incident_id, IncidentResolution::AcceptedAsLost)
        })
        .await;
    assert_eq!(
        code(stale),
        (ErrorCode::Conflict, vec!["revision_conflict".to_owned()])
    );
    assert_eq!(desk.count(closures), 0);
}
