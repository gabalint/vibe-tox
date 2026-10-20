//! `catalog-preview` v2 with the v1 data contract through the public API: determinism, a golden digest
//! and the PDF standards (proposed REQ-OPS-015, NFR-07). Synthetic data only.

use lis_report::catalog_preview_v1::{self, CatalogPreviewDocument};
use lis_report::{RenderedPdf, TemplateRef};

const FIXTURE: &str = include_str!("fixtures/synthetic-catalog-preview.json");

/// 2026-10-13T14:30:00Z.
const ISSUED_AT: jiff::Timestamp = jiff::Timestamp::constant(1_791_901_800, 0);

/// Digest of the PDF for the fixture. A change means the rendered bytes
/// changed: a different template, renderer or font. Check the change is
/// intended before updating (a template edit also needs a new version).
const GOLDEN_SHA256: &str = "92a21539fa0a0272664d4d55fa05e538cb7c04ac7ff4c8ee1aa2f2a44791f2dd";

#[cfg(test)]
fn render(data: &CatalogPreviewDocument) -> RenderedPdf {
    catalog_preview_v1::render_json(
        &catalog_preview_v1::to_json(data).expect("serializes"),
        ISSUED_AT,
        lis_report::Marking::Unmarked,
    )
    .expect("renders")
}

#[cfg(test)]
fn fixture() -> CatalogPreviewDocument {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

// NFR-07: identical inputs give identical bytes.
#[test]
fn identical_inputs_render_identical_bytes() {
    let first = render(&fixture());
    let second = render(&fixture());
    assert_eq!(first.bytes, second.bytes);
    assert_eq!(first.sha256, second.sha256);
}

#[test]
fn golden_digest_of_the_synthetic_fixture() {
    assert_eq!(render(&fixture()).sha256, GOLDEN_SHA256);
}

#[test]
fn output_is_a_pdf_a2a_and_ua1_document_from_the_template() {
    let pdf = render(&fixture());
    assert!(pdf.bytes.starts_with(b"%PDF-"));
    assert_eq!(
        pdf.template,
        TemplateRef {
            id: "catalog-preview".to_owned(),
            version: 2
        }
    );
    let text = String::from_utf8_lossy(&pdf.bytes);
    assert!(text.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
}

#[test]
fn a_changed_value_changes_the_digest() {
    let mut data = fixture();
    data.methods[0].versions[0].analytes[1].cutoff = Some("50.1".to_owned());
    assert_ne!(render(&data).sha256, render(&fixture()).sha256);
}

#[test]
fn json_is_camel_case_strings_or_null() {
    let json = catalog_preview_v1::to_json(&fixture()).expect("serializes");
    let value: serde_json::Value = serde_json::from_str(&json).expect("parses");
    assert_eq!(value["methods"][0]["versions"][0]["status"], "draft");
    assert_eq!(
        value["methods"][0]["versions"][0]["analytes"][1]["cutoffSource"]["kind"],
        "published"
    );
    assert_eq!(
        value["methods"][0]["versions"][0]["analytes"][0]["cutoff"],
        serde_json::Value::Null
    );
}

// Proposed REQ-OPS-016: the demo laboratory's preview is watermarked; the laboratory's is
// the golden output above.
#[test]
fn demo_preview_is_marked_and_conforms() {
    let json = catalog_preview_v1::to_json(&fixture()).expect("serializes");
    let demo = catalog_preview_v1::render_json(&json, ISSUED_AT, lis_report::Marking::Demo)
        .expect("renders");
    assert_ne!(demo.sha256, GOLDEN_SHA256);
    assert_eq!(demo.template_sha256, render(&fixture()).template_sha256);
    let text = String::from_utf8_lossy(&demo.bytes);
    assert!(text.contains("<pdfaid:part>2</pdfaid:part>"));
    assert!(text.contains("<pdfuaid:part>1</pdfuaid:part>"));
}

// NFR-07: the earlier template remains reproducible after source-label corrections.
#[test]
fn earlier_catalog_preview_template_is_unchanged() {
    let pdf = lis_report::render_marked(
        &TemplateRef {
            id: "catalog-preview".to_owned(),
            version: 1,
        },
        &catalog_preview_v1::to_json(&fixture()).unwrap(),
        ISSUED_AT,
        lis_report::Marking::Unmarked,
    )
    .unwrap();
    assert_eq!(
        pdf.sha256,
        "f56013f80a45f1fde18d23a5ef0a70fd4b06e0e24b49475157e487d9be55d913"
    );
}
