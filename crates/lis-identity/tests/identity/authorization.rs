//! Every command refuses a principal lacking its capability and writes nothing,
//! even when the principal holds every other capability.

use lis_core::{Capability, ErrorCode, Principal};
use lis_identity::{
    AddClientLocation, AddSubjectIdentifier, AssertDemographics, ClientId, ConfirmDistinctSubjects,
    CreateClient, DeactivateSubjectIdentifier, IdentityError, IdentityHoldId, RegisterPractitioner,
    RegisterSubject, SubjectId, SubjectIdentifierId, add_client_location, add_subject_identifier,
    assert_demographics, confirm_distinct_subjects, create_client, deactivate_subject_identifier,
    get_subject, register_practitioner, register_subject, set_laboratory_profile,
};
use lis_store::CommandContext;

use crate::{assert_code, counts, demographics, identifier, open, read, register, run, run_as};

const ALL: [Capability; 14] = [
    Capability::Accession,
    Capability::EnterOrders,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
    Capability::ReleaseReports,
    Capability::ManageQuality,
    Capability::Billing,
    Capability::ViewAudit,
    Capability::Export,
    Capability::BackupRestore,
    Capability::ManageUsers,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
    Capability::InstrumentIntake,
];

#[derive(Clone, Copy)]
struct Fixture {
    subject: SubjectId,
    identifier: SubjectIdentifierId,
    hold: IdentityHoldId,
    client: ClientId,
}

type Case = fn(Fixture, &mut CommandContext<'_>) -> Result<(), IdentityError>;

const CASES: [(&str, Capability, Case); 9] = [
    ("register_subject", Capability::EnterOrders, |_, ctx| {
        register_subject(
            ctx,
            RegisterSubject {
                demographics: demographics("Newperson", "Echo", "2001-01-01"),
                source: "requisition".to_owned(),
                identifiers: vec![],
                acknowledged_candidates: vec![],
            },
        )
        .map(drop)
    }),
    (
        "assert_demographics",
        Capability::ManageQuality,
        |f, ctx| {
            assert_demographics(
                ctx,
                AssertDemographics {
                    subject_id: f.subject,
                    expected_revision: 1,
                    demographics: demographics("Testperson", "Zeta", "1990-01-01"),
                    source: "client update".to_owned(),
                    reason: "Correction".to_owned(),
                    acknowledged_candidates: vec![],
                },
            )
            .map(drop)
        },
    ),
    (
        "add_subject_identifier",
        Capability::EnterOrders,
        |f, ctx| {
            add_subject_identifier(
                ctx,
                AddSubjectIdentifier {
                    subject_id: f.subject,
                    expected_revision: 1,
                    identifier: identifier("CLINIC-Z", "MR", "Z-1"),
                },
            )
            .map(drop)
        },
    ),
    (
        "deactivate_subject_identifier",
        Capability::ManageQuality,
        |f, ctx| {
            deactivate_subject_identifier(
                ctx,
                DeactivateSubjectIdentifier {
                    identifier_id: f.identifier,
                    expected_subject_revision: 1,
                    reason: "Mistyped".to_owned(),
                },
            )
            .map(drop)
        },
    ),
    (
        "confirm_distinct_subjects",
        Capability::ManageQuality,
        |f, ctx| {
            confirm_distinct_subjects(
                ctx,
                ConfirmDistinctSubjects {
                    hold_id: f.hold,
                    expected_revision: 1,
                    reason: "Different people".to_owned(),
                },
            )
            .map(drop)
        },
    ),
    (
        "create_client",
        Capability::ManageConfiguration,
        |_, ctx| {
            create_client(
                ctx,
                CreateClient {
                    code: "NEW".to_owned(),
                    name: "New Clinic".to_owned(),
                },
            )
            .map(drop)
        },
    ),
    (
        "add_client_location",
        Capability::ManageConfiguration,
        |f, ctx| {
            add_client_location(
                ctx,
                AddClientLocation {
                    client_id: f.client,
                    code: "SOUTH".to_owned(),
                    name: "South Site".to_owned(),
                },
            )
            .map(drop)
        },
    ),
    (
        "set_laboratory_profile",
        Capability::ManageConfiguration,
        |_, ctx| set_laboratory_profile(ctx, crate::laboratory::profile_input(None)).map(drop),
    ),
    (
        "register_practitioner",
        Capability::ManageConfiguration,
        |_, ctx| {
            register_practitioner(
                ctx,
                RegisterPractitioner {
                    npi: "1000000004".to_owned(),
                    family_name: "Doctorperson".to_owned(),
                    given_names: None,
                },
            )
            .map(drop)
        },
    ),
];

fn fixture(t: &lis_store::testing::TestDatabase) -> Fixture {
    let a = register(
        t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![identifier("CLINIC-A", "MR", "MRN-0001")],
        vec![],
    )
    .unwrap();
    let b = register(
        t,
        demographics("Testperson", "Alpha", "1990-01-01"),
        vec![],
        vec![a.subject_id],
    )
    .unwrap();
    let client = run(t, &[Capability::ManageConfiguration], |ctx| {
        create_client(
            ctx,
            CreateClient {
                code: "CLI-1".to_owned(),
                name: "Clinic".to_owned(),
            },
        )
    })
    .unwrap();
    let identifier = read(t, move |conn| get_subject(conn, a.subject_id)).identifiers[0].id;
    Fixture {
        subject: a.subject_id,
        identifier,
        hold: b.identity_hold_id.unwrap(),
        client,
    }
}

#[test]
fn every_command_refuses_without_its_capability_and_writes_nothing() {
    let t = open();
    let f = fixture(&t);
    for (name, required, case) in CASES {
        let others: Vec<Capability> = ALL.into_iter().filter(|c| *c != required).collect();
        let before = counts(&t);
        let result = run(&t, &others, move |ctx| case(f, ctx));
        assert_code(result, ErrorCode::Forbidden);
        assert_eq!(counts(&t), before, "{name} wrote despite the refusal");
    }
}

#[test]
fn every_command_succeeds_with_only_its_capability_and_audits() {
    for (name, required, case) in CASES {
        // A fresh database per case, so every fixture revision is still 1.
        let t = open();
        let f = fixture(&t);
        let before: i64 = *counts(&t).last().unwrap();
        run(&t, &[required], move |ctx| case(f, ctx)).unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(
            *counts(&t).last().unwrap() > before,
            "{name} wrote no audit event"
        );
    }
}

#[test]
fn identity_corrections_require_a_signed_in_user() {
    let t = open();
    let f = fixture(&t);
    let before = counts(&t);
    let demographic = run_as(
        &t,
        Principal::system("identity-importer", [Capability::ManageQuality]),
        move |ctx| {
            assert_demographics(
                ctx,
                AssertDemographics {
                    subject_id: f.subject,
                    expected_revision: 1,
                    demographics: demographics("Testperson", "Zeta", "1990-01-01"),
                    source: "client update".to_owned(),
                    reason: "Correction".to_owned(),
                    acknowledged_candidates: vec![],
                },
            )
        },
    );
    assert!(matches!(demographic, Err(IdentityError::UserRequired)));
    let deactivation = run_as(
        &t,
        Principal::system("identity-importer", [Capability::ManageQuality]),
        move |ctx| {
            deactivate_subject_identifier(
                ctx,
                DeactivateSubjectIdentifier {
                    identifier_id: f.identifier,
                    expected_subject_revision: 1,
                    reason: "Correction".to_owned(),
                },
            )
        },
    );
    assert!(matches!(deactivation, Err(IdentityError::UserRequired)));
    assert_eq!(counts(&t), before);
}
