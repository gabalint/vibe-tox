//! Brief §3.4 row 2 and §3.6 "releaser authority": the session's principal
//! is resolved before the write begins, so release, withdrawal and
//! preparation reread the user's grants inside their own transaction. A
//! principal still holding `release_reports` whose stored roles no longer
//! grant it is refused before anything else happens (NFR-08, NFR-01).

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_auth::CreateFirstAdministratorInput;
use lis_core::{Capability, CorrelationId, Principal, SessionId, UserId, WorkstationId};
use lis_reporting::{
    CandidateId, PrepareReportCandidateInput, ReleaseReportInput, ReportId, ReportingError,
    WithdrawReportInput,
};
use lis_store::StoreError;
use lis_store::testing::TestDatabase;
use secrecy::SecretString;

const PASSWORD: &str = "synthetic passphrase 1";

fn database() -> TestDatabase {
    TestDatabase::open(&[
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
        lis_results::SCHEMA,
        lis_reporting::SCHEMA,
    ])
    .unwrap()
}

/// The first administrator: stored roles grant no clinical capability.
fn administrator(test: &TestDatabase) -> UserId {
    test.db
        .write_blocking(
            Principal::system("sign-in-screen", []),
            CorrelationId::generate(),
            |ctx| {
                lis_auth::create_first_administrator(
                    ctx,
                    CreateFirstAdministratorInput {
                        username: "admin1".to_owned(),
                        display_name: "Synthetic Administrator".to_owned(),
                        password: SecretString::from(PASSWORD),
                        one_person: false,
                        load_demo_data: None,
                    },
                )
            },
        )
        .unwrap()
}

/// A principal resolved with `release_reports`, as a session would be just
/// before the grant was revoked.
fn stale(user_id: UserId) -> Principal {
    Principal::user(
        user_id,
        SessionId::generate(),
        WorkstationId::generate(),
        [Capability::ReleaseReports],
    )
}

fn assert_denied(result: Result<impl std::fmt::Debug, ReportingError>) {
    match result {
        Err(ReportingError::Store(StoreError::Denied(denied))) => {
            assert_eq!(denied.capability, Capability::ReleaseReports);
        }
        other => panic!("expected a denial, got {other:?}"),
    }
}

fn events(test: &TestDatabase, action: &'static str) -> i64 {
    test.db
        .read_blocking(move |conn| {
            conn.query_row(
                "SELECT count(*) FROM audit_events WHERE action = ?1",
                [action],
                |row| row.get::<_, i64>(0),
            )
            .map_err(StoreError::from)
        })
        .unwrap()
}

#[test]
fn a_revoked_release_grant_is_refused_inside_the_transaction() {
    let test = database();
    let user = administrator(&test);
    let release = test
        .db
        .write_blocking(stale(user), CorrelationId::generate(), |ctx| {
            lis_reporting::release_candidate(
                ctx,
                ReleaseReportInput {
                    candidate_id: CandidateId::generate(),
                    expected_revision: 1,
                    content_sha256: "0".repeat(64),
                    acknowledge_partial: true,
                    password: SecretString::from(PASSWORD).into(),
                    self_approval: false,
                },
                &"0".repeat(64),
            )
        });
    assert_denied(release);
    // Refused before fresh authentication: no attempt was counted or audited.
    assert_eq!(events(&test, "auth.reauthenticate"), 0);

    let withdraw = test
        .db
        .write_blocking(stale(user), CorrelationId::generate(), |ctx| {
            lis_reporting::preflight_withdrawal(
                ctx,
                WithdrawReportInput {
                    report_id: ReportId::generate(),
                    expected_revision_no: 1,
                    reason: "Synthetic".to_owned(),
                    password: SecretString::from(PASSWORD).into(),
                },
            )
        });
    assert_denied(withdraw);

    let prepare = test
        .db
        .write_blocking(stale(user), CorrelationId::generate(), |ctx| {
            lis_reporting::prepare_candidate(
                ctx,
                PrepareReportCandidateInput {
                    condition_review: None,
                    specimen_id: lis_specimens::SpecimenId::generate(),
                    amendment_reason: None,
                },
            )
        });
    assert_denied(prepare);
    assert_eq!(events(&test, "auth.reauthenticate"), 0);
}
