//! A specimen with a validity component reaches a final report through
//! the engine (REQ-RPT-008, validity part; coordinator decision
//! 2026-10-13): the validity line prints the measured value, its units and
//! the validity outcome, and method limits only where the method version
//! defines them. Expected wording comes from `docs/planning/m3-design-brief.md`
//! and the decision, never from the implementation. Synthetic data only.

use lis_engine::GetReportRevisionInput;
use lis_reporting::ReportCompleteness;

use crate::m3_fixture::{Bench, Staff, Variant};
use crate::reporting::condition_not_met;

#[tokio::test]
async fn a_validity_line_prints_its_value_units_and_outcome() {
    let bench = Bench::start(Variant::Validity).await;
    condition_not_met(bench.desk().await, bench.index).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    assert_eq!(candidate.completeness, ReportCompleteness::Final);
    let revision = bench.release(&candidate).await.unwrap();
    let detail = bench
        .lab()
        .engine()
        .get_report_revision(GetReportRevisionInput {
            revision_id: revision.revision_id,
        })
        .await
        .unwrap();
    let rows: Vec<_> = detail
        .content
        .result_groups
        .iter()
        .flat_map(|group| &group.rows)
        .collect();
    let row = |analyte: &str| {
        rows.iter()
            .find(|row| row.analyte == analyte)
            .unwrap_or_else(|| panic!("no {analyte} row"))
    };
    // The validity method defines no limit: none is invented.
    let creatinine = row("Synthetic SYN-CRE");
    assert_eq!(
        (
            creatinine.result.as_str(),
            creatinine.units.as_str(),
            creatinine.limits.as_str(),
            creatinine.comparison.as_str()
        ),
        ("120.0", "mg/dL", "None defined", "Synthetic SYN-VALID")
    );
    // The drug method defines its cutoff, which prints.
    assert_eq!(row("Synthetic SYN-MOR").limits, "Cutoff 50 ng/mL");
    assert_eq!(detail.content.validity.outcome, "Synthetic SYN-VALID");
}
