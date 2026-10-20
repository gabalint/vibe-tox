//! Self-approval of source registrations under the laboratory's
//! separation-of-duties policy (ADR 0015; REQ-ANA-003), and the upgrade of an
//! installed database to the rebuilt registrations table (migration 5).

use lis_analytical::{
    AnalyticalError, ApproveSourceInput, RegisterSourceInput, RegistrationState, RevisionIndicator,
    SampleIdField, SourceRegistrationId, SourceRegistrationView, SourceTestCodeInput,
};
use lis_core::{ErrorCode, ModuleSchema, Principal, SessionId, UserId, WorkstationId};
use lis_store::testing::TEST_START;
use lis_store::{DATABASE_FILE, Database};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension};

use crate::{Fixture, MODULES, STAFF, counts, fixture, fixture_on, run_as};

fn user(id: UserId) -> Principal {
    Principal::user(
        id,
        SessionId::generate(),
        WorkstationId::generate(),
        STAFF.iter().copied(),
    )
}

/// A draft registration by `author` mapping urine analyte A to `code`.
fn draft(f: &Fixture, author: UserId, code: &str) -> SourceRegistrationView {
    let input = RegisterSourceInput {
        au_profile: None,
        instrument_id: f.instrument,
        sender_name: "SYNTHETIC".to_owned(),
        sender_version: "3.0".to_owned(),
        sample_id_field: SampleIdField::InstrumentSpecimenId,
        revision_indicator: RevisionIndicator::None,
        reports_final_concentration: false,
        test_codes: vec![SourceTestCodeInput {
            source_test_code: code.to_owned(),
            analyte_id: f.urine.analytes[0],
        }],
    };
    run_as(f, user(author), move |ctx| {
        lis_analytical::register_source(ctx, input)
    })
    .unwrap()
}

fn approve(
    f: &Fixture,
    approver: UserId,
    registration: &SourceRegistrationView,
    self_approval: bool,
) -> Result<SourceRegistrationView, AnalyticalError> {
    let input = ApproveSourceInput {
        registration_id: registration.registration_id,
        expected_revision: registration.revision,
        self_approval,
    };
    run_as(f, user(approver), move |ctx| {
        lis_analytical::approve_source(ctx, input)
    })
}

/// Records a policy version that lets the author approve configuration, as a
/// quality manager would (`lis_auth::set_separation_policy`).
fn allow_self_approval(conn: &Connection) {
    conn.execute(
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (1, 1, 0, 0, 'Synthetic one-person laboratory', 'synthetic-quality-manager', ?1)",
        [TEST_START],
    )
    .unwrap();
}

/// The `self_approval` detail of the registration's approval event.
fn self_approval_mark(f: &Fixture, registration: &SourceRegistrationView) -> Option<i64> {
    f.raw()
        .query_row(
            "SELECT json_extract(details, '$.self_approval') FROM audit_events \
             WHERE action = 'analytical.approve_source' AND record_id = ?1",
            [registration.registration_id.to_string()],
            |row| row.get(0),
        )
        .optional()
        .unwrap()
        .flatten()
}

#[test]
fn the_default_policy_refuses_the_author_even_when_confirmed() {
    let f = fixture();
    let author = UserId::generate();
    let registration = draft(&f, author, "S1");
    let before = counts(&f);
    let error = approve(&f, author, &registration, true).unwrap_err();
    assert!(matches!(error, AnalyticalError::SameAuthor), "{error:?}");
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    let message = error.to_string();
    assert!(message.contains("separation-of-duties policy"), "{message}");
    assert!(message.contains("quality manager"), "{message}");
    assert_eq!(counts(&f), before);
}

#[test]
fn an_allowed_self_approval_still_needs_the_authors_confirmation() {
    let f = fixture();
    let author = UserId::generate();
    let registration = draft(&f, author, "S1");
    allow_self_approval(&f.raw());
    let before = counts(&f);
    let error = approve(&f, author, &registration, false).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let fields = error.field_errors();
    assert_eq!(fields.len(), 1);
    assert_eq!(
        (fields[0].field.as_str(), fields[0].code.as_str()),
        ("selfApproval", "self_approval_unconfirmed")
    );
    assert_eq!(counts(&f), before);
}

#[test]
fn a_confirmed_self_approval_activates_the_registration_and_is_marked() {
    let f = fixture();
    let author = UserId::generate();
    let registration = draft(&f, author, "S1");
    allow_self_approval(&f.raw());
    let active = approve(&f, author, &registration, true).unwrap();
    assert_eq!(active.state, RegistrationState::Active);
    assert_eq!(
        active.approved_by.as_deref(),
        Some(active.authored_by.as_str())
    );
    assert_eq!(self_approval_mark(&f, &registration), Some(1));
}

#[test]
fn another_approver_is_unaffected_by_the_policy_and_not_marked() {
    for allowed in [false, true] {
        let f = fixture();
        let registration = draft(&f, UserId::generate(), "S1");
        if allowed {
            allow_self_approval(&f.raw());
        }
        let active = approve(&f, UserId::generate(), &registration, false).unwrap();
        assert_eq!(active.state, RegistrationState::Active);
        assert_ne!(
            active.approved_by.as_deref(),
            Some(active.authored_by.as_str())
        );
        assert_eq!(self_approval_mark(&f, &registration), None);
    }
}

const ACTIVATE_BY_AUTHOR: &str = "UPDATE ana_source_registrations SET state = 'active', \
     approved_by = authored_by, approved_at = authored_at, revision = revision + 1 \
     WHERE state = 'draft'";

#[test]
fn the_schema_refuses_an_author_activation_while_the_policy_needs_a_second_person() {
    let f = fixture();
    draft(&f, UserId::generate(), "S1");
    let conn = f.raw();
    let message = conn
        .execute(ACTIVATE_BY_AUTHOR, [])
        .unwrap_err()
        .to_string();
    assert!(message.contains("separation of duties"), "{message}");
    allow_self_approval(&conn);
    assert_eq!(conn.execute(ACTIVATE_BY_AUTHOR, []).unwrap(), 1);
}

/// Every row of `table`, ordered by its first two columns.
fn rows(conn: &Connection, table: &str) -> Vec<Vec<Value>> {
    let mut statement = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
        .unwrap();
    let width = statement.column_count();
    statement
        .query_map([], |row| (0..width).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The rebuilt tables of both modules and tables that refer to them.
const TABLES: [&str; 6] = [
    "ana_source_registrations",
    "ana_source_test_codes",
    "cat_method_versions",
    "cat_orderable_versions",
    "ord_order_items",
    "audit_events",
];

/// [`MODULES`] with the catalog and analytical schemas before migration 5.
fn legacy_modules() -> [ModuleSchema; 7] {
    let mut legacy = MODULES;
    legacy[2] = ModuleSchema {
        migrations: &lis_catalog::MIGRATIONS[..4],
        ..lis_catalog::SCHEMA
    };
    legacy[6] = ModuleSchema {
        migrations: &lis_analytical::MIGRATIONS[..4],
        ..lis_analytical::SCHEMA
    };
    legacy
}

/// Writes a registration with one test code and moves it to `state` along its
/// lifecycle, as an installation from before migration 5 holds it. Raw SQL, so
/// the seed does not depend on today's commands running on the old schema.
fn seed_registration(conn: &Connection, f: &Fixture, code: &str, state: &str) -> String {
    let id = SourceRegistrationId::generate().to_string();
    let instrument = f.instrument.to_string();
    let analyte = f.urine.analytes[0].to_string();
    conn.execute(
        "INSERT INTO ana_source_registrations (id, instrument_id, sender_name, sender_version, \
         sample_id_field, revision_indicator, reports_final_concentration, state, authored_by, \
         authored_at, revision) VALUES (?1, ?2, 'SYNTHETIC', '1.0', 'specimen_id', 'none', 0, \
         'draft', 'synthetic-author', ?3, 1)",
        [id.as_str(), instrument.as_str(), TEST_START],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ana_source_test_codes (registration_id, source_test_code, analyte_id) \
         VALUES (?1, ?2, ?3)",
        [id.as_str(), code, analyte.as_str()],
    )
    .unwrap();
    if state != "draft" {
        conn.execute(
            "UPDATE ana_source_registrations SET state = 'active', \
             approved_by = 'synthetic-approver', approved_at = ?2, revision = 2 WHERE id = ?1",
            [id.as_str(), TEST_START],
        )
        .unwrap();
    }
    if state == "retired" {
        conn.execute(
            "UPDATE ana_source_registrations SET state = 'retired', retired_at = ?2, \
             revision = 3 WHERE id = ?1",
            [id.as_str(), TEST_START],
        )
        .unwrap();
    }
    id
}

/// The first column of every row of `sql`.
fn strings(conn: &Connection, sql: &str) -> Vec<String> {
    conn.prepare(sql)
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

/// The draft-only triggers of the dependent tables still read the rebuilt table.
fn assert_draft_only_guards(conn: &Connection, analyte: &str) {
    for (sql, value) in [
        (
            "INSERT INTO ana_source_test_codes (registration_id, source_test_code, analyte_id) \
             SELECT id, 'Z9', ?1 FROM ana_source_registrations WHERE state = 'active'",
            analyte,
        ),
        (
            "INSERT INTO ana_source_file_layouts (registration_id, layout_json, layout_version) \
             SELECT id, ?1, 1 FROM ana_source_registrations WHERE state = 'active'",
            "{}",
        ),
    ] {
        let message = conn.execute(sql, [value]).unwrap_err().to_string();
        assert!(message.contains("approved"), "{sql}: {message}");
    }
}

#[test]
fn an_installed_database_keeps_its_registrations_and_guards_through_the_rebuild() {
    let f = fixture_on(&legacy_modules());
    // Approved catalog versions, an accepted order and three registrations.
    f.urine_material();
    let raw = f.raw();
    seed_registration(&raw, &f, "S1", "retired");
    seed_registration(&raw, &f, "S2", "active");
    let pending = seed_registration(&raw, &f, "S3", "draft");
    raw.execute_batch(
        "DROP TRIGGER cat_method_report_notes_frozen;
         ALTER TABLE cat_method_versions DROP COLUMN report_notes;
         ALTER TABLE cat_method_version_analytes DROP COLUMN reference_interval;
         ALTER TABLE cat_method_version_analytes DROP COLUMN range_policy;",
    )
    .unwrap();
    let before: Vec<_> = TABLES.iter().map(|table| rows(&f.raw(), table)).collect();

    let upgraded = f.dir.path().join("upgraded");
    std::fs::create_dir(&upgraded).unwrap();
    f.db.snapshot(&upgraded.join(DATABASE_FILE)).unwrap();
    let mut target = MODULES;
    target[2] = ModuleSchema {
        migrations: &lis_catalog::MIGRATIONS[..5],
        ..lis_catalog::SCHEMA
    };
    drop(Database::open(&upgraded, &target, f.clock.clone()).unwrap());
    let conn = Connection::open(upgraded.join(DATABASE_FILE)).unwrap();

    let after: Vec<_> = TABLES.iter().map(|table| rows(&conn, table)).collect();
    assert_eq!(after, before);
    let states = strings(
        &conn,
        "SELECT state FROM ana_source_registrations ORDER BY state",
    );
    assert_eq!(states, ["active", "draft", "retired"]);
    let applied = strings(
        &conn,
        "SELECT module FROM store_migrations WHERE version = 5 AND name = 'self_approval' \
         ORDER BY module",
    );
    assert_eq!(applied, ["analytical", "catalog"]);
    let violations: i64 = conn
        .query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(violations, 0);
    assert_draft_only_guards(&conn, &f.urine.analytes[1].to_string());

    // The separation guard applies from now on.
    let message = conn
        .execute(ACTIVATE_BY_AUTHOR, [])
        .unwrap_err()
        .to_string();
    assert!(message.contains("separation of duties"), "{message}");
    let still_draft = strings(
        &conn,
        &format!("SELECT state FROM ana_source_registrations WHERE id = '{pending}'"),
    );
    assert_eq!(still_draft, ["draft"]);
}
