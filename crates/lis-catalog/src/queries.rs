//! Read-side use cases. They take a connection (a read connection, or the
//! caller's transaction) and perform no authorization: the engine checks the
//! capability before calling them. Catalog content holds no PHI.

use rusqlite::Connection;

use crate::error::CatalogError;
use crate::lifecycle::VersionSummary;
use crate::model::{
    AnalyteId, MethodId, MethodVersion, MethodVersionId, OrderableId, OrderableVersion,
    OrderableVersionId, SpecimenTypeId, ValidityPolicyVersion, ValidityPolicyVersionId,
};

/// Current human label for an audit record, when this module owns one.
/// Labels are resolved at read time and never copied into audit storage.
pub fn audit_record_label(
    conn: &Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, CatalogError> {
    match record_type {
        "specimen_type" => {
            let Ok(id) = record_id.parse::<SpecimenTypeId>() else {
                return Ok(None);
            };
            Ok(list_specimen_types(conn)?
                .into_iter()
                .find(|record| record.id == id)
                .map(|record| record.name))
        }
        "analyte" => {
            let Ok(id) = record_id.parse::<AnalyteId>() else {
                return Ok(None);
            };
            Ok(list_analytes(conn)?
                .into_iter()
                .find(|record| record.id == id)
                .map(|record| record.name))
        }
        "method" => {
            let Ok(id) = record_id.parse::<MethodId>() else {
                return Ok(None);
            };
            Ok(list_methods(conn)?
                .into_iter()
                .find(|record| record.id == id)
                .map(|record| record.name))
        }
        "orderable" => {
            let Ok(id) = record_id.parse::<OrderableId>() else {
                return Ok(None);
            };
            Ok(list_orderables(conn)?
                .into_iter()
                .find(|record| record.id == id)
                .map(|record| record.name))
        }
        "method_version" => match record_id.parse::<MethodVersionId>() {
            Ok(id) => get_method_version(conn, id)
                .map(|version| Some(format!("{} v{}", version.method_name, version.version)))
                .or_else(not_found_is_none),
            Err(_) => Ok(None),
        },
        "orderable_version" => match record_id.parse::<OrderableVersionId>() {
            Ok(id) => get_orderable_version(conn, id)
                .map(|version| Some(format!("{} v{}", version.name, version.version)))
                .or_else(not_found_is_none),
            Err(_) => Ok(None),
        },
        "validity_policy_version" => match record_id.parse::<ValidityPolicyVersionId>() {
            Ok(id) => validity_policy(conn, id)
                .map(|version| {
                    Some(format!(
                        "{} policy v{}",
                        version.specimen_type_code, version.version
                    ))
                })
                .or_else(not_found_is_none),
            Err(_) => Ok(None),
        },
        _ => Ok(None),
    }
}

fn not_found_is_none(error: CatalogError) -> Result<Option<String>, CatalogError> {
    match error {
        CatalogError::NotFound(_) => Ok(None),
        other @ (CatalogError::Invalid(_)
        | CatalogError::RevisionConflict
        | CatalogError::Duplicate(_)
        | CatalogError::UserRequired
        | CatalogError::NotDraft
        | CatalogError::NotDraftAuthor
        | CatalogError::SelfApproval
        | CatalogError::DraftExists
        | CatalogError::NotApproved
        | CatalogError::AlreadyRetired
        | CatalogError::IncompleteConfiguration
        | CatalogError::MethodInUse { .. }
        | CatalogError::StandardFile(_)
        | CatalogError::StandardVersionMismatch
        | CatalogError::StandardItemsUnavailable { .. }
        | CatalogError::OmittedAllAnalytes { .. }
        | CatalogError::CatalogChanged { .. }
        | CatalogError::PreviewAlreadyReviewed
        | CatalogError::ReviewerAuthoredDrafts { .. }
        | CatalogError::BlankCutoffsUnconfirmed { .. }
        | CatalogError::OrderablesOnReplacedMethods { .. }
        | CatalogError::Store(_)) => Err(other),
    }
}
use crate::repo::{lifecycle, methods, orderables, reference, validity};
use crate::views::{
    AnalyteView, MethodVersionView, MethodView, OrderableVersionView, OrderableView,
    SpecimenTypeView, ValidityPolicyView,
};

/// A method version by ID in any state. For an approved or retired version
/// this is exactly what was approved (REQ-ANA-002).
pub fn get_method_version(
    conn: &Connection,
    id: MethodVersionId,
) -> Result<MethodVersionView, CatalogError> {
    let mut view = methods::version(conn, id)?.ok_or(CatalogError::NotFound("method_version"))?;
    view.analytes = methods::analytes(conn, id)?;
    view.external_approval_reference = crate::approval_references::get(conn, "method_version", id)?;
    Ok(view)
}

/// An orderable version with its manifest by ID in any state. Orders freeze
/// this ID, so later catalog changes never alter what it returns (REQ-ORD-002).
pub fn get_orderable_version(
    conn: &Connection,
    id: OrderableVersionId,
) -> Result<OrderableVersionView, CatalogError> {
    let mut view =
        orderables::version(conn, id)?.ok_or(CatalogError::NotFound("orderable_version"))?;
    view.components = orderables::components(conn, id)?;
    for component in &mut view.components {
        component.analytes = orderables::component_analytes(conn, id, component.position)?;
    }
    Ok(view)
}

/// The orderable versions offered for new orders: approved, effective period
/// open, by orderable code, each with its manifest.
pub fn list_active_orderables(
    conn: &Connection,
) -> Result<Vec<OrderableVersionView>, CatalogError> {
    orderables::current_version_ids(conn)?
        .into_iter()
        .map(|id| get_orderable_version(conn, id))
        .collect()
}

/// Every version of a method, oldest first.
pub fn list_method_versions(
    conn: &Connection,
    method_id: MethodId,
) -> Result<Vec<VersionSummary<MethodVersion>>, CatalogError> {
    Ok(lifecycle::history::<MethodVersion>(conn, method_id)?)
}

/// Every version of an orderable, oldest first.
pub fn list_orderable_versions(
    conn: &Connection,
    orderable_id: OrderableId,
) -> Result<Vec<VersionSummary<OrderableVersion>>, CatalogError> {
    Ok(lifecycle::history::<OrderableVersion>(conn, orderable_id)?)
}

/// A validity policy version with its categories by ID, in any state. For an
/// approved or retired version this is exactly what was approved (REQ-SPC-008).
pub fn validity_policy(
    conn: &Connection,
    id: ValidityPolicyVersionId,
) -> Result<ValidityPolicyView, CatalogError> {
    let mut view =
        validity::version(conn, id)?.ok_or(CatalogError::NotFound("validity_policy_version"))?;
    view.categories = validity::categories(conn, id)?;
    Ok(view)
}

/// The current approved validity policy of the specimen type with `code`, if
/// the laboratory has approved one. `None` means no policy: required validity
/// evidence cannot be assessed (REQ-SPC-004, REQ-SPC-008).
pub fn approved_validity_policy(
    conn: &Connection,
    specimen_type_code: &str,
) -> Result<Option<ValidityPolicyView>, CatalogError> {
    validity::current_for_specimen_type(conn, specimen_type_code)?
        .map(|id| validity_policy(conn, id))
        .transpose()
}

/// Every validity policy version of a specimen type, oldest first.
pub fn list_validity_policy_versions(
    conn: &Connection,
    specimen_type_id: SpecimenTypeId,
) -> Result<Vec<VersionSummary<ValidityPolicyVersion>>, CatalogError> {
    Ok(lifecycle::history::<ValidityPolicyVersion>(
        conn,
        specimen_type_id,
    )?)
}

/// Every specimen type, by code, with the numbers of its validity policy
/// version in effect and of its draft awaiting approval.
pub fn list_specimen_types(conn: &Connection) -> Result<Vec<SpecimenTypeView>, CatalogError> {
    Ok(reference::specimen_types(conn)?)
}

/// Every analyte, by code.
pub fn list_analytes(conn: &Connection) -> Result<Vec<AnalyteView>, CatalogError> {
    Ok(reference::analytes(conn)?)
}

/// Every method, by code, with the numbers of its version in effect and of
/// its draft awaiting approval.
pub fn list_methods(conn: &Connection) -> Result<Vec<MethodView>, CatalogError> {
    Ok(reference::methods(conn)?)
}

/// Every orderable, by code, with the numbers of its version in effect and of
/// its draft awaiting approval.
pub fn list_orderables(conn: &Connection) -> Result<Vec<OrderableView>, CatalogError> {
    Ok(reference::orderables(conn)?)
}
