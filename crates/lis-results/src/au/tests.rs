use lis_analytical::{AuTestDefinition, RevisionIndicator};
use lis_au::{FormatConfig, FrameEnd, FramingConfig, TransmissionClass, encode_frame};
use lis_catalog::AnalyteId;
use lis_core::DecimalText;
use proptest::prelude::*;

use super::*;

fn profile() -> AuConfig {
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
            sample_id_digits: 12,
            sex: false,
            age: false,
            month: false,
            patient_info_digits: [0; 6],
            result_digits: 6,
        },
    }
}

fn wire(kind: &str, sample: &str, value: &str, flags: &str, block: &str) -> Vec<u8> {
    let number = match kind {
        "DQ" => "Q001",
        "DA" => "A001",
        "DR" => "R001",
        _ => "0001",
    };
    let reserved = match kind {
        "DH" | "dH" => "0001",
        "DQ" | "DA" => "  01",
        _ => "    ",
    };
    let sample = if kind == "DR" { "" } else { sample };
    let text = format!("{kind}000101U{number}{sample:<12}{reserved}{block}01{value}{flags}");
    encode_frame(
        &profile(),
        text.as_bytes(),
        if block == "E" {
            FrameEnd::Text
        } else {
            FrameEnd::Block
        },
    )
    .unwrap()
}

fn row(value: &str, flags: &str) -> ExtractedRow {
    extract_artifact(&wire("D ", "SYN-SAMPLE", value, flags, "E"), &profile())
        .unwrap()
        .rows
        .remove(0)
}

fn declared() -> AuSourceProfile {
    AuSourceProfile {
        protocol: profile(),
        tests: vec![AuTestDefinition {
            test_code: "01".to_owned(),
            unit: Some("ng/mL".to_owned()),
        }],
    }
}

fn analyte() -> MethodAnalyte {
    MethodAnalyte {
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
        analyte_id: AnalyteId::generate(),
        unit: Some("ng/mL".to_owned()),
        cutoff: None,
        limit_of_detection: None,
        limit_of_quantitation: None,
        reportable_low: None,
        reportable_high: None,
        display_scale: None,
    }
}

#[test]
fn raw_source_fields_stay_exact_and_missing_fields_stay_absent() {
    let extracted =
        extract_artifact(&wire("DH", "SYN-SAMPLE", " 12.50", "  ", "E"), &profile()).unwrap();
    assert_eq!(
        (
            extracted.sender_name,
            extracted.sender_version,
            extracted.message_time_text
        ),
        (None, None, None)
    );
    let received = &extracted.rows[0];
    assert_eq!(received.sample_label.as_deref(), Some("SYN-SAMPLE  "));
    assert_eq!(received.value_text.as_deref(), Some(" 12.50"));
    assert_eq!(received.flags_text.as_deref(), Some("  "));
    assert_eq!(received.sample_type_text.as_deref(), Some("DH"));
    assert_eq!(
        (
            received.units_text.as_deref(),
            received.status_text.as_deref()
        ),
        (None, None)
    );
    let incoming = lis_analytical::reexport::SourceRow {
        value_text: received.value_text.clone(),
        units_text: None,
        flags_text: received.flags_text.clone(),
        status_text: None,
    };
    let previous = lis_analytical::reexport::SourceRow {
        value_text: Some(" 20.00".to_owned()),
        ..incoming.clone()
    };
    assert_eq!(
        lis_analytical::reexport::decide(Some(&previous), &incoming, RevisionIndicator::None),
        lis_analytical::reexport::RowDecision::Conflict
    );
}

#[test]
fn every_au_classification_must_match_the_run_position_role() {
    for (kind, role) in [
        ("D ", PositionRole::Patient),
        ("DH", PositionRole::Patient),
        ("d ", PositionRole::Patient),
        ("dH", PositionRole::Patient),
        ("DQ", PositionRole::Control),
        ("DA", PositionRole::Calibrator),
        ("DR", PositionRole::Blank),
    ] {
        let received = extract_artifact(&wire(kind, "SYN-SAMPLE", " 12.50", "  ", "E"), &profile())
            .unwrap()
            .rows
            .remove(0);
        for candidate in [
            PositionRole::Patient,
            PositionRole::Control,
            PositionRole::Calibrator,
            PositionRole::Blank,
        ] {
            assert_eq!(role_mismatch(&received, candidate), candidate != role);
        }
    }
}

#[test]
fn declared_numeric_values_preserve_raw_padding_and_decimal_precision() {
    for (raw, expected) in [
        (" 12.50", "12.50"),
        ("000120", "000120"),
        ("- 1.50", "-1.50"),
    ] {
        let received = row(raw, "  ");
        let parsed = patient_value(
            &received,
            OutputScale::Quantitative,
            &analyte(),
            &declared(),
        )
        .unwrap();
        assert_eq!(
            parsed.value,
            Some(crate::SourceValue::Quantity(
                DecimalText::parse(expected).unwrap()
            ))
        );
        assert_eq!(parsed.units.as_deref(), Some("ng/mL"));
        assert_eq!(received.value_text.as_deref(), Some(raw));
        assert_eq!(received.units_text, None);
    }
}

#[test]
fn overflow_flags_unmapped_lih_and_unapproved_units_cannot_become_patient_values() {
    for (raw, flags) in [
        ("999999", "  "),
        (" 12.50", "F "),
        (" 12.50", " N"),
        ("      ", "  "),
        ("< 1.50", "  "),
        (" <1.50", "  "),
    ] {
        assert_eq!(
            patient_value(
                &row(raw, flags),
                OutputScale::Quantitative,
                &analyte(),
                &declared()
            ),
            Err(QuarantineReason::MalformedValue)
        );
    }
    let mut lih = row("001002", "  ");
    lih.test_code = Some("99".to_owned());
    assert_eq!(
        patient_value(&lih, OutputScale::Quantitative, &analyte(), &declared()),
        Err(QuarantineReason::UnknownAnalyte)
    );
    let mut wrong = analyte();
    wrong.unit = Some("mg/L".to_owned());
    assert_eq!(
        patient_value(
            &row(" 12.50", "  "),
            OutputScale::Quantitative,
            &wrong,
            &declared()
        ),
        Err(QuarantineReason::IncompatibleUnit)
    );
    assert_eq!(
        patient_value(
            &row(" 12.50", "  "),
            OutputScale::Qualitative,
            &analyte(),
            &declared()
        ),
        Err(QuarantineReason::ScaleMismatch)
    );
}

#[test]
fn block_assembly_requires_final_block_and_rejects_wire_corruption() {
    let first = wire("D ", "SYN-SAMPLE", " 12.50", "  ", "0");
    assert!(extract_artifact(&first, &profile()).is_err());
    let mut complete = first.clone();
    complete.extend(first);
    complete.extend(wire("D ", "SYN-SAMPLE", " 20.00", "  ", "E"));
    let extracted = extract_artifact(&complete, &profile()).unwrap();
    assert_eq!(extracted.rows.len(), 2);
    complete.pop();
    assert!(extract_artifact(&complete, &profile()).is_err());
    assert_eq!(
        extract_artifact(&[6], &profile()),
        Err(AuExtractFailure::UnexpectedMessage)
    );
}

proptest! {
    #[test]
    fn arbitrary_wire_bytes_cannot_panic(bytes in prop::collection::vec(any::<u8>(), 0..2048)) {
        let _ = extract_artifact(&bytes, &profile());
    }
}
