//! The synthetic M2 ASTM fixtures extract to the rows written in
//! fixtures/astm/m2/README.md (REQ-ANA-003, NFR-10).

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_astm::encode_records;
use lis_results::astm::{ExtractedImport, SampleIdField, SourceProfile, extract_artifact};

const PROFILE: SourceProfile = SourceProfile {
    sample_id_field: SampleIdField::SpecimenId,
};

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/astm/m2/",
            $name
        ))
    };
}

const JOURNEY: &[u8] = fixture!("journey.astm");
const JOURNEY_DUPLICATE: &[u8] = fixture!("journey_duplicate.astm");
const CORRECTION: &[u8] = fixture!("correction.astm");
const PARTIAL: &[u8] = fixture!("partial.astm");
const UNKNOWN_SAMPLE: &[u8] = fixture!("unknown_sample.astm");
const UNIT_MISMATCH: &[u8] = fixture!("unit_mismatch.astm");
const MALFORMED_VALUE: &[u8] = fixture!("malformed_value.astm");

/// (sample label, test code, value, units, status) per row.
type Row<'a> = (&'a str, &'a str, &'a str, &'a str, &'a str);

fn rows(import: &ExtractedImport) -> Vec<Row<'_>> {
    import
        .rows
        .iter()
        .map(|row| {
            (
                row.sample_label.as_deref().unwrap(),
                row.test_code.as_deref().unwrap(),
                row.value_text.as_deref().unwrap(),
                row.units_text.as_deref().unwrap(),
                row.status_text.as_deref().unwrap(),
            )
        })
        .collect()
}

fn extract(bytes: &[u8], time: &str) -> ExtractedImport {
    let import = extract_artifact(bytes, &PROFILE).unwrap();
    assert_eq!(import.sender_name.as_deref(), Some("SYNTH-LCMS"));
    assert_eq!(import.sender_version.as_deref(), Some("1.0"));
    assert_eq!(import.message_time_text.as_deref(), Some(time));
    import
}

#[test]
fn journey_has_qc_and_patient_rows_in_source_order() {
    let import = extract(JOURNEY, "20261012093000");
    assert_eq!(
        rows(&import),
        [
            ("QC-LOW-SYN", "SYN-MOR", "37.6", "ng/mL", "F"),
            ("QC-LOW-SYN", "SYN-OXY", "38.1", "ng/mL", "F"),
            ("C00000001", "SYN-MOR", "152.40", "ng/mL", "F"),
            ("C00000001", "SYN-OXY", "<5.0", "ng/mL", "F"),
            ("C00000002", "SYN-MOR", "50.0", "ng/mL", "F"),
            ("C00000002", "SYN-OXY", "49.9", "ng/mL", "F"),
            ("C00000003", "SYN-MOR", ">1000", "ng/mL", "F"),
            ("C00000003", "SYN-OXY", "0.00", "ng/mL", "F"),
            ("QC-HIGH-SYN", "SYN-MOR", "187.9", "ng/mL", "F"),
            ("QC-HIGH-SYN", "SYN-OXY", "191.2", "ng/mL", "F"),
        ]
    );
    assert_eq!(import.rows[2].record_path, "P2/O1/R1");
    assert_eq!(import.rows[2].flags_text.as_deref(), Some("H"));
    assert_eq!(import.rows[2].operator_text.as_deref(), Some("SYN-TECH-01"));
    assert_eq!(
        import.rows[2].completed_text.as_deref(),
        Some("20261012091500")
    );
}

#[test]
fn duplicate_is_byte_identical_to_the_journey() {
    assert_eq!(JOURNEY_DUPLICATE, JOURNEY);
}

#[test]
fn correction_re_exports_one_changed_row_with_status_c() {
    let import = extract(CORRECTION, "20261012110000");
    assert_eq!(
        rows(&import),
        [("C00000001", "SYN-MOR", "148.90", "ng/mL", "C")]
    );
}

#[test]
fn partial_re_export_repeats_only_one_sample_unchanged() {
    let import = extract(PARTIAL, "20261012113000");
    assert_eq!(
        rows(&import),
        [
            ("C00000002", "SYN-MOR", "50.0", "ng/mL", "F"),
            ("C00000002", "SYN-OXY", "49.9", "ng/mL", "F"),
        ]
    );
}

#[test]
fn unknown_sample_unit_mismatch_and_malformed_value_are_extracted_as_received() {
    let unknown = extract(UNKNOWN_SAMPLE, "20261012120000");
    assert_eq!(
        rows(&unknown),
        [("C99999999", "SYN-MOR", "75.0", "ng/mL", "F")]
    );
    let units = extract(UNIT_MISMATCH, "20261012123000");
    assert_eq!(
        rows(&units),
        [
            ("C00000004", "SYN-MOR", "88.0", "ug/L", "F"),
            ("C00000004", "SYN-OXY", "12.0", "ng/mL", "F"),
        ]
    );
    let malformed = extract(MALFORMED_VALUE, "20261012130000");
    assert_eq!(
        rows(&malformed),
        [
            ("C00000005", "SYN-MOR", "< 5.0", "ng/mL", "F"),
            ("C00000005", "SYN-OXY", "8.4", "ng/mL", "F"),
        ]
    );
}

#[test]
fn every_fixture_splits_into_records_that_frame_for_the_wire() {
    for bytes in [
        JOURNEY,
        CORRECTION,
        PARTIAL,
        UNKNOWN_SAMPLE,
        UNIT_MISMATCH,
        MALFORMED_VALUE,
    ] {
        let records: Vec<Vec<u8>> = bytes
            .strip_suffix(b"\r")
            .unwrap()
            .split(|byte| *byte == b'\r')
            .map(<[u8]>::to_vec)
            .collect();
        assert!(!encode_records(&records).unwrap().is_empty());
    }
}
