//! Pure quality authority can reach factual correction without originating care.
use super::*;
use lis_engine::{
    AssertDemographics, DeactivateSubjectIdentifier, GetSubjectInput, RegisterSubject,
    SubjectMatchQuery, SubjectSearch,
};
use lis_identity::{DemographicsInput, IdentifierInput};

#[tokio::test]
async fn quality_reads_separation_policy_but_only_administrator_activates_it() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("qualityreader", &[RoleCode::QualityManager])
        .await;
    t.create_user("policyadmin", &[RoleCode::Administrator])
        .await;
    t.first_sign_in("qualityreader").await;
    let shown = t.engine.get_separation_policy().await.unwrap();
    assert_eq!(
        code_of(t.engine.set_separation_policy(policy_change(&shown)).await),
        ErrorCode::Forbidden
    );
    let (administrator, _) = t.first_sign_in("policyadmin").await;
    let mut stale = policy_change(&shown);
    stale.expected_version = Some(-1);
    assert_eq!(
        code_of(t.engine.set_separation_policy(stale).await),
        ErrorCode::Conflict
    );
    let mut missing_reason = policy_change(&shown);
    missing_reason.reason.clear();
    assert_eq!(
        code_of(t.engine.set_separation_policy(missing_reason).await),
        ErrorCode::Validation
    );
    let set = t
        .engine
        .set_separation_policy(policy_change(&shown))
        .await
        .unwrap();
    assert_eq!(set.version, Some(shown.version.unwrap_or(0) + 1));
    assert_eq!(set.set_by, Some(administrator.user_id.to_string()));
    assert_eq!(
        t.engine
            .get_separation_policy()
            .await
            .unwrap()
            .set_by_display
            .as_deref(),
        Some("Test policyadmin")
    );
    assert!(set.set_at.is_some());
    assert!(set.values.self_approve_configuration);
    assert_eq!(
        set.values.self_release_reports,
        shown.values.self_release_reports
    );
    let attribution: (String, String, String) = t
        .conn()
        .query_row(
            "SELECT principal_id,authority,reason FROM audit_events WHERE stream='security' \
         AND action='auth.set_separation_policy' AND outcome='succeeded' ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .unwrap();
    assert_eq!(
        attribution,
        (
            administrator.user_id.to_string(),
            "approve_configuration".into(),
            "Synthetic administrator policy approval".into()
        )
    );
    assert_eq!(
        code_of(t.engine.register_subject(registration()).await),
        ErrorCode::Forbidden
    );
}

fn policy_change(shown: &lis_auth::SeparationPolicyView) -> lis_auth::SetSeparationPolicyInput {
    lis_auth::SetSeparationPolicyInput {
        expected_version: shown.version,
        values: lis_auth::SeparationPolicy {
            self_approve_configuration: true,
            ..shown.values
        },
        reason: "Synthetic administrator policy approval".into(),
        password: crate::fixture::secret(crate::fixture::NEW_PASSWORD).into(),
    }
}

fn demographics(date: &str) -> DemographicsInput {
    DemographicsInput {
        family_name: Phi::new("SYNTHETIC QUALITY CANARY".into()),
        given_names: Some(Phi::new("Patient".into())),
        birth_date: Phi::new(date.into()),
        administrative_sex: None,
    }
}
fn registration() -> RegisterSubject {
    RegisterSubject {
        demographics: demographics("1980-01-01"),
        source: "Synthetic requisition".into(),
        identifiers: vec![IdentifierInput {
            assigning_authority: "SYN-CLINIC".into(),
            identifier_type: "MR".into(),
            value: Phi::new("SYN-QUALITY-1".into()),
        }],
        acknowledged_candidates: vec![],
    }
}

#[tokio::test]
async fn pure_quality_can_find_correct_and_deactivate_but_cannot_register_patients_or_orders() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    t.create_user("qualityonly", &[RoleCode::QualityManager])
        .await;
    t.create_user("identitydesk", &[RoleCode::Accessioner])
        .await;
    t.first_sign_in("identitydesk").await;
    let registered = t.engine.register_subject(registration()).await.unwrap();
    let (quality, _) = t.first_sign_in("qualityonly").await;
    let detail = read_identity(&t, registered.subject_id, quality.user_id).await;
    correct_and_deactivate(&t, detail, quality.user_id).await;
    assert_eq!(
        code_of(t.engine.register_subject(registration()).await),
        ErrorCode::Forbidden
    );
    assert_eq!(
        code_of(
            crate::orders::orders_call(&t.engine, "register_order", quality.user_id)
                .await
                .unwrap()
        ),
        ErrorCode::Forbidden
    );
    assert!(!audit_text(&t).contains("SYNTHETIC QUALITY CANARY"));
    let mutation_authorities: Vec<(String, String)> = t.conn().prepare(
        "SELECT principal_id,authority FROM audit_events WHERE stream='clinical' AND action IN ('identity.assert_demographics','identity.deactivate_identifier') ORDER BY seq"
    ).unwrap().query_map([], |r| Ok((r.get(0)?, r.get(1)?))).unwrap().collect::<Result<_,_>>().unwrap();
    assert_eq!(
        mutation_authorities,
        vec![(quality.user_id.to_string(), "manage_quality".into()); 2]
    );
}

async fn read_identity(
    t: &TestEngine,
    subject_id: lis_identity::SubjectId,
    actor: lis_core::UserId,
) -> lis_identity::SubjectView {
    let found = t
        .engine
        .search_subjects(SubjectSearch {
            query: None,
            offset: None,
            identifier: None,
            family_name: Some(Phi::new("SYNTHETIC QUALITY".into())),
            given_names: None,
            birth_date: None,
            limit: 10,
        })
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].subject_id, subject_id);
    let candidates = t
        .engine
        .match_subject_candidates(SubjectMatchQuery {
            demographics: Some(demographics("1980-01-01")),
            identifiers: vec![],
        })
        .await
        .unwrap();
    assert_eq!(candidates.candidates[0].subject_id, subject_id);
    let detail = t
        .engine
        .get_subject(GetSubjectInput { subject_id })
        .await
        .unwrap();
    for (action, record) in [
        ("identity.search_subjects", None),
        ("identity.match_subject_candidates", None),
        ("identity.get_subject", Some(subject_id.to_string())),
    ] {
        assert_access(
            t,
            action,
            &actor.to_string(),
            "manage_quality",
            record.as_deref(),
        );
    }
    detail
}

async fn correct_and_deactivate(
    t: &TestEngine,
    detail: lis_identity::SubjectView,
    actor: lis_core::UserId,
) {
    let corrected = t
        .engine
        .assert_demographics(AssertDemographics {
            subject_id: detail.id,
            expected_revision: detail.revision,
            demographics: demographics("1980-01-02"),
            source: "Synthetic corrected requisition".into(),
            reason: "Verified transcription correction against source".into(),
            acknowledged_candidates: vec![],
        })
        .await
        .unwrap();
    t.engine
        .deactivate_subject_identifier(DeactivateSubjectIdentifier {
            identifier_id: detail.identifiers[0].id,
            expected_subject_revision: corrected.revision,
            reason: "Verified identifier transcription error".into(),
        })
        .await
        .unwrap();
    let retained = t
        .engine
        .get_subject(GetSubjectInput {
            subject_id: detail.id,
        })
        .await
        .unwrap();
    assert_eq!(retained.assertions.len(), 2);
    assert_eq!(retained.assertions[0].birth_date.expose(), "1980-01-01");
    assert_eq!(retained.assertions[1].birth_date.expose(), "1980-01-02");
    assert_eq!(retained.assertions[1].asserted_by_id, actor.to_string());
    assert!(!retained.identifiers[0].active);
    assert_eq!(retained.identifiers[0].value.expose(), "SYN-QUALITY-1");
}
