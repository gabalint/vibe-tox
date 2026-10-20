//! Signed synthetic WebAuthn ceremonies and transactional authorization tests.
//! Software authenticator is test-only and deliberately simulates key verification.
#![cfg(test)]

#[path = "security_keys/ceremonies.rs"]
mod ceremonies;
#[path = "security_keys/policy.rs"]
mod policy;
#[path = "security_keys/recovery.rs"]
mod recovery;
#[path = "security_keys/unlock_pin.rs"]
mod unlock_pin;

use lis_auth::{
    Attempt, AuthError, FirstKeyAdministratorInput, KeyRegistrationAccount, NewSession,
    ReauthPurpose, Reauthentication, SecurityKeyId, SecurityKeyVerifier, VerifiedKeyAuthentication,
    VerifiedKeyRegistration,
};
use lis_core::{CorrelationId, Principal, SessionId, UserId};
use lis_store::testing::TestDatabase;
use lis_store::{CommandContext, DATABASE_FILE};
use rusqlite::Connection;
use secrecy::SecretString;
use webauthn_authenticator_rs::{WebauthnAuthenticator, softpasskey::SoftPasskey};
use webauthn_rs::prelude::{CreationChallengeResponse, Url};

const ORIGIN: &str = "https://lis.example.test";
type TestKey = WebauthnAuthenticator<SoftPasskey>;

fn public() -> Principal {
    Principal::system("sign-in-screen", [])
}

fn write<T: Send + 'static>(
    db: &TestDatabase,
    principal: &Principal,
    f: impl FnOnce(&mut CommandContext<'_>) -> Result<T, AuthError> + Send + 'static,
) -> Result<T, AuthError> {
    db.db
        .write_blocking(principal.clone(), CorrelationId::generate(), f)
}

fn accepted<T>(attempt: Attempt<T>) -> T {
    match attempt {
        Attempt::Accepted(value) => value,
        Attempt::Rejected(reason) => panic!("unexpected refusal: {reason}"),
    }
}

fn key() -> TestKey {
    WebauthnAuthenticator::new(SoftPasskey::new(true))
}

fn verifier() -> SecurityKeyVerifier {
    SecurityKeyVerifier::new("lis.example.test", ORIGIN).unwrap()
}

fn first_account(db: &TestDatabase) -> KeyRegistrationAccount {
    db.db
        .read_blocking(|conn| {
            lis_auth::prepare_first_key_registration(
                conn,
                FirstKeyAdministratorInput {
                    username: "key_operator".to_owned(),
                    display_name: "Synthetic Operator".to_owned(),
                    one_person: true,
                    load_demo_data: None,
                },
            )
        })
        .unwrap()
}

fn register(
    verifier: &SecurityKeyVerifier,
    account: KeyRegistrationAccount,
    key: &mut TestKey,
) -> VerifiedKeyRegistration {
    let (options, pending) = verifier.begin_registration(account).unwrap();
    let options: CreationChallengeResponse = serde_json::from_value(options).unwrap();
    let response = key
        .do_registration(Url::parse(ORIGIN).unwrap(), options)
        .unwrap();
    verifier
        .finish_registration(pending, serde_json::to_value(response).unwrap())
        .unwrap()
}

struct Fixture {
    db: TestDatabase,
    verifier: SecurityKeyVerifier,
    key: TestKey,
    user_id: UserId,
    key_id: SecurityKeyId,
    principal: Principal,
    session: NewSession,
    recovery_code: SecretString,
}

impl Fixture {
    fn new() -> Self {
        let db = TestDatabase::open(&[lis_auth::SCHEMA]).unwrap();
        let verifier = verifier();
        let mut key = key();
        let registration = register(&verifier, first_account(&db), &mut key);
        let enrolled = write(&db, &public(), move |ctx| {
            lis_auth::complete_first_key_registration(ctx, registration, "Everyday key")
        })
        .unwrap();
        let user_id = enrolled.user_id();
        let recovery_code = enrolled.recovery_code().clone();
        let workstation_id = write(&db, &public(), lis_auth::register_workstation).unwrap();
        let session = NewSession {
            id: SessionId::generate(),
            workstation_id,
            webview: None,
            replaces: None,
        };
        let new_session = session.clone();
        let user = write(&db, &public(), move |ctx| {
            lis_auth::complete_key_enrollment_session(ctx, enrolled, &new_session)
        })
        .unwrap();
        let principal = Principal::user(user_id, session.id, workstation_id, user.capabilities());
        let key_id = db
            .db
            .read_blocking(move |conn| lis_auth::get_authentication_status(conn, user_id))
            .unwrap()
            .credentials[0]
            .credential_id;
        Self {
            db,
            verifier,
            key,
            user_id,
            key_id,
            principal,
            session,
            recovery_code,
        }
    }

    fn assertion(&mut self) -> VerifiedKeyAuthentication {
        let id = self.key_id;
        let account = accepted(
            write(&self.db, &public(), move |ctx| {
                lis_auth::prepare_key_authentication(ctx, "key_operator", id)
            })
            .unwrap(),
        );
        let (options, pending) = self.verifier.begin_authentication(account).unwrap();
        let response = self
            .key
            .do_authentication(
                Url::parse(ORIGIN).unwrap(),
                serde_json::from_value(options).unwrap(),
            )
            .unwrap();
        self.verifier
            .finish_authentication(pending, serde_json::to_value(response).unwrap())
            .unwrap()
    }

    fn proof(&mut self, purpose: ReauthPurpose) -> Reauthentication {
        let assertion = self.assertion();
        accepted(
            write(&self.db, &self.principal, move |ctx| {
                lis_auth::issue_key_action_proof(ctx, assertion, purpose)
            })
            .unwrap(),
        )
    }

    fn spare(&mut self) -> (TestKey, SecurityKeyId) {
        let proof = self.proof(ReauthPurpose::SecurityKeyEnrollment);
        let account = accepted(
            write(&self.db, &self.principal, move |ctx| {
                lis_auth::prepare_own_key_registration(ctx, &proof)
            })
            .unwrap(),
        );
        let mut spare = key();
        let verified = register(&self.verifier, account, &mut spare);
        let enrolled = write(&self.db, &self.principal, move |ctx| {
            lis_auth::complete_own_key_registration(ctx, verified, "Spare key")
        })
        .unwrap();
        self.recovery_code = enrolled.recovery_code().clone();
        let user_id = self.user_id;
        let id = self
            .db
            .db
            .read_blocking(move |conn| lis_auth::get_authentication_status(conn, user_id))
            .unwrap()
            .credentials
            .into_iter()
            .find(|key| key.label == "Spare key")
            .unwrap()
            .credential_id;
        (spare, id)
    }

    fn conn(&self) -> Connection {
        Connection::open(self.db.data_dir().join(DATABASE_FILE)).unwrap()
    }
}

fn duplicate(proof: &Reauthentication) -> Reauthentication {
    match proof {
        Reauthentication::Password(value) => value.clone().into(),
        Reauthentication::SecurityKey { key_proof } => Reauthentication::SecurityKey {
            key_proof: key_proof.clone(),
        },
    }
}

#[test]
fn key_signup_needs_no_password_or_totp_and_recovery_is_hashed() {
    use secrecy::ExposeSecret;
    let f = Fixture::new();
    let user_id = f.user_id;
    let status =
        f.db.db
            .read_blocking(move |conn| lis_auth::get_authentication_status(conn, user_id))
            .unwrap();
    assert_eq!(status.method, lis_auth::AuthenticationMethod::SecurityKey);
    assert!(status.recovery_available);
    let stored: String = f
        .conn()
        .query_row("SELECT code_hash FROM auth_key_recovery_codes", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert!(stored.starts_with("$argon2id$"));
    assert!(!stored.contains(f.recovery_code.expose_secret()));
    let account = write(&f.db, &public(), |ctx| {
        lis_auth::check_password(
            ctx,
            &lis_auth::SignInInput {
                username: "key_operator".to_owned(),
                password: SecretString::from("synthetic passphrase"),
            },
        )
    })
    .unwrap();
    assert!(matches!(account, Attempt::Rejected(_)));
    assert_eq!(
        f.conn()
            .query_row(
                "SELECT count(*) FROM auth_users WHERE totp_secret IS NOT NULL",
                [],
                |row| row.get::<_, i64>(0)
            )
            .unwrap(),
        0
    );
}

#[test]
fn proofs_bind_purpose_and_are_single_use() {
    let mut f = Fixture::new();
    let proof = f.proof(ReauthPurpose::ReportRelease);
    let replay = duplicate(&proof);
    let wrong_purpose = duplicate(&proof);
    let refused = write(&f.db, &f.principal, move |ctx| {
        lis_auth::reauthenticate(ctx, &wrong_purpose, ReauthPurpose::RoleChange)
    })
    .unwrap();
    assert!(matches!(refused, Attempt::Rejected(_)));
    accepted(
        write(&f.db, &f.principal, move |ctx| {
            lis_auth::reauthenticate(ctx, &proof, ReauthPurpose::ReportRelease)
        })
        .unwrap(),
    );
    assert!(matches!(
        write(&f.db, &f.principal, move |ctx| lis_auth::reauthenticate(
            ctx,
            &replay,
            ReauthPurpose::ReportRelease
        ))
        .unwrap(),
        Attempt::Rejected(_)
    ));
}

#[test]
fn key_proofs_expire_and_ended_sessions_cannot_use_them() {
    use lis_store::testing::test_start;
    use std::time::Duration;
    let mut f = Fixture::new();
    let proof = f.proof(ReauthPurpose::ReportRelease);
    f.db.clock
        .set(test_start().checked_add(Duration::from_secs(60)).unwrap());
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
    let session_id = f.session.id;
    write(&f.db, &f.principal, move |ctx| {
        lis_auth::end_session(ctx, session_id, lis_auth::EndReason::SignedOut)
    })
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
