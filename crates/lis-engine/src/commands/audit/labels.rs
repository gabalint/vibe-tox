//! Readable labels of audit records, resolved from their owner modules at read time
//! (REQ-OPS-001): beside record IDs in the trail and on Find's choices. Patient names are
//! shown only to users who may read patients, as in the patient directory.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use lis_core::{Capability, Principal, UserId};

use super::AuditReadError;
use crate::registry::Access;

/// The access of the patient directory (`search_subjects`, `get_subject`).
const READ_PATIENTS: Access =
    Access::EitherCapability(Capability::EnterOrders, Capability::ManageQuality);

/// Whether `principal` may see patient names and find patients in the audit trail.
pub(super) fn reads_patients(principal: &Principal) -> bool {
    READ_PATIENTS.require(principal).is_ok()
}

/// The label of each record that has one; `users` holds the names of user records.
pub(super) fn record_labels(
    conn: &rusqlite::Connection,
    records: &BTreeSet<(String, String)>,
    users: &HashMap<UserId, String>,
    patients: bool,
) -> Result<BTreeMap<(String, String), String>, AuditReadError> {
    let mut labels = BTreeMap::new();
    for (record_type, record_id) in records {
        let label = match record_type.as_str() {
            "user" => record_id
                .parse::<UserId>()
                .ok()
                .and_then(|id| users.get(&id).cloned()),
            "subject" if !patients => None,
            _ => owner_label(conn, record_type, record_id)?,
        };
        if let Some(label) = label {
            labels.insert((record_type.clone(), record_id.clone()), label);
        }
    }
    Ok(labels)
}

/// The label from the module that owns `record_type`. An order's accession numbers come
/// before its placer order number.
fn owner_label(
    conn: &rusqlite::Connection,
    record_type: &str,
    record_id: &str,
) -> Result<Option<String>, AuditReadError> {
    if let Some(label) = lis_catalog::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_identity::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_reporting::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_specimens::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_orders::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_billing::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_interfaces::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    if let Some(label) = lis_ops::audit_records::audit_record_label(conn, record_type, record_id)? {
        return Ok(Some(label));
    }
    Ok(lis_auth::audit_record_label(conn, record_type, record_id)?)
}
