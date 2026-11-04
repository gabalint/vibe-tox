//! Code mappings of HL7 connections (REQ-INT-004): changes are drafted, and a
//! second person approves them as a new version, or the person who drafted
//! them where the laboratory's separation-of-duties policy allows (ADR 0015).
//! A version applies to messages not yet written; written ones never change.

use std::collections::BTreeMap;

use lis_audit::AuditDetail;
use lis_auth::Duty;
use lis_core::{Capability, required_text};
use lis_store::CommandContext;
use rusqlite::Connection;

use super::connections::{authorize_self, current, event, person, reason, record};
use super::error::{PartnerError, invalid};
use super::mapping_model::{
    ApprovePartnerMappings, ApprovedMappings, CodeMapping, DiscardPartnerMappingDraft, LisRecordId,
    MappingDraft, MappingKind, MappingRow, MappingValues, NewMappingDraft, PartnerMappings,
};
use super::mapping_repo::{self as repo, StoredMapping};
use super::model::{ConnectionChange, PartnerConnectionId, PartnerMessageType};
use super::repo::{ConnectionRow, load_connection, update_connection};
use crate::model::ChangeStamp;
use crate::validation::valid_text;

const CODE_MAX: usize = 50;
const UNIT_MAX: usize = 30;

/// Drafts a mapping change of an HL7 connection; the next approval applies it.
pub fn draft_partner_mapping(
    ctx: &mut CommandContext<'_>,
    draft: NewMappingDraft,
) -> Result<(), PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let input = draft.input;
    let connection = hl7_connection(ctx.tx(), input.connection_id)?;
    let values = input
        .mapping
        .map(|values| valid_values(input.kind, values))
        .transpose()?;
    if values.is_none()
        && approved_mappings(ctx.tx(), connection.id)?
            .is_none_or(|approved| approved.get(input.kind, input.lis_id).is_none())
    {
        return Err(PartnerError::WrongState(
            "This test or analyte has no approved mapping to remove.",
        ));
    }
    let stored = StoredMapping {
        kind: input.kind,
        lis_id: input.lis_id,
        lis_code: draft.lis_code,
        lis_name: draft.lis_name,
        values,
    };
    repo::upsert_draft(ctx.tx(), connection.id, &stored, &actor, ctx.now())?;
    ctx.audit(
        event("interfaces.draft_mapping", &connection)
            .detail("kind", AuditDetail::code(stored.kind.as_str()))
            .detail("lis_id", AuditDetail::id(stored.lis_id))
            .detail("remove", AuditDetail::boolean(stored.values.is_none())),
    )?;
    Ok(())
}

/// Discards one drafted change.
pub fn discard_partner_mapping_draft(
    ctx: &mut CommandContext<'_>,
    input: DiscardPartnerMappingDraft,
) -> Result<(), PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    person(ctx)?;
    let connection = hl7_connection(ctx.tx(), input.connection_id)?;
    let removed = repo::delete_drafts(ctx.tx(), connection.id, Some((input.kind, input.lis_id)))?;
    if removed == 0 {
        return Err(PartnerError::NotFound("mapping draft"));
    }
    ctx.audit(
        event("interfaces.discard_mapping_draft", &connection)
            .detail("kind", AuditDetail::code(input.kind.as_str()))
            .detail("lis_id", AuditDetail::id(input.lis_id)),
    )?;
    Ok(())
}

/// Approves every drafted change as the connection's next mapping version, with a
/// reason; the connection's revision goes up and its history shows the approval.
pub fn approve_partner_mappings(
    ctx: &mut CommandContext<'_>,
    input: ApprovePartnerMappings,
) -> Result<i64, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let reason = reason(&input.reason)?;
    let connection = current(ctx, input.connection_id, input.expected_revision)?;
    hl7_only(&connection)?;
    let drafts = repo::drafts(ctx.tx(), connection.id)?;
    if drafts.is_empty() {
        return Err(PartnerError::WrongState(
            "No mapping change waits for approval.",
        ));
    }
    let self_approval = drafts.iter().any(|draft| draft.drafted_by == actor);
    if self_approval {
        authorize_self(ctx, Duty::ConfigurationApproval, input.self_approval)?;
    }
    let previous = repo::current_version(ctx.tx(), connection.id)?;
    let mut set: BTreeMap<Key, StoredMapping> = BTreeMap::new();
    if let Some((version, _, _)) = previous {
        for mapping in repo::mappings(ctx.tx(), connection.id, version)? {
            set.insert(key(mapping.kind, mapping.lis_id), mapping);
        }
    }
    for draft in &drafts {
        let key = key(draft.mapping.kind, draft.mapping.lis_id);
        match draft.mapping.values {
            Some(_) => set.insert(key, draft.mapping.clone()),
            None => set.remove(&key),
        };
    }
    let version = previous.map_or(1, |(version, _, _)| version + 1);
    let mappings: Vec<StoredMapping> = set.into_values().collect();
    let approval = (reason.as_str(), actor.as_str(), ctx.now(), self_approval);
    repo::insert_version(ctx.tx(), connection.id, version, approval, &mappings)?;
    repo::delete_drafts(ctx.tx(), connection.id, None)?;
    let row = ConnectionRow {
        revision: connection.revision + 1,
        ..connection.clone()
    };
    update_connection(ctx.tx(), &row)?;
    record(
        ctx,
        &row,
        ConnectionChange::MappingsApproved,
        Some(reason.clone()),
    )?;
    let mut event = event("interfaces.approve_mappings", &row)
        .old_revision(connection.revision)
        .new_revision(row.revision)
        .detail("version", AuditDetail::integer(version))
        .detail("changes", AuditDetail::count(drafts.len()))
        .reason(reason);
    if self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    Ok(version)
}

/// The approved version in force, if any.
pub fn approved_mappings(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<Option<ApprovedMappings>, PartnerError> {
    let Some((version, _, _)) = repo::current_version(conn, connection_id)? else {
        return Ok(None);
    };
    let mappings = repo::mappings(conn, connection_id, version)?
        .into_iter()
        .filter_map(|mapping| {
            Some(CodeMapping {
                kind: mapping.kind,
                lis_id: mapping.lis_id,
                values: mapping.values?,
            })
        })
        .collect();
    Ok(Some(ApprovedMappings { version, mappings }))
}

/// A connection's mappings: the approved version, the drafts, and `held`, the LIS tests
/// and analytes (kind, ID, code, name) a waiting message needs, as the engine found them.
pub fn partner_mappings(
    conn: &Connection,
    connection_id: PartnerConnectionId,
    held: &[(MappingKind, LisRecordId, String, String)],
) -> Result<PartnerMappings, PartnerError> {
    let connection = hl7_connection(conn, connection_id)?;
    let current = repo::current_version(conn, connection.id)?;
    let mut rows: BTreeMap<Key, MappingRow> = BTreeMap::new();
    if let Some((version, _, _)) = current {
        for mapping in repo::mappings(conn, connection.id, version)? {
            let row = row_of(&mapping);
            rows.insert(
                key(row.kind, row.lis_id),
                MappingRow {
                    approved: mapping.values,
                    ..row
                },
            );
        }
    }
    let drafts = repo::drafts(conn, connection.id)?;
    for draft in &drafts {
        let entry = rows
            .entry(key(draft.mapping.kind, draft.mapping.lis_id))
            .or_insert_with(|| row_of(&draft.mapping));
        entry.draft = Some(MappingDraft {
            values: draft.mapping.values.clone(),
            drafted: ChangeStamp {
                by: draft.drafted_by.clone(),
                by_display: None,
                at: draft.drafted_at,
            },
        });
    }
    for (kind, lis_id, lis_code, lis_name) in held {
        rows.entry(key(*kind, *lis_id))
            .or_insert_with(|| MappingRow {
                kind: *kind,
                lis_id: *lis_id,
                lis_code: lis_code.clone(),
                lis_name: lis_name.clone(),
                approved: None,
                draft: None,
                held: false,
            })
            .held = true;
    }
    let mut rows: Vec<MappingRow> = rows.into_values().collect();
    rows.sort_by_key(|row| (row.kind != MappingKind::Test, row.lis_name.to_lowercase()));
    let mut allowed_actions = vec!["draft_partner_mapping".to_owned()];
    if !drafts.is_empty() {
        allowed_actions.push("discard_partner_mapping_draft".to_owned());
        allowed_actions.push("approve_partner_mappings".to_owned());
    }
    Ok(PartnerMappings {
        connection_id: connection.id,
        version: current.as_ref().map(|(version, _, _)| *version),
        approved: current.map(|(_, by, at)| ChangeStamp {
            by,
            by_display: None,
            at,
        }),
        rows,
        allowed_actions,
    })
}

fn row_of(mapping: &StoredMapping) -> MappingRow {
    MappingRow {
        kind: mapping.kind,
        lis_id: mapping.lis_id,
        lis_code: mapping.lis_code.clone(),
        lis_name: mapping.lis_name.clone(),
        approved: None,
        draft: None,
        held: false,
    }
}

/// A test or analyte by kind and LIS ID.
type Key = (&'static str, LisRecordId);

fn key(kind: MappingKind, lis_id: LisRecordId) -> Key {
    (kind.as_str(), lis_id)
}

fn hl7_connection(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<ConnectionRow, PartnerError> {
    let connection =
        load_connection(conn, connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    hl7_only(&connection)?;
    Ok(connection)
}

fn hl7_only(connection: &ConnectionRow) -> Result<(), PartnerError> {
    match connection.settings.message_type {
        PartnerMessageType::Hl7Oru => Ok(()),
        PartnerMessageType::BillingJson | PartnerMessageType::ReportPdf => Err(
            PartnerError::WrongState("Only HL7 connections have code mappings."),
        ),
    }
}

/// The partner's code is required; a LOINC code looks like `12345-6`; a unit belongs to
/// an analyte only.
fn valid_values(kind: MappingKind, values: MappingValues) -> Result<MappingValues, PartnerError> {
    let partner_code = required_text("mapping.partnerCode", &values.partner_code, CODE_MAX)
        .map_err(PartnerError::Invalid)?;
    let loinc = optional(values.loinc);
    if let Some(loinc) = &loinc
        && !is_loinc(loinc)
    {
        return Err(invalid(
            "mapping.loinc",
            "invalid_loinc",
            "Enter a LOINC code such as 12345-6, or leave it empty to use the catalog's.",
        ));
    }
    let ucum_unit = optional(values.ucum_unit);
    if let Some(unit) = &ucum_unit {
        if kind == MappingKind::Test {
            return Err(invalid(
                "mapping.ucumUnit",
                "not_for_tests",
                "Only an analyte has a unit.",
            ));
        }
        if !valid_text(unit, UNIT_MAX) || unit.contains(char::is_whitespace) {
            return Err(invalid(
                "mapping.ucumUnit",
                "invalid_unit",
                "Enter a UCUM unit of up to 30 characters without spaces, such as ng/mL.",
            ));
        }
    }
    Ok(MappingValues {
        partner_code,
        loinc,
        ucum_unit,
    })
}

fn optional(text: Option<String>) -> Option<String> {
    text.map(|text| text.trim().to_owned())
        .filter(|text| !text.is_empty())
}

fn is_loinc(code: &str) -> bool {
    code.split_once('-').is_some_and(|(number, check)| {
        (1..=7).contains(&number.len())
            && number.bytes().all(|byte| byte.is_ascii_digit())
            && check.len() == 1
            && check.bytes().all(|byte| byte.is_ascii_digit())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_are_checked_and_trimmed() {
        let values = |code: &str, loinc: Option<&str>, unit: Option<&str>| MappingValues {
            partner_code: code.to_owned(),
            loinc: loinc.map(str::to_owned),
            ucum_unit: unit.map(str::to_owned),
        };
        let valid = valid_values(
            MappingKind::Analyte,
            values(" CA-AMP ", Some(" "), Some("ng/mL")),
        )
        .unwrap();
        assert_eq!(valid, values("CA-AMP", None, Some("ng/mL")));
        assert!(valid_values(MappingKind::Analyte, values("", None, None)).is_err());
        assert!(valid_values(MappingKind::Analyte, values("X", Some("1234"), None)).is_err());
        assert!(valid_values(MappingKind::Test, values("X", None, Some("ng/mL"))).is_err());
        assert!(valid_values(MappingKind::Analyte, values("X", None, Some("ng mL"))).is_err());
        assert!(valid_values(MappingKind::Test, values("X", Some("12345-6"), None)).is_ok());
    }
}
