//! Attributable directory corrections and retirement without replacing record IDs (DM-05).
use crate::error::{IdentityError, ensure_revision, invalid};
use crate::model::{
    ActorRef, CODE_MAX, ClientId, ClientLocationId, NAME_MAX, Npi, PractitionerId, REASON_MAX,
    optional_text, required_text,
};
use crate::views::{ClientLocationView, ClientView, PractitionerView};
use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Deserialize;
use ts_rs::TS;

mod history;
pub use history::{
    DirectoryHistoryEntry, DirectoryHistoryInput, DirectoryRecord, DirectorySnapshot,
    directory_history,
};

/// Captures current projected values once, in the same transaction as registration.
pub(crate) fn record_registration(
    ctx: &mut CommandContext<'_>,
    record: DirectoryRecord,
) -> Result<(), IdentityError> {
    let actor = ActorRef::user(ctx.principal())?;
    let (kind, id) = record.key();
    let sql = match record {
        DirectoryRecord::Client(_) => {
            "SELECT code,name,NULL,active,revision FROM idn_client_organizations WHERE id=?1"
        }
        DirectoryRecord::Location(_) => {
            "SELECT code,name,NULL,active,revision FROM idn_client_locations WHERE id=?1"
        }
        DirectoryRecord::Practitioner(_) => {
            "SELECT npi,family_name,given_names,active,revision FROM idn_practitioners WHERE id=?1"
        }
    };
    let (code, name, given, active, revision): (String, String, Option<String>, bool, i64) =
        ctx.tx().query_row(sql, [&id], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?))
        })?;
    ctx.tx().execute("INSERT INTO idn_directory_history(record_kind,record_id,revision,code,name,given_names,active,recorded_at,recorded_by,origin) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,'registered')",params![kind,id,revision,code,name,given,active,ctx.now(),actor.id])?;
    Ok(())
}

struct Correction<'a> {
    record: DirectoryRecord,
    revision: i64,
    code: &'a str,
    name: &'a str,
    given: Option<&'a str>,
    active: bool,
    reason: &'a str,
}
fn append_correction(
    ctx: &mut CommandContext<'_>,
    value: Correction<'_>,
) -> Result<(), IdentityError> {
    let actor = ActorRef::user(ctx.principal())?;
    let reason = required_text("reason", value.reason, REASON_MAX)?;
    let (kind, id) = value.record.key();
    ctx.tx().execute("INSERT INTO idn_directory_history(record_kind,record_id,revision,code,name,given_names,active,recorded_at,recorded_by,reason,origin) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,'corrected')",params![kind,id,value.revision+1,value.code,value.name,value.given,value.active,ctx.now(),actor.id,reason])?;
    let action = match value.record {
        DirectoryRecord::Client(_) => "identity.update_client",
        DirectoryRecord::Location(_) => "identity.update_client_location",
        DirectoryRecord::Practitioner(_) => "identity.update_practitioner",
    };
    ctx.audit(
        AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded)
            .record(kind, id)
            .old_revision(value.revision)
            .new_revision(value.revision + 1)
            .reason(reason),
    )?;
    Ok(())
}

/// Corrects a client or changes its availability for new ordering.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateClient {
    /// Stable client identity.
    pub client_id: ClientId,
    /// Revision displayed before editing.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Correct code, still unique among all retained clients.
    pub code: String,
    /// Correct display name.
    pub name: String,
    /// Availability for new ordering; false retires the selection.
    pub active: bool,
    /// Evidence-backed explanation; never a transfer to a different organization.
    pub reason: String,
}
/// Corrects client facts without rewriting earlier history or issued reports.
pub fn update_client(
    ctx: &mut CommandContext<'_>,
    input: UpdateClient,
) -> Result<ClientView, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let old = crate::get_client(ctx.tx(), input.client_id)?;
    ensure_revision(old.revision, input.expected_revision)?;
    let code = required_text("code", &input.code, CODE_MAX)?;
    let name = required_text("name", &input.name, NAME_MAX)?;
    if ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM idn_client_organizations WHERE code=?1 AND id<>?2)",
        params![code, input.client_id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(IdentityError::Duplicate("client"));
    }
    append_correction(
        ctx,
        Correction {
            record: DirectoryRecord::Client(input.client_id),
            revision: old.revision,
            code: &code,
            name: &name,
            given: None,
            active: input.active,
            reason: &input.reason,
        },
    )?;
    ctx.tx().execute("UPDATE idn_client_organizations SET code=?1,name=?2,active=?3,revision=revision+1 WHERE id=?4",params![code,name,input.active,input.client_id])?;
    crate::get_client(ctx.tx(), input.client_id)
}

/// Corrects a location within its original client.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateClientLocation {
    /// Stable location identity.
    pub location_id: ClientLocationId,
    /// Revision displayed before editing.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Correct code, unique within the original client.
    pub code: String,
    /// Correct display name.
    pub name: String,
    /// Availability for new ordering.
    pub active: bool,
    /// Required correction/retirement explanation.
    pub reason: String,
}
/// Corrects location facts; parent client and record identity cannot change.
pub fn update_client_location(
    ctx: &mut CommandContext<'_>,
    input: UpdateClientLocation,
) -> Result<ClientLocationView, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let (client, revision): (ClientId, i64) = ctx
        .tx()
        .query_row(
            "SELECT client_id,revision FROM idn_client_locations WHERE id=?1",
            [input.location_id],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .ok_or(IdentityError::NotFound("client location"))?;
    ensure_revision(revision, input.expected_revision)?;
    let code = required_text("code", &input.code, CODE_MAX)?;
    let name = required_text("name", &input.name, NAME_MAX)?;
    if ctx.tx().query_row("SELECT EXISTS(SELECT 1 FROM idn_client_locations WHERE client_id=?1 AND code=?2 AND id<>?3)",params![client,code,input.location_id],|r|r.get::<_,bool>(0))? { return Err(IdentityError::Duplicate("client location")); }
    append_correction(
        ctx,
        Correction {
            record: DirectoryRecord::Location(input.location_id),
            revision,
            code: &code,
            name: &name,
            given: None,
            active: input.active,
            reason: &input.reason,
        },
    )?;
    ctx.tx().execute(
        "UPDATE idn_client_locations SET code=?1,name=?2,active=?3,revision=revision+1 WHERE id=?4",
        params![code, name, input.active, input.location_id],
    )?;
    Ok(ClientLocationView {
        id: input.location_id,
        code,
        name,
        active: input.active,
        revision: revision + 1,
    })
}

/// Corrects a practitioner's supplied facts; does not authorize a different person.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdatePractitioner {
    /// Stable practitioner identity.
    pub practitioner_id: PractitionerId,
    /// Revision displayed before editing.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Correct NPI with valid check digit, unique among retained practitioners.
    pub npi: String,
    /// Correct supplied family name.
    pub family_name: String,
    /// Correct supplied given names, if known.
    pub given_names: Option<String>,
    /// Availability for new ordering.
    pub active: bool,
    /// Required explanation of the factual correction or retirement.
    pub reason: String,
}
/// Corrects practitioner facts with immutable prior versions and current authorization checks.
pub fn update_practitioner(
    ctx: &mut CommandContext<'_>,
    input: UpdatePractitioner,
) -> Result<PractitionerView, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let old = crate::get_practitioner(ctx.tx(), input.practitioner_id)?;
    ensure_revision(old.revision, input.expected_revision)?;
    let npi = Npi::parse(&input.npi)?;
    let name = required_text("familyName", &input.family_name, NAME_MAX)?;
    let given = optional_text("givenNames", input.given_names.as_deref(), NAME_MAX)?;
    if ctx.tx().query_row(
        "SELECT EXISTS(SELECT 1 FROM idn_practitioners WHERE npi=?1 AND id<>?2)",
        params![npi.as_str(), input.practitioner_id],
        |r| r.get::<_, bool>(0),
    )? {
        return Err(IdentityError::Duplicate("practitioner"));
    }
    append_correction(
        ctx,
        Correction {
            record: DirectoryRecord::Practitioner(input.practitioner_id),
            revision: old.revision,
            code: npi.as_str(),
            name: &name,
            given: given.as_deref(),
            active: input.active,
            reason: &input.reason,
        },
    )?;
    ctx.tx().execute("UPDATE idn_practitioners SET npi=?1,family_name=?2,given_names=?3,active=?4,revision=revision+1 WHERE id=?5",params![npi.as_str(),name,given,input.active,input.practitioner_id])?;
    crate::get_practitioner(ctx.tx(), input.practitioner_id)
}

/// New ordering must use currently active source directory records. Existing accepted
/// orders retain their identities and are not stopped by later directory retirement.
pub fn assert_active_ordering_source(
    conn: &Connection,
    client_id: ClientId,
    location_id: Option<ClientLocationId>,
    practitioner_id: PractitionerId,
) -> Result<(), IdentityError> {
    let client = crate::get_client(conn, client_id)?;
    if !client.active {
        return Err(invalid(
            "clientId",
            "retired",
            "Choose an active client for new ordering.",
        ));
    }
    if let Some(id) = location_id {
        let location = client
            .locations
            .iter()
            .find(|l| l.id == id)
            .ok_or_else(|| {
                invalid(
                    "clientLocationId",
                    "wrong_client",
                    "Choose a location belonging to the selected client.",
                )
            })?;
        if !location.active {
            return Err(invalid(
                "clientLocationId",
                "retired",
                "Choose an active client location for new ordering.",
            ));
        }
    }
    if !crate::get_practitioner(conn, practitioner_id)?.active {
        return Err(invalid(
            "practitionerId",
            "retired",
            "Choose an active practitioner for new ordering.",
        ));
    }
    Ok(())
}
/// Active client selections with only active locations; history queries keep all records.
pub fn list_active_clients(conn: &Connection) -> Result<Vec<ClientView>, IdentityError> {
    let mut rows = crate::list_clients(conn)?;
    rows.retain(|r| r.active);
    for r in &mut rows {
        r.locations.retain(|l| l.active);
    }
    Ok(rows)
}
/// Active practitioner selections; historical lookup by ID remains available.
pub fn list_active_practitioners(
    conn: &Connection,
) -> Result<Vec<PractitionerView>, IdentityError> {
    Ok(crate::list_practitioners(conn)?
        .into_iter()
        .filter(|r| r.active)
        .collect())
}
