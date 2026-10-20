//! Renders the synthetic report through the public API (synthetic data only).

use lis_report::{DataError, RENDERER_VERSION, RenderError, TemplateRef, render};

const SYNTHETIC: &str = include_str!("fixtures/synthetic-report.json");

fn tox_v1() -> TemplateRef {
    TemplateRef {
        id: "tox-report".to_owned(),
        version: 1,
    }
}

/// 2026-10-12T14:30:00Z.
const ISSUED_AT: jiff::Timestamp = jiff::Timestamp::constant(1_791_815_400, 0);

/// The synthetic fixture with one JSON value replaced. `cfg(test)` marks the
/// helper as test code, where clippy allows `expect`.
#[cfg(test)]
fn with(pointer: &str, value: serde_json::Value) -> String {
    let mut data: serde_json::Value = serde_json::from_str(SYNTHETIC).expect("fixture parses");
    *data.pointer_mut(pointer).expect("pointer exists") = value;
    data.to_string()
}

#[test]
fn renders_the_synthetic_report_as_pdf() {
    let pdf = render(&tox_v1(), SYNTHETIC, ISSUED_AT).expect("renders");
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    assert!(pdf.bytes.len() > 10_000, "only {} bytes", pdf.bytes.len());
    assert_eq!(pdf.sha256.len(), 64);
    assert_eq!(pdf.renderer_version, RENDERER_VERSION);
    assert_eq!(pdf.template, tox_v1());
    assert_eq!(pdf.template_sha256.len(), 64);
    assert!(!pdf.fonts.is_empty());
}

// NFR-07: identical inputs give identical bytes.
#[test]
fn identical_inputs_render_identical_bytes() {
    let first = render(&tox_v1(), SYNTHETIC, ISSUED_AT).expect("renders");
    let second = render(&tox_v1(), SYNTHETIC, ISSUED_AT).expect("renders");
    assert_eq!(first.sha256, second.sha256);
    assert_eq!(first.bytes, second.bytes);
}

#[test]
fn issued_time_changes_the_bytes() {
    let first = render(&tox_v1(), SYNTHETIC, ISSUED_AT).expect("renders");
    let later = "2026-10-12T14:31:00Z".parse().expect("valid timestamp");
    let second = render(&tox_v1(), SYNTHETIC, later).expect("renders");
    assert_ne!(first.sha256, second.sha256);
}

// REQ-RPT-010: PDF/A-2 archival and PDF/UA-1 identification in the metadata.
#[test]
fn output_declares_pdf_a2a_and_pdf_ua1() {
    let pdf = render(&tox_v1(), SYNTHETIC, ISSUED_AT).expect("renders");
    let text = String::from_utf8_lossy(&pdf.bytes);
    assert!(text.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(text.contains("<pdfaid:conformance>A</pdfaid:conformance>"));
    assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
}

#[test]
fn unknown_template_is_refused() {
    let template = TemplateRef {
        id: "tox-report".to_owned(),
        version: 99,
    };
    let error = render(&template, SYNTHETIC, ISSUED_AT).unwrap_err();
    assert_eq!(
        error,
        RenderError::UnknownTemplate {
            id: "tox-report".to_owned(),
            version: 99
        }
    );
}

#[test]
fn malformed_json_is_refused() {
    let error = render(&tox_v1(), "{\"report\": ", ISSUED_AT).unwrap_err();
    assert!(matches!(
        error,
        RenderError::InvalidData(DataError::Syntax { .. })
    ));
}

#[test]
fn numeric_result_is_refused_so_the_lexical_form_is_kept() {
    let data = with("/result_groups/0/rows/1/result", serde_json::json!(125.0));
    let error = render(&tox_v1(), &data, ISSUED_AT).unwrap_err();
    assert_eq!(error, RenderError::InvalidData(DataError::NonStringScalar));
}

// REQ-RPT-008: a displayed analyte lacking its cutoff fails template validation.
#[test]
fn row_without_cutoff_fails_and_error_hides_values() {
    let mut data: serde_json::Value = serde_json::from_str(SYNTHETIC).expect("fixture parses");
    data["result_groups"][0]["rows"][1]
        .as_object_mut()
        .expect("row object")
        .remove("cutoff");
    let error = render(&tox_v1(), &data.to_string(), ISSUED_AT).unwrap_err();
    assert!(
        matches!(error, RenderError::Compile { errors: 1.., .. }),
        "{error:?}"
    );
    assert!(!error.to_string().contains("Synthetic analyte"));
}

#[test]
fn empty_cutoff_fails() {
    let data = with("/result_groups/0/rows/0/cutoff", serde_json::json!(""));
    assert!(matches!(
        render(&tox_v1(), &data, ISSUED_AT),
        Err(RenderError::Compile { .. })
    ));
}

#[test]
fn unknown_method_certainty_fails() {
    let data = with(
        "/result_groups/0/rows/0/method_certainty",
        serde_json::json!("probable"),
    );
    assert!(matches!(
        render(&tox_v1(), &data, ISSUED_AT),
        Err(RenderError::Compile { .. })
    ));
}

#[test]
fn corrected_report_requires_an_amendment() {
    let data = with("/report/status", serde_json::json!("corrected"));
    assert!(matches!(
        render(&tox_v1(), &data, ISSUED_AT),
        Err(RenderError::Compile { .. })
    ));
}

#[test]
fn corrected_report_with_amendment_renders() {
    let mut data: serde_json::Value = serde_json::from_str(SYNTHETIC).expect("fixture parses");
    data["report"]["status"] = "corrected".into();
    data["report"]["revision"] = "2".into();
    data["report"]["amendment"] = serde_json::json!({ "supersedes_revision": "1", "reason": "Synthetic transcription correction" });
    render(&tox_v1(), &data.to_string(), ISSUED_AT).expect("renders");
}

#[test]
fn final_report_with_amendment_fails() {
    let data = with(
        "/report/amendment",
        serde_json::json!({ "supersedes_revision": "1", "reason": "Synthetic" }),
    );
    assert!(matches!(
        render(&tox_v1(), &data, ISSUED_AT),
        Err(RenderError::Compile { .. })
    ));
}
