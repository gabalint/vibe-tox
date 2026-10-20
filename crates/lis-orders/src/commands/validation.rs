//! Shared trust-boundary validation for supplied order text.

use std::collections::BTreeSet;

use lis_catalog::{OrderableId, OrderableVersionId, VersionStatus};
use lis_identity::{ClientId, ClientLocationId, PractitionerId, SubjectId};
use rusqlite::Connection;

use crate::error::{OrderError, invalid};
use crate::inputs::{EvidenceInput, RegisterOrderInput};

pub(super) const TEXT_MAX: usize = 1000;

pub(super) fn required(field: &str, value: &str) -> Result<(), OrderError> {
    text(field, value)?;
    if value.trim().is_empty() {
        Err(invalid(field, "required", "Enter a value."))
    } else {
        Ok(())
    }
}

pub(super) fn text(field: &str, value: &str) -> Result<(), OrderError> {
    if value.chars().count() > TEXT_MAX {
        return Err(invalid(field, "too_long", "Use 1000 characters or fewer."));
    }
    if value.chars().any(char::is_control) {
        return Err(invalid(
            field,
            "control_character",
            "Remove control characters.",
        ));
    }
    Ok(())
}

pub(super) fn optional(field: &str, value: Option<&str>) -> Result<(), OrderError> {
    if let Some(value) = value {
        text(field, value)?;
    }
    Ok(())
}

pub(super) fn reason(field: &str, value: &str) -> Result<(), OrderError> {
    required(field, value)
}

pub(crate) fn iso_date(field: &str, value: &str) -> Result<(), OrderError> {
    let bytes = value.as_bytes();
    let shape = bytes.len() == 10
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| matches!(index, 4 | 7) || byte.is_ascii_digit());
    let valid = shape
        && value
            .parse::<jiff::civil::Date>()
            .is_ok_and(|date| date.to_string() == value);
    if valid {
        Ok(())
    } else {
        Err(invalid(
            field,
            "invalid_date",
            "Use a date in YYYY-MM-DD form.",
        ))
    }
}

pub(super) fn validate_evidence(input: &EvidenceInput) -> Result<(), OrderError> {
    optional(
        "evidence.authority.documentRef",
        input.authority.document_ref.as_deref(),
    )?;
    for (index, diagnosis) in input.diagnoses.iter().enumerate() {
        required(
            &format!("evidence.diagnoses[{index}].code"),
            &diagnosis.code,
        )?;
        required(
            &format!("evidence.diagnoses[{index}].codeSystem"),
            &diagnosis.code_system,
        )?;
        optional(
            &format!("evidence.diagnoses[{index}].codeSystemVersion"),
            diagnosis.code_system_version.as_deref(),
        )?;
        required(
            &format!("evidence.diagnoses[{index}].documentRef"),
            &diagnosis.document_ref,
        )?;
    }
    for (index, medication) in input.medications.iter().enumerate() {
        required(
            &format!("evidence.medications[{index}].receivedText"),
            &medication.received_text,
        )?;
        optional(
            &format!("evidence.medications[{index}].code"),
            medication.code.as_deref(),
        )?;
        optional(
            &format!("evidence.medications[{index}].codeSystem"),
            medication.code_system.as_deref(),
        )?;
        if medication.code.is_some() != medication.code_system.is_some() {
            return Err(invalid(
                &format!("evidence.medications[{index}].codeSystem"),
                "paired",
                "Supply both the code and code system, or neither.",
            ));
        }
        optional(
            &format!("evidence.medications[{index}].reporter"),
            medication.reporter.as_deref(),
        )?;
        optional(
            &format!("evidence.medications[{index}].providedOn"),
            medication.provided_on.as_deref(),
        )?;
        if let Some(date) = medication.as_of_date.as_deref() {
            iso_date(&format!("evidence.medications[{index}].asOfDate"), date)?;
        }
    }
    for (index, result) in input.point_of_care_results.iter().enumerate() {
        required(
            &format!("evidence.pointOfCareResults[{index}].deviceOrTest"),
            &result.device_or_test,
        )?;
        required(
            &format!("evidence.pointOfCareResults[{index}].analyteText"),
            &result.analyte_text,
        )?;
        required(
            &format!("evidence.pointOfCareResults[{index}].resultText"),
            &result.result_text,
        )?;
        optional(
            &format!("evidence.pointOfCareResults[{index}].performedAtText"),
            result.performed_at_text.as_deref(),
        )?;
        required(
            &format!("evidence.pointOfCareResults[{index}].source"),
            &result.source,
        )?;
    }
    Ok(())
}

pub(super) fn validate_registration(input: &RegisterOrderInput) -> Result<(), OrderError> {
    match (&input.placer_order_id, &input.placer_authority) {
        (Some(id), Some(authority)) => {
            required("placerOrderId", id)?;
            required("placerAuthority", authority)?;
        }
        (None, None) => {}
        _ => {
            return Err(invalid(
                "placerOrderId",
                "paired",
                "Supply both placer fields, or neither.",
            ));
        }
    }
    optional("sourceDocumentRef", input.source_document_ref.as_deref())?;
    optional("facilitySetting", input.facility_setting.as_deref())?;
    match &input.template_use {
        crate::TemplateUseInput::None => {}
        crate::TemplateUseInput::ClinicTemplate { name, .. } => required("templateUse.name", name)?,
    }
    if input.items.is_empty() {
        return Err(invalid(
            "items",
            "required",
            "Add at least one requested test.",
        ));
    }
    let mut versions = BTreeSet::new();
    for (index, item) in input.items.iter().enumerate() {
        if !versions.insert(item.orderable_version_id) {
            return Err(invalid(
                &format!("items[{index}]"),
                "duplicate",
                "This orderable version is listed twice.",
            ));
        }
        let mut scope = BTreeSet::new();
        if item
            .reflex_scope_orderable_ids
            .iter()
            .any(|id| !scope.insert(*id))
        {
            return Err(invalid(
                &format!("items[{index}].reflexScopeOrderableIds"),
                "duplicate",
                "A reflex scope entry is listed twice.",
            ));
        }
    }
    validate_evidence(&input.evidence)
}

pub(super) fn validate_references(
    conn: &Connection,
    subject_id: SubjectId,
    client_id: ClientId,
    location_id: Option<ClientLocationId>,
    practitioner_id: PractitionerId,
) -> Result<(), OrderError> {
    lis_identity::get_subject(conn, subject_id)?;
    let client = lis_identity::list_clients(conn)?
        .into_iter()
        .find(|client| client.id == client_id)
        .ok_or(OrderError::NotFound("client"))?;
    if location_id.is_some_and(|id| !client.locations.iter().any(|location| location.id == id)) {
        return Err(OrderError::NotFound("client_location"));
    }
    if !lis_identity::list_practitioners(conn)?
        .iter()
        .any(|p| p.id == practitioner_id)
    {
        return Err(OrderError::NotFound("practitioner"));
    }
    Ok(())
}

pub(super) fn active_orderable(
    conn: &Connection,
    id: OrderableVersionId,
) -> Result<lis_catalog::OrderableVersionView, OrderError> {
    let view = lis_catalog::get_orderable_version(conn, id)?;
    if view.lifecycle.status != VersionStatus::Approved || view.lifecycle.effective_to.is_some() {
        return Err(OrderError::GateBlocked);
    }
    Ok(view)
}

pub(super) fn orderable_exists(conn: &Connection, id: OrderableId) -> Result<(), OrderError> {
    if lis_catalog::list_orderables(conn)?
        .iter()
        .any(|orderable| orderable.id == id)
    {
        Ok(())
    } else {
        Err(OrderError::NotFound("orderable"))
    }
}
