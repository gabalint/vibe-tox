//! Old and new values behind an identity audit event (REQ-OPS-001), read from
//! this module's immutable histories when the event is viewed: demographic
//! assertions, identifiers, patient financial profile versions, laboratory
//! profile versions and directory history. Patient values come back marked
//! with the capabilities that may read them; the engine authorizes and audits
//! that read. Nothing here writes or copies values into the audit trail.

use lis_audit::{ChangeQuery, RecordChanges, differing};
use lis_core::Capability;
use rusqlite::Connection;
use serde_json::Value;

use crate::billing::get_patient_billing_profile;
use crate::directories::{DirectoryHistoryInput, DirectoryRecord, DirectorySnapshot};
use crate::error::IdentityError;
use crate::model::SubjectId;
use crate::repo;
use crate::views::LaboratoryProfileView;

/// Who may read demographics and identifiers: as `get_subject`.
const IDENTITY_READ: (Capability, Capability) =
    (Capability::EnterOrders, Capability::ManageQuality);
/// Who may read the patient financial profile: as `get_patient_billing_profile`.
const FINANCIAL_READ: (Capability, Capability) = (Capability::EnterOrders, Capability::Billing);

type Row = (String, Option<String>, Option<String>);

/// The changes behind `query`, when this module owns its record and keeps its history.
pub fn audit_record_changes(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    match (query.record_type, query.action) {
        ("subject", "identity.register_subject" | "identity.assert_demographics") => {
            demographics(conn, query)
        }
        ("subject", "identity.add_identifier" | "identity.deactivate_identifier") => {
            identifier(conn, query)
        }
        ("subject", "identity.set_patient_billing_profile") => financial(conn, query),
        ("laboratory_profile", _) => laboratory(conn, query),
        ("client" | "location" | "practitioner", _) => directory(conn, query),
        _ => Ok(None),
    }
}

fn subject(query: &ChangeQuery<'_>) -> Option<SubjectId> {
    query.record_id.parse().ok()
}

fn demographics(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    let Some(subject_id) = subject(query) else {
        return Ok(None);
    };
    // Registration records the first assertion; a correction names the one it added.
    let sequence = match query.action {
        "identity.register_subject" => 1,
        _ => match query.integer("assertion_sequence") {
            Some(sequence) => sequence,
            None => return Ok(None),
        },
    };
    let assertions = repo::subjects::assertions(conn, subject_id)?;
    let find = |wanted: i64| assertions.iter().find(|row| row.sequence == wanted);
    let Some(new) = find(sequence) else {
        return Ok(None);
    };
    let old = find(sequence - 1);
    let values = |row: Option<&crate::views::AssertionView>| -> [Option<String>; 5] {
        row.map_or([None, None, None, None, None], |row| {
            [
                Some(row.family_name.expose().clone()),
                row.given_names.as_ref().map(|value| value.expose().clone()),
                Some(row.birth_date.expose().clone()),
                row.administrative_sex
                    .as_ref()
                    .map(|value| value.expose().clone()),
                Some(row.source.clone()),
            ]
        })
    };
    let labels = ["Family name", "Given names", "Birth date", "Sex", "Source"];
    let rows = labels
        .into_iter()
        .zip(values(old).into_iter().zip(values(Some(new))))
        .map(|(label, (old, new))| (label.to_owned(), old, new));
    Ok(Some(RecordChanges::patient(
        differing(rows),
        subject_id.to_string(),
        IDENTITY_READ,
    )))
}

fn identifier(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    let (Some(subject_id), Some(identifier_id)) = (subject(query), query.text("identifier_id"))
    else {
        return Ok(None);
    };
    let identifiers = repo::subjects::identifiers(conn, subject_id)?;
    let Some(row) = identifiers
        .iter()
        .find(|row| row.id.to_string() == identifier_id)
    else {
        return Ok(None);
    };
    let label = format!(
        "Identifier {} ({})",
        row.identifier_type, row.assigning_authority
    );
    let value = Some(row.value.expose().clone());
    // Adding shows the value appear; deactivating shows it leave the active identifiers.
    let field = if query.action == "identity.add_identifier" {
        (label, None, value)
    } else {
        (label, value, None)
    };
    Ok(Some(RecordChanges::patient(
        differing([field]),
        subject_id.to_string(),
        IDENTITY_READ,
    )))
}

fn financial(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    let (Some(subject_id), Some(new_version)) = (subject(query), query.new_revision) else {
        return Ok(None);
    };
    let content = |version: i64| -> Result<Option<Vec<(String, String)>>, IdentityError> {
        let Some(profile) = get_patient_billing_profile(conn, subject_id, Some(version))? else {
            return Ok(None);
        };
        let value = serde_json::to_value(&profile.content)
            .map_err(|_| IdentityError::InvalidFinancialRecord)?;
        let mut fields = Vec::new();
        flatten(&value, "", &mut fields);
        Ok(Some(fields))
    };
    let Some(new) = content(new_version)? else {
        return Ok(None);
    };
    let old = match query.old_revision.filter(|version| *version > 0) {
        Some(version) => content(version)?.unwrap_or_default(),
        None => Vec::new(),
    };
    Ok(Some(RecordChanges::patient(
        differing(pair_by_label(&old, &new)),
        subject_id.to_string(),
        FINANCIAL_READ,
    )))
}

/// Every label of `new`, then labels only `old` has, each with both values.
fn pair_by_label(old: &[(String, String)], new: &[(String, String)]) -> Vec<Row> {
    let value = |fields: &[(String, String)], label: &str| {
        fields
            .iter()
            .find(|(field, _)| field == label)
            .map(|(_, value)| value.clone())
    };
    let removed = old.iter().filter(|(label, _)| value(new, label).is_none());
    new.iter()
        .chain(removed)
        .map(|(label, _)| (label.clone(), value(old, label), value(new, label)))
        .collect()
}

/// The leaves of a JSON value as `(label, text)`, labels joined with ` › `;
/// list items are numbered from 1 and empty values are left out.
fn flatten(value: &Value, label: &str, out: &mut Vec<(String, String)>) {
    match value {
        Value::Object(map) => {
            for (key, child) in map {
                flatten(child, &join(label, &words(key)), out);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                flatten(item, &format!("{label} {}", index + 1), out);
            }
        }
        Value::Null => {}
        Value::Bool(_) | Value::Number(_) | Value::String(_) => {
            out.push((label.to_owned(), lis_audit::detail_text(value)));
        }
    }
}

fn join(label: &str, key: &str) -> String {
    if label.is_empty() {
        key.to_owned()
    } else {
        format!("{label} › {key}")
    }
}

/// `memberId` → `Member id`.
fn words(key: &str) -> String {
    let mut text = String::new();
    for (index, character) in key.chars().enumerate() {
        if index == 0 {
            text.extend(character.to_uppercase());
        } else if character.is_ascii_uppercase() {
            text.push(' ');
            text.push(character.to_ascii_lowercase());
        } else {
            text.push(character);
        }
    }
    text
}

fn laboratory(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    let profiles = repo::laboratory::profiles(conn)?;
    let Some(new) = profiles
        .iter()
        .find(|profile| profile.profile_id.to_string() == query.record_id)
    else {
        return Ok(None);
    };
    let old = profiles
        .iter()
        .find(|profile| Some(profile.version) == query.old_revision);
    let rows = laboratory_values(old)
        .into_iter()
        .zip(laboratory_values(Some(new)))
        .map(|((label, old), (_, new))| (label, old, new));
    Ok(Some(RecordChanges::configuration(differing(rows))))
}

fn laboratory_values(profile: Option<&LaboratoryProfileView>) -> Vec<(String, Option<String>)> {
    let line = |index: usize| profile.and_then(|profile| profile.address_lines.get(index).cloned());
    vec![
        (
            "Name".to_owned(),
            profile.map(|profile| profile.name.clone()),
        ),
        ("Address line 1".to_owned(), line(0)),
        ("Address line 2".to_owned(), line(1)),
        ("Address line 3".to_owned(), line(2)),
        (
            "CLIA ID".to_owned(),
            profile.map(|profile| profile.clia_id.clone()),
        ),
        (
            "Director".to_owned(),
            profile.map(|profile| profile.director.clone()),
        ),
        (
            "Phone".to_owned(),
            profile.map(|profile| profile.phone.clone()),
        ),
        (
            "Time zone".to_owned(),
            profile.map(|profile| profile.time_zone.clone()),
        ),
        (
            "Billing NPI".to_owned(),
            profile.and_then(|profile| profile.billing_npi.clone()),
        ),
        (
            "Tax ID".to_owned(),
            profile.and_then(|profile| profile.tax_id.clone()),
        ),
    ]
}

fn directory(
    conn: &Connection,
    query: &ChangeQuery<'_>,
) -> Result<Option<RecordChanges>, IdentityError> {
    let id = query.record_id;
    let record = match query.record_type {
        "client" => id.parse().ok().map(DirectoryRecord::Client),
        "location" => id.parse().ok().map(DirectoryRecord::Location),
        _ => id.parse().ok().map(DirectoryRecord::Practitioner),
    };
    let (Some(record), Some(new_revision)) = (record, query.produced_revision()) else {
        return Ok(None);
    };
    let history =
        match crate::directories::directory_history(conn, DirectoryHistoryInput { record }) {
            Ok(history) => history,
            Err(IdentityError::NotFound(_)) => return Ok(None),
            Err(error) => return Err(error),
        };
    let snapshot = |revision: Option<i64>| {
        history
            .iter()
            .find(|entry| Some(entry.revision) == revision)
            .map(|entry| &entry.snapshot)
    };
    let Some(new) = snapshot(Some(new_revision)) else {
        return Ok(None);
    };
    let old = snapshot(query.old_revision);
    let rows = directory_values(new, old)
        .into_iter()
        .map(|(label, old, new)| (label.to_owned(), old, new));
    Ok(Some(RecordChanges::configuration(differing(rows))))
}

fn directory_values(
    new: &DirectorySnapshot,
    old: Option<&DirectorySnapshot>,
) -> Vec<(&'static str, Option<String>, Option<String>)> {
    let facts = |snapshot: &DirectorySnapshot| match snapshot {
        DirectorySnapshot::Client { code, name, active }
        | DirectorySnapshot::Location { code, name, active } => vec![
            ("Code", Some(code.clone())),
            ("Name", Some(name.clone())),
            ("Active", Some(yes_no(*active))),
        ],
        DirectorySnapshot::Practitioner {
            npi,
            family_name,
            given_names,
            active,
        } => vec![
            ("NPI", Some(npi.clone())),
            ("Family name", Some(family_name.clone())),
            ("Given names", given_names.clone()),
            ("Active", Some(yes_no(*active))),
        ],
    };
    let old = old.map(facts).unwrap_or_default();
    facts(new)
        .into_iter()
        .map(|(label, new)| {
            let old = old
                .iter()
                .find(|(field, _)| *field == label)
                .and_then(|(_, value)| value.clone());
            (label, old, new)
        })
        .collect()
}

fn yes_no(value: bool) -> String {
    String::from(if value { "Yes" } else { "No" })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flatten_labels_nested_leaves_and_numbers_list_items() {
        let value = serde_json::json!({
            "contact": {"addressLines": ["1 Main St"], "city": "Cluj", "phone": null},
            "coverages": [{"memberId": "M1", "active": true}]
        });
        let mut fields = Vec::new();
        flatten(&value, "", &mut fields);
        assert!(fields.contains(&(
            "Contact › Address lines 1".to_owned(),
            "1 Main St".to_owned()
        )));
        assert!(fields.contains(&("Contact › City".to_owned(), "Cluj".to_owned())));
        assert!(fields.contains(&("Coverages 1 › Member id".to_owned(), "M1".to_owned())));
        assert!(fields.contains(&("Coverages 1 › Active".to_owned(), "Yes".to_owned())));
        assert!(!fields.iter().any(|(label, _)| label == "Contact › Phone"));
    }

    #[test]
    fn pairing_keeps_new_order_then_removed_fields() {
        let old = vec![
            ("City".to_owned(), "Cluj".to_owned()),
            ("Fax".to_owned(), "1".to_owned()),
        ];
        let new = vec![("City".to_owned(), "Buzău".to_owned())];
        let changes = differing(pair_by_label(&old, &new));
        assert_eq!(changes.len(), 2);
        assert_eq!(changes[0].field, "City");
        assert_eq!(changes[0].old.as_deref(), Some("Cluj"));
        assert_eq!(changes[0].new.as_deref(), Some("Buzău"));
        assert_eq!(
            (changes[1].old.as_deref(), changes[1].new.as_deref()),
            (Some("1"), None)
        );
    }

    #[test]
    fn words_split_camel_case() {
        assert_eq!(words("memberId"), "Member id");
        assert_eq!(words("city"), "City");
    }
}
