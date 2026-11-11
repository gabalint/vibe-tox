//! Report content rules on synthetic snapshots (brief §3.2, §3.3, §4.1;
//! REQ-RPT-001, REQ-RPT-002, REQ-RPT-005, REQ-RPT-008): relationship to the
//! current revision, the printed data, and that the pinned template renders
//! it. Expected outcomes come from the brief.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_catalog::{
    AnalyteId, ComponentPurpose, MethodCertainty, OrderableId, OutputScale, ValidityAction,
    ValidityPolicyVersionId,
};
use lis_core::{Comparator, DecimalText, UtcTimestamp};
use lis_identity::{LaboratoryProfileId, SubjectId};
use lis_orders::{MedicationSource, OrderComponentId, OrderId, OrderItemId, SequenceRole};
use lis_report::tox_v2::{ToxOtherTestStatus, to_json};
use lis_reporting::Manifest;
use lis_reporting::gates::content_gates;
use lis_reporting::relationship::relationship;
use lis_reporting::report_data::{Header, tox_report_data};
use lis_reporting::snapshot::{
    AnalyteLine, ComponentLine, LaboratorySection, MedicationSection, OrderSection, ReportSnapshot,
    ResultSection, SpecimenSection, SubjectSection, TriggerLine, ValiditySection,
};
use lis_reporting::{LineDisposition, ReportRelationship, ReportingError};
use lis_results::{ResultId, ResultOutcome, ResultRevisionId, ValidityAssessmentId};
use lis_specimens::{AccessionId, CollectionPrecision, SpecimenAcceptance, SpecimenId};

fn at(text: &str) -> UtcTimestamp {
    UtcTimestamp::parse(text).unwrap()
}

fn dec(text: &str) -> Option<DecimalText> {
    Some(DecimalText::parse(text).unwrap())
}

fn analyte(name: &str, magnitude: &str, comparator: Option<Comparator>) -> AnalyteLine {
    AnalyteLine {
        analyte_id: AnalyteId::generate(),
        name: name.to_owned(),
        unit: Some("ng/mL".to_owned()),
        cutoff: dec("50"),
        limit_of_detection: None,
        limit_of_quantitation: dec("5"),
        reference_interval: None,
        result: Some(ResultSection {
            result_id: ResultId::generate(),
            revision_id: ResultRevisionId::generate(),
            revision_no: 1,
            outcome: if comparator.is_some() {
                ResultOutcome::Censored
            } else {
                ResultOutcome::Quantity
            },
            comparator,
            magnitude: dec(magnitude),
            units_text: Some("ng/mL".to_owned()),
            code_text: None,
            classification: "meets_cutoff".to_owned(),
            recorded_at: Some(at("2026-10-12T14:00:00.000000000Z")),
            first_reported_at: None,
            performing_sites: Vec::new(),
            performing_site_unknown: true,
        }),
    }
}

/// A synthetic specimen with one reported definitive component and one
/// reflex awaiting its trigger decision: a partial report.
fn specimen_context() -> SpecimenSection {
    SpecimenSection {
        specimen_id: SpecimenId::generate(),
        specimen_type_code: "SYN-UR".to_owned(),
        specimen_type_name: "Synthetic urine".to_owned(),
        accession_id: AccessionId::generate(),
        accession_number: "A26000001".to_owned(),
        order_item_ids: vec![OrderItemId::generate()],
        acceptance: SpecimenAcceptance::Accepted,
        latest_decision_id: None,
        conditional_policy: None,
        conditional_scope: None,
        discrepancy_codes: Vec::new(),
        condition_history: Vec::new(),
        condition_report_review: None,
        collection_precision: CollectionPrecision::Datetime,
        collected_at: Some(at("2026-10-12T07:30:00.000000000Z")),
        utc_offset_minutes: Some(-240),
        collected_date: None,
        collector: Some("Synthetic Collector".to_owned()),
        observations: Vec::new(),
        received_at: at("2026-10-12T12:00:00.000000000Z"),
        container_labels: vec!["C00000001".to_owned()],
    }
}

fn snapshot() -> ReportSnapshot {
    let mut snapshot = ReportSnapshot {
        specimen: specimen_context(),
        subject: SubjectSection {
            subject_id: SubjectId::generate(),
            revision: 1,
            family_name: "SYNTHETIC".to_owned(),
            given_names: Some("Patient Mu".to_owned()),
            birth_date: "1980-01-01".to_owned(),
            sex: None,
            identifiers: Vec::new(),
        },
        order: OrderSection {
            order_id: OrderId::generate(),
            version: 1,
            placer_order_id: None,
            practitioner_name: "Synthetic, Orderer".to_owned(),
            practitioner_npi: "1234567893".to_owned(),
            client_name: "Synthetic Clinic".to_owned(),
            location_name: None,
            medications: vec![MedicationSection {
                name: "Synthetic medication A".to_owned(),
                source: MedicationSource::OrderingClinic,
                as_of: None,
            }],
        },
        lines: vec![ComponentLine {
            component_id: OrderComponentId::generate(),
            item_id: OrderItemId::generate(),
            orderable_id: OrderableId::generate(),
            orderable_name: "Synthetic opioid confirmation".to_owned(),
            method_name: "Synthetic LC-MS/MS".to_owned(),
            method_version: 3,
            report_notes: None,
            certainty: MethodCertainty::Definitive,
            output_scale: OutputScale::Quantitative,
            sequence_role: SequenceRole::Confirmation,
            purpose: ComponentPurpose::Analytical,
            disposition: LineDisposition::Reported,
            analytes: vec![
                analyte("Synthetic analyte 1", "152.40", None),
                analyte("Synthetic analyte 2", "0.50", Some(Comparator::Lt)),
            ],
        }],
        triggers: vec![TriggerLine {
            parent_item_id: OrderItemId::generate(),
            orderable_id: OrderableId::generate(),
            orderable_name: "Synthetic reflex".to_owned(),
            disposition: LineDisposition::TriggerPending,
        }],
        validity: None,
        laboratory: LaboratorySection {
            profile_id: LaboratoryProfileId::generate(),
            version: 1,
            name: "Synthetic Toxicology Laboratory".to_owned(),
            address_lines: vec!["100 Example Way".to_owned()],
            clia_id: "00D0000000".to_owned(),
            director: "Dr. Example Director".to_owned(),
            phone: "555-0100".to_owned(),
            time_zone: "America/Chicago".to_owned(),
        },
    };
    for result in snapshot
        .lines
        .iter_mut()
        .flat_map(|line| &mut line.analytes)
        .filter_map(|a| a.result.as_mut())
    {
        result.performing_sites = vec![snapshot.laboratory.clone()];
        result.performing_site_unknown = false;
    }
    snapshot
}

fn header(relationship: ReportRelationship) -> Header {
    let follows = relationship != ReportRelationship::Initial;
    Header {
        change_summary: Vec::new(),
        report_number: "A26000001-SYN-UR".to_owned(),
        revision_no: if follows { 2 } else { 1 },
        relationship,
        supersedes: follows.then(|| (1, at("2026-10-12T13:00:00.000000000Z"))),
        amendment_reason: (relationship == ReportRelationship::Correction)
            .then(|| "Synthetic correction".to_owned()),
        prepared_at: at("2026-10-12T14:30:00.000000000Z"),
        released_by: "Synthetic Releaser".to_owned(),
    }
}

#[test]
fn relationships_follow_what_the_report_prints() {
    let current = snapshot();
    assert_eq!(
        relationship(None, &current).unwrap(),
        ReportRelationship::Initial
    );
    assert!(matches!(
        relationship(Some(&current), &current),
        Err(ReportingError::NothingChanged)
    ));
    // Provenance alone prints nothing new.
    let mut provenance = current.clone();
    provenance.order.version = 2;
    provenance.subject.revision = 3;
    assert!(matches!(
        relationship(Some(&current), &provenance),
        Err(ReportingError::NothingChanged)
    ));
    // A reflex resolved without changing reported lines: supplement.
    let mut resolved = current.clone();
    resolved.triggers[0].disposition = LineDisposition::NotActivated;
    assert_eq!(
        relationship(Some(&current), &resolved).unwrap(),
        ReportRelationship::Supplement
    );
    // A reported value changed: correction.
    let mut changed = current.clone();
    changed.lines[0].analytes[0]
        .result
        .as_mut()
        .unwrap()
        .magnitude = dec("148.90");
    assert_eq!(
        relationship(Some(&current), &changed).unwrap(),
        ReportRelationship::Correction
    );
    // A reported line is held again: correction.
    let mut held = current.clone();
    held.lines[0].disposition = LineDisposition::Held;
    assert_eq!(
        relationship(Some(&current), &held).unwrap(),
        ReportRelationship::Correction
    );
    // The identity changed: correction.
    let mut renamed = current.clone();
    renamed.subject.family_name = "SYNTHETIC-RENAMED".to_owned();
    assert_eq!(
        relationship(Some(&current), &renamed).unwrap(),
        ReportRelationship::Correction
    );
}

// REQ-RPT-008, L7: exact values, engine wording, tests not reported.
#[test]
fn printed_data_keeps_exact_values_and_lists_tests_not_reported() {
    let data = tox_report_data(&snapshot(), &header(ReportRelationship::Initial)).unwrap();
    assert_eq!(data.report.issued_at, "2026-10-12 09:30 CDT");
    let rows = &data.result_groups[0].rows;
    assert_eq!(rows[0].result, "152.40");
    assert_eq!(rows[1].result, "<0.50");
    assert_eq!(rows[0].limits, "Cutoff 50 ng/mL; LOQ 5 ng/mL");
    assert_eq!(rows[0].comparison, "Meets cutoff");
    assert_eq!(
        data.other_tests[0].status,
        ToxOtherTestStatus::TriggerPending
    );
    assert_eq!(
        data.specimen.collected.as_deref(),
        Some("2026-10-12 03:30 (UTC-04:00)")
    );
    assert_eq!(data.subject.name, "SYNTHETIC, Patient Mu");
}

#[test]
fn a_reported_analyte_without_any_limit_is_a_gate() {
    let mut missing = snapshot();
    let analyte = &mut missing.lines[0].analytes[0];
    analyte.cutoff = None;
    analyte.limit_of_quantitation = None;
    match tox_report_data(&missing, &header(ReportRelationship::Initial)) {
        Err(ReportingError::GateBlocked(gates)) => assert_eq!(gates[0].code, "limit_missing"),
        other => panic!("expected limit_missing, got {other:?}"),
    }
}

/// The synthetic snapshot plus a reported validity component whose method
/// version defines no limit, assessed with a qualifier.
fn with_validity_line() -> ReportSnapshot {
    let mut snapshot = snapshot();
    let mut creatinine = analyte("Synthetic creatinine", "18.2", None);
    creatinine.result.as_mut().unwrap().performing_sites = vec![snapshot.laboratory.clone()];
    creatinine.result.as_mut().unwrap().performing_site_unknown = false;
    creatinine.unit = Some("mg/dL".to_owned());
    creatinine.cutoff = None;
    creatinine.limit_of_quantitation = None;
    let result = creatinine.result.as_mut().unwrap();
    result.units_text = Some("mg/dL".to_owned());
    result.classification = "not_applicable".to_owned();
    let mut line = snapshot.lines[0].clone();
    line.orderable_name = "Synthetic validity panel".to_owned();
    line.purpose = ComponentPurpose::Validity;
    line.analytes = vec![creatinine];
    snapshot.lines.push(line);
    snapshot.validity = Some(ValiditySection {
        assessment_id: ValidityAssessmentId::generate(),
        policy_version_id: ValidityPolicyVersionId::generate(),
        category_code: "dilute".to_owned(),
        label: "Dilute".to_owned(),
        action: ValidityAction::Qualifier,
        qualifier_text: Some("Synthetic dilute-specimen qualifier".to_owned()),
    });
    snapshot
}

// Coordinator decision 2026-10-13: a validity line needs no limit, prints its
// measured value and units, the current assessment and qualifier, and no
// invented range; analytical lines still need one.
#[test]
fn a_validity_line_prints_the_assessment_and_no_invented_limits() {
    let snapshot = with_validity_line();
    let data = tox_report_data(&snapshot, &header(ReportRelationship::Initial)).unwrap();
    let row = &data.result_groups[1].rows[0];
    assert_eq!(
        (
            row.result.as_str(),
            row.units.as_str(),
            row.limits.as_str(),
            row.comparison.as_str()
        ),
        (
            "18.2",
            "mg/dL",
            "None defined",
            "Dilute: Synthetic dilute-specimen qualifier"
        )
    );
    let json = to_json(&data).unwrap();
    let prepared_at = at("2026-10-12T14:30:00.000000000Z");
    assert!(
        lis_reporting::render::render_report(
            &lis_reporting::layouts::ReportLayout::DEFAULT.template(),
            &json,
            prepared_at,
            lis_report::Marking::Unmarked
        )
        .is_ok()
    );
    let mut defined = snapshot;
    defined.lines[1].analytes[0].limit_of_quantitation = dec("1.0");
    let data = tox_report_data(&defined, &header(ReportRelationship::Initial)).unwrap();
    assert_eq!(data.result_groups[1].rows[0].limits, "LOQ 1.0 mg/dL");
}

// REQ-RPT-008: `limit_missing` holds analytical lines only.
#[test]
fn only_analytical_lines_need_a_limit_to_be_prepared() {
    let snapshot = with_validity_line();
    let manifest = Manifest {
        specimen_id: snapshot.specimen.specimen_id,
        accession_id: snapshot.specimen.accession_id,
        order_id: snapshot.order.order_id,
        components: Vec::new(),
        triggers: Vec::new(),
        shared_items: Vec::new(),
    };
    assert!(content_gates(&snapshot, &manifest).is_empty());
    let mut analytical = snapshot;
    analytical.lines[1].purpose = ComponentPurpose::Analytical;
    let codes: Vec<String> = content_gates(&analytical, &manifest)
        .into_iter()
        .map(|g| g.code)
        .collect();
    assert_eq!(codes, ["limit_missing"]);
}

// NFR-07, REQ-RPT-002: the pinned template accepts the printed data, a
// correction carries its reason, and identical input gives identical bytes.
#[test]
fn the_pinned_template_renders_the_printed_data_deterministically() {
    for relationship in [ReportRelationship::Initial, ReportRelationship::Correction] {
        let data = tox_report_data(&snapshot(), &header(relationship)).unwrap();
        let json = to_json(&data).unwrap();
        // Brief §4.4: the issued file carries no delivery status.
        assert!(!json.to_lowercase().contains("deliver"));
        let prepared_at = at("2026-10-12T14:30:00.000000000Z");
        let first = lis_reporting::render::render_report(
            &lis_reporting::layouts::ReportLayout::DEFAULT.template(),
            &json,
            prepared_at,
            lis_report::Marking::Unmarked,
        )
        .unwrap();
        let second = lis_reporting::render::render_report(
            &lis_reporting::layouts::ReportLayout::DEFAULT.template(),
            &json,
            prepared_at,
            lis_report::Marking::Unmarked,
        )
        .unwrap();
        assert!(first.bytes.starts_with(b"%PDF"));
        assert_eq!(first.bytes, second.bytes);
        assert_eq!(first.template.version, 3);
    }
}

#[test]
fn legacy_frozen_receipt_descriptions_keep_their_original_report_mapping() {
    let mut snapshot = snapshot();
    snapshot
        .specimen
        .condition_history
        .push(lis_reporting::snapshot::ConditionSection {
            decision_id: lis_specimens::ReceiptDecisionId::generate(),
            decision: lis_specimens::ReceiptDecision::Held,
            recorded_at: at("2026-10-12T12:00:00.000000000Z"),
            code: lis_specimens::ReceiptDiscrepancyCode::Other,
            description: Some("Synthetic seal leakage observed".to_owned()),
        });
    let data = tox_report_data(&snapshot, &header(ReportRelationship::Initial)).unwrap();
    assert!(
        data.specimen
            .condition_notes
            .iter()
            .any(|n| n.contains("Synthetic seal leakage observed") && n.contains("held"))
    );
}

#[test]
fn correction_summary_identifies_changed_analyte_and_context() {
    let previous = snapshot();
    let mut next = previous.clone();
    let name = next.lines[0].analytes[0].name.clone();
    next.lines[0].analytes[0].result.as_mut().unwrap().magnitude = dec("91.230");
    next.subject.family_name = "CORRECTED SYNTHETIC".to_owned();
    let changes = lis_reporting::relationship::change_summary(Some(&previous), &next);
    assert!(changes.iter().any(|change| change.contains(&name)));
    assert!(changes.iter().any(|change| change == "Patient identity"));
    assert_ne!(
        previous.lines[0].analytes[0].result,
        next.lines[0].analytes[0].result
    );
}

#[test]
fn cumulative_rows_retain_distinct_result_recorded_dates() {
    let mut snapshot = snapshot();
    snapshot.lines[0].analytes[1]
        .result
        .as_mut()
        .unwrap()
        .recorded_at = Some(at("2026-10-13T14:00:00.000000000Z"));
    let data = tox_report_data(&snapshot, &header(ReportRelationship::Initial)).unwrap();
    let rows = &data.result_groups[0].rows;
    assert_ne!(rows[0].recorded_at, rows[1].recorded_at);
    assert!(rows[0].recorded_at.as_ref().unwrap().contains("2026-10-12"));
    assert!(rows[1].recorded_at.as_ref().unwrap().contains("2026-10-13"));
}

#[test]
fn unresolved_performing_site_blocks_new_report_but_not_an_explicit_unperformed_result() {
    let mut snapshot = snapshot();
    let manifest = Manifest {
        specimen_id: snapshot.specimen.specimen_id,
        accession_id: snapshot.specimen.accession_id,
        order_id: snapshot.order.order_id,
        components: vec![],
        triggers: vec![],
        shared_items: vec![],
    };
    let result = snapshot.lines[0].analytes[0].result.as_mut().unwrap();
    result.performing_sites.clear();
    result.performing_site_unknown = true;
    assert!(
        content_gates(&snapshot, &manifest)
            .iter()
            .any(|gate| gate.code == "performing_site_unresolved")
    );
    snapshot.lines[0].analytes[0]
        .result
        .as_mut()
        .unwrap()
        .outcome = lis_results::ResultOutcome::NotPerformed;
    assert!(
        !content_gates(&snapshot, &manifest)
            .iter()
            .any(|gate| gate.code == "performing_site_unresolved")
    );
}

#[test]
fn approved_reference_information_is_distinct_from_exact_cutoff_and_method_notes() {
    let mut snapshot = snapshot();
    let before = tox_report_data(&snapshot, &header(ReportRelationship::Initial)).unwrap();
    snapshot.lines[0].report_notes =
        Some("Synthetic approved limitation — not clinical advice".into());
    snapshot.lines[0].analytes[0].reference_interval =
        Some("Synthetic reference supplied separately".into());
    let data = tox_report_data(&snapshot, &header(ReportRelationship::Initial)).unwrap();
    assert_eq!(
        data.result_groups[0].report_notes,
        snapshot.lines[0].report_notes
    );
    assert_eq!(
        data.result_groups[0].rows[0].reference_interval,
        snapshot.lines[0].analytes[0].reference_interval
    );
    assert_eq!(
        data.result_groups[0].rows[0].limits,
        before.result_groups[0].rows[0].limits
    );
}

#[test]
fn reportability_changes_are_corrections_but_internal_rationale_is_not_printed_content() {
    let mut first = snapshot();
    let id = lis_specimens::ReceiptDecisionId::generate();
    first.specimen.latest_decision_id = Some(id);
    first
        .specimen
        .condition_history
        .push(lis_reporting::snapshot::ConditionSection {
            decision_id: id,
            decision: lis_specimens::ReceiptDecision::AcceptedConditionally,
            recorded_at: at("2026-10-12T12:00:00.000000000Z"),
            code: lis_specimens::ReceiptDiscrepancyCode::Other,
            description: Some("Synthetic reportable leakage".to_owned()),
        });
    first.specimen.condition_report_review = Some(lis_reporting::ConditionReportReview {
        source_sha256: "a".repeat(64),
        reason: "Internal rationale".to_owned(),
        decisions: vec![lis_reporting::ConditionReportDecision {
            decision_id: id,
            code: lis_specimens::ReceiptDiscrepancyCode::Other,
            include_in_report: false,
        }],
    });
    let mut next = first.clone();
    next.specimen
        .condition_report_review
        .as_mut()
        .unwrap()
        .reason = "Changed internal rationale".to_owned();
    assert!(matches!(
        relationship(Some(&first), &next),
        Err(ReportingError::NothingChanged)
    ));
    next.specimen
        .condition_report_review
        .as_mut()
        .unwrap()
        .decisions[0]
        .include_in_report = true;
    assert_eq!(
        relationship(Some(&first), &next).unwrap(),
        ReportRelationship::Correction
    );
    let data = tox_report_data(&next, &header(ReportRelationship::Correction)).unwrap();
    assert!(
        data.specimen
            .condition_notes
            .iter()
            .any(|note| note.contains("reportable leakage"))
    );
    assert!(!to_json(&data).unwrap().contains("internal rationale"));
}
