//! Pure class-based performed-service derivation (REQ-BIL-005, VAL-23).
//!
//! There is deliberately no positivity, classification, panel-name, QC or
//! payer-code field in [`Inputs`]. Those facts cannot affect class counts.

use std::collections::{BTreeMap, BTreeSet};

use lis_analytical::{ExecutionId, ExecutionState};
use lis_catalog::{AnalyteId, ComponentPurpose, MethodCertainty};
use lis_identity::{LaboratoryProfileId, SubjectId};
use lis_orders::{OrderComponentId, OrderId};
use lis_reporting::ReportRevisionId;
use lis_results::{ResultOutcome, ResultRevisionId, TechnicalReviewId};
use lis_specimens::SpecimenId;
use serde::Serialize;

mod flags;
use flags::{add_aggregate_flags, add_flag, add_global_flags};

use crate::model::{
    BillingClass, BillingClassMappingEntry, BillingClassMappingVersionId,
    BillingComponentDisposition, BillingDerivationState, BillingFlag, BillingInputKind, ClassFact,
    DerivationCompleteness, DerivationFacts, DerivedComponent, DerivedSpecimen,
};

/// One exact record reference that participates in the idempotency digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct InputRef {
    pub kind: BillingInputKind,
    pub ref_id: String,
}

/// One analytical execution included in the canonical digest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExecutionInput {
    pub execution_id: ExecutionId,
    pub state: ExecutionState,
}

/// Current technical-review state used by the pure performed rule.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReviewInput {
    None,
    Current { review_id: TechnicalReviewId },
    Stale,
}

/// Expected analyte facts. Performance requires a measured current revision
/// with a current approved review; reported membership is frozen separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AnalyteInput {
    pub analyte_id: AnalyteId,
    pub certainty: MethodCertainty,
    pub result_revision_id: Option<ResultRevisionId>,
    pub outcome: Option<ResultOutcome>,
    pub review: ReviewInput,
    pub reported_on_current_final: bool,
}

/// One frozen ordered component on one specimen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentInput {
    pub component_id: OrderComponentId,
    pub purpose: ComponentPurpose,
    pub cancelled: bool,
    pub analytes: Vec<AnalyteInput>,
    pub executions: Vec<ExecutionInput>,
}

/// One specimen grouped into a subject/date-of-service derivation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecimenInput {
    pub specimen_id: SpecimenId,
    pub order_id: OrderId,
    pub order_version: i64,
    pub report_revision_id: Option<ReportRevisionId>,
    pub report_is_current_final: bool,
    pub components: Vec<ComponentInput>,
    pub performed_not_ordered_refs: Vec<String>,
}

/// Canonical clinical facts gathered inside the engine transaction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Inputs {
    pub subject_id: SubjectId,
    pub date_of_service: Option<String>,
    pub dos_rule: String,
    pub mapping_version_id: Option<BillingClassMappingVersionId>,
    pub mapping_entries: Vec<BillingClassMappingEntry>,
    pub laboratory_profile_id: Option<LaboratoryProfileId>,
    pub laboratory_profile_version: Option<i64>,
    pub laboratory_billing_identity_complete: bool,
    /// The subject's identity revision, because the handoff carries identity.
    pub subject_revision: i64,
    pub specimens: Vec<SpecimenInput>,
    pub input_refs: Vec<InputRef>,
    /// Financial capture/reconciliation flags never change the class derivation.
    pub billing_flags: Vec<BillingFlag>,
}

/// Deterministic facts plus the digest used for idempotent persistence.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DerivationOutcome {
    pub inputs_sha256: String,
    pub facts: DerivationFacts,
    pub input_refs: Vec<InputRef>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct MutableClassFact {
    class: BillingClass,
    certainty: MethodCertainty,
    ordered: bool,
    performed: bool,
    reported: bool,
}

/// Derives classes tested, never positive results or panel-name counts.
#[must_use]
pub fn derive(inputs: &Inputs) -> DerivationOutcome {
    let mapping: BTreeMap<String, BillingClass> = inputs
        .mapping_entries
        .iter()
        .map(|entry| {
            (
                entry.analyte_id.to_string(),
                BillingClass {
                    code: entry.class_code.clone(),
                    label: entry.class_label.clone(),
                },
            )
        })
        .collect();
    let mut flags: BTreeSet<BillingFlag> = inputs.billing_flags.iter().cloned().collect();
    add_global_flags(inputs, &mut flags);
    let mut specimens = Vec::new();
    let mut components = Vec::new();
    let mut classes = Vec::new();
    for specimen in &inputs.specimens {
        derive_specimen(
            specimen,
            &mapping,
            &mut flags,
            &mut specimens,
            &mut components,
            &mut classes,
        );
    }
    add_aggregate_flags(&classes, &mut flags);
    specimens.sort_by_key(|item| item.specimen_id);
    components.sort_by_key(|item| (item.specimen_id, item.component_id));
    classes.sort_by(|left, right| class_sort_key(left).cmp(&class_sort_key(right)));
    let flags: Vec<_> = flags.into_iter().collect();
    let completeness = if inputs
        .specimens
        .iter()
        .all(|specimen| specimen.report_is_current_final)
    {
        DerivationCompleteness::Complete
    } else {
        DerivationCompleteness::AwaitingFinal
    };
    let state = if flags.iter().any(|flag| flag.blocking) {
        BillingDerivationState::Held
    } else {
        BillingDerivationState::Candidate
    };
    let facts = DerivationFacts {
        subject_id: inputs.subject_id,
        date_of_service: inputs.date_of_service.clone(),
        dos_rule: inputs.dos_rule.clone(),
        mapping_version_id: inputs.mapping_version_id,
        completeness,
        state,
        specimens,
        components,
        classes,
        flags,
    };
    let mut input_refs = inputs.input_refs.clone();
    input_refs.sort_by(|left, right| input_ref_key(left).cmp(&input_ref_key(right)));
    input_refs.dedup_by(|left, right| input_ref_key(left) == input_ref_key(right));
    let inputs_sha256 = crate::derivation_digest::digest(inputs, &facts, &input_refs);
    DerivationOutcome {
        inputs_sha256,
        facts,
        input_refs,
    }
}

fn derive_specimen(
    specimen: &SpecimenInput,
    mapping: &BTreeMap<String, BillingClass>,
    flags: &mut BTreeSet<BillingFlag>,
    specimens: &mut Vec<DerivedSpecimen>,
    components: &mut Vec<DerivedComponent>,
    classes: &mut Vec<ClassFact>,
) {
    if !specimen.report_is_current_final {
        add_flag(
            flags,
            "report_not_final",
            &specimen.specimen_id.to_string(),
            true,
        );
    }
    for reference in &specimen.performed_not_ordered_refs {
        add_flag(flags, "performed_not_ordered", reference, false);
    }
    specimens.push(DerivedSpecimen {
        specimen_id: specimen.specimen_id,
        order_id: specimen.order_id,
        order_version: specimen.order_version,
        report_revision_id: specimen.report_revision_id,
    });
    let mut specimen_classes = BTreeMap::new();
    for component in &specimen.components {
        let disposition = component_disposition(component);
        components.push(DerivedComponent {
            component_id: component.component_id,
            specimen_id: specimen.specimen_id,
            disposition,
        });
        derive_component(
            component,
            disposition,
            specimen.report_is_current_final,
            mapping,
            flags,
            &mut specimen_classes,
        );
    }
    classes.extend(specimen_classes.into_values().map(|fact| ClassFact {
        specimen_id: specimen.specimen_id,
        class: fact.class,
        certainty: fact.certainty,
        ordered: fact.ordered,
        performed: fact.performed,
        reported: fact.reported,
    }));
}

fn component_disposition(component: &ComponentInput) -> BillingComponentDisposition {
    if component.cancelled {
        return BillingComponentDisposition::Cancelled;
    }
    if component.purpose == ComponentPurpose::Validity {
        return BillingComponentDisposition::ValidityExcluded;
    }
    if component.analytes.iter().any(is_measured_and_reviewed) {
        return BillingComponentDisposition::Performed;
    }
    if !component.executions.is_empty()
        && component
            .executions
            .iter()
            .all(|execution| execution.state == ExecutionState::Failed)
    {
        return BillingComponentDisposition::FailedOnly;
    }
    BillingComponentDisposition::NotPerformed
}

fn derive_component(
    component: &ComponentInput,
    disposition: BillingComponentDisposition,
    report_is_current_final: bool,
    mapping: &BTreeMap<String, BillingClass>,
    flags: &mut BTreeSet<BillingFlag>,
    classes: &mut BTreeMap<(String, String), MutableClassFact>,
) {
    if disposition == BillingComponentDisposition::Cancelled
        || disposition == BillingComponentDisposition::ValidityExcluded
    {
        return;
    }
    if disposition == BillingComponentDisposition::FailedOnly {
        add_flag(
            flags,
            "failed_attempt_only",
            &component.component_id.to_string(),
            false,
        );
    }
    let mut component_performed = false;
    for analyte in &component.analytes {
        let Some(class) = mapping.get(&analyte.analyte_id.to_string()) else {
            add_flag(
                flags,
                "mapping_missing",
                &analyte.analyte_id.to_string(),
                true,
            );
            continue;
        };
        let key = (class.code.clone(), analyte.certainty.as_str().to_owned());
        let fact = classes.entry(key).or_insert_with(|| MutableClassFact {
            class: class.clone(),
            certainty: analyte.certainty,
            ordered: false,
            performed: false,
            reported: false,
        });
        fact.ordered = true;
        let measured = is_measured(analyte);
        let reported = measured && report_is_current_final && analyte.reported_on_current_final;
        fact.reported |= reported;
        if measured && is_currently_reviewed(analyte) {
            component_performed = true;
            fact.performed = true;
            if !reported {
                add_flag(
                    flags,
                    "performed_not_reported",
                    &analyte.analyte_id.to_string(),
                    false,
                );
            }
        }
    }
    if !component_performed && disposition == BillingComponentDisposition::NotPerformed {
        add_flag(
            flags,
            "ordered_not_performed",
            &component.component_id.to_string(),
            false,
        );
    }
}

fn class_sort_key(fact: &ClassFact) -> (SpecimenId, &str, &str) {
    (
        fact.specimen_id,
        fact.certainty.as_str(),
        fact.class.code.as_str(),
    )
}

fn input_ref_key(input: &InputRef) -> (&str, &str) {
    (input.kind.as_str(), input.ref_id.as_str())
}

fn is_measured_and_reviewed(analyte: &AnalyteInput) -> bool {
    is_measured(analyte) && is_currently_reviewed(analyte)
}

fn is_measured(analyte: &AnalyteInput) -> bool {
    matches!(
        analyte.outcome,
        Some(ResultOutcome::Quantity | ResultOutcome::Censored | ResultOutcome::Qualitative)
    )
}

fn is_currently_reviewed(analyte: &AnalyteInput) -> bool {
    matches!(analyte.review, ReviewInput::Current { .. })
}

#[cfg(test)]
mod tests {
    use super::*;
    use lis_catalog::ComponentPurpose;
    use proptest::prelude::*;

    fn analyte(id: AnalyteId, certainty: MethodCertainty, performed: bool) -> AnalyteInput {
        AnalyteInput {
            analyte_id: id,
            certainty,
            result_revision_id: performed.then(ResultRevisionId::generate),
            outcome: performed.then_some(ResultOutcome::Quantity),
            review: if performed {
                ReviewInput::Current {
                    review_id: TechnicalReviewId::generate(),
                }
            } else {
                ReviewInput::None
            },
            reported_on_current_final: performed,
        }
    }

    fn inputs(analytes: Vec<AnalyteInput>) -> Inputs {
        let specimen_id = SpecimenId::generate();
        let mapping_entries = analytes
            .iter()
            .map(|item| BillingClassMappingEntry {
                analyte_id: item.analyte_id,
                class_code: "SYN-CLASS".to_owned(),
                class_label: "Synthetic class".to_owned(),
            })
            .collect();
        Inputs {
            subject_id: SubjectId::generate(),
            date_of_service: Some("2026-10-13".to_owned()),
            dos_rule: "collection_local_date_v1".to_owned(),
            mapping_version_id: Some(BillingClassMappingVersionId::generate()),
            mapping_entries,
            laboratory_profile_id: Some(LaboratoryProfileId::generate()),
            laboratory_profile_version: Some(1),
            laboratory_billing_identity_complete: true,
            subject_revision: 1,
            billing_flags: Vec::new(),
            specimens: vec![SpecimenInput {
                specimen_id,
                order_id: OrderId::generate(),
                order_version: 1,
                report_revision_id: Some(ReportRevisionId::generate()),
                report_is_current_final: true,
                components: vec![ComponentInput {
                    component_id: OrderComponentId::generate(),
                    purpose: ComponentPurpose::Analytical,
                    cancelled: false,
                    analytes,
                    executions: vec![ExecutionInput {
                        execution_id: ExecutionId::generate(),
                        state: ExecutionState::Active,
                    }],
                }],
                performed_not_ordered_refs: Vec::new(),
            }],
            input_refs: Vec::new(),
        }
    }

    #[test]
    fn two_analytes_in_one_class_count_once() {
        let input = inputs(vec![
            analyte(AnalyteId::generate(), MethodCertainty::Definitive, true),
            analyte(AnalyteId::generate(), MethodCertainty::Definitive, true),
        ]);
        let outcome = derive(&input);
        assert_eq!(outcome.facts.classes.len(), 1);
        assert!(outcome.facts.classes[0].performed);
    }

    #[test]
    fn financial_holds_and_reconciliation_change_digest_without_changing_services() {
        let mut input = inputs(vec![analyte(
            AnalyteId::generate(),
            MethodCertainty::Definitive,
            true,
        )]);
        let original = derive(&input);
        input.billing_flags.push(BillingFlag {
            code: "billing_context_conflict".to_owned(),
            ref_id: String::new(),
            blocking: true,
        });
        let held = derive(&input);
        assert_eq!(held.facts.state, BillingDerivationState::Held);
        assert_eq!(held.facts.classes, original.facts.classes);
        assert_ne!(held.inputs_sha256, original.inputs_sha256);
        input.billing_flags[0].blocking = false;
        input.input_refs.push(InputRef {
            kind: BillingInputKind::ConflictResolution,
            ref_id: "synthetic-resolution".to_owned(),
        });
        let resolved = derive(&input);
        assert_eq!(resolved.facts.state, BillingDerivationState::Candidate);
        assert_eq!(resolved.facts.classes, original.facts.classes);
        assert_ne!(resolved.inputs_sha256, held.inputs_sha256);
    }

    #[test]
    fn an_empty_previous_day_has_no_classes_or_mapping_requirement() {
        let mut input = inputs(vec![]);
        input.specimens.clear();
        input.mapping_version_id = None;
        input.mapping_entries.clear();
        let outcome = derive(&input);
        assert!(outcome.facts.classes.is_empty());
        assert_eq!(outcome.facts.state, BillingDerivationState::Candidate);
    }

    #[test]
    fn repeats_cancellation_failure_and_validity_add_no_performed_class() {
        let mut input = inputs(vec![analyte(
            AnalyteId::generate(),
            MethodCertainty::Definitive,
            false,
        )]);
        let component = &mut input.specimens[0].components[0];
        component.executions = vec![
            ExecutionInput {
                execution_id: ExecutionId::generate(),
                state: ExecutionState::Failed,
            },
            ExecutionInput {
                execution_id: ExecutionId::generate(),
                state: ExecutionState::Failed,
            },
        ];
        let failed = derive(&input);
        assert!(failed.facts.classes.iter().all(|fact| !fact.performed));
        assert!(
            failed
                .facts
                .flags
                .iter()
                .any(|flag| flag.code == "failed_attempt_only")
        );

        input.specimens[0].components[0].cancelled = true;
        assert!(derive(&input).facts.classes.is_empty());
        input.specimens[0].components[0].cancelled = false;
        input.specimens[0].components[0].purpose = ComponentPurpose::Validity;
        assert!(derive(&input).facts.classes.is_empty());
    }

    #[test]
    fn performed_not_ordered_is_excluded_and_flagged() {
        let mut input = inputs(vec![analyte(
            AnalyteId::generate(),
            MethodCertainty::Definitive,
            true,
        )]);
        input.specimens[0]
            .performed_not_ordered_refs
            .push("synthetic-quarantine-item".to_owned());
        let outcome = derive(&input);
        assert_eq!(outcome.facts.classes.len(), 1);
        assert!(outcome.facts.flags.iter().any(|flag| {
            flag.code == "performed_not_ordered" && flag.ref_id == "synthetic-quarantine-item"
        }));
    }

    #[test]
    fn stale_review_and_non_measured_outcome_never_become_performed() {
        let mut item = analyte(AnalyteId::generate(), MethodCertainty::Definitive, true);
        item.review = ReviewInput::Stale;
        let stale = derive(&inputs(vec![item.clone()]));
        assert!(!stale.facts.classes[0].performed);
        assert!(stale.facts.classes[0].reported);
        assert!(
            stale
                .facts
                .flags
                .iter()
                .any(|flag| flag.code == "ordered_not_performed")
        );

        item.review = ReviewInput::Current {
            review_id: TechnicalReviewId::generate(),
        };
        item.outcome = Some(ResultOutcome::Invalid);
        let invalid = derive(&inputs(vec![item]));
        assert!(!invalid.facts.classes[0].performed);
        assert!(!invalid.facts.classes[0].reported);
    }

    #[test]
    fn reported_requires_a_current_final_report() {
        let mut input = inputs(vec![analyte(
            AnalyteId::generate(),
            MethodCertainty::Definitive,
            true,
        )]);
        input.specimens[0].report_is_current_final = false;
        let outcome = derive(&input);
        assert!(outcome.facts.classes[0].performed);
        assert!(!outcome.facts.classes[0].reported);
        assert_eq!(outcome.facts.state, BillingDerivationState::Held);
    }

    proptest! {
        #[test]
        fn repeats_never_change_class_membership(repeat_count in 1usize..20) {
            let mut input = inputs(vec![analyte(
                AnalyteId::generate(),
                MethodCertainty::Definitive,
                true,
            )]);
            let expected = derive(&input).facts.classes;
            input.specimens[0].components[0].executions = (0..repeat_count)
                .map(|_| ExecutionInput {
                    execution_id: ExecutionId::generate(),
                    state: ExecutionState::Active,
                })
                .collect();
            prop_assert_eq!(derive(&input).facts.classes, expected);
        }

        #[test]
        fn input_order_never_changes_digest(reverse in any::<bool>()) {
            let a = analyte(AnalyteId::generate(), MethodCertainty::Definitive, true);
            let b = analyte(AnalyteId::generate(), MethodCertainty::Definitive, true);
            let mut input = inputs(vec![a, b]);
            let original = derive(&input).inputs_sha256;
            if reverse {
                input.mapping_entries.reverse();
                input.specimens[0].components[0].analytes.reverse();
            }
            prop_assert_eq!(derive(&input).inputs_sha256, original);
        }

        #[test]
        fn number_of_measured_analytes_in_one_class_never_changes_count(
            count in 1usize..30,
        ) {
            let analytes = (0..count)
                .map(|_| analyte(AnalyteId::generate(), MethodCertainty::Presumptive, true))
                .collect();
            let outcome = derive(&inputs(analytes));
            prop_assert_eq!(outcome.facts.classes.len(), 1);
        }
    }
}
