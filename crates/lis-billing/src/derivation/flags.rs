//! Clinical and financial readiness flags, independent of service counts.

use super::Inputs;
use crate::{BillingFlag, ClassFact};
use lis_catalog::MethodCertainty;
use lis_specimens::SpecimenId;
use std::collections::{BTreeMap, BTreeSet};

pub(super) fn add_global_flags(inputs: &Inputs, flags: &mut BTreeSet<BillingFlag>) {
    if inputs.date_of_service.is_none() {
        add_flag(flags, "date_of_service_unknown", "", true);
    }
    if inputs.mapping_version_id.is_none() && !inputs.specimens.is_empty() {
        add_flag(flags, "mapping_missing", "", true);
    }
    if !inputs.laboratory_billing_identity_complete {
        add_flag(flags, "laboratory_billing_identity_missing", "", true);
    }
}

pub(super) fn add_aggregate_flags(classes: &[ClassFact], flags: &mut BTreeSet<BillingFlag>) {
    let mut by_specimen: BTreeMap<SpecimenId, (bool, bool)> = BTreeMap::new();
    let mut presumptive_specimens = BTreeSet::new();
    for fact in classes.iter().filter(|fact| fact.performed) {
        let certainty = by_specimen.entry(fact.specimen_id).or_default();
        match fact.certainty {
            MethodCertainty::Presumptive => {
                certainty.0 = true;
                presumptive_specimens.insert(fact.specimen_id);
            }
            MethodCertainty::Definitive => certainty.1 = true,
        }
    }
    for (specimen_id, (has_presumptive, has_definitive)) in by_specimen {
        if has_presumptive && has_definitive {
            add_flag(
                flags,
                "presumptive_and_definitive_same_specimen",
                &specimen_id.to_string(),
                false,
            );
        }
    }
    if presumptive_specimens.len() > 1 {
        add_flag(flags, "multiple_presumptive_same_dos", "", false);
    }
}

pub(super) fn add_flag(
    flags: &mut BTreeSet<BillingFlag>,
    code: &str,
    ref_id: &str,
    blocking: bool,
) {
    flags.insert(BillingFlag {
        code: code.to_owned(),
        ref_id: ref_id.to_owned(),
        blocking,
    });
}
