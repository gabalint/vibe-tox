//! Laboratory output stays byte-identical, and demo output is marked (proposed
//! REQ-OPS-016; NFR-07). Synthetic data only.

use lis_report::{notice_v1, tox_v2};

/// 2026-10-12T14:30:00Z.
const ISSUED_AT: jiff::Timestamp = jiff::Timestamp::constant(1_791_815_400, 0);

const REPORT: &str = include_str!("fixtures/synthetic-report-v2.json");
const NOTICE: &str = include_str!("fixtures/synthetic-notice.json");

/// Digests of the synthetic fixtures rendered before the demo marking existed: a
/// laboratory installation's output must never change.
const REPORT_GOLDEN: &str = "b5b5bffabe75d86651480ffdd712c571cf484b9576e84b7866e2566d50f019ad";
const NOTICE_GOLDEN: &str = "dc699fd95d95a182ede276355b16102c1343e9824f427c5dcb7b98e14898c741";

#[cfg(test)]
fn report_json() -> String {
    let data: tox_v2::ToxReportData = serde_json::from_str(REPORT).expect("fixture parses");
    tox_v2::to_json(&data).expect("serializes")
}

#[test]
fn historical_v2_serialization_does_not_add_v3_defaults() {
    let original: serde_json::Value = serde_json::from_str(REPORT).unwrap();
    let serialized: serde_json::Value = serde_json::from_str(&report_json()).unwrap();
    assert_eq!(serialized, original);
    // The renderer's document identifier includes the exact serialized input.
    // Empty new fields must not alter historical JSON or the pinned PDF bytes.
    assert!(!report_json().contains("generatedAt"));
    assert!(!report_json().contains("performingSites"));
}

#[cfg(test)]
fn notice_json() -> String {
    let data: notice_v1::ReportNoticeData = serde_json::from_str(NOTICE).expect("fixture parses");
    notice_v1::to_json(&data).expect("serializes")
}

#[test]
fn laboratory_output_is_byte_identical_to_the_golden_digests() {
    let report = tox_v2::render_json(&report_json(), ISSUED_AT).expect("renders");
    let notice = notice_v1::render_json(&notice_json(), ISSUED_AT).expect("renders");
    assert_eq!(
        (report.sha256.as_str(), notice.sha256.as_str()),
        (REPORT_GOLDEN, NOTICE_GOLDEN)
    );
}

// Proposed REQ-OPS-016: demo output is a conforming PDF of the same template, marked, and
// as deterministic as laboratory output.
#[test]
fn demo_output_conforms_keeps_its_template_and_differs_from_laboratory_output() {
    let template = lis_report::TemplateRef {
        id: "tox-report".to_owned(),
        version: 2,
    };
    let json = report_json();
    let lab = tox_v2::render_json(&json, ISSUED_AT).expect("renders");
    let demo = || {
        lis_report::render_marked(&template, &json, ISSUED_AT, lis_report::Marking::Demo)
            .expect("renders")
    };
    let (first, second) = (demo(), demo());
    assert_eq!(first.bytes, second.bytes);
    assert_ne!(first.sha256, lab.sha256);
    assert_eq!(
        (&first.template, &first.template_sha256),
        (&lab.template, &lab.template_sha256)
    );
    let text = String::from_utf8_lossy(&first.bytes);
    assert!(text.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
    let unmarked =
        lis_report::render_marked(&template, &json, ISSUED_AT, lis_report::Marking::Unmarked)
            .expect("renders");
    assert_eq!(unmarked.sha256, REPORT_GOLDEN);
}
