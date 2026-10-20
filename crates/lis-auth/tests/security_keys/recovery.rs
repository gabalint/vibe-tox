use super::*;

#[test]
fn sole_operator_recovery_replaces_keys_ends_sessions_and_cannot_open_session() {
    let mut f = Fixture::new();
    super::unlock_pin::enable(&mut f);
    let pending_assertion = f.assertion();
    let code = f.recovery_code.clone();
    let replay_code = code.clone();
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_recovery(ctx, "key_operator", &code)
        })
        .unwrap(),
    );
    let mut replacement = key();
    let verified = register(&f.verifier, account, &mut replacement);
    let enrolled = write(&f.db, &public(), move |ctx| {
        lis_auth::complete_key_recovery(ctx, verified, "Replacement")
    })
    .unwrap();
    assert!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_key_unlock(ctx, pending_assertion)
        })
        .is_err()
    );
    let new_session = NewSession {
        id: SessionId::generate(),
        ..f.session.clone()
    };
    assert!(matches!(
        write(&f.db, &public(), move |ctx| {
            lis_auth::complete_key_enrollment_session(ctx, enrolled, &new_session)
        }),
        Err(AuthError::ChallengeExpired)
    ));
    assert!(matches!(
        write(&f.db, &public(), move |ctx| lis_auth::prepare_key_recovery(
            ctx,
            "key_operator",
            &replay_code
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
    let user_id = f.user_id;
    let status =
        f.db.db
            .read_blocking(move |conn| lis_auth::get_authentication_status(conn, user_id))
            .unwrap();
    assert_eq!(status.credentials.len(), 1);
    assert_eq!(status.credentials[0].label, "Replacement");
    let open: i64 = f
        .conn()
        .query_row(
            "SELECT count(*) FROM auth_sessions WHERE ended_at IS NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(open, 0);
    assert_eq!(
        f.conn()
            .query_row("SELECT count(*) FROM auth_unlock_pins", [], |row| row
                .get::<_, i64>(0))
            .unwrap(),
        0
    );
}

#[test]
fn last_key_cannot_be_revoked_and_spare_revocation_ends_sessions() {
    let mut f = Fixture::new();
    let proof = f.proof(ReauthPurpose::SecurityKeyRevocation);
    let key_id = f.key_id;
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::revoke_security_key(ctx, key_id, 1, &proof, "Synthetic loss")
        }),
        Err(AuthError::LastSecurityKey)
    ));
    let (_, spare_id) = f.spare();
    super::unlock_pin::enable(&mut f);
    let proof = f.proof(ReauthPurpose::SecurityKeyRevocation);
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::revoke_security_key(ctx, spare_id, 1, &proof, "Synthetic loss")
        })
        .unwrap(),
    );
    assert!(matches!(
        super::unlock_pin::attempt(&f, "482905"),
        Err(AuthError::SessionUnavailable)
    ));
    let user_id = f.user_id;
    let session_id = f.session.id;
    assert!(
        !f.db
            .db
            .read_blocking(move |conn| lis_auth::session_is_open(conn, session_id, user_id))
            .unwrap()
    );
    assert!(matches!(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", spare_id)
        })
        .unwrap(),
        Attempt::Rejected(_)
    ));
}

#[test]
fn cancelled_recovery_keeps_original_key_and_code_usable() {
    let mut f = Fixture::new();
    let code = f.recovery_code.clone();
    let again = code.clone();
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_recovery(ctx, "key_operator", &code)
        })
        .unwrap(),
    );
    let (_, pending) = f.verifier.begin_registration(account).unwrap();
    drop(pending);
    assert!(matches!(
        write(&f.db, &public(), move |ctx| lis_auth::prepare_key_recovery(
            ctx,
            "key_operator",
            &again
        ))
        .unwrap(),
        Attempt::Accepted(_)
    ));
    let assertion = f.assertion();
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::complete_key_unlock(ctx, assertion)
        })
        .unwrap(),
    );
}

#[test]
fn failed_replacement_rolls_back_code_consumption_and_key_revocation() {
    let f = Fixture::new();
    let code = f.recovery_code.clone();
    let again = code.clone();
    let account = accepted(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_recovery(ctx, "key_operator", &code)
        })
        .unwrap(),
    );
    let verified = register(&f.verifier, account, &mut key());
    assert!(
        write(
            &f.db,
            &public(),
            move |ctx| lis_auth::complete_key_recovery(ctx, verified, "")
        )
        .is_err()
    );
    assert!(matches!(
        write(&f.db, &public(), move |ctx| lis_auth::prepare_key_recovery(
            ctx,
            "key_operator",
            &again
        ))
        .unwrap(),
        Attempt::Accepted(_)
    ));
    let key_id = f.key_id;
    assert!(matches!(
        write(&f.db, &public(), move |ctx| {
            lis_auth::prepare_key_authentication(ctx, "key_operator", key_id)
        })
        .unwrap(),
        Attempt::Accepted(_)
    ));
}
