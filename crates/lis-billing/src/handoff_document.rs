//! The `bil-handoff-v2` file (brief §5.3, REQ-BIL-002): canonical JSON,
//! fields in a fixed order, strings, booleans and integers only. One item
//! per approved derivation, with the laboratory's billing identity, the
//! subject's current identity, date of service and its rule, mapping
//! version, flags, and per specimen the order lineage, practitioner NPI,
//! billing responsibility, diagnoses exactly as supplied (never inferred),
//! classes tested, financial snapshots and component dispositions. It carries
//! no procedure-code content, monetary claim charges or result values.

use lis_core::UtcTimestamp;
use lis_identity::{LaboratoryProfileView, SubjectId, SubjectSummary};
use lis_orders::{DiagnosisView, OrderDetail};
use serde::Serialize;

use crate::derivation_view::DerivationSpecimenView;
use crate::error::BillingError;
use crate::model::{
    BillingDerivationId, BillingFlag, BillingHandoffId, BillingHandoffItemId, HandoffItemKind,
};

/// The format code stored on every handoff.
pub(crate) const HANDOFF_FORMAT: &str = "bil-handoff-v2";

/// The whole file.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct HandoffDocument {
    pub(crate) format: &'static str,
    pub(crate) handoff_id: BillingHandoffId,
    pub(crate) generated_at: UtcTimestamp,
    pub(crate) laboratory: LaboratoryPart,
    pub(crate) items: Vec<ItemPart>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct LaboratoryPart {
    name: String,
    clia_id: String,
    billing_npi: String,
    tax_id: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ItemPart {
    pub(crate) item_id: BillingHandoffItemId,
    pub(crate) kind: HandoffItemKind,
    pub(crate) adjusts_item_id: Option<BillingHandoffItemId>,
    pub(crate) derivation_id: BillingDerivationId,
    pub(crate) derivation_no: i64,
    pub(crate) subject: SubjectPart,
    pub(crate) date_of_service: Option<String>,
    pub(crate) date_of_service_rule: String,
    pub(crate) mapping_version: Option<i64>,
    pub(crate) flags: Vec<BillingFlag>,
    pub(crate) specimens: Vec<SpecimenPart>,
    pub(crate) voids_prior_services: bool,
    pub(crate) context_resolution: Option<crate::BillingContextResolution>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SubjectPart {
    subject_id: SubjectId,
    family_name: String,
    given_names: Option<String>,
    birth_date: String,
    administrative_sex: Option<String>,
    identifiers: Vec<IdentifierPart>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct IdentifierPart {
    assigning_authority: String,
    identifier_type: String,
    value: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct SpecimenPart {
    accession_number: String,
    order_id: lis_orders::OrderId,
    placer_order_id: Option<String>,
    order_version: i64,
    practitioner_npi: String,
    billing_responsibility: lis_orders::BillingResponsibility,
    diagnoses: Vec<DiagnosisPart>,
    classes: Vec<ClassPart>,
    components: Vec<ComponentPart>,
    ordering_client: ClientPart,
    client_location: Option<lis_identity::ClientLocationView>,
    billing_client: Option<ClientPart>,
    facility_setting: Option<String>,
    ordering_practitioner: lis_identity::PractitionerView,
    source_document_ref: Option<String>,
    financial_context: lis_orders::OrderBillingContext,
    billing_reviews: Vec<lis_orders::BillingReviewRecord>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClientPart {
    id: lis_identity::ClientId,
    code: String,
    name: String,
}

impl From<lis_identity::ClientView> for ClientPart {
    fn from(client: lis_identity::ClientView) -> Self {
        Self {
            id: client.id,
            code: client.code,
            name: client.name,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct DiagnosisPart {
    evidence_id: lis_orders::OrderEvidenceId,
    code: String,
    code_system: String,
    code_system_version: Option<String>,
    document_ref: String,
    preprinted_on_lab_form: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ClassPart {
    class_code: String,
    class_label: String,
    certainty: lis_catalog::MethodCertainty,
    ordered: bool,
    performed: bool,
    reported: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ComponentPart {
    component_id: lis_orders::OrderComponentId,
    orderable: String,
    disposition: crate::model::BillingComponentDisposition,
}

/// The laboratory part; billing needs both the billing NPI and the TIN.
pub(crate) fn laboratory_part(
    profile: &LaboratoryProfileView,
) -> Result<LaboratoryPart, BillingError> {
    let (Some(billing_npi), Some(tax_id)) = (&profile.billing_npi, &profile.tax_id) else {
        return Err(BillingError::LaboratoryBillingIdentityMissing);
    };
    Ok(LaboratoryPart {
        name: profile.name.clone(),
        clia_id: profile.clia_id.clone(),
        billing_npi: billing_npi.clone(),
        tax_id: tax_id.clone(),
    })
}

/// The subject's current identity, exposed explicitly for the file.
pub(crate) fn subject_part(subject: &SubjectSummary) -> SubjectPart {
    SubjectPart {
        subject_id: subject.subject_id,
        family_name: subject.family_name.expose().clone(),
        given_names: subject
            .given_names
            .as_ref()
            .map(|name| name.expose().clone()),
        birth_date: subject.birth_date.expose().clone(),
        administrative_sex: subject
            .administrative_sex
            .as_ref()
            .map(|sex| sex.expose().clone()),
        identifiers: subject
            .identifiers
            .iter()
            .filter(|identifier| identifier.active)
            .map(|identifier| IdentifierPart {
                assigning_authority: identifier.assigning_authority.clone(),
                identifier_type: identifier.identifier_type.clone(),
                value: identifier.value.expose().clone(),
            })
            .collect(),
    }
}

/// One specimen: its derived facts and the order's lineage as supplied.
/// Only diagnoses current on the order version count; none is added.
pub(crate) fn specimen_part(
    conn: &rusqlite::Connection,
    specimen: DerivationSpecimenView,
    order: &OrderDetail,
    practitioner: lis_identity::PractitionerView,
) -> Result<SpecimenPart, BillingError> {
    let context = lis_orders::get_order_billing_context(conn, order.order_id)?
        .ok_or(BillingError::Inconsistent("approved billing context"))?;
    let client = lis_identity::get_client(conn, order.client_id)?;
    let client_location = client
        .locations
        .iter()
        .find(|location| Some(location.id) == order.client_location_id)
        .cloned();
    let billing_client = context
        .selection
        .client_id
        .map(|id| lis_identity::get_client(conn, id))
        .transpose()?
        .map(ClientPart::from);
    Ok(SpecimenPart {
        accession_number: specimen.accession_number,
        order_id: order.order_id,
        placer_order_id: order.placer_order_id.clone(),
        order_version: specimen.order_version,
        practitioner_npi: practitioner.npi.as_str().to_owned(),
        billing_responsibility: context.selection.responsibility,
        ordering_client: client.into(),
        client_location,
        billing_client,
        facility_setting: order.facility_setting.clone(),
        ordering_practitioner: practitioner,
        source_document_ref: order.source_document_ref.clone(),
        financial_context: context,
        billing_reviews: lis_orders::billing_review_items(conn, order.order_id)?,
        diagnoses: order
            .evidence
            .diagnoses
            .iter()
            .filter(|diagnosis| diagnosis.removed_in_version.is_none())
            .map(diagnosis_part)
            .collect(),
        classes: specimen
            .classes
            .into_iter()
            .map(|class| ClassPart {
                class_code: class.class_code,
                class_label: class.class_label,
                certainty: class.certainty,
                ordered: class.ordered,
                performed: class.performed,
                reported: class.reported,
            })
            .collect(),
        components: specimen
            .components
            .into_iter()
            .map(|component| ComponentPart {
                component_id: component.component_id,
                orderable: component.orderable,
                disposition: component.disposition,
            })
            .collect(),
    })
}

fn diagnosis_part(diagnosis: &DiagnosisView) -> DiagnosisPart {
    DiagnosisPart {
        evidence_id: diagnosis.evidence_id,
        code: diagnosis.code.clone(),
        code_system: diagnosis.code_system.clone(),
        code_system_version: diagnosis.code_system_version.clone(),
        document_ref: diagnosis.document_ref.clone(),
        preprinted_on_lab_form: diagnosis.preprinted_on_lab_form,
    }
}

/// The file's bytes: pretty JSON with a final newline. The same document
/// always gives the same bytes.
pub(crate) fn to_bytes(document: &HandoffDocument) -> Result<Vec<u8>, BillingError> {
    let mut bytes = serde_json::to_vec_pretty(document)
        .map_err(|_| BillingError::Inconsistent("handoff document"))?;
    bytes.push(b'\n');
    Ok(bytes)
}
