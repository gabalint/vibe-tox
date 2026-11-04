//! An order message as an order the laboratory can accept or reject (REQ-ORD-001,
//! REQ-ORD-005): every value comes from the message or the connection's approved code
//! mappings, never from a default. The client is the connection's; the location, the
//! ordering provider (by NPI) and the tests (by the partner's order codes) must be known;
//! the patient is found only by an identifier the message carries, and a registered
//! patient whose name or birth date differs is held for a person.

use lis_core::Phi;
use lis_identity::{IdentifierInput, SubjectId, SubjectMatchQuery};
use lis_interfaces::partner::hl7_order::OrderMessage;
use lis_interfaces::partner::{IntakeSource, MappingKind};
use lis_orders::{
    AuthorityEvidenceInput, AuthorityEvidenceKind, BillingResponsibility, DiagnosisInput,
    EvidenceInput, OrderItemInput, ReflexAuthorization, RegisterOrderInput, SequenceRole,
    SourceKind, TemplateUseInput,
};
use rusqlite::Connection;

use super::IntakeError;

/// Why a message is not an order yet. The codes carry no patient data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Problem {
    /// No registered patient has an identifier the message carries.
    UnknownPatient,
    /// A person decides: the codes are `patient_identifier_conflict` and
    /// `patient_details_differ`.
    Held(&'static str),
    /// Refused: the file moves to Failed with this code.
    Rejected(&'static str),
}

/// The order `message` asks for, or why it is not one yet. `source_ref` names the stored
/// original message, as the order's source document.
pub(super) fn order_input(
    conn: &Connection,
    source: &IntakeSource,
    message: &OrderMessage,
    source_ref: &str,
) -> Result<Result<RegisterOrderInput, Problem>, IntakeError> {
    let client = lis_identity::list_active_clients(conn)?
        .into_iter()
        .find(|client| client.code == source.client_code);
    let Some(client) = client else {
        return Ok(Err(Problem::Rejected("unknown_client")));
    };
    let location = match &message.location {
        None => None,
        Some(code) => match client
            .locations
            .iter()
            .find(|location| location.active && &location.code == code)
        {
            Some(location) => Some(location.id),
            None => return Ok(Err(Problem::Rejected("unknown_location"))),
        },
    };
    let Some(npi) = &message.provider_id else {
        return Ok(Err(Problem::Rejected("no_provider")));
    };
    let practitioner = lis_identity::list_practitioners(conn)?
        .into_iter()
        .find(|practitioner| practitioner.active && practitioner.npi.as_str() == npi);
    let Some(practitioner) = practitioner else {
        return Ok(Err(Problem::Rejected("unknown_provider")));
    };
    let items = match items(conn, source, &message.tests)? {
        Ok(items) => items,
        Err(problem) => return Ok(Err(problem)),
    };
    let diagnoses = match diagnoses(message, source_ref) {
        Ok(diagnoses) => diagnoses,
        Err(problem) => return Ok(Err(problem)),
    };
    let subject_id = match patient(conn, source, message)? {
        Ok(subject_id) => subject_id,
        Err(problem) => return Ok(Err(problem)),
    };
    Ok(Ok(RegisterOrderInput {
        subject_id,
        client_id: client.id,
        client_location_id: location,
        practitioner_id: practitioner.id,
        placer_order_id: Some(message.placer_order.clone()),
        placer_authority: Some(
            message
                .placer_authority
                .clone()
                .unwrap_or_else(|| message.sending_facility.clone()),
        ),
        source_kind: SourceKind::Hl7Interface,
        source_document_ref: Some(source_ref.to_owned()),
        billing_responsibility: BillingResponsibility::NotSupplied,
        facility_setting: None,
        standing_order_id: None,
        template_use: TemplateUseInput::None,
        items,
        evidence: EvidenceInput {
            // An HL7 order carries no signature; the laboratory's order policy decides at
            // acceptance (REQ-ORD-005).
            authority: AuthorityEvidenceInput {
                kind: AuthorityEvidenceKind::NotSupplied,
                document_ref: Some(source_ref.to_owned()),
            },
            diagnoses,
            medications: Vec::new(),
            point_of_care_results: Vec::new(),
        },
    }))
}

/// Each partner order code's LIS test and sequence role, from the approved mappings.
fn items(
    conn: &Connection,
    source: &IntakeSource,
    codes: &[String],
) -> Result<Result<Vec<OrderItemInput>, Problem>, IntakeError> {
    let Some(mappings) = lis_interfaces::partner::approved_mappings(conn, source.connection_id)?
    else {
        return Ok(Err(Problem::Rejected("unknown_test")));
    };
    let versions = lis_catalog::list_active_orderables(conn)?;
    let mut items: Vec<OrderItemInput> = Vec::with_capacity(codes.len());
    for code in codes {
        let mapping = mappings.mappings.iter().find(|mapping| {
            mapping.kind == MappingKind::Test && &mapping.values.partner_code == code
        });
        let role = mapping.and_then(|mapping| match mapping.values.sequence_role.as_deref() {
            Some("first_line") => Some(SequenceRole::FirstLine),
            Some("confirmation") => Some(SequenceRole::Confirmation),
            _ => None,
        });
        let version = mapping.and_then(|mapping| {
            versions
                .iter()
                .find(|version| version.orderable_id.cast() == mapping.lis_id)
        });
        let (Some(role), Some(version)) = (role, version) else {
            return Ok(Err(Problem::Rejected("unknown_test")));
        };
        if items
            .iter()
            .any(|item| item.orderable_version_id == version.id)
        {
            return Ok(Err(Problem::Rejected("duplicate_test")));
        }
        items.push(OrderItemInput {
            orderable_version_id: version.id,
            sequence_role: role,
            reflex_authorization: ReflexAuthorization::NotStated,
            reflex_scope_orderable_ids: Vec::new(),
        });
    }
    Ok(Ok(items))
}

/// DG1 diagnoses as supplied; one without its coding system is refused, never guessed.
fn diagnoses(message: &OrderMessage, source_ref: &str) -> Result<Vec<DiagnosisInput>, Problem> {
    message
        .diagnoses
        .iter()
        .map(|(code, system)| {
            let system = system
                .clone()
                .ok_or(Problem::Rejected("diagnosis_system_missing"))?;
            Ok(DiagnosisInput {
                code: code.clone(),
                code_system: system,
                code_system_version: None,
                document_ref: source_ref.to_owned(),
                preprinted_on_lab_form: false,
            })
        })
        .collect()
}

/// The one registered patient with an identifier the message carries, by its assigning
/// authority (else the sending facility) and type; an identifier without a type is not
/// looked up.
fn patient(
    conn: &Connection,
    source: &IntakeSource,
    message: &OrderMessage,
) -> Result<Result<SubjectId, Problem>, IntakeError> {
    let identifiers = message
        .patient
        .identifiers
        .iter()
        .filter_map(|(value, authority, kind)| {
            Some(IdentifierInput {
                assigning_authority: authority
                    .clone()
                    .unwrap_or_else(|| source.intake.accept_from.clone()),
                identifier_type: kind.clone()?,
                value: Phi::new(value.clone()),
            })
        })
        .collect();
    let query = SubjectMatchQuery {
        demographics: None,
        identifiers,
    };
    let matches = lis_identity::find_subject_matches(conn, &query)?;
    let subject_id = match matches.identifier_matches.as_slice() {
        [] => return Ok(Err(Problem::UnknownPatient)),
        [subject_id] => *subject_id,
        _ => return Ok(Err(Problem::Held("patient_identifier_conflict"))),
    };
    let summary = lis_identity::subject_summary(conn, subject_id)?;
    let same_name = message
        .patient
        .family_name
        .as_deref()
        .is_none_or(|family| family.eq_ignore_ascii_case(summary.family_name.expose()));
    let same_birth = message.patient.birth_date.as_deref().is_none_or(|birth| {
        birth.get(..8) == Some(summary.birth_date.expose().replace('-', "").as_str())
    });
    Ok(if same_name && same_birth {
        Ok(subject_id)
    } else {
        Err(Problem::Held("patient_details_differ"))
    })
}
