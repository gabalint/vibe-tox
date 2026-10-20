//! Approved AU source settings and numeric units (REQ-INT-001/004/009).

use lis_analytical::{
    AnalyticalError, ApproveSourceInput, AuSourceProfile, AuTestDefinition, RegisterSourceInput,
    RegistrationState, RevisionIndicator, SampleIdField, SourceRegistrationView,
    SourceTestCodeInput,
};
use lis_au::{AuConfig, FormatConfig, FramingConfig, TransmissionClass};
use lis_core::{Capability, ErrorCode, InstrumentId, Principal, SessionId, UserId, WorkstationId};
use lis_interfaces::{
    AuConnectionSettings, ConnectionSettings, RegisterInstrument, UpdateInstrumentConnection,
};

use crate::{Fixture, STAFF, code_of, counts, fixture, last_event, read, run, run_as};

fn protocol() -> AuConfig {
    AuConfig {
        framing: FramingConfig {
            start: vec![2],
            end: vec![3],
            bcc: true,
            use_etb: true,
            max_wire_len: 512,
            class: TransmissionClass::B,
        },
        format: FormatConfig {
            unit_no: false,
            rack_digits: 4,
            sample_id_digits: 20,
            sex: false,
            age: false,
            month: false,
            patient_info_digits: [0; 6],
            result_digits: 9,
        },
    }
}

fn connection(protocol: AuConfig) -> ConnectionSettings {
    ConnectionSettings::AuTcp {
        bind_address: "127.0.0.1".parse().unwrap(),
        port: 19001,
        allowed_peers: vec!["127.0.0.1".parse().unwrap()],
        au: AuConnectionSettings {
            protocol,
            frame_timeout_ms: 2000,
            block_timeout_ms: 9000,
        },
    }
}

fn instrument(f: &Fixture) -> InstrumentId {
    f.setup(|ctx| {
        lis_interfaces::register_instrument(
            ctx,
            RegisterInstrument {
                code: "syn-au".to_owned(),
                name: "Synthetic AU400".to_owned(),
                connection: connection(protocol()),
                enabled: false,
            },
        )
    })
    .id
}

fn input(f: &Fixture, instrument_id: InstrumentId) -> RegisterSourceInput {
    RegisterSourceInput {
        instrument_id,
        sender_name: "Declared synthetic AU400".to_owned(),
        sender_version: "Synthetic software 1".to_owned(),
        sample_id_field: SampleIdField::SpecimenId,
        revision_indicator: RevisionIndicator::None,
        reports_final_concentration: true,
        test_codes: vec![SourceTestCodeInput {
            source_test_code: "01".to_owned(),
            analyte_id: f.urine.analytes[0],
        }],
        au_profile: Some(AuSourceProfile {
            protocol: protocol(),
            tests: vec![AuTestDefinition {
                test_code: "01".to_owned(),
                unit: Some("ng/mL".to_owned()),
            }],
        }),
    }
}

fn approve(
    f: &Fixture,
    draft: &SourceRegistrationView,
) -> Result<SourceRegistrationView, AnalyticalError> {
    let input = ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    };
    run(f, &[Capability::ApproveConfiguration], move |ctx| {
        lis_analytical::approve_source(ctx, input)
    })
}

fn field_of<T: std::fmt::Debug>(result: Result<T, AnalyticalError>) -> String {
    match result.unwrap_err() {
        AnalyticalError::Invalid(error) => error.field,
        error => panic!("Expected validation failure: {error:?}"),
    }
}

#[test]
fn approved_au_profile_round_trips_as_declared_configuration_with_audit() {
    let f = fixture();
    let instrument = instrument(&f);
    let input = input(&f, instrument);
    let expected = input.au_profile.clone();
    let draft = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, input)
    })
    .unwrap();
    assert_eq!(draft.state, RegistrationState::Draft);
    assert_eq!(draft.au_profile, expected);
    assert_eq!(draft.file_layout, None);
    assert_eq!(last_event(&f).0, "analytical.register_source");
    let active = approve(&f, &draft).unwrap();
    assert_eq!(active.state, RegistrationState::Active);
    assert_eq!(active.au_profile, expected);
    assert_eq!(last_event(&f).0, "analytical.approve_source");
    let stored = read(&f, move |conn| {
        lis_analytical::active_registration(conn, instrument)
    })
    .unwrap();
    assert_eq!(stored.au_profile, expected);
    for sql in [
        "UPDATE ana_source_au_profiles SET profile_json = '{}'",
        "DELETE FROM ana_source_au_profiles",
    ] {
        assert!(f.raw().execute(sql, []).is_err());
    }
}

#[test]
fn au_registration_and_approval_keep_their_authorization_boundaries() {
    let f = fixture();
    let instrument = instrument(&f);
    let before = counts(&f);
    let request = input(&f, instrument);
    let denied = run(&f, &[Capability::ApproveConfiguration], move |ctx| {
        lis_analytical::register_source(ctx, request)
    });
    assert_eq!(code_of(denied), ErrorCode::Forbidden);
    assert_eq!(counts(&f), before);
    let author = UserId::generate();
    let request = input(&f, instrument);
    let user = || {
        Principal::user(
            author,
            SessionId::generate(),
            WorkstationId::generate(),
            STAFF.iter().copied(),
        )
    };
    let draft = run_as(&f, user(), move |ctx| {
        lis_analytical::register_source(ctx, request)
    })
    .unwrap();
    let request = ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    };
    let denied = run_as(&f, user(), move |ctx| {
        lis_analytical::approve_source(ctx, request)
    });
    assert!(matches!(denied, Err(AnalyticalError::SameAuthor)));
    let request = ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    };
    let denied = run(&f, &[Capability::ManageConfiguration], move |ctx| {
        lis_analytical::approve_source(ctx, request)
    });
    assert_eq!(code_of(denied), ErrorCode::Forbidden);
    // Approvers do not need the author's ManageConfiguration capability.
    approve(&f, &draft).unwrap();
}

type Change = fn(&mut RegisterSourceInput);

fn profile(input: &mut RegisterSourceInput) -> &mut AuSourceProfile {
    input.au_profile.as_mut().unwrap()
}

#[test]
fn au_source_refuses_incomplete_or_ambiguous_numeric_declarations() {
    let f = fixture();
    let instrument = instrument(&f);
    let before = counts(&f);
    let cases: [(Change, &str); 11] = [
        (|i| i.au_profile = None, "instrumentId"),
        (
            |i| i.revision_indicator = RevisionIndicator::ResultStatusCorrection,
            "auProfile",
        ),
        (
            |i| profile(i).protocol.format.sample_id_digits = 10,
            "instrumentId",
        ),
        (
            |i| profile(i).protocol.format.sample_id_digits = 0,
            "auProfile.protocol",
        ),
        (|i| profile(i).tests.clear(), "auProfile.tests"),
        (
            |i| profile(i).tests[0].test_code = "02".to_owned(),
            "auProfile.tests",
        ),
        (
            |i| profile(i).tests[0].test_code = "00".to_owned(),
            "auProfile.tests[0].testCode",
        ),
        (
            |i| {
                let p = profile(i);
                p.tests.push(p.tests[0].clone());
            },
            "auProfile.tests[1].testCode",
        ),
        (
            |i| profile(i).tests[0].unit = Some(" ng/mL".to_owned()),
            "auProfile.tests[0].unit",
        ),
        (
            |i| profile(i).tests[0].unit = Some(String::new()),
            "auProfile.tests[0].unit",
        ),
        (
            |i| i.sample_id_field = SampleIdField::InstrumentSpecimenId,
            "auProfile",
        ),
    ];
    for (change, field) in cases {
        let mut request = input(&f, instrument);
        change(&mut request);
        let refused = run(&f, STAFF, move |ctx| {
            lis_analytical::register_source(ctx, request)
        });
        assert_eq!(field_of(refused), field);
        assert_eq!(counts(&f), before);
    }
}

#[test]
fn au_profile_is_refused_on_astm_and_its_declared_unit_may_be_unitless() {
    let f = fixture();
    let request = input(&f, f.instrument);
    assert_eq!(
        field_of(run(&f, STAFF, move |ctx| lis_analytical::register_source(
            ctx, request
        ))),
        "instrumentId"
    );
    let instrument = instrument(&f);
    let mut request = input(&f, instrument);
    request.au_profile.as_mut().unwrap().tests[0].unit = None;
    let draft = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, request)
    })
    .unwrap();
    assert_eq!(draft.au_profile.unwrap().tests[0].unit, None);
}

#[test]
fn approval_refuses_wire_configuration_drift_and_keeps_the_previous_active_source() {
    let f = fixture();
    let instrument = instrument(&f);
    let request = input(&f, instrument);
    let first = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, request)
    })
    .unwrap();
    let active = approve(&f, &first).unwrap();
    let request = input(&f, instrument);
    let draft = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, request)
    })
    .unwrap();
    let mut wire = protocol();
    wire.format.result_digits = 6;
    f.setup(move |ctx| {
        lis_interfaces::update_instrument_connection(
            ctx,
            UpdateInstrumentConnection {
                instrument_id: instrument,
                expected_revision: 1,
                connection: connection(wire),
            },
        )
    });
    let before = counts(&f);
    assert_eq!(field_of(approve(&f, &draft)), "instrumentId");
    assert_eq!(counts(&f), before);
    let current = read(&f, move |conn| {
        lis_analytical::active_registration(conn, instrument)
    })
    .unwrap();
    assert_eq!(current.registration_id, active.registration_id);
}

#[test]
fn source_formats_cannot_change_from_astm_or_files_to_au_before_approval() {
    let f = fixture();
    let mut request = input(&f, f.instrument);
    request.au_profile = None;
    let astm = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, request)
    })
    .unwrap();
    let file_id = f
        .setup(|ctx| {
            lis_interfaces::register_instrument(
                ctx,
                RegisterInstrument {
                    code: "syn-file-drift".to_owned(),
                    name: "Synthetic files".to_owned(),
                    connection: ConnectionSettings::ResultFile {},
                    enabled: false,
                },
            )
        })
        .id;
    let request = lis_analytical::RegisterFileSourceInput {
        instrument_id: file_id,
        software_name: "Synthetic files".to_owned(),
        software_version: "1".to_owned(),
        reports_final_concentration: true,
        layout: super::file_sources::layout(),
        test_codes: input(&f, file_id).test_codes,
    };
    let file = run(&f, STAFF, move |ctx| {
        lis_analytical::register_file_source(ctx, request)
    })
    .unwrap();
    for (draft, endpoint) in [(astm, 19001), (file, 19002)] {
        let instrument_id = draft.instrument_id;
        let mut updated = connection(protocol());
        if let ConnectionSettings::AuTcp { port, .. } = &mut updated {
            *port = endpoint;
        }
        f.setup(move |ctx| {
            lis_interfaces::update_instrument_connection(
                ctx,
                UpdateInstrumentConnection {
                    instrument_id,
                    expected_revision: 1,
                    connection: updated,
                },
            )
        });
        let before = counts(&f);
        assert_eq!(field_of(approve(&f, &draft)), "instrumentId");
        assert_eq!(counts(&f), before);
    }
}
