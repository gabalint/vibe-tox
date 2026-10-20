//! The derivation read model returned by `derive_services`,
//! `approve_service_derivation` and `get_service_derivation` (brief §7). It
//! carries subject identity, so the engine audits every read of it.

use std::collections::{BTreeMap, BTreeSet};

use lis_catalog::MethodCertainty;
use lis_identity::SubjectSummary;
use lis_orders::{OrderComponentId, OrderId};
use lis_reporting::ReportRevisionId;
use lis_specimens::SpecimenId;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::derivation_repo::{self as repo, DerivationRow};
use crate::derivations::is_stale;
use crate::error::BillingError;
use crate::mapping_support::load_summary;
use crate::model::{
    BillingClassMappingVersionId, BillingComponentDisposition, BillingDerivationId,
    BillingDerivationState, BillingFlag, DerivationCompleteness,
};

/// One derivation with its frozen facts and whether its inputs changed since.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DerivationView {
    pub derivation_id: BillingDerivationId,
    #[ts(type = "number")]
    pub derivation_no: i64,
    /// The `expectedRevision` of `approve_service_derivation`.
    #[ts(type = "number")]
    pub revision: i64,
    pub state: BillingDerivationState,
    pub completeness: DerivationCompleteness,
    pub subject: SubjectSummary,
    /// `YYYY-MM-DD`; `null` when the collection date is unknown.
    pub date_of_service: Option<String>,
    pub dos_rule: String,
    pub mapping_version: Option<MappingVersionRef>,
    pub specimens: Vec<DerivationSpecimenView>,
    pub flags: Vec<BillingFlag>,
    /// Current owner-provided labels keyed by the stable flag reference ID.
    #[ts(optional)]
    pub flag_reference_labels: Option<BTreeMap<String, String>>,
    pub adjustment_of: Option<DerivationRef>,
    /// The inputs rebuilt now differ from the frozen ones.
    pub stale: bool,
    #[ts(optional)]
    pub context_resolution: Option<crate::reconciliation::BillingContextResolution>,
    pub allowed_actions: Vec<String>,
}

/// The mapping version a derivation used.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MappingVersionRef {
    pub version_id: BillingClassMappingVersionId,
    #[ts(type = "number")]
    pub version: i64,
}

/// The derivation an adjustment replaces.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DerivationRef {
    pub derivation_id: BillingDerivationId,
    #[ts(type = "number")]
    pub derivation_no: i64,
}

/// One specimen's lineage, classes and component dispositions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DerivationSpecimenView {
    pub specimen_id: SpecimenId,
    pub accession_number: String,
    pub order_id: OrderId,
    #[ts(type = "number")]
    pub order_version: i64,
    pub report_revision_id: Option<ReportRevisionId>,
    pub classes: Vec<DerivationClassView>,
    pub components: Vec<DerivationComponentView>,
}

/// Class membership for one certainty: classes tested, never positives.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DerivationClassView {
    pub class_code: String,
    pub class_label: String,
    pub certainty: MethodCertainty,
    pub ordered: bool,
    pub performed: bool,
    pub reported: bool,
}

/// One ordered component and how it counted.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DerivationComponentView {
    pub component_id: OrderComponentId,
    /// The orderable's name in the frozen version.
    pub orderable: String,
    pub disposition: BillingComponentDisposition,
}

/// The derivation `id` as it is now. Usable inside a transaction.
pub fn get_service_derivation(
    conn: &Connection,
    id: BillingDerivationId,
) -> Result<DerivationView, BillingError> {
    let row = repo::load(conn, id)?;
    let stale = is_stale(conn, &row)?;
    let mapping_version = match row.mapping_version_id {
        Some(version_id) => Some(MappingVersionRef {
            version_id,
            version: load_summary(conn, version_id)?.version,
        }),
        None => None,
    };
    let adjustment_of = match row.adjustment_of_id {
        Some(original) => Some(DerivationRef {
            derivation_id: original,
            derivation_no: repo::load(conn, original)?.derivation_no,
        }),
        None => None,
    };
    let specimens = specimen_views(conn, &row)?;
    let flags = repo::flags(conn, row.id)?;
    let flag_reference_labels = flag_reference_labels(conn, &flags, &specimens)?;
    let mut actions = allowed_actions(row.state, stale);
    if !stale
        && flags
            .iter()
            .any(|flag| flag.code == "billing_context_conflict" && flag.blocking)
    {
        actions.push("resolve_billing_context_conflict".to_owned());
    }
    Ok(DerivationView {
        derivation_id: row.id,
        derivation_no: row.derivation_no,
        revision: row.revision,
        state: row.state,
        completeness: row.completeness,
        subject: lis_identity::subject_summary(conn, row.subject_id)?,
        date_of_service: row.date_of_service.clone(),
        dos_rule: row.dos_rule.clone(),
        mapping_version,
        specimens,
        flags,
        flag_reference_labels: Some(flag_reference_labels),
        adjustment_of,
        allowed_actions: actions,
        context_resolution: crate::reconciliation::resolution_for(conn, row.id)?,
        stale,
    })
}

fn flag_reference_labels(
    conn: &Connection,
    flags: &[BillingFlag],
    specimens: &[DerivationSpecimenView],
) -> Result<BTreeMap<String, String>, BillingError> {
    let analytes: BTreeMap<String, String> = lis_catalog::list_analytes(conn)?
        .into_iter()
        .map(|analyte| (analyte.id.to_string(), analyte.name))
        .collect();
    let specimen_labels: BTreeMap<String, String> = specimens
        .iter()
        .map(|specimen| {
            (
                specimen.specimen_id.to_string(),
                specimen.accession_number.clone(),
            )
        })
        .collect();
    let component_labels = current_reference_labels(conn, specimens)?;
    Ok(flags
        .iter()
        .filter_map(|flag| {
            flag_reference_label(flag, &analytes, &specimen_labels, &component_labels)
                .map(|label| (flag.ref_id.clone(), label))
        })
        .collect())
}

fn current_reference_labels(
    conn: &Connection,
    specimens: &[DerivationSpecimenView],
) -> Result<BTreeMap<String, String>, BillingError> {
    let order_ids: BTreeSet<_> = specimens.iter().map(|specimen| specimen.order_id).collect();
    let mut labels = BTreeMap::new();
    for order_id in order_ids {
        let accession_labels = specimens
            .iter()
            .filter(|specimen| specimen.order_id == order_id)
            .map(|specimen| specimen.accession_number.as_str())
            .collect::<Vec<_>>()
            .join(", ");
        labels.insert(order_id.to_string(), accession_labels.clone());
        for review in lis_orders::billing_review_items(conn, order_id)? {
            labels.insert(review.id.to_string(), accession_labels.clone());
        }
        for item in lis_orders::get_order(conn, order_id)?.items {
            let name = lis_catalog::get_orderable_version(conn, item.orderable_version_id)?.name;
            for component in item.components {
                labels.insert(component.component_id.to_string(), name.clone());
            }
        }
    }
    Ok(labels)
}

fn flag_reference_label(
    flag: &BillingFlag,
    analytes: &BTreeMap<String, String>,
    specimens: &BTreeMap<String, String>,
    components: &BTreeMap<String, String>,
) -> Option<String> {
    match flag.code.as_str() {
        "mapping_missing" | "performed_not_reported" => analytes.get(&flag.ref_id).cloned(),
        "failed_attempt_only"
        | "ordered_not_performed"
        | "billing_context_missing"
        | "billing_context_incomplete"
        | "billing_review_open" => components.get(&flag.ref_id).cloned(),
        "report_not_final"
        | "presumptive_and_definitive_same_specimen"
        | "collection_review_pending" => specimens.get(&flag.ref_id).cloned(),
        "date_of_service_unknown"
        | "laboratory_billing_identity_missing"
        | "multiple_presumptive_same_dos"
        | "performed_not_ordered" => None,
        _ => None,
    }
}

/// The engine's next actions for a derivation: re-derive whenever it is held
/// or its inputs changed, otherwise approve a candidate and hand off an
/// approved one.
fn allowed_actions(state: BillingDerivationState, stale: bool) -> Vec<String> {
    let action = match state {
        BillingDerivationState::Superseded => None,
        BillingDerivationState::Held => Some("derive_services"),
        BillingDerivationState::Candidate
        | BillingDerivationState::Approved
        | BillingDerivationState::HandedOff
            if stale =>
        {
            Some("derive_services")
        }
        BillingDerivationState::Candidate => Some("approve_service_derivation"),
        BillingDerivationState::Approved => Some("create_billing_handoff"),
        BillingDerivationState::HandedOff => None,
    };
    action.into_iter().map(str::to_owned).collect()
}

pub(crate) fn specimen_views(
    conn: &Connection,
    row: &DerivationRow,
) -> Result<Vec<DerivationSpecimenView>, BillingError> {
    let accessions: BTreeMap<SpecimenId, String> =
        lis_specimens::specimens_for_subject(conn, row.subject_id)?
            .into_iter()
            .map(|s| (s.specimen_id, s.accession_number))
            .collect();
    let classes = repo::classes(conn, row.id)?;
    let components = repo::components(conn, row.id)?;
    let mut views = Vec::new();
    for specimen in repo::specimens(conn, row.id)? {
        let orderables = orderable_names(conn, specimen.specimen_id)?;
        let mut component_views = Vec::new();
        for component in components
            .iter()
            .filter(|component| component.specimen_id == specimen.specimen_id)
        {
            component_views.push(DerivationComponentView {
                component_id: component.component_id,
                orderable: orderables
                    .get(&component.component_id)
                    .cloned()
                    .ok_or(BillingError::Inconsistent("derivation component"))?,
                disposition: component.disposition,
            });
        }
        views.push(DerivationSpecimenView {
            specimen_id: specimen.specimen_id,
            accession_number: accessions
                .get(&specimen.specimen_id)
                .cloned()
                .ok_or(BillingError::Inconsistent("derivation specimen"))?,
            order_id: specimen.order_id,
            order_version: specimen.order_version,
            report_revision_id: specimen.report_revision_id,
            classes: classes
                .iter()
                .filter(|fact| fact.specimen_id == specimen.specimen_id)
                .map(|fact| DerivationClassView {
                    class_code: fact.class.code.clone(),
                    class_label: fact.class.label.clone(),
                    certainty: fact.certainty,
                    ordered: fact.ordered,
                    performed: fact.performed,
                    reported: fact.reported,
                })
                .collect(),
            components: component_views,
        });
    }
    Ok(views)
}

/// The frozen orderable name of each expected component of a specimen.
fn orderable_names(
    conn: &Connection,
    specimen_id: SpecimenId,
) -> Result<BTreeMap<OrderComponentId, String>, BillingError> {
    Ok(lis_reporting::manifest(conn, specimen_id)?
        .components
        .into_iter()
        .map(|component| (component.component_id, component.orderable_name))
        .collect())
}
