//! Instrument readiness, source registrations and lots (REQ-ANA-003
//! registration part, REQ-ANA-007, REQ-OPS-001).

use lis_analytical::{
    AnalyticalError, ApproveSourceInput, InstrumentStatus, LotState, MaterialKind,
    RegisterLotInput, RegisterSourceInput, RegistrationState, RevisionIndicator, SampleIdField,
    SetInstrumentStatusInput, SourceTestCodeInput, WithdrawLotInput,
};
use lis_core::{Capability, ErrorCode, InstrumentId, Principal, SessionId, UserId, WorkstationId};

use crate::{STAFF, code_of, counts, fixture, last_event, read, run, run_as};

fn source_input(f: &crate::Fixture, codes: &[(&str, usize)]) -> RegisterSourceInput {
    RegisterSourceInput {
        au_profile: None,
        instrument_id: f.instrument,
        sender_name: "SYNTHETIC".to_owned(),
        sender_version: "2.0".to_owned(),
        sample_id_field: SampleIdField::InstrumentSpecimenId,
        revision_indicator: RevisionIndicator::None,
        reports_final_concentration: false,
        test_codes: codes
            .iter()
            .map(|(code, analyte)| SourceTestCodeInput {
                source_test_code: (*code).to_owned(),
                analyte_id: f.urine.analytes[*analyte],
            })
            .collect(),
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

#[test]
fn an_instrument_without_a_status_event_is_not_ready() {
    let f = fixture();
    let listed = read(&f, lis_analytical::list_instrument_readiness);
    assert_eq!(listed.len(), 1);
    assert_eq!((listed[0].status, listed[0].since), (None, None));
    assert_eq!(listed[0].active_registration, None);
}

#[test]
fn status_events_are_recorded_and_audited_with_their_reason() {
    let f = fixture();
    f.set_status(f.instrument, InstrumentStatus::OutOfService);
    f.set_status(f.instrument, InstrumentStatus::InService);
    let listed = read(&f, lis_analytical::list_instrument_readiness);
    assert_eq!(listed[0].status, Some(InstrumentStatus::InService));
    let (action, record, reason) = last_event(&f);
    assert_eq!(action, "analytical.set_instrument_status");
    assert_eq!(record, Some(f.instrument.to_string()));
    assert_eq!(reason.as_deref(), Some("Synthetic maintenance log"));
    // Events are append-only.
    let raw = f.raw();
    assert!(
        raw.execute("DELETE FROM ana_instrument_status_events", [])
            .is_err()
    );
    assert!(
        raw.execute(
            "UPDATE ana_instrument_status_events SET status = 'in_service'",
            []
        )
        .is_err()
    );
}

#[test]
fn a_status_needs_a_known_instrument_and_a_reason() {
    let f = fixture();
    let unknown = run(&f, STAFF, |ctx| {
        lis_analytical::set_instrument_status(
            ctx,
            SetInstrumentStatusInput {
                instrument_id: InstrumentId::generate(),
                status: InstrumentStatus::InService,
                reason: "Synthetic".to_owned(),
            },
        )
    });
    assert_eq!(code_of(unknown), ErrorCode::NotFound);
    let instrument_id = f.instrument;
    let blank = run(&f, STAFF, move |ctx| {
        lis_analytical::set_instrument_status(
            ctx,
            SetInstrumentStatusInput {
                instrument_id,
                status: InstrumentStatus::InService,
                reason: "  ".to_owned(),
            },
        )
    });
    assert_eq!(code_of(blank), ErrorCode::Validation);
}

#[test]
fn registration_test_codes_are_validated() {
    let f = fixture();
    let before = counts(&f);
    for (codes, expected) in [
        (vec![], ErrorCode::Validation),
        (vec![("A1", 0), ("A1", 1)], ErrorCode::Validation),
        (vec![(" A1", 0)], ErrorCode::Validation),
    ] {
        let input = source_input(&f, &codes);
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::register_source(ctx, input)
        });
        assert_eq!(code_of(result), expected, "{codes:?}");
    }
    let mut unknown = source_input(&f, &[("A1", 0)]);
    unknown.test_codes[0].analyte_id = lis_catalog::AnalyteId::generate();
    let result = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, unknown)
    });
    assert_eq!(code_of(result), ErrorCode::NotFound);
    assert_eq!(counts(&f), before);
}

#[test]
fn approval_needs_another_signed_in_user_and_retires_the_previous_registration() {
    let f = fixture();
    let author = UserId::generate();
    let input = source_input(&f, &[("A1", 0)]);
    let first = run_as(&f, user(author), move |ctx| {
        lis_analytical::register_source(ctx, input)
    })
    .unwrap();
    assert_eq!(first.state, RegistrationState::Draft);
    assert_eq!(first.allowed_actions, vec!["approve_source".to_owned()]);
    let approve = |id, revision| ApproveSourceInput {
        registration_id: id,
        expected_revision: revision,
        self_approval: false,
    };
    let own = approve(first.registration_id, 1);
    let same_author = run_as(&f, user(author), move |ctx| {
        lis_analytical::approve_source(ctx, own)
    });
    assert!(matches!(same_author, Err(AnalyticalError::SameAuthor)));
    let system = Principal::system("test-worker", [Capability::ApproveConfiguration]);
    let by_system = approve(first.registration_id, 1);
    let refused = run_as(&f, system, move |ctx| {
        lis_analytical::approve_source(ctx, by_system)
    });
    assert!(matches!(refused, Err(AnalyticalError::UserRequired)));
    let stale = approve(first.registration_id, 7);
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::approve_source(
            ctx, stale
        ))),
        ErrorCode::Conflict
    );

    let ok = approve(first.registration_id, 1);
    let active = run(&f, STAFF, move |ctx| {
        lis_analytical::approve_source(ctx, ok)
    })
    .unwrap();
    assert_eq!(
        (active.state, active.revision),
        (RegistrationState::Active, 2)
    );
    let input = source_input(&f, &[("A2", 0), ("B2", 1)]);
    let second = run(&f, STAFF, move |ctx| {
        lis_analytical::register_source(ctx, input)
    })
    .unwrap();
    let ok = approve(second.registration_id, 1);
    run(&f, STAFF, move |ctx| {
        lis_analytical::approve_source(ctx, ok)
    })
    .unwrap();

    let instrument = f.instrument;
    let all = read(&f, move |conn| {
        lis_analytical::list_source_registrations(conn, instrument)
    });
    let states: Vec<RegistrationState> = all.iter().map(|r| r.state).collect();
    assert_eq!(
        states,
        [RegistrationState::Active, RegistrationState::Retired]
    );
    let current = read(&f, move |conn| {
        lis_analytical::active_registration(conn, instrument)
    })
    .unwrap();
    assert_eq!(current.registration_id, second.registration_id);
    assert_eq!(current.test_codes.len(), 2);
    let again = approve(first.registration_id, 3);
    let retired = run(&f, STAFF, move |ctx| {
        lis_analytical::approve_source(ctx, again)
    });
    assert_eq!(code_of(retired), ErrorCode::Conflict);
}

#[test]
fn an_approved_registration_and_its_test_codes_are_immutable() {
    let f = fixture();
    f.register_source(RevisionIndicator::None, false);
    let raw = f.raw();
    for sql in [
        "UPDATE ana_source_registrations SET sender_version = '9.9'",
        "UPDATE ana_source_registrations SET revision_indicator = 'result_status_correction'",
        "UPDATE ana_source_registrations SET state = 'draft', revision = revision + 1",
        "DELETE FROM ana_source_registrations",
        "UPDATE ana_source_test_codes SET source_test_code = 'Z9'",
        "DELETE FROM ana_source_test_codes",
    ] {
        assert!(raw.execute(sql, []).is_err(), "{sql}");
    }
    let analyte = f.urine.analytes[0].to_string();
    let added = raw.execute(
        "INSERT INTO ana_source_test_codes (registration_id, source_test_code, analyte_id) \
         SELECT id, 'Z9', ?1 FROM ana_source_registrations",
        [analyte],
    );
    assert!(added.is_err());
}

fn lot_input(kind: MaterialKind, label: Option<&str>, expires_on: &str) -> RegisterLotInput {
    RegisterLotInput {
        material_kind: kind,
        product_name: "Synthetic control".to_owned(),
        lot_number: "L-100".to_owned(),
        level_text: Some("Level 1".to_owned()),
        sample_label: label.map(str::to_owned),
        expires_on: expires_on.to_owned(),
    }
}

#[test]
fn lots_validate_their_sample_label_date_and_uniqueness() {
    let f = fixture();
    for input in [
        lot_input(MaterialKind::Control, None, crate::LATER),
        lot_input(MaterialKind::Reagent, Some("QC-1"), crate::LATER),
        lot_input(MaterialKind::Control, Some("QC-1"), "2027-02-30"),
        lot_input(MaterialKind::Control, Some("QC-1"), "31/12/2027"),
    ] {
        let result = run(&f, STAFF, move |ctx| {
            lis_analytical::register_lot(ctx, input)
        });
        assert_eq!(code_of(result), ErrorCode::Validation);
    }
    let input = lot_input(MaterialKind::Control, Some("QC-1"), crate::LATER);
    let lot = run(&f, STAFF, move |ctx| {
        lis_analytical::register_lot(ctx, input)
    })
    .unwrap();
    assert_eq!(lot.sample_label.as_deref(), Some("QC-1"));
    assert!(!lot.expired);
    assert_eq!(last_event(&f).0, "analytical.register_lot");
    let again = lot_input(MaterialKind::Control, Some("QC-2"), crate::LATER);
    let duplicate = run(&f, STAFF, move |ctx| {
        lis_analytical::register_lot(ctx, again)
    });
    assert_eq!(code_of(duplicate), ErrorCode::Conflict);
}

#[test]
fn expiry_is_derived_and_withdrawal_needs_a_reason_once() {
    let f = fixture();
    let expired = f.lot(MaterialKind::Reagent, "R-OLD", "2026-01-01");
    let current = f.lot(MaterialKind::Reagent, "R-NEW", crate::LATER);
    let now = lis_store::testing::test_start();
    let lots = read(&f, move |conn| lis_analytical::list_lots(conn, false, now));
    let by_id = |id| lots.iter().find(|lot| lot.lot_id == id).unwrap().expired;
    assert!(by_id(expired));
    assert!(!by_id(current));

    let withdraw = |revision, reason: &str| WithdrawLotInput {
        lot_id: current,
        expected_revision: revision,
        reason: reason.to_owned(),
    };
    let blank = withdraw(1, " ");
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::withdraw_lot(
            ctx, blank
        ))),
        ErrorCode::Validation
    );
    let stale = withdraw(5, "Synthetic recall");
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::withdraw_lot(
            ctx, stale
        ))),
        ErrorCode::Conflict
    );
    let ok = withdraw(1, "Synthetic recall");
    let withdrawn = run(&f, STAFF, move |ctx| lis_analytical::withdraw_lot(ctx, ok)).unwrap();
    assert_eq!(
        (withdrawn.state, withdrawn.revision),
        (LotState::Withdrawn, 2)
    );
    assert_eq!(last_event(&f).2.as_deref(), Some("Synthetic recall"));
    let twice = withdraw(2, "Synthetic recall");
    assert_eq!(
        code_of(run(&f, STAFF, move |ctx| lis_analytical::withdraw_lot(
            ctx, twice
        ))),
        ErrorCode::Conflict
    );
    let active = read(&f, move |conn| lis_analytical::list_lots(conn, true, now));
    assert!(active.iter().all(|lot| lot.lot_id != current));
    assert!(
        f.raw()
            .execute("UPDATE ana_lots SET expires_on = '2099-01-01'", [])
            .is_err()
    );
}
