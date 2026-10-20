//! The laboratory profile (REQ-RPT-001, REQ-RPT-008, REQ-BIL-005; L15).

use lis_core::{Capability, ErrorCode, Principal};
use lis_identity::{
    IdentityError, LaboratoryProfileView, SetLaboratoryProfile, current_laboratory_profile,
    set_laboratory_profile,
};
use lis_store::testing::TestDatabase;

use crate::{assert_code, audit_actions, counts, open, read, run, run_as};

const CONFIGURE: &[Capability] = &[Capability::ManageConfiguration];

pub(crate) fn profile_input(expected_version: Option<i64>) -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version,
        name: "Synthetic Toxicology Laboratory".to_owned(),
        address_lines: vec![
            "100 Synthetic Way".to_owned(),
            "Testville, ZZ 00000".to_owned(),
        ],
        clia_id: "00D0000000".to_owned(),
        director: "Director Synthetic, PhD".to_owned(),
        phone: "555-0100".to_owned(),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}

fn set(
    t: &TestDatabase,
    input: SetLaboratoryProfile,
) -> Result<LaboratoryProfileView, IdentityError> {
    run(t, CONFIGURE, move |ctx| set_laboratory_profile(ctx, input))
}

#[test]
fn the_first_profile_is_version_one_without_a_reason() {
    let t = open();
    assert_eq!(read(&t, current_laboratory_profile), None);
    let set = set(&t, profile_input(None)).unwrap();
    assert_eq!(set.version, 1);
    assert_eq!(set.clia_id, "00D0000000");
    assert_eq!(set.address_lines.len(), 2);
    assert_eq!(set.reason, None);
    assert_eq!(read(&t, current_laboratory_profile), Some(set));
    assert_eq!(audit_actions(&t), ["identity.set_laboratory_profile"]);
}

#[test]
fn a_change_is_a_new_version_with_a_reason_and_keeps_the_old_one() {
    let t = open();
    let first = set(&t, profile_input(None)).unwrap();
    let mut change = profile_input(Some(1));
    change.time_zone = "America/New_York".to_owned();
    change.billing_npi = Some("1234567893".to_owned());
    change.tax_id = Some("12-3456789".to_owned());
    assert_code(set(&t, change.clone()), ErrorCode::Validation);
    change.reason = Some("Synthetic: moved east".to_owned());
    let second = set(&t, change).unwrap();
    assert_eq!(second.version, 2);
    assert_eq!(second.tax_id.as_deref(), Some("123456789"));
    assert_eq!(second.billing_npi.as_deref(), Some("1234567893"));
    assert_eq!(read(&t, current_laboratory_profile), Some(second));
    let old_id = first.profile_id;
    assert_eq!(
        read(&t, move |conn| lis_identity::get_laboratory_profile_by_id(
            conn, old_id
        )),
        first
    );
    let versions: Vec<(i64, String)> = read(&t, |conn| {
        let mut statement = conn
            .prepare("SELECT version, time_zone FROM idn_laboratory_profiles ORDER BY version")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    });
    assert_eq!(
        versions,
        [(1, first.time_zone), (2, "America/New_York".to_owned())]
    );
    let reason: Option<String> = read(&t, |conn| {
        Ok(conn.query_row(
            "SELECT reason FROM audit_events WHERE action = 'identity.set_laboratory_profile' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?)
    });
    assert_eq!(reason.as_deref(), Some("Synthetic: moved east"));
}

#[test]
fn a_stale_expected_version_is_a_conflict_and_writes_nothing() {
    let t = open();
    set(&t, profile_input(None)).unwrap();
    let before = counts(&t);
    assert_code(set(&t, profile_input(None)), ErrorCode::Conflict);
    let mut ahead = profile_input(Some(2));
    ahead.reason = Some("Synthetic".to_owned());
    assert_code(set(&t, ahead), ErrorCode::Conflict);
    assert_eq!(counts(&t), before);
}

#[test]
fn invalid_fields_are_refused_by_name() {
    let t = open();
    type Change = fn(&mut SetLaboratoryProfile);
    let cases: [(&str, Change); 5] = [
        ("timeZone", |input| input.time_zone = "Central".to_owned()),
        ("cliaId", |input| input.clia_id = "12345".to_owned()),
        ("addressLines", |input| input.address_lines.clear()),
        ("billingNpi", |input| {
            input.billing_npi = Some("1234567890".to_owned());
        }),
        ("taxId", |input| input.tax_id = Some("12345".to_owned())),
    ];
    for (field, change) in cases {
        let mut input = profile_input(None);
        change(&mut input);
        match set(&t, input) {
            Err(IdentityError::Invalid(error)) => assert_eq!(error.field, field),
            other => panic!("{field}: {other:?}"),
        }
    }
    assert_eq!(read(&t, current_laboratory_profile), None);
}

#[test]
fn only_a_signed_in_user_with_manage_configuration_sets_the_profile() {
    let t = open();
    let before = counts(&t);
    let system = run_as(
        &t,
        Principal::system("importer", [Capability::ManageConfiguration]),
        |ctx| set_laboratory_profile(ctx, profile_input(None)),
    );
    assert!(matches!(system, Err(IdentityError::UserRequired)));
    let without = run(&t, &[Capability::ManageUsers], |ctx| {
        set_laboratory_profile(ctx, profile_input(None))
    });
    assert_code(without, ErrorCode::Forbidden);
    assert_eq!(counts(&t), before);
}

#[test]
fn profile_versions_cannot_be_edited_deleted_or_skipped() {
    let t = open();
    set(&t, profile_input(None)).unwrap();
    let raw = rusqlite::Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    for statement in [
        "UPDATE idn_laboratory_profiles SET name = 'Other'",
        "DELETE FROM idn_laboratory_profiles",
        "INSERT INTO idn_laboratory_profiles (id, version, name, address_line_1, clia_id, \
         director_name, phone, time_zone, reason, set_by, set_at) SELECT 'x', 3, name, \
         address_line_1, clia_id, director_name, phone, time_zone, 'skip', set_by, set_at \
         FROM idn_laboratory_profiles",
    ] {
        assert!(raw.execute(statement, []).is_err(), "{statement}");
    }
}
