use proptest::prelude::*;

use super::*;

const SPECIMEN_ID: SourceProfile = SourceProfile {
    sample_id_field: SampleIdField::SpecimenId,
};
const INSTRUMENT_ID: SourceProfile = SourceProfile {
    sample_id_field: SampleIdField::InstrumentSpecimenId,
};

fn artifact(records: &[&str]) -> Vec<u8> {
    records
        .iter()
        .flat_map(|r| format!("{r}\r").into_bytes())
        .collect()
}

#[test]
fn rows_keep_every_text_exactly_and_take_the_label_the_profile_names() {
    let bytes = artifact(&[
        "H|\\^&|||SYNTH-LCMS^1.0|||||||P|LIS2-A2|20261012093000",
        "P|1",
        "O|1|C00000001|SYN-POS-03||R",
        "R|1|^^^SYN-MOR| 152.40|ng/mL||H||F||SYN-TECH-01||20261012092500|SYNTH-LCMS-01",
        "R|2|^^^SYN-OXY|<5.0|ng/mL||||F",
        "L|1|N",
    ]);
    let import = extract_artifact(&bytes, &SPECIMEN_ID).unwrap();
    assert_eq!(import.sender_name.as_deref(), Some("SYNTH-LCMS"));
    assert_eq!(import.sender_version.as_deref(), Some("1.0"));
    assert_eq!(import.message_time_text.as_deref(), Some("20261012093000"));
    assert_eq!(
        import.rows[0],
        ExtractedRow {
            record_path: "P1/O1/R1".to_owned(),
            sample_label: Some("C00000001".to_owned()),
            test_code: Some("SYN-MOR".to_owned()),
            value_text: Some(" 152.40".to_owned()),
            units_text: Some("ng/mL".to_owned()),
            flags_text: Some("H".to_owned()),
            status_text: Some("F".to_owned()),
            operator_text: Some("SYN-TECH-01".to_owned()),
            completed_text: Some("20261012092500".to_owned()),
            sample_type_text: None,
            dilution_text: None,
        }
    );
    assert_eq!(import.rows[1].record_path, "P1/O1/R2");
    assert_eq!(import.rows[1].value_text.as_deref(), Some("<5.0"));
    assert_eq!(import.rows[1].flags_text, None);
    assert_eq!(import.rows[1].completed_text, None);

    let by_instrument_id = extract_artifact(&bytes, &INSTRUMENT_ID).unwrap();
    assert_eq!(
        by_instrument_id.rows[0].sample_label.as_deref(),
        Some("SYN-POS-03")
    );
}

#[test]
fn missing_label_test_code_and_value_stay_absent_and_the_row_is_kept() {
    let bytes = artifact(&["H|\\^&", "P|1", "O|1", "R|1|SYN-MOR||ng/mL", "L|1|N"]);
    let import = extract_artifact(&bytes, &SPECIMEN_ID).unwrap();
    assert_eq!(import.sender_name, None);
    assert_eq!(import.rows.len(), 1);
    let row = &import.rows[0];
    assert_eq!(row.sample_label, None);
    // The code must be in the fourth component; the first is not used.
    assert_eq!(row.test_code, None);
    assert_eq!(row.value_text, None);
}

#[test]
fn escaped_delimiters_are_decoded_but_not_otherwise_changed() {
    let bytes = artifact(&["H|\\^&", "P|1", "O|1|C&F&1", "R|1|^^^SYN&S&A|POS", "L|1|N"]);
    let row = &extract_artifact(&bytes, &SPECIMEN_ID).unwrap().rows[0];
    assert_eq!(row.sample_label.as_deref(), Some("C|1"));
    assert_eq!(row.test_code.as_deref(), Some("SYN^A"));
}

#[test]
fn malformed_artifacts_fail_without_rows() {
    assert!(matches!(
        extract_artifact(b"H|\\^&\rP|1\rO|1\rR|1|^^^SYN-A|1", &SPECIMEN_ID),
        Err(ExtractFailure::Records(
            RecordError::MissingRecordTerminator
        ))
    ));
    assert_eq!(
        extract_artifact(&artifact(&["H|\\^&", "P|1", "O|1", "L|1|N"]), &SPECIMEN_ID),
        Err(ExtractFailure::Message(MessageError::OrderWithoutResult))
    );
    assert_eq!(
        extract_artifact(
            &artifact(&["H|\\^&", "Q|1|^SYN-0001", "L|1|N"]),
            &SPECIMEN_ID
        ),
        Err(ExtractFailure::NotAResultMessage)
    );
}

fn field() -> impl Strategy<Value = String> {
    "[ -%'-~]{0,10}"
}

fn record(kind: &str, sequence: usize, fields: &[String]) -> String {
    format!("{kind}|{sequence}|{}", fields.join("|"))
}

type Order = (Vec<String>, Vec<Vec<String>>);

/// Appends one order and its results; returns the number of results.
fn push_order(records: &mut Vec<String>, sequence: usize, (fields, results): &Order) -> usize {
    records.push(record("O", sequence, fields));
    for (r, result) in results.iter().enumerate() {
        records.push(record("R", r + 1, result));
    }
    results.len()
}

/// Messages with a valid H/P/O/R/L skeleton and arbitrary printable field text.
fn message() -> impl Strategy<Value = (Vec<u8>, usize)> {
    let results = prop::collection::vec(prop::collection::vec(field(), 12), 1..4);
    let order = (prop::collection::vec(field(), 3), results);
    let patient = prop::collection::vec(order, 1..3);
    (field(), prop::collection::vec(patient, 1..3)).prop_map(|(sender, patients)| {
        let mut records = vec![format!("H|\\^&|||{sender}")];
        let mut results = 0;
        for (p, orders) in patients.iter().enumerate() {
            records.push(format!("P|{}", p + 1));
            for (o, order) in orders.iter().enumerate() {
                results += push_order(&mut records, o + 1, order);
            }
        }
        records.push("L|1|N".to_owned());
        let refs: Vec<&str> = records.iter().map(String::as_str).collect();
        (artifact(&refs), results)
    })
}

proptest! {
    #[test]
    fn extract_never_panics_on_arbitrary_bytes(bytes in prop::collection::vec(any::<u8>(), 0..1024)) {
        let _ = extract_artifact(&bytes, &SPECIMEN_ID);
    }

    #[test]
    fn every_result_of_a_parsed_message_becomes_exactly_one_row((bytes, results) in message()) {
        let parsed = parse_message_records(&bytes).map_err(ExtractFailure::from);
        let Ok(message) = parsed.and_then(|records| Ok(assemble_message(records)?)) else {
            return Ok(());
        };
        for profile in [SPECIMEN_ID, INSTRUMENT_ID] {
            let import = extract(&message, &profile).unwrap();
            prop_assert_eq!(import.rows.len(), results);
        }
    }
}
