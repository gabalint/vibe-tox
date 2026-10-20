//! Row outcomes of a result file in the order of the brief's §5 table:
//! malformed rows, not-a-result names, unplaced batch rows, role mismatch,
//! value words and the zero rule, units, and the dilution cross-check.

use lis_analytical::AttemptKind;
use lis_core::Comparator;
use lis_results::{
    ImportAttemptDetail, ImportOutcome, QuarantineReason, ResultCause, ResultOutcome, SetAsideKind,
    SpecimenResults,
};

use crate::*;

/// One data row of the fixture's header.
pub(crate) fn row(
    label: &str,
    kind: &str,
    component: &str,
    value: &str,
    units: &str,
    dilution: &str,
) -> String {
    format!("{label},{kind},{component},{value},{units},{dilution}")
}

/// `(reason, record path)` of every quarantined row, by record path.
pub(crate) fn quarantined(detail: &ImportAttemptDetail) -> Vec<(QuarantineReason, String)> {
    detail
        .quarantine
        .iter()
        .map(|item| (item.reason, item.record_path.clone()))
        .collect()
}

/// The current revision of the analyte `code`'s line.
pub(crate) fn current(results: &SpecimenResults, code: &str) -> lis_results::ResultRevisionView {
    results
        .lines
        .iter()
        .find(|line| line.analyte_code == code)
        .and_then(|line| line.current.clone())
        .unwrap_or_else(|| panic!("no current {code} revision"))
}

/// `(registration, word)` a revision cites.
fn rule(f: &Fixture, revision: lis_results::ResultRevisionId) -> (Option<String>, Option<String>) {
    f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT value_rule_registration_id, value_rule_word FROM res_result_revisions \
             WHERE id = ?1",
            [revision],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    })
}

#[test]
fn every_row_of_a_file_ends_in_exactly_one_counted_outcome() {
    let f = fixture();
    let s = f.sample();
    let l = s.label.as_str();
    let attempt = f.import(&[
        row(l, "Unknown", "Morphine", "152.4", UNIT, "1"),
        row(l, "Unknown", "Morphine-D6", "", UNIT, "1"),
        row(l, "Unknown", "Oxycodone", "12.0", UNIT, "1"),
        row(l, "Unknown", "Fentanyl", "0.8", UNIT, "1"),
        row(CONTROL, "QC", "Morphine", "48.9", UNIT, "1"),
        row("CAL-1", "Standard", "Morphine", "10.2", UNIT, "1"),
        row("C99999999", "Unknown", "Morphine", "75.0", UNIT, "1"),
        format!("{l},Unknown,Morphine,1.0"),
    ]);
    assert_eq!(attempt.outcome, ImportOutcome::ProcessedWithQuarantine);
    assert_eq!(
        (
            attempt.rows_total,
            attempt.rows_new,
            attempt.rows_qc,
            attempt.rows_quarantined,
            attempt.rows_not_result,
            attempt.rows_set_aside,
            attempt.rows_retained,
            attempt.rows_duplicate,
        ),
        (8, 2, 1, 2, 1, 1, 1, 0)
    );
    let detail = f.detail(&attempt);
    assert_eq!(
        quarantined(&detail),
        [
            (QuarantineReason::UnknownSample, "L8".to_owned()),
            (QuarantineReason::MalformedRow, "L9".to_owned()),
        ]
    );
    let set_aside = &detail.set_aside[0];
    assert_eq!(
        (
            set_aside.kind,
            set_aside.reason,
            set_aside.record_path.as_str(),
            set_aside.sample_label.as_deref(),
            set_aside.type_word.as_deref(),
            set_aside.value_text.as_deref(),
        ),
        (
            SetAsideKind::BatchRowNotPlaced,
            None,
            "L7",
            Some("CAL-1"),
            Some("Standard"),
            Some("10.2")
        )
    );
    assert_eq!(
        detail.registration_id,
        f.registration().map(|r| r.registration_id)
    );
    let file = detail.file.unwrap();
    assert_eq!(file.file_name.expose(), "batch-1.csv");
    assert_eq!(file.sha256.as_deref().map(str::len), Some(64));
    let parser: String = f.read(move |conn| {
        Ok(conn.query_row(
            "SELECT parser_version FROM res_import_attempts WHERE id = ?1",
            [attempt.import_attempt_id],
            |row| row.get(0),
        )?)
    });
    assert_eq!(parser, "delimited-v1");
    let results = f.results(&s);
    assert_eq!(
        current(&results, "SYN-MOR").magnitude.as_deref(),
        Some("152.4")
    );
    assert!(
        results
            .lines
            .iter()
            .all(|line| line.analyte_code != "SYN-FEN")
    );
}

// §7: the attempt list says how each file arrived, without opening the attempt.
#[test]
fn an_attempt_says_how_its_file_arrived() {
    let f = fixture();
    let s = f.sample();
    let attempt = f.import(&[row(&s.label, "Unknown", "Morphine", "30", UNIT, "1")]);
    assert_eq!(attempt.origin, Some(lis_interfaces::FileOrigin::Upload));
    let listed = f.read(|conn| lis_results::list_import_attempts(conn, None, 10, None));
    assert_eq!(listed, std::slice::from_ref(&attempt));
    assert_eq!(f.detail(&attempt).summary, attempt);
}

// F7: the word stays the measurement; the revision is `<` the frozen method
// version's limit, or not performed, and cites the rule. A word skips the
// unit check (F8).
#[test]
fn approved_words_keep_the_word_and_derive_a_result_citing_the_rule() {
    let f = fixture();
    let (a, b) = (f.sample(), f.sample());
    let attempt = f.import(&[
        row(&a.label, "Unknown", "Morphine", "N/F", "", "1"),
        row(&b.label, "Unknown", "Morphine", "ND", UNIT, "1"),
        row(&b.label, "Unknown", "Oxycodone", "NP", "ug/L", "1"),
    ]);
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (3, 0));
    assert_eq!(
        f.measurements(&a, f.analytes.mor),
        [("word".to_owned(), Some("N/F".to_owned()), None)]
    );
    let registration = f.registration().unwrap().registration_id.to_string();
    let below_loq = current(&f.results(&a), "SYN-MOR");
    assert_eq!(
        (
            below_loq.cause,
            below_loq.outcome,
            below_loq.comparator,
            below_loq.magnitude.as_deref(),
            below_loq.units.as_deref(),
            below_loq.classification.as_str(),
        ),
        (
            ResultCause::Initial,
            ResultOutcome::Censored,
            Some(Comparator::Lt),
            Some("5"),
            Some(UNIT),
            "does_not_meet_cutoff"
        )
    );
    assert_eq!(
        rule(&f, below_loq.revision_id),
        (Some(registration.clone()), Some("N/F".to_owned()))
    );
    let results = f.results(&b);
    let below_lod = current(&results, "SYN-MOR");
    assert_eq!(
        (below_lod.comparator, below_lod.magnitude.as_deref()),
        (Some(Comparator::Lt), Some("1"))
    );
    let not_performed = current(&results, "SYN-OXY");
    assert_eq!(
        (
            not_performed.outcome,
            not_performed.magnitude,
            not_performed.classification.as_str()
        ),
        (ResultOutcome::NotPerformed, None, "not_applicable")
    );
    assert_eq!(
        rule(&f, not_performed.revision_id),
        (Some(registration), Some("NP".to_owned()))
    );
}

#[test]
fn a_word_whose_limit_the_method_version_lacks_is_quarantined() {
    let f = fixture();
    let s = f.sample();
    let attempt = f.import(&[
        row(&s.label, "Unknown", "Oxycodone", "N/F", UNIT, "1"),
        row(&s.label, "Unknown", "Oxycodone", "ND", UNIT, "1"),
        row(&s.label, "Unknown", "Morphine", "N/F", UNIT, "1"),
    ]);
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (1, 2));
    assert_eq!(
        quarantined(&f.detail(&attempt)),
        [
            (QuarantineReason::MalformedValue, "L2".to_owned()),
            (QuarantineReason::MalformedValue, "L3".to_owned()),
        ]
    );
    assert!(f.measurements(&s, f.analytes.oxy).is_empty());
}

// F7: an empty cell never means anything; zero, negative and `<0`/`>0` are
// quarantined unless the exact text is mapped; the comparison is numeric.
#[test]
fn empty_unmapped_zero_and_negative_values_are_quarantined_unless_mapped() {
    let f = fixture();
    let (s, mapped) = (f.sample(), f.sample());
    let refused = [
        "",
        "N/A",
        "0",
        "-1.2",
        ">0",
        "0.000",
        "\"1,234.5\"",
        "1.2E+02",
    ];
    let mut rows: Vec<String> = refused
        .iter()
        .map(|value| row(&s.label, "Unknown", "Morphine", value, UNIT, "1"))
        .collect();
    rows.push(row(&mapped.label, "Unknown", "Morphine", "<0", UNIT, "1"));
    rows.push(row(
        &mapped.label,
        "Unknown",
        "Oxycodone",
        "<0.50",
        UNIT,
        "1",
    ));
    let attempt = f.import(&rows);
    assert_eq!(attempt.rows_quarantined, 8);
    assert!(
        quarantined(&f.detail(&attempt))
            .iter()
            .all(|(reason, _)| *reason == QuarantineReason::MalformedValue)
    );
    assert!(f.measurements(&s, f.analytes.mor).is_empty());
    let results = f.results(&mapped);
    let word = current(&results, "SYN-MOR");
    assert_eq!(
        (word.comparator, word.magnitude.as_deref()),
        (Some(Comparator::Lt), Some("5"))
    );
    let censored = current(&results, "SYN-OXY");
    assert_eq!(
        (censored.outcome, censored.magnitude.as_deref()),
        (ResultOutcome::Censored, Some("0.50"))
    );
    assert_eq!(rule(&f, censored.revision_id), (None, None));
}

// F9: the run position's role decides the route; a typed row that disagrees
// is quarantined, and an unmapped or empty type word is untyped.
#[test]
fn a_typed_row_on_a_position_of_another_role_is_a_role_mismatch() {
    let f = fixture();
    let s = f.sample();
    let attempt = f.import(&[
        row(CONTROL, "Unknown", "Morphine", "48.9", UNIT, "1"),
        row(&s.label, "QC", "Morphine", "12.0", UNIT, "1"),
        row(&s.label, "Sample", "Oxycodone", "13.0", UNIT, "1"),
        row(&s.label, "", "Morphine", "14.0", UNIT, "1"),
    ]);
    assert_eq!(
        (attempt.rows_quarantined, attempt.rows_new, attempt.rows_qc),
        (2, 2, 0)
    );
    assert_eq!(
        quarantined(&f.detail(&attempt)),
        [
            (QuarantineReason::RoleMismatch, "L2".to_owned()),
            (QuarantineReason::RoleMismatch, "L3".to_owned()),
        ]
    );
    let results = f.results(&s);
    let gates = &results
        .lines
        .iter()
        .find(|line| line.analyte_code == "SYN-MOR")
        .unwrap()
        .gates;
    assert!(
        gates.iter().any(|gate| gate.code == "quarantine_open"),
        "{gates:?}"
    );
}

// F9: only a calibrator or blank row whose label is no container's and no
// lot's is set aside; anything else unplaced is `unknown_sample`.
#[test]
fn calibrator_and_blank_rows_the_lis_does_not_know_are_set_aside() {
    let f = fixture();
    let elsewhere = f.unplaced_sample(f.panel);
    let attempt = f.import(&[
        row("CAL-1", "Standard", "Morphine", "10.2", UNIT, "1"),
        row("BLANK-1", "Blank", "Morphine", "", UNIT, "1"),
        row(&elsewhere.label, "Standard", "Morphine", "10.2", UNIT, "1"),
        row(CALIBRATOR_LOT, "Standard", "Morphine", "10.2", UNIT, "1"),
        row("CAL-2", "Unknown", "Morphine", "10.2", UNIT, "1"),
        row("CAL-3", "", "Morphine", "10.2", UNIT, "1"),
    ]);
    assert_eq!((attempt.rows_set_aside, attempt.rows_quarantined), (2, 4));
    let detail = f.detail(&attempt);
    let set_aside: Vec<_> = detail
        .set_aside
        .iter()
        .map(|row| (row.kind, row.record_path.as_str(), row.type_word.as_deref()))
        .collect();
    assert_eq!(
        set_aside,
        [
            (SetAsideKind::BatchRowNotPlaced, "L2", Some("Standard")),
            (SetAsideKind::BatchRowNotPlaced, "L3", Some("Blank")),
        ]
    );
    assert!(
        quarantined(&detail)
            .iter()
            .all(|(reason, _)| *reason == QuarantineReason::UnknownSample)
    );
}

// F11: an execution without a factor matches an empty cell or 1; with a
// factor, the cell must equal it numerically. The LIS never multiplies.
#[test]
fn the_dilution_column_must_equal_the_executions_factor() {
    let f = fixture();
    let (a, b) = (f.sample(), f.sample());
    let initial = f.import(&[
        row(&a.label, "Unknown", "Morphine", "20", UNIT, ""),
        row(&a.label, "Unknown", "Oxycodone", "21", UNIT, "1"),
        row(&b.label, "Unknown", "Morphine", "22", UNIT, "1.0"),
        row(&b.label, "Unknown", "Oxycodone", "23", UNIT, "10"),
    ]);
    assert_eq!((initial.rows_new, initial.rows_quarantined), (3, 1));
    assert_eq!(
        quarantined(&f.detail(&initial)),
        [(QuarantineReason::DilutionMismatch, "L5".to_owned())]
    );
    f.close_run(f.run);
    let second = f.open_run(false);
    let diluted = f.place(b, second, AttemptKind::Dilution, Some("10"));
    let attempt = f.import(&[
        row(&diluted.label, "Unknown", "Morphine", "220", UNIT, "10.0"),
        row(&diluted.label, "Unknown", "Oxycodone", "230", UNIT, "1"),
        row(&diluted.label, "Unknown", "Oxycodone", "231", UNIT, ""),
    ]);
    assert_eq!((attempt.rows_new, attempt.rows_quarantined), (1, 2));
    assert_eq!(
        f.measurements(&diluted, f.analytes.mor)
            .into_iter()
            .map(|(_, value, _)| value.unwrap())
            .collect::<Vec<_>>(),
        ["22", "220"]
    );
}

// F8: the units column must equal the method's unit exactly; a template
// without one records the unit it declares.
#[test]
fn units_must_equal_the_methods_unless_the_template_declares_them() {
    let f = fixture();
    let (s, t) = (f.sample(), f.sample());
    let attempt = f.import(&[
        row(&s.label, "Unknown", "Morphine", "15", "ug/L", "1"),
        row(&s.label, "Unknown", "Oxycodone", "16", "", "1"),
    ]);
    assert_eq!(attempt.rows_quarantined, 2);
    assert!(
        quarantined(&f.detail(&attempt))
            .iter()
            .all(|(reason, _)| *reason == QuarantineReason::IncompatibleUnit)
    );
    let mut declared = layout();
    declared.columns.units = None;
    declared.declared_unit = Some(UNIT.to_owned());
    f.replace_layout(declared.clone());
    let attempt = f.import(&[row(&s.label, "Unknown", "Morphine", "15", "ug/L", "1")]);
    assert_eq!(attempt.rows_new, 1);
    assert_eq!(
        f.measurements(&s, f.analytes.mor),
        [(
            "quantity".to_owned(),
            Some("15".to_owned()),
            Some(UNIT.to_owned())
        )]
    );
    declared.declared_unit = Some("ug/L".to_owned());
    f.replace_layout(declared);
    let attempt = f.import(&[row(&t.label, "Unknown", "Morphine", "15", UNIT, "1")]);
    assert_eq!(
        quarantined(&f.detail(&attempt)),
        [(QuarantineReason::IncompatibleUnit, "L2".to_owned())]
    );
}
