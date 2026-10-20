//! AU patient/QC semantics share the existing command-built clinical world.

use lis_analytical::{
    AuSourceProfile, AuTestDefinition, RegisterSourceInput, RevisionIndicator, SampleIdField,
};
use lis_au::{AuConfig, FormatConfig, FrameEnd, FramingConfig, TransmissionClass, encode_frame};
use lis_results::{QuarantineReason, QuarantineResolution, ResolveQuarantineItemInput};

use super::*;

#[path = "../support/au_receipt.rs"]
mod receipt;

fn protocol() -> AuConfig {
    AuConfig {
        framing: FramingConfig {
            start: vec![2],
            end: vec![3],
            bcc: true,
            use_etb: true,
            max_wire_len: 256,
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

pub(super) fn connection(protocol: AuConfig) -> ConnectionSettings {
    ConnectionSettings::AuTcp {
        bind_address: "127.0.0.1".parse().unwrap(),
        port: 62004,
        allowed_peers: vec!["127.0.0.1".parse().unwrap()],
        au: lis_interfaces::AuConnectionSettings {
            protocol,
            frame_timeout_ms: 2000,
            block_timeout_ms: 2000,
        },
    }
}

pub(super) fn register(f: &Fixture, protocol: AuConfig, unit: &'static str) {
    let codes = [
        ("01", f.analytes.mor),
        ("02", f.analytes.oxy),
        ("03", f.analytes.fen),
    ];
    let instrument_id = f.instrument;
    let draft = f.setup(move |ctx| {
        lis_analytical::register_source(
            ctx,
            RegisterSourceInput {
                instrument_id,
                sender_name: "Synthetic declared AU".to_owned(),
                sender_version: "1.0".to_owned(),
                sample_id_field: SampleIdField::SpecimenId,
                revision_indicator: RevisionIndicator::None,
                reports_final_concentration: true,
                test_codes: codes
                    .iter()
                    .map(|(code, analyte)| SourceTestCodeInput {
                        source_test_code: (*code).to_owned(),
                        analyte_id: *analyte,
                    })
                    .collect(),
                au_profile: Some(AuSourceProfile {
                    protocol,
                    tests: codes
                        .iter()
                        .map(|(code, _)| AuTestDefinition {
                            test_code: (*code).to_owned(),
                            unit: Some(unit.to_owned()),
                        })
                        .collect(),
                }),
            },
        )
    });
    f.setup(move |ctx| {
        lis_analytical::approve_source(
            ctx,
            ApproveSourceInput {
                registration_id: draft.registration_id,
                expected_revision: draft.revision,
                self_approval: false,
            },
        )
    });
}

fn wire(kind: &str, label: &str, code: &str, value: &str, flags: &str) -> Vec<u8> {
    let (number, reserved) = match kind {
        "DQ" => ("Q001", "  01"),
        "DH" => ("0001", "0001"),
        _ => ("0001", "    "),
    };
    let text = format!("{kind}000101U{number}{label:<20}{reserved}E{code}{value:>9}{flags}");
    encode_frame(&protocol(), text.as_bytes(), FrameEnd::Text).unwrap()
}

fn import(f: &Fixture, wire: &[u8]) -> ImportAttemptSummary {
    let profile = protocol();
    let id = receipt::seed(&f.t, f.instrument, &profile, wire);
    f.import_source(
        id,
        1,
        ImportSource::Au {
            extracted: lis_results::au::extract_artifact(wire, &profile),
            profile,
        },
    )
}

#[test]
fn approved_au_patient_quantity_keeps_raw_unit_absent_and_uses_declared_result_unit() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    let bytes = wire("D ", &s.label, "01", "12.50", "  ");
    let attempt = import(&f, &bytes);
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (1, 0));
    assert_eq!(
        f.measurements(&s, f.analytes.mor),
        [("quantity".to_owned(), Some("    12.50".to_owned()), None)]
    );
    let current = rows::current(&f.results(&s), "SYN-MOR");
    assert_eq!(current.magnitude.as_deref(), Some("12.50"));
    assert_eq!(current.units.as_deref(), Some(UNIT));
    assert_eq!(current.evidence[0].status_text, None);
    let replay = import(&f, &bytes);
    assert_eq!((replay.rows_new, replay.rows_duplicate), (0, 1));
}

#[test]
fn selecting_au_evidence_uses_original_declared_units_after_source_rotation() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    import(&f, &wire("D ", &s.label, "01", "12.50", "  "));
    let line = f
        .results(&s)
        .lines
        .into_iter()
        .find(|line| line.analyte_code == "SYN-MOR")
        .unwrap();
    let evidence = line.current.unwrap().evidence[0].measurement_id;
    register(&f, protocol(), "mg/L");
    let selected = f.setup(move |ctx| {
        lis_results::select_result_evidence(
            ctx,
            lis_results::SelectResultEvidenceInput {
                result_id: line.result_id,
                expected_revision: line.revision,
                measurement_ids: vec![evidence],
                reason: "Synthetic reviewed evidence".to_owned(),
            },
        )
    });
    assert_eq!(selected.magnitude.as_deref(), Some("12.50"));
    assert_eq!(selected.units.as_deref(), Some(UNIT));
    assert_eq!(selected.evidence[0].units_text, None);
}

#[test]
fn au_integer_is_numeric_only_when_approved_and_overflow_or_flags_stay_quarantined() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    let integer = import(&f, &wire("D ", &s.label, "01", "120", "  "));
    assert_eq!(integer.rows_new, 1);
    assert_eq!(
        rows::current(&f.results(&s), "SYN-MOR")
            .magnitude
            .as_deref(),
        Some("120")
    );
    for (code, value, flags, reason) in [
        ("02", "999999999", "  ", QuarantineReason::MalformedValue),
        ("02", "1.50", "F ", QuarantineReason::MalformedValue),
        ("99", "001002", "  ", QuarantineReason::UnknownAnalyte),
    ] {
        let attempt = import(&f, &wire("D ", &s.label, code, value, flags));
        assert_eq!(f.detail(&attempt).quarantine[0].reason, reason);
    }
    assert!(f.measurements(&s, f.analytes.oxy).is_empty());
}

#[test]
fn an_approved_au_declaration_with_a_different_method_unit_stays_quarantined() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    register(&f, protocol(), "mg/L");
    let attempt = import(&f, &wire("D ", &s.label, "01", "12.50", "  "));
    assert_eq!(
        f.detail(&attempt).quarantine[0].reason,
        QuarantineReason::IncompatibleUnit
    );
    assert!(f.measurements(&s, f.analytes.mor).is_empty());
}

#[test]
fn au_qc_classification_cannot_promote_a_patient_position() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    let attempt = import(&f, &wire("DQ", &s.label, "01", "12.50", "  "));
    assert_eq!(
        (attempt.rows_new, attempt.rows_qc, attempt.rows_quarantined),
        (0, 0, 1)
    );
    assert_eq!(
        f.detail(&attempt).quarantine[0].reason,
        QuarantineReason::RoleMismatch
    );
    let qc = import(&f, &wire("DQ", CONTROL, "01", "12.50", "  "));
    assert_eq!((qc.rows_new, qc.rows_qc), (0, 1));
}

#[test]
fn changed_au_rerun_is_a_conflict_and_explicit_reprocessing_keeps_au_value_rules() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    import(&f, &wire("D ", &s.label, "01", "12.50", "  "));
    let changed = import(&f, &wire("DH", &s.label, "01", "10.25", "  "));
    let detail = f.detail(&changed);
    assert_eq!(
        detail.quarantine[0].reason,
        QuarantineReason::ConflictingDuplicate
    );
    assert_eq!(
        rows::current(&f.results(&s), "SYN-MOR")
            .magnitude
            .as_deref(),
        Some("12.50")
    );
    f.setup(move |ctx| {
        lis_results::resolve_quarantine_item(
            ctx,
            ResolveQuarantineItemInput {
                item_id: detail.quarantine[0].item_id,
                expected_revision: detail.quarantine[0].revision,
                resolution: QuarantineResolution::AcceptAsReprocessing,
                reason: "Synthetic reviewed rerun".to_owned(),
            },
        )
    });
    let current = rows::current(&f.results(&s), "SYN-MOR");
    assert_eq!(current.magnitude.as_deref(), Some("10.25"));
    assert_eq!(current.units.as_deref(), Some(UNIT));
    assert_eq!(current.evidence[0].units_text, None);
}

#[test]
fn au_dilution_execution_requires_additional_supported_final_concentration_contract() {
    let f = fixture_with_au_profile(Some(protocol()));
    let s = f.sample();
    f.close_run(f.run);
    let run = f.open_run(false);
    let s = f.place(s, run, AttemptKind::Dilution, Some("10"));
    let attempt = import(&f, &wire("D ", &s.label, "01", "12.50", "  "));
    assert_eq!(
        f.detail(&attempt).quarantine[0].reason,
        QuarantineReason::DilutionNotSupported
    );
}
