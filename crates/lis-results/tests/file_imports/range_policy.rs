//! DM-04: approved opt-in range policy gates review without changing source quantities.
use crate::rows::{current, row};
use crate::*;
use lis_catalog::RangePolicy;
use lis_results::{RecordResultDispositionInput, ResultOutcome};

fn configured(policy: RangePolicy) -> Fixture {
    let mut f = fixture();
    let id = f.method;
    let old = f.read(move |conn| Ok(lis_catalog::get_method_version(conn, id)?));
    let mut analytes: Vec<_> = old.analytes.into_iter().map(|a| a.limits).collect();
    let mor = analytes
        .iter_mut()
        .find(|a| a.analyte_id == f.analytes.mor)
        .unwrap();
    mor.reportable_low = Some(DecimalText::parse("10.00").unwrap());
    mor.reportable_high = Some(DecimalText::parse("100.00").unwrap());
    mor.range_policy = policy;
    let method_id = f.setup(|ctx| lis_catalog::create_method(ctx, coded("syn-range-method")));
    let method = f
        .setup(move |ctx| {
            lis_catalog::draft_method_version(
                ctx,
                DraftMethodVersion {
                    method_id,
                    content: MethodVersionContent {
                        specimen_type_id: old.specimen_type_id,
                        certainty: old.certainty,
                        output_scale: old.output_scale,
                        threshold_rule: old.threshold_rule,
                        threshold_rounding: old.threshold_rounding,
                        report_notes: None,
                        analytes,
                    },
                },
            )
        })
        .id;
    f.setup(move |ctx| {
        lis_catalog::approve_method_version(
            ctx,
            ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id: method,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    f.close_run(f.run);
    f.method = method;
    f.panel = orderable(
        &f.t,
        "syn-range-panel",
        method,
        vec![f.analytes.mor, f.analytes.oxy],
    );
    f.run = f.open_run(false);
    f
}

fn range_blocked(f: &Fixture, sample: &Sample) -> bool {
    let revision = current(&f.results(sample), "SYN-MOR").revision_id;
    f.read(move |conn| lis_results::review_gates(conn, revision))
        .iter()
        .any(|gate| gate.code == "reportable_range_disposition_required")
}

#[test]
fn method_range_policy_is_opt_in_and_uses_the_pinned_approved_version() {
    for policy in [RangePolicy::ReportOnly, RangePolicy::RequireWithinRange] {
        let f = configured(policy);
        for (text, outside) in [
            ("9.999", true),
            ("10.000", false),
            ("100", false),
            ("100.001", true),
        ] {
            let sample = f.sample();
            f.import(&[row(&sample.label, "Unknown", "Morphine", text, UNIT, "1")]);
            assert_eq!(
                range_blocked(&f, &sample),
                policy == RangePolicy::RequireWithinRange && outside
            );
            assert_eq!(
                current(&f.results(&sample), "SYN-MOR").magnitude.as_deref(),
                Some(text)
            );
            assert_eq!(
                f.measurements(&sample, f.analytes.mor)[0].1.as_deref(),
                Some(text)
            );
        }
    }
}

#[test]
fn explicit_nonnumeric_disposition_resolves_range_gate_and_retains_observation() {
    let f = configured(RangePolicy::RequireWithinRange);
    let sample = f.sample();
    f.import(&[row(
        &sample.label,
        "Unknown",
        "Morphine",
        "100.001",
        UNIT,
        "1",
    )]);
    assert!(range_blocked(&f, &sample));
    let line = f
        .results(&sample)
        .lines
        .into_iter()
        .find(|line| line.analyte_id == f.analytes.mor)
        .unwrap();
    let disposition = RecordResultDispositionInput {
        result_id: Some(line.result_id),
        expected_revision: line.revision,
        component_id: None,
        analyte_id: None,
        specimen_id: None,
        outcome: ResultOutcome::Invalid,
        reason: "Synthetic approved nonnumeric disposition after range investigation".to_owned(),
    };
    f.setup(move |ctx| lis_results::record_result_disposition(ctx, disposition));
    assert!(!range_blocked(&f, &sample));
    assert_eq!(
        f.measurements(&sample, f.analytes.mor)[0].1.as_deref(),
        Some("100.001")
    );
}
