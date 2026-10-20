//! Canonical, order-independent digest for billing derivation inputs.

use sha2::{Digest, Sha256};

use crate::derivation::{InputRef, Inputs, ReviewInput};
use crate::model::DerivationFacts;

pub(crate) fn digest(inputs: &Inputs, facts: &DerivationFacts, input_refs: &[InputRef]) -> String {
    let mut hash = Sha256::new();
    digest_text(&mut hash, "billing-input-v1");
    digest_text(&mut hash, &facts.subject_id.to_string());
    digest_text(&mut hash, facts.date_of_service.as_deref().unwrap_or(""));
    digest_text(&mut hash, &facts.dos_rule);
    digest_text(
        &mut hash,
        &facts
            .mapping_version_id
            .map_or_else(String::new, |id| id.to_string()),
    );
    digest_text(&mut hash, facts.completeness.as_str());
    digest_text(
        &mut hash,
        &inputs
            .laboratory_profile_id
            .map_or_else(String::new, |id| id.to_string()),
    );
    digest_text(
        &mut hash,
        &inputs
            .laboratory_profile_version
            .map_or_else(String::new, |version| version.to_string()),
    );
    digest_text(
        &mut hash,
        if inputs.laboratory_billing_identity_complete {
            "1"
        } else {
            "0"
        },
    );
    digest_text(&mut hash, &inputs.subject_revision.to_string());
    digest_specimens(&mut hash, inputs, facts);
    digest_classes(&mut hash, facts);
    for flag in &facts.flags {
        digest_text(&mut hash, &flag.code);
        digest_text(&mut hash, &flag.ref_id);
        digest_text(&mut hash, if flag.blocking { "1" } else { "0" });
    }
    for input in input_refs {
        digest_text(&mut hash, input.kind.as_str());
        digest_text(&mut hash, &input.ref_id);
    }
    hex::encode(hash.finalize())
}

fn digest_specimens(hash: &mut Sha256, inputs: &Inputs, facts: &DerivationFacts) {
    for specimen in &facts.specimens {
        digest_text(hash, &specimen.specimen_id.to_string());
        digest_text(hash, &specimen.order_id.to_string());
        digest_text(hash, &specimen.order_version.to_string());
        digest_text(
            hash,
            &specimen
                .report_revision_id
                .map_or_else(String::new, |id| id.to_string()),
        );
    }
    for component in &facts.components {
        digest_text(hash, &component.component_id.to_string());
        digest_text(hash, &component.specimen_id.to_string());
        digest_text(hash, component.disposition.as_str());
        let source = inputs
            .specimens
            .iter()
            .flat_map(|specimen| &specimen.components)
            .find(|source| source.component_id == component.component_id);
        if let Some(source) = source {
            digest_component_source(hash, source);
        }
    }
}

fn digest_component_source(hash: &mut Sha256, source: &crate::derivation::ComponentInput) {
    let mut executions = source.executions.clone();
    executions.sort_by_key(|execution| execution.execution_id);
    for execution in executions {
        digest_text(hash, &execution.execution_id.to_string());
        digest_text(hash, execution.state.as_str());
    }
    let mut analytes = source.analytes.clone();
    analytes.sort_by_key(|analyte| analyte.analyte_id);
    for analyte in analytes {
        digest_text(hash, &analyte.analyte_id.to_string());
        digest_text(
            hash,
            &analyte
                .result_revision_id
                .map_or_else(String::new, |id| id.to_string()),
        );
        digest_text(
            hash,
            analyte
                .outcome
                .map_or("", lis_results::ResultOutcome::as_str),
        );
        let (review_state, review_id) = match analyte.review {
            ReviewInput::None => ("none", String::new()),
            ReviewInput::Current { review_id } => ("current", review_id.to_string()),
            ReviewInput::Stale => ("stale", String::new()),
        };
        digest_text(hash, review_state);
        digest_text(hash, &review_id);
        digest_text(
            hash,
            if analyte.reported_on_current_final {
                "1"
            } else {
                "0"
            },
        );
    }
}

fn digest_classes(hash: &mut Sha256, facts: &DerivationFacts) {
    for fact in &facts.classes {
        digest_text(hash, &fact.specimen_id.to_string());
        digest_text(hash, &fact.class.code);
        digest_text(hash, &fact.class.label);
        digest_text(hash, fact.certainty.as_str());
        digest_text(hash, if fact.ordered { "1" } else { "0" });
        digest_text(hash, if fact.performed { "1" } else { "0" });
        digest_text(hash, if fact.reported { "1" } else { "0" });
    }
}

fn digest_text(hash: &mut Sha256, value: &str) {
    hash.update(value.len().to_be_bytes());
    hash.update(value.as_bytes());
}
