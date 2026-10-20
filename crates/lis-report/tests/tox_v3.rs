//! Synthetic v3 report cases; optionally retain exact artifacts for visual review.
use lis_report::tox_v2::{
    ToxCompleteness, ToxOtherTest, ToxOtherTestStatus, ToxRelationship, ToxReportData,
    ToxSupersedes, to_json,
};
use lis_report::{TemplateRef, render};

#[test]
fn report_v3_final_partial_correction_long_and_unicode_artifacts() {
    let data: ToxReportData =
        serde_json::from_str(include_str!("fixtures/synthetic-report-v3.json")).unwrap();
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
    correction.report.generated_at = Some("2026-10-13 09:30 CDT".to_owned());
    correction.report.issued_at = "2026-10-13 09:30 CDT".to_owned();
    correction.result_groups[0].rows[1].first_reported_at = Some("2026-10-12 09:30 CDT".to_owned());
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
    for (name, report) in [
        ("final", data),
        ("partial", partial),
        ("correction", correction),
        ("long-unicode", long),
    ] {
        let json = to_json(&report).unwrap();
        let pdf = render(
            &TemplateRef {
                id: "tox-report".to_owned(),
                version: 3,
            },
            &json,
            jiff::Timestamp::constant(1_791_815_400, 0),
        )
        .unwrap();
        assert!(pdf.bytes.starts_with(b"%PDF"));
        if let Some(dir) = std::env::var_os("VIBE_REPORT_ARTIFACT_DIR") {
            let dir = std::path::PathBuf::from(dir);
            std::fs::create_dir_all(&dir).unwrap();
            std::fs::write(dir.join(format!("report-v3-{name}.pdf")), &pdf.bytes).unwrap();
            std::fs::write(dir.join(format!("report-v3-{name}.json")), &json).unwrap();
        }
    }
}
