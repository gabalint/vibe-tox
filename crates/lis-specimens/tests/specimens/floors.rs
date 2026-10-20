//! Number floors (REQ-OPS-002; M4 design brief §3.8): a floor only goes up, has the
//! sequence's format, and the next numbers jump above it. All numbers are synthetic.

use lis_core::{Capability, ErrorCode, Principal};
use lis_specimens::{
    NumberFloors, RaiseNumberFloorsInput, SpecimenError, number_floors, raise_number_floors,
    raise_number_floors_after_restore,
};
use lis_store::testing::test_start;

use crate::{counts, fixture, last_event, read, run, run_as};

fn floors(
    accession: Option<&str>,
    container: Option<&str>,
    reason: &str,
) -> RaiseNumberFloorsInput {
    RaiseNumberFloorsInput {
        accession_floor: accession.map(str::to_owned),
        container_floor: container.map(str::to_owned),
        reason: reason.to_owned(),
    }
}

fn raise(f: &crate::Fixture, input: RaiseNumberFloorsInput) -> Result<NumberFloors, SpecimenError> {
    run(&f.t, &[Capability::ManageQuality], move |ctx| {
        raise_number_floors(ctx, input)
    })
}

fn restore_principal(capabilities: &[Capability]) -> Principal {
    Principal::system("restore", capabilities.iter().copied())
}

#[test]
fn raised_floors_make_the_next_numbers_jump() {
    let f = fixture();
    let first = f.accession(1);
    assert_eq!(first.accession_number, "A26000001");
    let raised = raise(
        &f,
        floors(
            Some("A26000500"),
            Some("C00001000"),
            "Labels up to A26000500 found",
        ),
    )
    .unwrap();
    assert_eq!(
        raised.accession.highest_issued.as_deref(),
        Some("A26000001")
    );
    assert_eq!(raised.accession.floor.as_deref(), Some("A26000500"));
    assert_eq!(raised.accession.next, "A26000501");
    assert_eq!(raised.container.next, "C00001001");
    let (action, _, reason) = last_event(&f.t);
    assert_eq!(action, "specimen.raise_number_floor");
    assert_eq!(reason.as_deref(), Some("Labels up to A26000500 found"));

    let next = f.accession(2);
    assert_eq!(next.accession_number, "A26000501");
    let codes: Vec<&str> = next.specimens[0]
        .containers
        .iter()
        .map(|container| container.label_code.as_str())
        .collect();
    assert_eq!(codes, ["C00001001", "C00001002"]);
    let now = read(&f.t, |conn| number_floors(conn, test_start()));
    assert_eq!(now.accession.next, "A26000502");
    assert_eq!(now.container.highest_issued.as_deref(), Some("C00001002"));
}

#[test]
fn a_floor_only_goes_up_and_needs_the_format_and_a_reason() {
    let f = fixture();
    f.accession(1);
    raise(&f, floors(Some("A26000500"), None, "Synthetic")).unwrap();
    let before = counts(&f.t);
    let refused = [
        floors(Some("A26000500"), None, "Same floor"),
        floors(Some("A26000100"), None, "Lower floor"),
        floors(None, Some("C00000001"), "At the highest issued code"),
        floors(Some("A2600050"), None, "Too short"),
        floors(Some("X26000600"), None, "Wrong letter"),
        floors(Some("A26999999"), None, "No number left above it"),
        floors(None, Some("C0000100x"), "Not digits"),
        floors(None, None, "Nothing to raise"),
        floors(Some("A26000600"), None, " "),
    ];
    for input in refused {
        let error = raise(&f, input).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Validation, "{error:?}");
    }
    assert_eq!(counts(&f.t), before, "a refused floor writes nothing");
}

#[test]
fn a_restore_skips_a_margin_above_the_highest_issued_numbers() {
    let f = fixture();
    let restored = f.accession(2);
    assert_eq!(restored.accession_number, "A26000001");
    let raised = run_as(
        &f.t,
        restore_principal(&[Capability::ManageQuality]),
        |ctx| raise_number_floors_after_restore(ctx, 1_000, 5_000),
    )
    .unwrap();
    assert_eq!(raised.accession.floor.as_deref(), Some("A26001001"));
    assert_eq!(raised.container.floor.as_deref(), Some("C00005002"));
    assert_eq!(last_event(&f.t).0, "specimen.raise_number_floor");

    let next = f.accession(1);
    assert_eq!(next.accession_number, "A26001002");
    assert_eq!(next.specimens[0].containers[0].label_code, "C00005003");
}

#[test]
fn only_a_system_principal_with_manage_quality_raises_floors_after_a_restore() {
    let f = fixture();
    let before = counts(&f.t);
    let user = run(&f.t, &[Capability::ManageQuality], |ctx| {
        raise_number_floors_after_restore(ctx, 1_000, 5_000)
    });
    assert_eq!(user.unwrap_err().code(), ErrorCode::Forbidden);
    let without = run_as(
        &f.t,
        restore_principal(&[Capability::BackupRestore]),
        |ctx| raise_number_floors_after_restore(ctx, 1_000, 5_000),
    );
    assert_eq!(without.unwrap_err().code(), ErrorCode::Forbidden);
    assert_eq!(counts(&f.t), before);
}

#[test]
fn floors_are_append_only_and_the_table_refuses_a_lower_floor() {
    let f = fixture();
    raise(&f, floors(Some("A26000500"), None, "Synthetic")).unwrap();
    let attempts = [
        "INSERT INTO spc_number_floors VALUES ('x', 'accession', 'A26000400', 'manual', 'u', 't')",
        "UPDATE spc_number_floors SET floor_number = 'A26000900'",
        "DELETE FROM spc_number_floors",
    ];
    for sql in attempts {
        let result = run(&f.t, &[], move |ctx| Ok(ctx.tx().execute(sql, [])?));
        assert!(result.is_err(), "{sql}");
    }
}
