//! Independent remediation IPC acceptance: real engine/store, synthetic data only.
use crate::fixture::{TestEngine, code_of};
use lis_auth::RoleCode;
use lis_core::{ErrorCode, Phi};
use lis_engine::{
    CreateClient, GetContainerHistoryInput, GetDeliveryObligationInput, GetDeviationInput,
    GetExcursionInput, ListExcursionsInput,
};
use lis_identity::{
    BirthEvidenceKind, DirectoryHistoryInput, DirectoryRecord, IntakeDisposition,
    ListIntakeDeferralsInput, RecordIntakeDeferralInput, ResolveIntakeDeferralInput, UpdateClient,
};

const CANARY: &str = "Synthetic incomplete birth evidence canary";
fn intake() -> RecordIntakeDeferralInput {
    RecordIntakeDeferralInput {
        supplied_name: Phi::new(CANARY.into()),
        birth_evidence_kind: BirthEvidenceKind::SuppliedAge,
        birth_evidence_text: Some(Phi::new("Age 42 as supplied".into())),
        source_reference: "Synthetic requisition R-1".into(),
        reason: "DOB not supplied".into(),
    }
}
fn intake_list() -> ListIntakeDeferralsInput {
    ListIntakeDeferralsInput {
        include_closed: true,
        after_id: None,
    }
}
fn correction(id: lis_identity::ClientId) -> UpdateClient {
    UpdateClient {
        client_id: id,
        expected_revision: 1,
        code: "SYN-CL".into(),
        name: "Synthetic corrected clinic".into(),
        active: false,
        reason: "Verified organization rename, retired for new work".into(),
    }
}

mod dispatch;
mod identity_quality;
mod material;
mod method_continuity;
mod paging;
mod review_paging;
pub(crate) use dispatch::remediation_call;

fn assert_access(t: &TestEngine, action: &str, actor: &str, authority: &str, record: Option<&str>) {
    let actual: (String, String, Option<String>) = t.conn().query_row(
        "SELECT principal_id, authority, record_id FROM audit_events WHERE stream='access' AND action=?1 ORDER BY seq DESC LIMIT 1",
        [action], |r| Ok((r.get(0)?,r.get(1)?,r.get(2)?))).unwrap();
    assert_eq!(
        actual,
        (actor.into(), authority.into(), record.map(str::to_owned))
    );
}
fn audit_text(t: &TestEngine) -> String {
    t.conn()
        .prepare("SELECT coalesce(details,'') || coalesce(reason,'') FROM audit_events")
        .unwrap()
        .query_map([], |r| r.get::<_, String>(0))
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap()
        .join("\n")
}

#[tokio::test]
async fn incomplete_intake_is_retained_without_a_fabricated_patient_then_resolved_from_source() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("intake1", &[RoleCode::Accessioner]).await;
    assert_eq!(
        code_of(t.engine.record_intake_deferral(intake()).await),
        ErrorCode::Forbidden
    );
    assert_eq!(
        code_of(t.engine.list_intake_deferrals(intake_list()).await),
        ErrorCode::Forbidden
    );
    let (session, _) = t.first_sign_in("intake1").await;
    let recorded = t.engine.record_intake_deferral(intake()).await.unwrap();
    assert_eq!(recorded.revision, 1);
    assert_eq!(recorded.subject_id, None);
    let patients: i64 = t
        .conn()
        .query_row("SELECT count(*) FROM idn_subjects", [], |r| r.get(0))
        .unwrap();
    assert_eq!(
        patients, 0,
        "incomplete evidence is not a testable identity"
    );
    let rows = t.engine.list_intake_deferrals(intake_list()).await.unwrap();
    assert_eq!(
        rows[0].birth_evidence_text.as_ref().unwrap().expose(),
        "Age 42 as supplied"
    );
    assert_access(
        &t,
        "identity.list_intake_deferrals",
        &session.user_id.to_string(),
        "enter_orders",
        None,
    );
    let subject_id = register_source_verified_subject(&t).await;
    let input = ResolveIntakeDeferralInput {
        intake_id: recorded.intake_id,
        expected_revision: 1,
        disposition: IntakeDisposition::Resolved,
        subject_id: Some(subject_id),
        source_reference: "Synthetic requisition R-2".into(),
        reason: "Verified complete demographics".into(),
    };
    let resolved = t
        .engine
        .resolve_intake_deferral(input.clone())
        .await
        .unwrap();
    assert_eq!(resolved.subject_id, Some(subject_id));
    assert_eq!(resolved.source_reference, "Synthetic requisition R-1");
    assert_eq!(resolved.supplied_name.expose(), CANARY);
    assert_eq!(resolved.recorded_at, recorded.recorded_at);
    assert_eq!(resolved.disposed_by, Some(session.user_id.to_string()));
    assert_eq!(
        code_of(t.engine.resolve_intake_deferral(input).await),
        ErrorCode::Conflict
    );
    let open = t
        .engine
        .list_intake_deferrals(ListIntakeDeferralsInput {
            include_closed: false,
            after_id: None,
        })
        .await
        .unwrap();
    assert!(open.is_empty());
    assert!(!audit_text(&t).contains(CANARY));
    assert!(!audit_text(&t).contains("Age 42"));
    let events = t.events("clinical");
    assert_eq!(
        events
            .iter()
            .filter(|(a, _)| a == "identity.resolve_intake_deferral")
            .count(),
        1
    );
}

async fn register_source_verified_subject(t: &TestEngine) -> lis_identity::SubjectId {
    t.engine
        .register_subject(lis_identity::RegisterSubject {
            demographics: lis_identity::DemographicsInput {
                family_name: Phi::new("Synthetic Sourceverified".into()),
                given_names: None,
                birth_date: Phi::new("1984-05-17".into()),
                administrative_sex: None,
            },
            source: "Corrected synthetic requisition R-2 supplied exact DOB".into(),
            identifiers: vec![],
            acknowledged_candidates: vec![],
        })
        .await
        .unwrap()
        .subject_id
}

#[tokio::test]
async fn directory_correction_preserves_facts_actor_and_revision_and_rejects_stale_or_unprivileged_writes()
 {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("desk1", &[RoleCode::Accessioner]).await;
    let client = t
        .engine
        .create_client(CreateClient {
            code: "SYN-CL".into(),
            name: "Synthetic original clinic".into(),
        })
        .await
        .unwrap();
    let history = DirectoryHistoryInput {
        record: DirectoryRecord::Client(client),
    };
    let initial = t.engine.directory_history(history.clone()).await.unwrap();
    assert_eq!(initial.len(), 1);
    let corrected = t.engine.update_client(correction(client)).await.unwrap();
    assert_eq!(corrected.id, client);
    assert!(!corrected.active);
    assert_eq!(corrected.revision, 2);
    assert_eq!(
        code_of(t.engine.update_client(correction(client)).await),
        ErrorCode::Conflict
    );
    let rows = t.engine.directory_history(history.clone()).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].recorded_by, rows[1].recorded_by);
    assert!(rows[1].recorded_by.is_some());
    assert_eq!(
        rows[1].recorded_by_display.as_deref(),
        Some("Test Administrator")
    );
    assert_eq!(
        rows[1].reason.as_deref(),
        Some("Verified organization rename, retired for new work")
    );
    let serialized = serde_json::to_value(&rows).unwrap().to_string();
    assert!(serialized.contains("Synthetic original clinic"));
    assert!(serialized.contains("Synthetic corrected clinic"));
    t.first_sign_in("desk1").await;
    assert_eq!(
        code_of(t.engine.update_client(correction(client)).await),
        ErrorCode::Forbidden
    );
    assert_eq!(
        code_of(t.engine.directory_history(history).await),
        ErrorCode::Forbidden
    );
    assert_eq!(
        t.events("clinical")
            .iter()
            .filter(|(a, _)| a == "identity.update_client")
            .count(),
        1
    );
}

#[tokio::test]
async fn exact_delivery_obligation_read_is_authorized_and_audited() {
    use crate::m2_lab::Role;
    use crate::m3_fixture::{Bench, Staff, Variant};
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    bench.release(&candidate).await.unwrap();
    let report = bench.report(Staff::Releaser).await;
    let obligation_id = report.obligations[0].obligation_id;
    let exact = bench
        .lab()
        .engine()
        .get_delivery_obligation(GetDeliveryObligationInput { obligation_id })
        .await
        .unwrap();
    assert_eq!(exact.obligation_id, obligation_id);
    assert_access(
        bench.lab().t(),
        "report.get_obligation",
        &bench.user_id(Staff::Releaser).to_string(),
        "deliver_reports",
        Some(&obligation_id.to_string()),
    );
    bench.sign_in(Staff::Desk(Role::Admin)).await;
    assert_eq!(
        code_of(
            bench
                .lab()
                .engine()
                .get_delivery_obligation(GetDeliveryObligationInput { obligation_id })
                .await
        ),
        ErrorCode::Forbidden
    );
    assert_eq!(
        bench.lab().t().events("security").last().unwrap(),
        &("report.get_obligation".into(), "denied".into())
    );
}
