use super::*;
use crate::renderer::{rendered_page_count, rendered_text};

const FIXTURE: &str = include_str!("../../tests/fixtures/synthetic-catalog-preview.json");

fn template() -> TemplateRef {
    TemplateRef {
        id: "catalog-preview".to_owned(),
        version: 2,
    }
}

fn fixture() -> CatalogPreviewDocument {
    serde_json::from_str(FIXTURE).expect("fixture parses")
}

fn text_of(data: &CatalogPreviewDocument) -> String {
    // The template adds zero-width spaces inside very long words.
    rendered_text(&template(), &to_json(data).expect("serializes"))
        .expect("renders")
        .replace('\u{200B}', "")
}

fn analyte_row(index: usize) -> PreviewMethodAnalyte {
    PreviewMethodAnalyte {
        code: format!("SYN-BULK-{index}"),
        name: format!("Synthetic bulk analyte {index}"),
        unit: Some("ng/mL".to_owned()),
        cutoff: Some("50".to_owned()),
        cutoff_source: SourceLabel::StarterValue,
        lod: None,
        loq: Some("5".to_owned()),
        reportable_low: Some("5".to_owned()),
        reportable_high: Some("5000".to_owned()),
        display_scale: Some("0".to_owned()),
    }
}

#[test]
fn fixture_round_trips_through_json() {
    let data = fixture();
    let back: CatalogPreviewDocument =
        serde_json::from_str(&to_json(&data).expect("serializes")).expect("round-trips");
    assert_eq!(back, data);
}

#[test]
fn page_one_has_disclaimer_counts_gaps_and_drafts() {
    let text = text_of(&fixture());
    for expected in [
        "Startercontent:checkeveryvaluebeforeuse.",
        "Gaps:blankcutoffs",
        "Draftsawaitingreview",
        "Cutoffisblank:laboratorytosupply",
        "Thresholdruleisastartervalue:confirm",
        "SyntheticManager",
        "Generated2026-10-1309:30CDT",
        "2026-10-1314:30UTC",
        "Page1of",
        "ReportCP-SYN-0001",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
}

#[test]
fn sections_print_in_order_with_their_values() {
    let text = text_of(&fixture());
    let positions: Vec<usize> = [
        "Gapstoresolve",
        "DraftsawaitingreviewRecordtype",
        "SpecimentypesCode",
        "AnalytesCode",
        "MethodsSYN-METHOD-IA",
        "TestsandpanelsTest",
        "ValiditypoliciesSpecimen",
        "SourcelabelsThese",
        "LOINCnoticeSyntheticLOINC",
    ]
    .iter()
    .map(|heading| {
        text.find(heading)
            .unwrap_or_else(|| panic!("missing {heading}"))
    })
    .collect();
    assert!(
        positions.windows(2).all(|pair| pair[0] < pair[1]),
        "{positions:?}"
    );
    for expected in [
        "version1(Draft)",
        "version2(Ineffect)",
        "Presumptive",
        "Definitive",
        "Syntheticoralfluid",
        "Syntheticanalyteone[Presence]inUrine",
        "0000-0",
        "Qualitative",
        "Quantitative",
        "Ator above",
        "Above",
        "Beforerounding",
        "Afterrounding",
        "50.0",
        "1.5",
        "5000",
        "Analytical",
        "Validity",
        "SYN-A1,SYN-A2",
        "Syntheticqualifiertext",
        "Qualifier",
        "Hold",
    ] {
        let squashed: String = expected.split_whitespace().collect();
        assert!(text.contains(&squashed), "missing {expected}");
    }
}

#[test]
fn every_source_label_renders_with_a_text_marker() {
    let text = text_of(&fixture());
    for expected in [
        "Published:SyntheticGuideline2026(Syntheticinitialtestcutoff)",
        "CONFIRMStartervalue:confirm",
        "BLANKBlank:laboratorytosupply",
        "DEMODemovalue:notarecommendedcutoff",
        "Differsfromcurrentstartervalue",
        "Enteredbythelaboratory",
    ] {
        assert!(text.contains(expected), "missing {expected}");
    }
    // The legend at the end names every label.
    let legend = &text[text.find("Sourcelabels").expect("legend")..];
    for expected in [
        "Published:",
        "Startervalue:confirm",
        "Blank:laboratorytosupply",
        "Demovalue:notarecommendedcutoff",
        "Differsfromcurrentstartervalue",
        "Enteredbythelaboratory",
    ] {
        assert!(legend.contains(expected), "legend missing {expected}");
    }
}

#[test]
fn empty_and_optional_sections_render() {
    let mut data = fixture();
    data.disclaimer = None;
    data.loinc_notice = None;
    data.gaps.clear();
    data.drafts.clear();
    data.specimen_types.clear();
    data.analytes.clear();
    data.methods.clear();
    data.orderables.clear();
    data.validity_policies.clear();
    let text = text_of(&data);
    assert!(!text.contains("Startercontent"));
    assert!(!text.contains("LOINCnotice"));
    assert!(text.matches("None.").count() >= 7, "{text}");
    assert!(text.contains("Sourcelabels"));
}

#[test]
fn null_limits_print_as_words() {
    let text = text_of(&fixture());
    assert!(text.contains("blank"));
    assert!(text.contains("BLANK"));
}

// Catalog names come from users: markup must print as text.
#[test]
fn hostile_text_is_escaped() {
    let hostile = r#"#panic() *bold* _em_ $x$ <label> @ref #import "@preview/x:0.1.0" ]["#;
    let long = "L".repeat(400);
    let mut data = fixture();
    data.laboratory_name = hostile.to_owned();
    data.report.generated_by = hostile.to_owned();
    data.specimen_types[0].name = hostile.to_owned();
    data.analytes[0].name = long.clone();
    data.methods[0].name = hostile.to_owned();
    data.methods[0].versions[0].analytes[1].name = long.clone();
    data.orderables[0].name = hostile.to_owned();
    data.validity_policies[0].versions[0].categories[0].label = hostile.to_owned();
    data.validity_policies[0].versions[0].categories[0].qualifier_text = Some(hostile.to_owned());
    data.loinc_notice = Some(hostile.to_owned());
    data.disclaimer = Some(hostile.to_owned());
    data.gaps[0].record = hostile.to_owned();
    data.methods[0].versions[0].threshold_rule.source = SourceLabel::Published {
        title: hostile.to_owned(),
        label: hostile.to_owned(),
    };
    let text = text_of(&data);
    let expected: String = hostile.split_whitespace().collect();
    assert!(
        text.matches(&expected).count() >= 10,
        "{}",
        text.matches(&expected).count()
    );
    assert!(text.contains(&long));
}

#[test]
fn three_hundred_analyte_rows_page() {
    let mut data = fixture();
    data.methods[0].versions[0].analytes = (0..300).map(analyte_row).collect();
    let json = to_json(&data).expect("serializes");
    let pages = rendered_page_count(&template(), &json).expect("renders");
    assert!((8..=40).contains(&pages), "{pages} pages");
    let text = rendered_text(&template(), &json).expect("renders");
    assert!(text.contains("SYN-BULK-0"));
    assert!(text.contains("SYN-BULK-299"));
    // The header row repeats on every page the table covers.
    assert!(text.matches("Analytecode").count() >= pages - 3);
}

#[test]
fn cutoff_must_agree_with_its_source() {
    let mut data = fixture();
    data.methods[0].versions[0].analytes[0].cutoff = Some("1".to_owned());
    let error = render_json(
        &to_json(&data).expect("serializes"),
        JSON_TIME,
        Marking::Unmarked,
    )
    .unwrap_err();
    assert!(matches!(error, RenderError::Compile { .. }), "{error:?}");
    let mut data = fixture();
    data.methods[0].versions[0].analytes[1].cutoff_source = SourceLabel::Blank;
    assert!(
        render_json(
            &to_json(&data).expect("serializes"),
            JSON_TIME,
            Marking::Unmarked
        )
        .is_err()
    );
}

#[test]
fn unknown_codes_and_empty_required_values_fail_without_echoing_data() {
    let mut json: serde_json::Value = serde_json::from_str(FIXTURE).expect("parses");
    json["methods"][0]["versions"][0]["certainty"] = "CANARY-UNKNOWN".into();
    let error = render_json(&json.to_string(), JSON_TIME, Marking::Unmarked).unwrap_err();
    assert!(matches!(error, RenderError::Compile { .. }));
    assert!(!format!("{error:?}").contains("CANARY"));
    let mut data = fixture();
    data.report.id = String::new();
    assert!(
        render_json(
            &to_json(&data).expect("serializes"),
            JSON_TIME,
            Marking::Unmarked
        )
        .is_err()
    );
}

/// 2026-10-13T14:30:00Z.
const JSON_TIME: jiff::Timestamp = jiff::Timestamp::constant(1_791_901_800, 0);
