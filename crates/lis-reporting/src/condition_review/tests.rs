use super::*;
use lis_core::UtcTimestamp;
use lis_specimens::{
    AccessionId, CollectionPrecision, ReceiptDecision, SpecimenAcceptance, SpecimenId,
};

fn specimen() -> SpecimenSection {
    let current_id = ReceiptDecisionId::generate();
    let at = UtcTimestamp::parse("2026-10-12T10:00:00.000000000Z").unwrap();
    SpecimenSection {
        specimen_id: SpecimenId::generate(),
        specimen_type_code: "syn".to_owned(),
        specimen_type_name: "Synthetic".to_owned(),
        accession_id: AccessionId::generate(),
        accession_number: "A26000001".to_owned(),
        order_item_ids: Vec::new(),
        acceptance: SpecimenAcceptance::AcceptedConditionally,
        latest_decision_id: Some(current_id),
        conditional_policy: Some("Synthetic policy".to_owned()),
        conditional_scope: Some("Original scope".to_owned()),
        discrepancy_codes: vec![ReceiptDiscrepancyCode::Other],
        condition_history: vec![
            ConditionSection {
                decision_id: ReceiptDecisionId::generate(),
                decision: ReceiptDecision::Held,
                recorded_at: at,
                code: ReceiptDiscrepancyCode::Other,
                description: Some("Internal routing note".to_owned()),
            },
            ConditionSection {
                decision_id: current_id,
                decision: ReceiptDecision::AcceptedConditionally,
                recorded_at: at,
                code: ReceiptDiscrepancyCode::Other,
                description: Some("Synthetic leakage".to_owned()),
            },
        ],
        condition_report_review: None,
        collection_precision: CollectionPrecision::Unknown,
        collected_at: None,
        utc_offset_minutes: None,
        collected_date: None,
        collector: None,
        observations: Vec::new(),
        received_at: at,
        container_labels: Vec::new(),
    }
}

fn review(specimen: &SpecimenSection, include: bool) -> ConditionReportReview {
    ConditionReportReview {
        source_sha256: source(specimen).unwrap().source_sha256,
        decisions: specimen
            .condition_history
            .iter()
            .map(|condition| ConditionReportDecision {
                decision_id: condition.decision_id,
                code: condition.code,
                include_in_report: include,
            })
            .collect(),
        reason: "Synthetic clinical relevance review".to_owned(),
    }
}

#[test]
fn explicit_selection_preserves_raw_evidence_and_mandatory_current_condition() {
    let mut specimen = specimen();
    let original = specimen.condition_history.clone();
    let selected = review(&specimen, false);
    apply(&mut specimen, Some(selected)).unwrap();
    let printed = projected(&specimen);
    assert_eq!(printed.len(), 1);
    assert_eq!(printed[0].decision, ReceiptDecision::AcceptedConditionally);
    assert_eq!(printed[0].code, ReceiptDiscrepancyCode::Other);
    assert_eq!(printed[0].description, None);
    assert_eq!(specimen.condition_history, original);
    let mut selected = review(&specimen, true);
    selected.decisions[1].include_in_report = false;
    apply(&mut specimen, Some(selected)).unwrap();
    assert_eq!(projected(&specimen)[0].description, original[0].description);
    assert_eq!(projected(&specimen)[1].description, None);
}

#[test]
fn missing_duplicate_unknown_and_stale_choices_are_typed_validation_errors() {
    let original = specimen();
    assert!(matches!(
        apply(&mut original.clone(), None),
        Err(ReportingError::Invalid(_))
    ));
    let selected = review(&original, true);
    let mut invalids = Vec::new();
    let mut missing = selected.clone();
    missing.decisions.pop();
    invalids.push(missing);
    let mut duplicate = selected.clone();
    duplicate.decisions[1] = duplicate.decisions[0].clone();
    invalids.push(duplicate);
    let mut unknown = selected.clone();
    unknown.decisions[0].decision_id = ReceiptDecisionId::generate();
    invalids.push(unknown);
    let mut stale = selected.clone();
    stale.source_sha256 = "0".repeat(64);
    invalids.push(stale);
    let mut unreasoned = selected;
    unreasoned.reason.clear();
    invalids.push(unreasoned);
    for invalid in invalids {
        assert!(matches!(
            apply(&mut original.clone(), Some(invalid)),
            Err(ReportingError::Invalid(_))
        ));
    }
}

#[test]
fn preview_excludes_unreviewed_notes_but_legacy_frozen_projection_stays_exact() {
    let mut specimen = specimen();
    assert_eq!(projected(&specimen), specimen.condition_history);
    assert_eq!(
        release_gate(&specimen).unwrap().code,
        "condition_review_required"
    );
    preview(&mut specimen);
    assert!(release_gate(&specimen).is_none());
    assert_eq!(projected(&specimen).len(), 1);
    assert_eq!(projected(&specimen)[0].description, None);
    assert_eq!(specimen.condition_history.len(), 2);
}

#[test]
fn source_digest_detects_changed_evidence_or_current_decision_even_with_equal_times() {
    let mut specimen = specimen();
    let old = review(&specimen, true);
    specimen.condition_history[0].description = Some("Corrected synthetic source".to_owned());
    assert!(apply(&mut specimen, Some(old)).is_err());
    let old = review(&specimen, true);
    specimen.latest_decision_id = Some(ReceiptDecisionId::generate());
    assert!(apply(&mut specimen, Some(old)).is_err());
}
