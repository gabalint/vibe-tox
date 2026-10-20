//! `tox-report` v2 through the public API: typed data, JSON and the template
//! rules (REQ-RPT-001, REQ-RPT-002, REQ-RPT-005, REQ-RPT-008, NFR-07).
//! Synthetic data only.

use lis_report::tox_v2::{
    self, ToxCompleteness, ToxOtherTest, ToxOtherTestStatus, ToxRelationship, ToxReportData,
    ToxSupersedes,
};
use lis_report::{RenderError, TemplateRef};
use serde_json::Value;

const FIXTURE: &str = include_str!("fixtures/synthetic-report-v2.json");

/// A value that must never appear in an error message.
const CANARY: &str = "CANARY-SYNTHETIC-PHI";

/// 2026-10-12T14:30:00Z.
const ISSUED_AT: jiff::Timestamp = jiff::Timestamp::constant(1_791_815_400, 0);

/// The fixture, a final initial report, with the canary as the patient name.
#[cfg(test)]
fn final_initial() -> ToxReportData {
    let mut data: ToxReportData = serde_json::from_str(FIXTURE).expect("fixture parses");
    data.subject.name = CANARY.to_owned();
    data
}

#[cfg(test)]
fn superseding(relationship: ToxRelationship) -> ToxReportData {
    let mut data = final_initial();
    data.report.revision = "2".to_owned();
    data.report.relationship = relationship;
    data.report.supersedes = Some(ToxSupersedes {
        revision: "1".to_owned(),
        released_at: "2026-10-12 09:30 CDT".to_owned(),
    });
    data
}

#[cfg(test)]
fn partial_supplement() -> ToxReportData {
    let mut data = superseding(ToxRelationship::Supplement);
    data.report.completeness = ToxCompleteness::Partial;
    data.other_tests.push(ToxOtherTest {
        name: "Synthetic pending test".to_owned(),
        status: ToxOtherTestStatus::Pending,
        note: String::new(),
    });
    data
}

#[cfg(test)]
fn correction() -> ToxReportData {
    let mut data = superseding(ToxRelationship::Correction);
    data.report.amendment_reason = Some("Synthetic transcription correction".to_owned());
    data
}

#[cfg(test)]
fn render(data: &ToxReportData) -> Result<lis_report::RenderedPdf, RenderError> {
    tox_v2::render_json(&tox_v2::to_json(data).expect("serializes"), ISSUED_AT)
}

/// Asserts the render fails in the template and the error hides data values.
#[cfg(test)]
fn assert_refused(result: Result<lis_report::RenderedPdf, RenderError>) {
    let error = result.expect_err("render must fail");
    assert!(matches!(error, RenderError::Compile { .. }), "{error:?}");
    assert!(!error.to_string().contains(CANARY));
    assert!(!format!("{error:?}").contains(CANARY));
}

/// Raw JSON of `data` with one value replaced, for codes the types cannot express.
#[cfg(test)]
fn json_with(data: &ToxReportData, pointer: &str, value: &str) -> String {
    let mut json: Value =
        serde_json::from_str(&tox_v2::to_json(data).expect("serializes")).expect("parses");
    *json.pointer_mut(pointer).expect("pointer exists") = value.into();
    json.to_string()
}

#[test]
fn renders_final_initial_partial_supplement_and_correction() {
    let expected = TemplateRef {
        id: "tox-report".to_owned(),
        version: 2,
    };
    for data in [final_initial(), partial_supplement(), correction()] {
        let pdf = render(&data).expect("renders");
        assert!(pdf.bytes.starts_with(b"%PDF"));
        assert_eq!(pdf.template, expected);
        let text = String::from_utf8_lossy(&pdf.bytes);
        assert!(text.contains("<pdfaid:part>2</pdfaid:part>"));
        assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
    }
}

// NFR-07: identical inputs give identical bytes.
#[test]
fn identical_inputs_render_identical_bytes() {
    let first = render(&correction()).expect("renders");
    let second = render(&correction()).expect("renders");
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.sha256, second.sha256);
}

// REQ-RPT-002: a correction states its reason and the revision it replaces.
#[test]
fn correction_without_amendment_reason_fails() {
    let mut data = correction();
    data.report.amendment_reason = None;
    assert_refused(render(&data));
}

#[test]
fn correction_without_supersedes_fails() {
    let mut data = correction();
    data.report.supersedes = None;
    assert_refused(render(&data));
}

#[test]
fn supplement_without_supersedes_fails() {
    let mut data = partial_supplement();
    data.report.supersedes = None;
    assert_refused(render(&data));
}

#[test]
fn initial_report_with_supersedes_fails() {
    let mut data = correction();
    data.report.relationship = ToxRelationship::Initial;
    data.report.amendment_reason = None;
    assert_refused(render(&data));
}

#[test]
fn amendment_reason_outside_a_correction_fails() {
    let mut data = partial_supplement();
    data.report.amendment_reason = Some("Synthetic reason".to_owned());
    assert_refused(render(&data));
}

// REQ-RPT-005: a partial report lists the tests it does not report.
#[test]
fn partial_with_no_other_tests_fails() {
    let mut data = partial_supplement();
    data.other_tests.clear();
    assert_refused(render(&data));
}

#[test]
fn partial_with_only_resolved_other_tests_fails() {
    let mut data = final_initial();
    data.report.completeness = ToxCompleteness::Partial;
    assert_refused(render(&data));
}

#[test]
fn final_with_a_pending_test_fails() {
    let mut data = partial_supplement();
    data.report.completeness = ToxCompleteness::Final;
    assert_refused(render(&data));
}

// REQ-RPT-008: every displayed analyte carries its limits.
#[test]
fn row_with_empty_limits_fails() {
    let mut data = final_initial();
    data.result_groups[0].rows[1].limits = String::new();
    assert_refused(render(&data));
}

#[test]
fn unknown_completeness_fails() {
    let json = json_with(&final_initial(), "/report/completeness", "draft");
    assert_refused(tox_v2::render_json(&json, ISSUED_AT));
}

#[test]
fn unknown_relationship_fails() {
    let json = json_with(&final_initial(), "/report/relationship", "amendment");
    assert_refused(tox_v2::render_json(&json, ISSUED_AT));
}

#[test]
fn unknown_other_test_status_fails() {
    let json = json_with(&final_initial(), "/otherTests/0/status", "lost");
    assert_refused(tox_v2::render_json(&json, ISSUED_AT));
}

#[test]
fn json_is_camel_case_strings_or_null_and_round_trips() {
    let data = partial_supplement();
    let json = tox_v2::to_json(&data).expect("serializes");
    let value: Value = serde_json::from_str(&json).expect("parses");
    assert_strings_and_camel_case(&value);
    assert_eq!(value["report"]["completeness"], "partial");
    assert_eq!(value["otherTests"][0]["status"], "not_activated");
    assert_eq!(value["otherTests"][1]["status"], "pending");
    assert_eq!(value["subject"]["identifiers"][0]["type"], "MRN");
    let back: ToxReportData = serde_json::from_str(&json).expect("round-trips");
    assert_eq!(back, data);
}

#[cfg(test)]
fn assert_strings_and_camel_case(value: &Value) {
    match value {
        Value::Null | Value::String(_) => {}
        Value::Array(items) => items.iter().for_each(assert_strings_and_camel_case),
        Value::Object(fields) => {
            for (key, field) in fields {
                assert!(!key.contains('_'), "{key}");
                assert_strings_and_camel_case(field);
            }
        }
        Value::Number(_) | Value::Bool(_) => panic!("non-string scalar"),
    }
}

#[test]
fn debug_output_is_redacted() {
    assert!(!format!("{:?}", final_initial()).contains(CANARY));
}
