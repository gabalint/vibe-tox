//! Clients, locations and practitioners.

use lis_core::{Capability, ErrorCode};
use lis_identity::{
    AddClientLocation, ClientId, CreateClient, IdentityError, PractitionerId, RegisterPractitioner,
    add_client_location, audit_record_label, create_client, get_client, get_practitioner,
    list_clients, list_practitioners, register_practitioner,
};
use lis_store::testing::TestDatabase;

use crate::{assert_code, audit_actions, counts, open, read, run};

const CONFIGURE: &[Capability] = &[Capability::ManageConfiguration];

fn client(t: &TestDatabase, code: &str) -> Result<ClientId, IdentityError> {
    let code = code.to_owned();
    run(t, CONFIGURE, move |ctx| {
        create_client(
            ctx,
            CreateClient {
                code,
                name: "Synthetic Clinic".to_owned(),
            },
        )
    })
}

fn location(
    t: &TestDatabase,
    client_id: ClientId,
    code: &str,
) -> Result<lis_identity::ClientLocationId, IdentityError> {
    let code = code.to_owned();
    run(t, CONFIGURE, move |ctx| {
        add_client_location(
            ctx,
            AddClientLocation {
                client_id,
                code,
                name: "Synthetic Site".to_owned(),
            },
        )
    })
}

fn practitioner(
    t: &TestDatabase,
    npi: &str,
) -> Result<lis_identity::PractitionerId, IdentityError> {
    let npi = npi.to_owned();
    run(t, CONFIGURE, move |ctx| {
        register_practitioner(
            ctx,
            RegisterPractitioner {
                npi,
                family_name: "Doctorperson".to_owned(),
                given_names: Some("Delta".to_owned()),
            },
        )
    })
}

#[test]
fn clients_locations_and_practitioners_are_created_and_listed() {
    let t = open();
    let client_id = client(&t, "CLI-1").unwrap();
    let north = location(&t, client_id, "NORTH").unwrap();
    // The same location code under another client is allowed.
    let other = client(&t, "CLI-2").unwrap();
    location(&t, other, "NORTH").unwrap();
    let practitioner_id = practitioner(&t, "1234567893").unwrap();

    let clients = read(&t, list_clients);
    assert_eq!(clients.len(), 2);
    assert_eq!(clients[0].code, "CLI-1");
    assert_eq!(clients[0].locations.len(), 1);
    assert_eq!(clients[0].locations[0].id, north);
    let practitioners = read(&t, list_practitioners);
    assert_eq!(practitioners.len(), 1);
    assert_eq!(practitioners[0].id, practitioner_id);
    assert_eq!(practitioners[0].npi.as_str(), "1234567893");
    assert_eq!(
        audit_actions(&t),
        [
            "identity.create_client",
            "identity.add_client_location",
            "identity.create_client",
            "identity.add_client_location",
            "identity.register_practitioner",
        ]
    );
}

#[test]
fn duplicate_codes_and_npis_are_refused() {
    let t = open();
    let client_id = client(&t, "CLI-1").unwrap();
    location(&t, client_id, "NORTH").unwrap();
    practitioner(&t, "1234567893").unwrap();
    let before = counts(&t);
    assert!(matches!(
        client(&t, "CLI-1"),
        Err(IdentityError::Duplicate("client"))
    ));
    assert!(matches!(
        location(&t, client_id, "NORTH"),
        Err(IdentityError::Duplicate("client location"))
    ));
    assert!(matches!(
        practitioner(&t, "1234567893"),
        Err(IdentityError::Duplicate("practitioner"))
    ));
    assert_eq!(counts(&t), before);
}

#[test]
fn an_invalid_npi_is_refused() {
    let t = open();
    for npi in ["1234567890", "123456789", "abcdefghij"] {
        assert_code(practitioner(&t, npi), ErrorCode::Validation);
    }
    assert!(read(&t, list_practitioners).is_empty());
}

#[test]
fn a_location_needs_an_existing_client() {
    let t = open();
    let result = location(&t, ClientId::generate(), "NORTH");
    assert!(matches!(result, Err(IdentityError::NotFound("client"))));
}

#[test]
fn one_client_or_practitioner_is_read_by_id() {
    let t = open();
    let client_id = client(&t, "CLI-1").unwrap();
    let north = location(&t, client_id, "NORTH").unwrap();
    client(&t, "CLI-2").unwrap();
    let practitioner_id = practitioner(&t, "1234567893").unwrap();
    let found = read(&t, move |conn| get_client(conn, client_id));
    assert_eq!(
        (found.code.as_str(), found.locations[0].id),
        ("CLI-1", north)
    );
    let found = read(&t, move |conn| get_practitioner(conn, practitioner_id));
    assert_eq!(found.npi.as_str(), "1234567893");
    let missing =
        t.db.read_blocking(|conn| get_client(conn, ClientId::generate()));
    assert!(matches!(missing, Err(IdentityError::NotFound("client"))));
    let missing =
        t.db.read_blocking(|conn| get_practitioner(conn, PractitionerId::generate()));
    assert!(matches!(
        missing,
        Err(IdentityError::NotFound("practitioner"))
    ));
}

#[test]
fn audit_labels_are_resolved_from_current_identity_reads() {
    let t = open();
    let client_id = client(&t, "CLI-1").unwrap();
    let practitioner_id = practitioner(&t, "1234567893").unwrap();
    let client_label = read(&t, move |conn| {
        audit_record_label(conn, "client", &client_id.to_string())
    });
    let practitioner_label = read(&t, move |conn| {
        audit_record_label(conn, "practitioner", &practitioner_id.to_string())
    });
    assert_eq!(client_label.as_deref(), Some("Synthetic Clinic"));
    assert_eq!(practitioner_label.as_deref(), Some("Doctorperson, Delta"));
    assert_eq!(
        read(&t, |conn| audit_record_label(conn, "client", "not-an-id")),
        None
    );
}

#[test]
fn directory_corrections_preserve_identity_attributed_history_and_immutable_records() {
    use lis_identity::{DirectoryHistoryInput, DirectoryRecord, DirectorySnapshot, UpdateClient};
    let t = open();
    let client_id = client(&t, "CLI-OLD").unwrap();
    let corrected = run(&t, CONFIGURE, move |ctx| {
        lis_identity::update_client(
            ctx,
            UpdateClient {
                client_id,
                expected_revision: 1,
                code: "CLI-CORRECT".into(),
                name: "Synthetic corrected clinic".into(),
                active: true,
                reason: "Verified original source spelling".into(),
            },
        )
    })
    .unwrap();
    assert_eq!(corrected.id, client_id);
    assert_eq!(corrected.revision, 2);
    let history = read(&t, move |conn| {
        lis_identity::directory_history(
            conn,
            DirectoryHistoryInput {
                record: DirectoryRecord::Client(client_id),
            },
        )
    });
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].origin, "registered");
    assert!(history[0].recorded_by.is_some());
    assert!(matches!(&history[0].snapshot,DirectorySnapshot::Client{code,..} if code=="CLI-OLD"));
    assert_eq!(
        history[1].reason.as_deref(),
        Some("Verified original source spelling")
    );
    // All fixture commands share a clock: revision, not timestamp, orders history.
    assert_eq!(history[0].recorded_at, history[1].recorded_at);
    read(&t, move |conn| {
        assert!(
            conn.execute("DELETE FROM idn_directory_history", [])
                .is_err()
        );
        assert!(
            conn.execute(
                "UPDATE idn_client_organizations SET name='silent rewrite' WHERE id=?1",
                [client_id]
            )
            .is_err()
        );
        Ok(())
    });
}

#[test]
fn retired_locations_and_practitioners_are_readable_but_refused_for_new_order_sources() {
    use lis_identity::{UpdateClientLocation, UpdatePractitioner};
    let t = open();
    let client_id = client(&t, "CLI-OLD").unwrap();
    let location_id = location(&t, client_id, "SITE-OLD").unwrap();
    let practitioner_id = practitioner(&t, "1234567893").unwrap();
    let retired = run(&t, CONFIGURE, move |ctx| {
        lis_identity::update_client_location(
            ctx,
            UpdateClientLocation {
                location_id,
                expected_revision: 1,
                code: "SITE-OLD".into(),
                name: "Synthetic Site".into(),
                active: false,
                reason: "Site no longer receives orders".into(),
            },
        )
    })
    .unwrap();
    assert!(!retired.active);
    let active = read(&t, lis_identity::list_active_clients);
    assert_eq!(active[0].id, client_id);
    assert!(active[0].locations.is_empty());
    let refusal = t.db.read_blocking(move |conn| {
        lis_identity::assert_active_ordering_source(
            conn,
            client_id,
            Some(location_id),
            practitioner_id,
        )
    });
    assert_code(refusal, ErrorCode::Validation);
    run(&t, CONFIGURE, move |ctx| {
        lis_identity::update_practitioner(
            ctx,
            UpdatePractitioner {
                practitioner_id,
                expected_revision: 1,
                npi: "1234567893".into(),
                family_name: "Corrected Doctorperson".into(),
                given_names: Some("Delta".into()),
                active: false,
                reason: "No longer ordering".into(),
            },
        )
    })
    .unwrap();
    assert!(read(&t, lis_identity::list_active_practitioners).is_empty());
    assert!(!read(&t, move |conn| get_practitioner(conn, practitioner_id)).active);
}

#[test]
fn directory_correction_refuses_permission_staleness_duplicates_and_missing_reason() {
    use lis_identity::UpdateClient;
    let t = open();
    let id = client(&t, "A").unwrap();
    client(&t, "B").unwrap();
    let input = UpdateClient {
        client_id: id,
        expected_revision: 1,
        code: "A".into(),
        name: "Synthetic corrected".into(),
        active: false,
        reason: "Supplied correction".into(),
    };
    let denied = input.clone();
    assert_code(
        run(&t, &[], move |ctx| lis_identity::update_client(ctx, denied)),
        ErrorCode::Forbidden,
    );
    let mut stale = input.clone();
    stale.expected_revision = 5;
    assert_code(
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_client(ctx, stale)
        }),
        ErrorCode::Conflict,
    );
    let mut duplicate = input.clone();
    duplicate.code = "B".into();
    assert_code(
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_client(ctx, duplicate)
        }),
        ErrorCode::Conflict,
    );
    let mut no_reason = input;
    no_reason.reason = "  ".into();
    assert_code(
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_client(ctx, no_reason)
        }),
        ErrorCode::Validation,
    );
    let history = read(&t, move |conn| {
        lis_identity::directory_history(
            conn,
            lis_identity::DirectoryHistoryInput {
                record: lis_identity::DirectoryRecord::Client(id),
            },
        )
    });
    assert_eq!(history.len(), 1);
    assert_eq!(read(&t, move |conn| get_client(conn, id)).revision, 1);
}

#[test]
fn directory_correction_refuses_a_correction_that_changes_nothing() {
    use lis_identity::{
        DirectoryHistoryInput, DirectoryRecord, UpdateClient, UpdateClientLocation,
        UpdatePractitioner, directory_history,
    };
    let t = open();
    let client_id = client(&t, "CLI-SAME").unwrap();
    let location_id = location(&t, client_id, "SITE-SAME").unwrap();
    let practitioner_id = practitioner(&t, "1234567893").unwrap();
    let reason = || "Synthetic test, no change".to_owned();
    let refusals = [
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_client(
                ctx,
                UpdateClient {
                    client_id,
                    expected_revision: 1,
                    code: "CLI-SAME".into(),
                    name: "Synthetic Clinic".into(),
                    active: true,
                    reason: reason(),
                },
            )
            .map(|_| ())
        }),
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_client_location(
                ctx,
                UpdateClientLocation {
                    location_id,
                    expected_revision: 1,
                    code: "SITE-SAME".into(),
                    name: "Synthetic Site".into(),
                    active: true,
                    reason: reason(),
                },
            )
            .map(|_| ())
        }),
        // Surrounding spaces are trimmed before the comparison.
        run(&t, CONFIGURE, move |ctx| {
            lis_identity::update_practitioner(
                ctx,
                UpdatePractitioner {
                    practitioner_id,
                    expected_revision: 1,
                    npi: " 1234567893 ".into(),
                    family_name: "Doctorperson ".into(),
                    given_names: Some(" Delta".into()),
                    active: true,
                    reason: reason(),
                },
            )
            .map(|_| ())
        }),
    ];
    for refusal in refusals {
        let error = refusal.unwrap_err();
        assert!(matches!(error, IdentityError::NothingChanged), "{error:?}");
        assert_eq!(error.code(), ErrorCode::Validation);
    }
    for record in [
        DirectoryRecord::Client(client_id),
        DirectoryRecord::Location(location_id),
        DirectoryRecord::Practitioner(practitioner_id),
    ] {
        let history = read(&t, move |conn| {
            directory_history(conn, DirectoryHistoryInput { record })
        });
        assert_eq!(history.len(), 1, "{record:?}");
    }
    assert_eq!(
        read(&t, move |conn| get_practitioner(conn, practitioner_id)).revision,
        1
    );
}

#[test]
fn migrated_directory_baseline_preserves_facts_without_inventing_attribution() {
    let conn = rusqlite::Connection::open_in_memory().unwrap();
    for migration in &lis_identity::SCHEMA.migrations[..3] {
        conn.execute_batch(migration.sql).unwrap();
    }
    let id = ClientId::generate();
    conn.execute("INSERT INTO idn_client_organizations(id,code,name,created_at) VALUES(?1,'LEGACY','Synthetic retained clinic','2020-01-02T03:04:05.000000000Z')",[id]).unwrap();
    conn.execute_batch(lis_identity::SCHEMA.migrations[3].sql)
        .unwrap();
    let current = get_client(&conn, id).unwrap();
    assert_eq!(current.code, "LEGACY");
    assert!(current.active);
    assert_eq!(current.revision, 1);
    let history = lis_identity::directory_history(
        &conn,
        lis_identity::DirectoryHistoryInput {
            record: lis_identity::DirectoryRecord::Client(id),
        },
    )
    .unwrap();
    assert_eq!(history.len(), 1);
    assert_eq!(history[0].origin, "legacy_baseline");
    assert!(history[0].recorded_by.is_none());
    assert!(history[0].reason.is_none());
    assert_eq!(
        history[0].recorded_at.to_string(),
        "2020-01-02T03:04:05.000000000Z"
    );
}
