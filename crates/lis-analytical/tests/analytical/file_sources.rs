//! Result-file source registrations and attaching a served component to the
//! execution that measured it (result files brief F2, F17, §13; REQ-ANA-003).

use lis_analytical::{
    AnalyticalError, ApproveSourceInput, EndOfData, ExecutionId, FailExecutionInput, FileColumns,
    FileDelimiter, FileEncoding, FileLayout, FileShape, RegisterFileSourceInput,
    RegisterSourceInput, RegistrationState, RevisionIndicator, SampleIdField, SourceTestCodeInput,
    ValueColumn,
};
use lis_core::{Capability, ErrorCode, InstrumentId, Principal, SessionId, UserId, WorkstationId};
use lis_interfaces::{ConnectionSettings, InstrumentTransport, RegisterInstrument};
use lis_orders::{
    DecideCancellationInput, DecideFollowOnInput, Decision, ItemKind, OrderComponentId,
    RequestCancellationInput, RequestFollowOnInput, SequenceRole,
};
use rusqlite::params;

use crate::{Fixture, STAFF, URINE, code_of, counts, fixture, last_event, read, run, run_as};

fn file_instrument(f: &Fixture) -> InstrumentId {
    let input = RegisterInstrument {
        code: "syn-files".to_owned(),
        name: "Synthetic result files".to_owned(),
        connection: ConnectionSettings::ResultFile {},
        enabled: true,
    };
    f.setup(move |ctx| lis_interfaces::register_instrument(ctx, input))
        .id
}

pub(crate) fn layout() -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![
            ["Sample Name", "Component Name", "Final Conc", "Units"]
                .map(str::to_owned)
                .to_vec(),
        ],
        shape: FileShape::Long {
            analyte_column: 1,
            value_column: 2,
        },
        columns: FileColumns {
            sample_label: 0,
            units: Some(3),
            sample_type: None,
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: None,
        },
        end_of_data: EndOfData::EndOfFile,
        sample_types: Vec::new(),
        value_words: Vec::new(),
        not_results: vec!["Morphine-D3".to_owned()],
        declared_unit: None,
    }
}

fn input(f: &Fixture, instrument_id: InstrumentId) -> RegisterFileSourceInput {
    RegisterFileSourceInput {
        instrument_id,
        software_name: "SynthQuant".to_owned(),
        software_version: "12.1".to_owned(),
        reports_final_concentration: true,
        layout: layout(),
        test_codes: vec![
            SourceTestCodeInput {
                source_test_code: "Morphine".to_owned(),
                analyte_id: f.urine.analytes[0],
            },
            SourceTestCodeInput {
                source_test_code: "Codeine".to_owned(),
                analyte_id: f.urine.analytes[1],
            },
        ],
    }
}

fn user(id: UserId) -> Principal {
    Principal::user(
        id,
        SessionId::generate(),
        WorkstationId::generate(),
        STAFF.iter().copied(),
    )
}

fn last_details(f: &Fixture) -> String {
    read(f, |conn| {
        Ok(conn.query_row(
            "SELECT details FROM audit_events WHERE stream = 'clinical' ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?)
    })
}

#[test]
fn a_file_source_is_a_draft_registration_with_its_layout_approved_by_another_user() {
    let f = fixture();
    let instrument = file_instrument(&f);
    let author = UserId::generate();
    let request = input(&f, instrument);
    let draft = run_as(&f, user(author), move |ctx| {
        lis_analytical::register_file_source(ctx, request)
    })
    .unwrap();
    assert_eq!(draft.state, RegistrationState::Draft);
    assert_eq!(
        (draft.sender_name.as_str(), draft.sender_version.as_str()),
        ("SynthQuant", "12.1")
    );
    assert_eq!(draft.sample_id_field, SampleIdField::SpecimenId);
    assert_eq!(draft.revision_indicator, RevisionIndicator::None);
    assert!(draft.reports_final_concentration);
    assert_eq!(draft.file_layout, Some(layout()));
    assert_eq!(draft.test_codes.len(), 2);
    assert_eq!(last_event(&f).0, "analytical.register_source");
    let details = last_details(&f);
    assert!(details.contains("\"format\":\"delimited\""), "{details}");
    assert!(details.contains("\"test_codes\":2"), "{details}");
    approve_as_another_user(&f, author, draft.registration_id);
    let active = read(&f, move |conn| {
        lis_analytical::active_registration(conn, instrument)
    })
    .unwrap();
    assert_eq!(active.registration_id, draft.registration_id);
    assert_eq!(active.file_layout, Some(layout()));
}

#[test]
fn an_astm_registration_carries_no_layout() {
    let f = fixture();
    f.register_source(RevisionIndicator::None, false);
    let astm = f.instrument;
    let astm = read(&f, move |conn| {
        lis_analytical::active_registration(conn, astm)
    });
    assert_eq!(astm.unwrap().file_layout, None);
}

// L7 of the wave-10 review: a result-file instrument's source is a file template;
// an ASTM registration for it is refused and nothing is written.
#[test]
fn an_astm_registration_is_refused_for_a_result_file_instrument() {
    let f = fixture();
    let instrument = file_instrument(&f);
    let before = counts(&f);
    let input = RegisterSourceInput {
        au_profile: None,
        instrument_id: instrument,
        sender_name: "SYNTHETIC".to_owned(),
        sender_version: "1.0".to_owned(),
        sample_id_field: SampleIdField::SpecimenId,
        revision_indicator: RevisionIndicator::None,
        reports_final_concentration: false,
        test_codes: vec![SourceTestCodeInput {
            source_test_code: "A1".to_owned(),
            analyte_id: f.urine.analytes[0],
        }],
    };
    let refused = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, input)
    });
    assert_eq!(field_of(refused), "instrumentId");
    assert_eq!(counts(&f), before);
}

// The bench readiness view says how each instrument's results arrive and whether
// its active source is a file template.
#[test]
fn readiness_shows_the_connection_kind_and_a_file_template() {
    let f = fixture();
    f.register_source(RevisionIndicator::None, false);
    let instrument = file_instrument(&f);
    let author = UserId::generate();
    let request = input(&f, instrument);
    let draft = run_as(&f, user(author), move |ctx| {
        lis_analytical::register_file_source(ctx, request)
    })
    .unwrap();
    approve_as_another_user(&f, author, draft.registration_id);
    let readiness = read(&f, lis_analytical::list_instrument_readiness);
    let of = |id: InstrumentId| {
        readiness
            .iter()
            .find(|view| view.instrument_id == id)
            .map(|view| {
                (
                    view.transport,
                    view.active_registration.as_ref().map(|r| r.file_template),
                )
            })
            .unwrap()
    };
    assert_eq!(
        of(instrument),
        (InstrumentTransport::ResultFile, Some(true))
    );
    assert_eq!(of(f.instrument).1, Some(false));
    assert_ne!(of(f.instrument).0, InstrumentTransport::ResultFile);
}

/// The author cannot approve the draft; another signed-in user can.
fn approve_as_another_user(
    f: &Fixture,
    author: UserId,
    registration_id: lis_analytical::SourceRegistrationId,
) {
    let approve = |id| ApproveSourceInput {
        registration_id: id,
        expected_revision: 1,
        self_approval: false,
    };
    let own = approve(registration_id);
    let same = run_as(f, user(author), move |ctx| {
        lis_analytical::approve_source(ctx, own)
    });
    assert!(matches!(same, Err(AnalyticalError::SameAuthor)));
    let other = approve(registration_id);
    run_as(f, user(UserId::generate()), move |ctx| {
        lis_analytical::approve_source(ctx, other)
    })
    .unwrap();
}

#[test]
fn an_approved_layout_is_fixed() {
    let f = fixture();
    let instrument = file_instrument(&f);
    let request = input(&f, instrument);
    let draft = run(&f, STAFF, move |ctx| {
        lis_analytical::register_file_source(ctx, request)
    })
    .unwrap();
    let raw = f.raw();
    for sql in [
        "UPDATE ana_source_file_layouts SET layout_json = '{}'",
        "DELETE FROM ana_source_file_layouts",
    ] {
        assert!(raw.execute(sql, []).is_err(), "{sql}");
    }
    let ok = ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    };
    run_as(&f, user(UserId::generate()), move |ctx| {
        lis_analytical::approve_source(ctx, ok)
    })
    .unwrap();
    let id = draft.registration_id.to_string();
    let replaced = raw.execute(
        "INSERT INTO ana_source_file_layouts (registration_id, layout_json, layout_version) \
         VALUES (?1, '{}', 1)",
        [id],
    );
    assert!(replaced.is_err());
}

#[test]
fn a_file_source_needs_the_capability_a_file_instrument_and_a_consistent_layout() {
    let f = fixture();
    let instrument = file_instrument(&f);
    let before = counts(&f);
    let request = input(&f, instrument);
    let denied = run(&f, &[Capability::ApproveConfiguration], move |ctx| {
        lis_analytical::register_file_source(ctx, request)
    });
    assert_eq!(code_of(denied), ErrorCode::Forbidden);
    let astm = input(&f, f.instrument);
    let refused = run(&f, STAFF, move |ctx| {
        lis_analytical::register_file_source(ctx, astm)
    });
    assert_eq!(field_of(refused), "instrumentId");
    let unknown = input(&f, InstrumentId::generate());
    let missing = run(&f, STAFF, move |ctx| {
        lis_analytical::register_file_source(ctx, unknown)
    });
    assert_eq!(code_of(missing), ErrorCode::NotFound);

    let cases: [(Change, &str); 5] = [
        (|i| i.software_name = " ".to_owned(), "softwareName"),
        (|i| i.layout.columns.units = Some(9), "layout.columns.units"),
        (|i| i.test_codes.clear(), "testCodes"),
        (
            |i| i.layout.not_results = vec!["Codeine".to_owned()],
            "layout.notResults[0]",
        ),
        (
            |i| {
                i.layout.not_results.clear();
                i.layout.shape = FileShape::Wide {
                    value_columns: vec![ValueColumn {
                        column: 2,
                        analyte_name: "Fentanyl".to_owned(),
                    }],
                };
            },
            "layout.shape.valueColumns[0].analyteName",
        ),
    ];
    for (change, field) in cases {
        let mut request = input(&f, instrument);
        change(&mut request);
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::register_file_source(ctx, request)
        });
        assert_eq!(field_of(result), field);
    }
    assert_eq!(counts(&f), before);
}

/// A change to a valid registration input.
type Change = fn(&mut RegisterFileSourceInput);

fn field_of<T: std::fmt::Debug>(result: Result<T, AnalyticalError>) -> String {
    match result.unwrap_err() {
        AnalyticalError::Invalid(error) => error.field,
        other => panic!("not a validation error: {other:?}"),
    }
}

fn attach(
    f: &Fixture,
    execution: ExecutionId,
    component: OrderComponentId,
) -> Result<(), AnalyticalError> {
    run(f, &[], move |ctx| {
        lis_analytical::attach_served_component(ctx, execution, component)
    })
}

fn fail(f: &Fixture, execution_id: ExecutionId) {
    let input = FailExecutionInput {
        execution_id,
        expected_revision: 1,
        reason: "Synthetic clot".to_owned(),
    };
    run(f, STAFF, move |ctx| {
        lis_analytical::fail_execution(ctx, input)
    })
    .unwrap();
}

/// An active execution of `container` written below the domain: no command
/// creates one without its served components, which these tests need.
fn bare_execution(f: &Fixture, container: lis_specimens::ContainerId) -> ExecutionId {
    let id = ExecutionId::generate();
    f.raw()
        .execute(
            "INSERT INTO ana_executions (id, container_id, method_version_id, attempt_kind, \
             state, created_by, created_at, revision) \
             VALUES (?1, ?2, ?3, 'initial', 'active', 'test', '2026-01-02T00:00:00Z', 1)",
            params![
                id.to_string(),
                container.to_string(),
                f.urine.method.to_string()
            ],
        )
        .unwrap();
    id
}

#[test]
fn a_served_component_attaches_once_to_an_active_execution() {
    let f = fixture();
    let material = f.urine_material();
    let detail = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let first = detail.positions[0].execution.clone().unwrap();
    let component = first.component_ids[0];
    // Served already by the active first execution.
    let served = attach(&f, first.execution_id, component);
    assert!(matches!(served, Err(AnalyticalError::AlreadyServed)));
    fail(&f, first.execution_id);
    assert_eq!(
        code_of(attach(&f, first.execution_id, component)),
        ErrorCode::Conflict
    );
    assert_eq!(
        code_of(attach(&f, ExecutionId::generate(), component)),
        ErrorCode::NotFound
    );
    let second = bare_execution(&f, material.container_id);
    let before = counts(&f);
    attach(&f, second, component).unwrap();
    let components = read(&f, move |conn| {
        lis_analytical::execution_components(conn, second)
    });
    assert_eq!(components, [component]);
    // No audit event or other row of its own: the caller audits.
    let after = counts(&f);
    let changed = before.iter().zip(&after).filter(|(b, a)| b != a).count();
    assert_eq!(changed, 1);
    assert!(matches!(
        attach(&f, second, component),
        Err(AnalyticalError::AlreadyServed)
    ));
}

#[test]
fn a_component_the_execution_does_not_serve_or_that_is_cancelled_is_refused() {
    let f = fixture();
    let material = f.urine_material();
    let detail = f.place(f.open_run().run_id, "S1", &material.label).unwrap();
    let first = detail.positions[0].execution.clone().unwrap();
    let component = first.component_ids[0];
    fail(&f, first.execution_id);
    let second = bare_execution(&f, material.container_id);
    // Another order's oral-fluid component: another accession and method.
    let other = f.material(f.oral_fluid.test, crate::ORAL_FLUID, true);
    let other_component = order_component(&f, other.order_id, ItemKind::Initial);
    assert_eq!(
        field_of(attach(&f, second, other_component)),
        "orderComponentId"
    );
    // An add-on accessioned as its own material is not served for this
    // accession (brief §13: served for the execution's accession).
    let add_on = accepted_add_on(&f, &material);
    assert_eq!(field_of(attach(&f, second, add_on)), "orderComponentId");
    cancel(&f, material.order_id, component);
    assert!(matches!(
        attach(&f, second, component),
        Err(AnalyticalError::WrongState("order component"))
    ));
}

fn order_component(f: &Fixture, order_id: lis_orders::OrderId, kind: ItemKind) -> OrderComponentId {
    let order = f.setup(move |ctx| lis_orders::get_order(ctx.tx(), order_id));
    let item = order.items.iter().find(|item| item.kind == kind).unwrap();
    item.components[0].component_id
}

/// An accepted urine add-on on `material`'s order, accessioned separately.
fn accepted_add_on(f: &Fixture, material: &crate::Material) -> OrderComponentId {
    let request = RequestFollowOnInput {
        order_id: material.order_id,
        expected_revision: material.order_revision,
        kind: ItemKind::AddOn,
        orderable_version_id: f.urine.test,
        sequence_role: SequenceRole::FirstLine,
        parent_item_id: None,
        authority_reference: Some("SYN-ADDON-2".to_owned()),
        poc_result_id: None,
        trigger_source: None,
        trigger_ref: None,
        reason: "Synthetic clinic call".to_owned(),
    };
    let requested = f.setup(move |ctx| lis_orders::request_follow_on(ctx, request));
    let item = requested
        .items
        .iter()
        .find(|item| item.kind == ItemKind::AddOn)
        .unwrap()
        .clone();
    let decide = DecideFollowOnInput {
        item_id: item.item_id,
        expected_revision: item.revision,
        decision: Decision::Accept,
        reason: "Synthetic authority on file".to_owned(),
    };
    f.setup(move |ctx| lis_orders::decide_follow_on(ctx, decide));
    let input = crate::accession_input(material.order_id, vec![item.item_id], URINE);
    f.setup(move |ctx| lis_specimens::create_accession(ctx, input));
    order_component(f, material.order_id, ItemKind::AddOn)
}

fn cancel(f: &Fixture, order_id: lis_orders::OrderId, component: OrderComponentId) {
    let order = f.setup(move |ctx| lis_orders::get_order(ctx.tx(), order_id));
    let item = order
        .items
        .iter()
        .find(|item| item.components.iter().any(|c| c.component_id == component))
        .unwrap()
        .item_id;
    let request = RequestCancellationInput {
        item_id: item,
        requested_by: "Synthetic clinic".to_owned(),
        reason: "Synthetic cancellation".to_owned(),
    };
    let order = f.setup(move |ctx| lis_orders::request_cancellation(ctx, request));
    let pending = order.cancellations[0].clone();
    let decide = DecideCancellationInput {
        request_id: pending.request_id,
        expected_revision: pending.revision,
        decision: Decision::Accept,
        reason: "Synthetic cancellation accepted".to_owned(),
    };
    f.setup(move |ctx| lis_orders::decide_cancellation(ctx, decide));
}
