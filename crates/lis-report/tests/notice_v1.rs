//! `report-notice` v1 through the public API (REQ-RPT-002, NFR-07).
//! Synthetic data only.

use lis_report::notice_v1::{self, ReportNoticeData, ReportNoticeKind};
use lis_report::{RenderError, TemplateRef};
use serde_json::Value;

const FIXTURE: &str = include_str!("fixtures/synthetic-notice.json");

/// 2026-10-13T15:05:00Z.
const ISSUED_AT: jiff::Timestamp = jiff::Timestamp::constant(1_791_903_900, 0);

#[cfg(test)]
fn notice() -> ReportNoticeData {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

#[cfg(test)]
fn notice_json() -> Value {
    serde_json::from_str(&notice_v1::to_json(&notice()).expect("serializes")).expect("parses")
}

#[cfg(test)]
fn assert_refused(json: &Value) {
    let error = notice_v1::render_json(&json.to_string(), ISSUED_AT).expect_err("must fail");
    assert!(matches!(error, RenderError::Compile { .. }), "{error:?}");
    assert!(!error.to_string().contains("ACC-SYN-0002"));
}

#[test]
fn withdrawal_notice_renders_deterministically() {
    let data = notice();
    assert_eq!(data.notice.kind, ReportNoticeKind::Withdrawal);
    let json = notice_v1::to_json(&data).expect("serializes");
    let first = notice_v1::render_json(&json, ISSUED_AT).expect("renders");
    let second = notice_v1::render_json(&json, ISSUED_AT).expect("renders");
    assert!(first.bytes.starts_with(b"%PDF"));
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(
        first.template,
        TemplateRef {
            id: "report-notice".to_owned(),
            version: 1
        }
    );
}

#[test]
fn unknown_kind_fails() {
    let mut json = notice_json();
    json["notice"]["kind"] = "retraction".into();
    assert_refused(&json);
}

// REQ-RPT-002: the notice carries no demographics, results or reason.
#[test]
fn notice_data_has_only_report_identity() {
    let json = notice_json();
    let keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    assert_eq!(keys, ["laboratory", "notice"]);
    let mut notice_keys: Vec<&str> = json["notice"]
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    notice_keys.sort_unstable();
    assert_eq!(
        notice_keys,
        [
            "accession",
            "issuedAt",
            "issuedBy",
            "kind",
            "reportNumber",
            "revision"
        ]
    );
    assert_eq!(json["notice"]["kind"], "withdrawal");
}

#[test]
fn extra_subject_section_fails() {
    let mut json = notice_json();
    json["subject"] = serde_json::json!({ "name": "SYNTHETIC, PATIENT ONE" });
    assert_refused(&json);
}

#[test]
fn extra_reason_in_notice_fails() {
    let mut json = notice_json();
    json["notice"]["reason"] = "Synthetic reason".into();
    assert_refused(&json);
}

#[test]
fn debug_output_is_redacted() {
    assert!(!format!("{:?}", notice()).contains("ACC-SYN-0002"));
}
