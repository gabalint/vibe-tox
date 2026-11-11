//! Interface contracts of connections (ticket #4555 step 5a; REQ-INT-001): a person drafts
//! the agreed terms, and a second person approves them as the connection's next version,
//! or the person who drafted them where the laboratory's separation-of-duties policy allows
//! (ADR 0015). Versions are kept; approving one makes the acceptance tests run again.

use lis_audit::AuditDetail;
use lis_auth::Duty;
use lis_core::{Capability, UtcTimestamp, optional_text, required_text};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use super::acceptance::acceptance_of;
use super::connections::{authorize_self, current, event, person, reason, record};
use super::contract_model::{
    ApprovePartnerContract, ContractDraft, ContractEdge, ContractVersion, DraftPartnerContract,
    InterfaceContract, PartnerContract,
};
use super::error::{PartnerError, invalid};
use super::model::{ConnectionChange, PartnerConnectionId};
use super::repo::{ConnectionRow, load_connection, update_connection};
use crate::model::ChangeStamp;

const PROFILE_MAX: usize = 200;
const TERMS_MAX: usize = 500;
const ENGINE_MAX: usize = 120;

/// Drafts the connection's next contract version; a newer draft replaces it.
pub fn draft_partner_contract(
    ctx: &mut CommandContext<'_>,
    input: DraftPartnerContract,
) -> Result<(), PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let row = load_connection(ctx.tx(), input.connection_id)?
        .ok_or(PartnerError::NotFound("connection"))?;
    let contract = valid_contract(input.contract)?;
    ctx.tx().execute(
        "INSERT INTO ifc_partner_contract_drafts (connection_id, contract, drafted_by, drafted_at) \
         VALUES (?1, ?2, ?3, ?4) ON CONFLICT (connection_id) DO UPDATE SET \
         contract = excluded.contract, drafted_by = excluded.drafted_by, \
         drafted_at = excluded.drafted_at",
        params![row.id, serde_json::to_string(&contract)?, actor, ctx.now()],
    )?;
    ctx.audit(
        event("interfaces.draft_contract", &row)
            .detail("edge", AuditDetail::code(contract.edge.as_str())),
    )?;
    Ok(())
}

/// Approves the draft as the connection's next contract version, with a reason; the
/// connection's revision goes up and its history shows the approval.
pub fn approve_partner_contract(
    ctx: &mut CommandContext<'_>,
    input: ApprovePartnerContract,
) -> Result<i64, PartnerError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = person(ctx)?;
    let reason = reason(&input.reason)?;
    let connection = current(ctx, input.connection_id, input.expected_revision)?;
    let Some(draft) = draft_of(ctx.tx(), connection.id)? else {
        return Err(PartnerError::WrongState(
            "No contract draft waits for approval.",
        ));
    };
    let self_approval = draft.drafted.by == actor;
    if self_approval {
        authorize_self(ctx, Duty::ConfigurationApproval, input.self_approval)?;
    }
    let version = latest_version(ctx.tx(), connection.id)? + 1;
    ctx.tx().execute(
        "INSERT INTO ifc_partner_contracts (connection_id, version, contract, drafted_by, \
         drafted_at, reason, approved_by, approved_at, self_approval) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
        params![
            connection.id,
            version,
            serde_json::to_string(&draft.contract)?,
            draft.drafted.by,
            draft.drafted.at,
            reason,
            actor,
            ctx.now(),
            self_approval
        ],
    )?;
    ctx.tx().execute(
        "DELETE FROM ifc_partner_contract_drafts WHERE connection_id = ?1",
        [connection.id],
    )?;
    let row = ConnectionRow {
        revision: connection.revision + 1,
        ..connection.clone()
    };
    update_connection(ctx.tx(), &row)?;
    record(
        ctx,
        &row,
        ConnectionChange::ContractApproved,
        Some(reason.clone()),
    )?;
    let mut event = event("interfaces.approve_contract", &row)
        .old_revision(connection.revision)
        .new_revision(row.revision)
        .detail("version", AuditDetail::integer(version))
        .reason(reason);
    if self_approval {
        event = event.self_approval();
    }
    ctx.audit(event)?;
    Ok(version)
}

/// The connection's contract in force, its draft and its acceptance tests.
pub fn partner_contract(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<PartnerContract, PartnerError> {
    let row = load_connection(conn, connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    let current = current_version(conn, row.id)?;
    let draft = draft_of(conn, row.id)?;
    let (acceptance, last_run) = acceptance_of(conn, row.id)?;
    let mut allowed_actions = vec!["draft_partner_contract".to_owned()];
    if draft.is_some() {
        allowed_actions.push("approve_partner_contract".to_owned());
    }
    if current.is_some() {
        allowed_actions.push("run_partner_acceptance_tests".to_owned());
    }
    Ok(PartnerContract {
        connection_id: row.id,
        current,
        draft,
        acceptance,
        last_run,
        allowed_actions,
    })
}

fn latest_version(conn: &Connection, id: PartnerConnectionId) -> Result<i64, PartnerError> {
    Ok(conn.query_row(
        "SELECT COALESCE(MAX(version), 0) FROM ifc_partner_contracts WHERE connection_id = ?1",
        [id],
        |row| row.get(0),
    )?)
}

fn current_version(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<Option<ContractVersion>, PartnerError> {
    let row = conn
        .query_row(
            "SELECT version, contract, drafted_by, drafted_at, approved_by, approved_at, \
             reason, self_approval FROM ifc_partner_contracts WHERE connection_id = ?1 \
             ORDER BY version DESC LIMIT 1",
            [id],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    stamp(row.get(2)?, row.get(3)?),
                    stamp(row.get(4)?, row.get(5)?),
                    row.get::<_, String>(6)?,
                    row.get::<_, bool>(7)?,
                ))
            },
        )
        .optional()?;
    row.map(
        |(version, contract, drafted, approved, reason, self_approval)| {
            Ok(ContractVersion {
                version,
                contract: serde_json::from_str(&contract)?,
                drafted,
                approved,
                reason,
                self_approval,
            })
        },
    )
    .transpose()
}

fn draft_of(
    conn: &Connection,
    id: PartnerConnectionId,
) -> Result<Option<ContractDraft>, PartnerError> {
    let row = conn
        .query_row(
            "SELECT contract, drafted_by, drafted_at FROM ifc_partner_contract_drafts \
             WHERE connection_id = ?1",
            [id],
            |row| Ok((row.get::<_, String>(0)?, stamp(row.get(1)?, row.get(2)?))),
        )
        .optional()?;
    row.map(|(contract, drafted)| {
        Ok(ContractDraft {
            contract: serde_json::from_str(&contract)?,
            drafted,
        })
    })
    .transpose()
}

const fn stamp(by: String, at: UtcTimestamp) -> ChangeStamp {
    ChangeStamp {
        by,
        by_display: None,
        at,
    }
}

/// Every term is required but the deviations; the engine is named when one sits between.
fn valid_contract(contract: InterfaceContract) -> Result<InterfaceContract, PartnerError> {
    let text = |field: &str, value: &str, max| -> Result<String, PartnerError> {
        required_text(&format!("contract.{field}"), value, max).map_err(PartnerError::Invalid)
    };
    let engine = optional_text("contract.engine", contract.engine.as_deref(), ENGINE_MAX)
        .map_err(PartnerError::Invalid)?;
    let engine = match (contract.edge, engine) {
        (ContractEdge::LabEngine, None) => {
            return Err(invalid(
                "contract.engine",
                "required",
                "Name the interface engine and its version.",
            ));
        }
        (ContractEdge::LabEngine, engine) => engine,
        (ContractEdge::Lis, _) => None,
    };
    let deviations = optional_text("contract.deviations", Some(&contract.deviations), TERMS_MAX)
        .map_err(PartnerError::Invalid)?
        .unwrap_or_default();
    Ok(InterfaceContract {
        profile: text("profile", &contract.profile, PROFILE_MAX)?,
        acknowledgements: text("acknowledgements", &contract.acknowledgements, TERMS_MAX)?,
        corrections: text("corrections", &contract.corrections, TERMS_MAX)?,
        edge: contract.edge,
        engine,
        owners: text("owners", &contract.owners, TERMS_MAX)?,
        deviations,
    })
}
