//! Verifying a delivery destination one created (REQ-RPT-012, L10) follows the
//! laboratory's separation-of-duties policy (ADR 0015): refused while the policy
//! needs a second person, allowed with the creator's confirmation once it does
//! not, and then marked as a self-approval in the audit trail. A verification
//! by another person is unaffected. Real SQLite and commands, synthetic data.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_auth::{
    Attempt, CreateFirstAdministratorInput, CreateUserInput, SeparationPolicy,
    SetSeparationPolicyInput,
};
use lis_core::{Capability, CorrelationId, ErrorCode, Principal, SessionId, UserId, WorkstationId};
use lis_identity::{ClientId, CreateClient};
use lis_reporting::{
    CreateDeliveryDestinationInput, DeliveryChannel, DestinationId, DestinationView,
    ReportingError, VerifyDeliveryDestinationInput,
};
use lis_store::StoreError;
use lis_store::testing::TestDatabase;
use secrecy::SecretString;

const PASSWORD: &str = "synthetic passphrase 1";

struct Lab {
    test: TestDatabase,
    /// The administrator, who creates destinations.
    creator: UserId,
    /// A second user.
    other: UserId,
    /// The ordering client of every destination.
    client: ClientId,
}

fn user(user_id: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        user_id,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

impl Lab {
    fn new() -> Self {
        let test = TestDatabase::open(&[
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
        .unwrap();
        let creator = test
            .db
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
            .unwrap();
        let created = test
            .db
            .write_blocking(
                user(creator, &[Capability::ManageUsers]),
                CorrelationId::generate(),
                |ctx| {
                    lis_auth::create_user(
                        ctx,
                        CreateUserInput {
                            username: "clerk1".to_owned(),
                            display_name: "Synthetic Clerk".to_owned(),
                            roles: Vec::new(),
                            temporary_password: SecretString::from("synthetic temporary 1"),
                            password: SecretString::from(PASSWORD).into(),
                        },
                    )
                },
            )
            .unwrap();
        let Attempt::Accepted(other) = created else {
            panic!("the second user was not created");
        };
        let client = test
            .db
            .write_blocking(
                user(creator, &[Capability::ManageConfiguration]),
                CorrelationId::generate(),
                |ctx| {
                    lis_identity::create_client(
                        ctx,
                        CreateClient {
                            code: "SYN-CLINIC".to_owned(),
                            name: "Synthetic Clinic".to_owned(),
                        },
                    )
                },
            )
            .unwrap();
        Self {
            test,
            creator,
            other: other.user_id,
            client,
        }
    }

    /// A pickup destination of the client, created by the administrator.
    fn destination(&self) -> DestinationView {
        let client_id = self.client;
        self.test
            .db
            .write_blocking(
                user(self.creator, &[Capability::ManageConfiguration]),
                CorrelationId::generate(),
                move |ctx| {
                    lis_reporting::create_delivery_destination(
                        ctx,
                        CreateDeliveryDestinationInput {
                            client_id,
                            client_location_id: None,
                            channel: DeliveryChannel::Pickup,
                            address: None,
                            label: "Synthetic pickup".to_owned(),
                        },
                    )
                },
            )
            .unwrap()
    }

    fn verify(
        &self,
        by: UserId,
        destination_id: DestinationId,
        self_approval: bool,
    ) -> Result<DestinationView, ReportingError> {
        self.test.db.write_blocking(
            user(by, &[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                lis_reporting::verify_delivery_destination(
                    ctx,
                    VerifyDeliveryDestinationInput {
                        destination_id,
                        expected_revision: 1,
                        method: "Synthetic: seen in person".to_owned(),
                        self_approval,
                    },
                )
            },
        )
    }

    /// The laboratory allows its destinations' creators to verify them.
    fn allow_self_verification(&self) {
        let set = self
            .test
            .db
            .write_blocking(
                user(self.creator, &[Capability::ApproveConfiguration]),
                CorrelationId::generate(),
                |ctx| {
                    lis_auth::set_separation_policy(
                        ctx,
                        SetSeparationPolicyInput {
                            expected_version: None,
                            values: SeparationPolicy {
                                self_approve_configuration: false,
                                self_release_reports: false,
                                self_verify_destinations: true,
                            },
                            reason: "Synthetic: one person runs the front desk".to_owned(),
                            password: SecretString::from(PASSWORD).into(),
                        },
                    )
                },
            )
            .unwrap();
        assert!(matches!(set, Attempt::Accepted(_)));
    }

    /// Each `report.verify_destination` event's `self_approval` detail, in order.
    fn verification_events(&self) -> Vec<Option<bool>> {
        self.test
            .db
            .read_blocking(|conn| {
                let mut statement = conn.prepare(
                    "SELECT json_extract(details, '$.self_approval') FROM audit_events \
                     WHERE action = 'report.verify_destination' ORDER BY seq",
                )?;
                let marks = statement
                    .query_map([], |row| row.get::<_, Option<bool>>(0))?
                    .collect::<rusqlite::Result<_>>()?;
                Ok::<_, StoreError>(marks)
            })
            .unwrap()
    }

    fn is_unverified(&self, destination_id: DestinationId) -> bool {
        self.test
            .db
            .read_blocking(move |conn| {
                conn.query_row(
                    "SELECT count(*) = 0 FROM rpt_destination_verifications \
                     WHERE destination_id = ?1",
                    [destination_id],
                    |row| row.get::<_, bool>(0),
                )
                .map_err(StoreError::from)
            })
            .unwrap()
    }
}

fn gate_codes(error: &ReportingError) -> Vec<String> {
    error.gate_reasons().into_iter().map(|g| g.code).collect()
}

// REQ-RPT-012, L10, ADR 0015: by default the creator may not verify, even
// confirming it, and nothing is written.
#[test]
fn a_creator_may_not_verify_under_the_default_policy() {
    let lab = Lab::new();
    let destination = lab.destination();
    let refused = lab
        .verify(lab.creator, destination.destination_id, true)
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::GateBlocked);
    assert_eq!(gate_codes(&refused), ["second_person_required"]);
    assert!(
        refused.gate_reasons()[0]
            .message
            .contains("separation-of-duties")
    );
    assert!(lab.is_unverified(destination.destination_id));
    assert!(lab.verification_events().is_empty());
}

// ADR 0015: once allowed, the creator must still confirm it; unconfirmed,
// nothing is written; confirmed, the verification is marked as theirs.
#[test]
fn an_allowed_self_verification_needs_its_confirmation_and_is_marked() {
    let lab = Lab::new();
    let destination = lab.destination();
    lab.allow_self_verification();

    let refused = lab
        .verify(lab.creator, destination.destination_id, false)
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    let fields = refused.field_errors();
    assert_eq!(fields.len(), 1);
    assert_eq!(
        (fields[0].field.as_str(), fields[0].code.as_str()),
        ("selfApproval", "self_approval_unconfirmed")
    );
    assert!(lab.is_unverified(destination.destination_id));
    assert!(lab.verification_events().is_empty());

    let verified = lab
        .verify(lab.creator, destination.destination_id, true)
        .unwrap();
    let verification = verified.verified.unwrap();
    assert_eq!(verification.by.user_id, lab.creator);
    assert!(verification.by_creator);
    assert_eq!(lab.verification_events(), [Some(true)]);
}

// REQ-RPT-012: a second person verifies as before under any policy; the
// event is not a self-approval, even when the input claims one.
#[test]
fn a_verification_by_another_person_is_unaffected_and_unmarked() {
    let lab = Lab::new();
    let first = lab.destination();
    let verified = lab.verify(lab.other, first.destination_id, false).unwrap();
    assert!(!verified.verified.unwrap().by_creator);

    lab.allow_self_verification();
    let second = lab.destination();
    let verified = lab.verify(lab.other, second.destination_id, true).unwrap();
    assert!(!verified.verified.unwrap().by_creator);
    assert_eq!(lab.verification_events(), [None, None]);
}
