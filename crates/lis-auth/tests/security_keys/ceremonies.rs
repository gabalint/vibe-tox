use super::*;

#[test]
fn cancelled_or_invalid_registration_creates_no_account() {
    let db = TestDatabase::open(&[lis_auth::SCHEMA]).unwrap();
    let verifier = verifier();
    let (_, pending) = verifier.begin_registration(first_account(&db)).unwrap();
    assert!(
        verifier
            .finish_registration(pending, serde_json::json!({}))
            .is_err()
    );
    assert!(
        db.db
            .read_blocking(lis_auth::setup_status)
            .unwrap()
            .needs_administrator
    );
}

#[test]
fn validly_signed_assertion_without_user_verification_is_refused() {
    let mut f = Fixture::new();
    let id = f.key_id;
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", id)
        })
        .unwrap(),
    );
    let (options, pending) = f.verifier.begin_authentication(account).unwrap();
    let mut options = options;
    assert_eq!(options["publicKey"]["userVerification"], "required");
    options["publicKey"]["userVerification"] = serde_json::json!("discouraged");
    let response = f
        .key
        .do_authentication(
            Url::parse(ORIGIN).unwrap(),
            serde_json::from_value(options).unwrap(),
        )
        .unwrap();
    assert!(
        f.verifier
            .finish_authentication(pending, serde_json::to_value(response).unwrap())
            .is_err()
    );
}

#[test]
fn fresh_challenge_rejects_an_earlier_signed_assertion() {
    let mut f = Fixture::new();
    let id = f.key_id;
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", id)
        })
        .unwrap(),
    );
    let (options, _) = f.verifier.begin_authentication(account).unwrap();
    let response = f
        .key
        .do_authentication(
            Url::parse(ORIGIN).unwrap(),
            serde_json::from_value(options).unwrap(),
        )
        .unwrap();
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", id)
        })
        .unwrap(),
    );
    let (_, fresh_pending) = f.verifier.begin_authentication(account).unwrap();
    assert!(
        f.verifier
            .finish_authentication(fresh_pending, serde_json::to_value(response).unwrap())
            .is_err()
    );
}

#[test]
fn account_revision_changes_refuse_a_verified_pending_assertion() {
    let mut f = Fixture::new();
    let assertion = f.assertion();
    f.conn()
        .execute("UPDATE auth_users SET revision = revision + 1", [])
        .unwrap();
    let result = write(&f.db, &f.principal, move |ctx| {
        lis_auth::complete_key_unlock(ctx, assertion)
    });
    assert!(matches!(result, Err(AuthError::StaleRevision)));
}

#[test]
fn repeated_verification_failures_apply_existing_lockout() {
    let f = Fixture::new();
    for _ in 0..5 {
        let _ = write(&f.db, &public(), |ctx| {
            lis_auth::record_key_failure(ctx, "key_operator")
        })
        .unwrap();
    }
    let id = f.key_id;
    assert!(matches!(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", id)
        })
        .unwrap(),
        Attempt::Rejected(lis_auth::Rejection::LockedOut)
    ));
    let events: i64 = f
        .conn()
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action = 'auth.lockout'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(events, 1);
}

#[test]
fn only_one_assertion_from_a_credential_snapshot_can_commit() {
    let mut f = Fixture::new();
    let first = f.assertion();
    let second = f.assertion();
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_key_unlock(ctx, second)
        })
        .unwrap(),
    );
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_key_unlock(ctx, first)
        }),
        Err(AuthError::SecurityKeyUnavailable)
    ));
}

#[test]
fn action_proof_cannot_cross_sessions_or_account_and_key_revisions() {
    let mut f = Fixture::new();
    let proof = f.proof(ReauthPurpose::ReportRelease);
    let session = NewSession {
        id: SessionId::generate(),
        ..f.session.clone()
    };
    let new_session = session.clone();
    let assertion = f.assertion();
    let user = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_sign_in(ctx, assertion, &new_session)
        })
        .unwrap(),
    );
    let different = Principal::user(
        f.user_id,
        session.id,
        session.workstation_id,
        user.capabilities(),
    );
    assert!(matches!(
        write(&f.db, &different, move |ctx| lis_auth::reauthenticate(
            ctx,
            &proof,
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
    let proof = f.proof(ReauthPurpose::ReportRelease);
    f.conn()
        .execute("UPDATE auth_users SET revision = revision + 1", [])
        .unwrap();
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| lis_auth::reauthenticate(
            ctx,
            &proof,
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
    let proof = f.proof(ReauthPurpose::ReportRelease);
    f.conn()
        .execute("UPDATE auth_security_keys SET revision = revision + 1", [])
        .unwrap();
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| lis_auth::reauthenticate(
            ctx,
            &proof,
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
}
