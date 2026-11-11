//! Synthetic cases for the report layout templates and their previews;
//! optionally retain exact artifacts for visual review.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_report::preview::render_preview;
use lis_report::tox_v2::{
    ToxComparisonCode, ToxCompleteness, ToxOtherTest, ToxOtherTestStatus, ToxRelationship,
    ToxReportData, ToxSupersedes, to_json,
};
use lis_report::{TemplateRef, render};

const LAYOUTS: &[&str] = &[
    "tox-report-standard",
    "tox-report-compact",
    "tox-report-results-first",
];

fn layout(id: &str) -> TemplateRef {
    TemplateRef {
        id: id.to_owned(),
        version: 1,
    }
}

fn issued() -> jiff::Timestamp {
    jiff::Timestamp::constant(1_791_815_400, 0)
}

fn keep(name: &str, bytes: &[u8]) {
    if let Some(dir) = std::env::var_os("VIBE_REPORT_ARTIFACT_DIR") {
        let dir = std::path::PathBuf::from(dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join(format!("{name}.pdf")), bytes).unwrap();
    }
}

fn cases() -> Vec<(&'static str, ToxReportData)> {
    let mut data: ToxReportData =
        serde_json::from_str(include_str!("fixtures/synthetic-report-v3.json")).unwrap();
    for row in &mut data.result_groups[0].rows {
        row.comparison_code = Some(match row.comparison.as_str() {
            "Meets cutoff" => ToxComparisonCode::MeetsCutoff,
            "Does not meet cutoff" => ToxComparisonCode::DoesNotMeetCutoff,
            _ => ToxComparisonCode::NotClassifiable,
        });
    }
    let mut partial = data.clone();
    partial.report.completeness = ToxCompleteness::Partial;
    partial.other_tests.push(ToxOtherTest {
        name: "Synthetic pending component".to_owned(),
        status: ToxOtherTestStatus::Pending,
        note: "Testing incomplete".to_owned(),
    });
    let mut correction = data.clone();
    correction.report.relationship = ToxRelationship::Correction;
    correction.report.revision = "2".to_owned();
    correction.report.supersedes = Some(ToxSupersedes {
        revision: "1".to_owned(),
        released_at: "2026-10-12 09:30 CDT".to_owned(),
    });
    correction.report.amendment_reason =
        Some("Synthetic corrected quantity after review".to_owned());
    correction.report.change_summary =
        vec!["Synthetic analyte: result changed from <0.50 to 153.400 ng/mL".to_owned()];
    correction.result_groups[0].rows[0].result = "153.400".to_owned();
    correction.result_groups[0].rows[0].comparison = "Meets cutoff".to_owned();
    correction.result_groups[0].rows[0].comparison_code = Some(ToxComparisonCode::MeetsCutoff);
    let mut long = correction.clone();
    long.subject.name = format!(
        "SYNTHETIC {} — {}",
        "García Müller ".repeat(14),
        "Δοκιμή García ".repeat(14)
    );
    long.laboratory.name =
        "Synthetic Laboratory with a long supplied organizational name ".repeat(3);
    long.specimen.condition_notes =
        vec!["Synthetic specimen condition with supplied description. ".repeat(30)];
    long.result_groups[0].rows = (0..60)
        .map(|i| {
            let mut row = correction.result_groups[0].rows[0].clone();
            row.analyte = format!("Synthetic analyte {i:02} — μg/L");
            row
        })
        .collect();
    vec![
        ("final", data),
        ("partial", partial),
        ("correction", correction),
        ("long-unicode", long),
    ]
}

// REQ-RPT-007: every layout renders the full v3 content, including partial
// reports, corrections and long supplied text.
#[test]
fn layouts_render_every_report_case() {
    for id in LAYOUTS {
        for (name, report) in cases() {
            let pdf = render(&layout(id), &to_json(&report).unwrap(), issued())
                .unwrap_or_else(|error| panic!("{id} {name}: {error}"));
            assert!(pdf.bytes.starts_with(b"%PDF"));
            keep(&format!("{id}-{name}"), &pdf.bytes);
        }
    }
}

// REQ-RPT-008: the layouts take every clinical word from the data.
#[test]
fn layouts_print_the_engine_wording_and_no_interpretation() {
    let source = [
        include_str!("../templates/tox-layouts/contract-v1.typ"),
        include_str!("../templates/tox-layouts/standard-v1.typ"),
        include_str!("../templates/tox-layouts/compact-v1.typ"),
        include_str!("../templates/tox-layouts/results-first-v1.typ"),
    ]
    .concat()
    .to_lowercase();
    for word in ["positive", "negative", "meets cutoff", "detected"] {
        assert!(!source.contains(word), "{word}");
    }
}

#[test]
fn previews_render_the_synthetic_sample() {
    for id in LAYOUTS.iter().copied().chain(["tox-report"]) {
        let template = TemplateRef {
            id: id.to_owned(),
            version: if id == "tox-report" { 3 } else { 1 },
        };
        let pdf = render_preview(&template, issued()).unwrap();
        keep(&format!("{id}-preview"), &pdf.bytes);
    }
}
