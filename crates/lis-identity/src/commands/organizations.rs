//! Client organizations, their locations and practitioners.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::error::IdentityError;
use crate::model::{
    CODE_MAX, ClientId, ClientLocationId, NAME_MAX, Npi, PractitionerId, optional_text,
    required_text,
};
use crate::repo::organizations::{self as repo, NewPractitioner};

/// Input of [`create_client`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateClient {
    /// Lab-assigned code, unique.
    pub code: String,
    /// Display name.
    pub name: String,
}

/// Creates a client organization.
pub fn create_client(
    ctx: &mut CommandContext<'_>,
    input: CreateClient,
) -> Result<ClientId, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let code = required_text("code", &input.code, CODE_MAX)?;
    let name = required_text("name", &input.name, NAME_MAX)?;
    if repo::client_code_exists(ctx.tx(), &code)? {
        return Err(IdentityError::Duplicate("client"));
    }
    let id = ClientId::generate();
    repo::insert_client(ctx.tx(), id, &code, &name, ctx.now())?;
    crate::directories::record_registration(ctx, crate::directories::DirectoryRecord::Client(id))?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.create_client",
            Outcome::Succeeded,
        )
        .record("client", id),
    )?;
    Ok(id)
}

/// Input of [`add_client_location`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AddClientLocation {
    /// The client organization.
    pub client_id: ClientId,
    /// Code, unique within the client.
    pub code: String,
    /// Display name.
    pub name: String,
}

/// Adds a location to a client organization.
pub fn add_client_location(
    ctx: &mut CommandContext<'_>,
    input: AddClientLocation,
) -> Result<ClientLocationId, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let code = required_text("code", &input.code, CODE_MAX)?;
    let name = required_text("name", &input.name, NAME_MAX)?;
    if !repo::client_exists(ctx.tx(), input.client_id)? {
        return Err(IdentityError::NotFound("client"));
    }
    if repo::location_code_exists(ctx.tx(), input.client_id, &code)? {
        return Err(IdentityError::Duplicate("client location"));
    }
    let id = ClientLocationId::generate();
    repo::insert_location(ctx.tx(), id, input.client_id, &code, &name, ctx.now())?;
    crate::directories::record_registration(
        ctx,
        crate::directories::DirectoryRecord::Location(id),
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.add_client_location",
            Outcome::Succeeded,
        )
        .record("client_location", id)
        .detail("client_id", AuditDetail::id(input.client_id)),
    )?;
    Ok(id)
}

/// Input of [`register_practitioner`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterPractitioner {
    /// The NPI: 10 digits with a valid check digit.
    pub npi: String,
    /// Family name.
    pub family_name: String,
    /// Given names.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub given_names: Option<String>,
}

/// Registers an ordering practitioner identified by NPI.
pub fn register_practitioner(
    ctx: &mut CommandContext<'_>,
    input: RegisterPractitioner,
) -> Result<PractitionerId, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let npi = Npi::parse(&input.npi)?;
    let family_name = required_text("familyName", &input.family_name, NAME_MAX)?;
    let given_names = optional_text("givenNames", input.given_names.as_deref(), NAME_MAX)?;
    if repo::npi_exists(ctx.tx(), &npi)? {
        return Err(IdentityError::Duplicate("practitioner"));
    }
    let id = PractitionerId::generate();
    repo::insert_practitioner(
        ctx.tx(),
        &NewPractitioner {
            id,
            npi: &npi,
            family_name: &family_name,
            given_names: given_names.as_deref(),
            created_at: ctx.now(),
        },
    )?;
    crate::directories::record_registration(
        ctx,
        crate::directories::DirectoryRecord::Practitioner(id),
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "identity.register_practitioner",
            Outcome::Succeeded,
        )
        .record("practitioner", id),
    )?;
    Ok(id)
}
