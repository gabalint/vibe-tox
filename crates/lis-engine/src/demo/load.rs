//! The `demo.load_demo_data` transaction (proposed REQ-OPS-016).

use std::collections::{BTreeMap, BTreeSet};

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_identity::{AddClientLocation, ClientLocationId, RegisterSubject, SubjectId};
use lis_ops::InstallationKind;
use lis_reporting::CreateDeliveryDestinationInput;
use lis_store::CommandContext;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::DemoError;
use super::cutoffs::fill_cutoffs;
use super::file::{DemoClient, DemoFile, DemoFileError, DemoPatient};

/// Audit action of a demo data load.
pub(crate) const LOAD_DEMO_DATA: &str = "demo.load_demo_data";

/// What a demo data load created. Loading again creates no parties and reports zeros.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct DemoDataLoaded {
    /// The demo file's content version.
    pub data_version: String,
    /// The file's statement that everything in it is fictitious, shown after loading.
    pub notice: String,
    /// The fictitious laboratory profile was set; an existing profile is kept.
    pub laboratory_profile_set: bool,
    #[ts(type = "number")]
    pub clients: usize,
    #[ts(type = "number")]
    pub client_locations: usize,
    #[ts(type = "number")]
    pub delivery_destinations: usize,
    #[ts(type = "number")]
    pub practitioners: usize,
    #[ts(type = "number")]
    pub patients: usize,
    /// Possible-duplicate identity holds the patients opened, by design of the file.
    #[ts(type = "number")]
    pub identity_holds: usize,
    /// Blank cutoffs of the caller's method drafts filled with demo values.
    #[ts(type = "number")]
    pub demo_cutoffs_filled: usize,
    /// Blank cutoffs left because another user authored the draft; only a draft's author
    /// may edit it, so the Standard Catalog must be loaded by the same user first.
    #[ts(type = "number")]
    pub demo_cutoffs_not_filled: usize,
}

/// Whether the file's parties are in the database: all, none or only some client codes.
enum Parties {
    Absent,
    Loaded,
    Partial,
}

fn parties(conn: &Connection, file: &DemoFile) -> Result<Parties, DemoError> {
    let existing: BTreeSet<String> = lis_identity::list_clients(conn)?
        .into_iter()
        .map(|client| client.code)
        .collect();
    let present = file
        .clients
        .iter()
        .filter(|client| existing.contains(&client.code))
        .count();
    Ok(match present {
        0 => Parties::Absent,
        n if n == file.clients.len() => Parties::Loaded,
        _ => Parties::Partial,
    })
}

/// Whether the demo file's parties are loaded.
pub(crate) fn demo_data_loaded(conn: &Connection) -> Result<bool, DemoError> {
    let (file, _) = DemoFile::embedded()?;
    Ok(matches!(parties(conn, &file)?, Parties::Loaded))
}

/// Loads the demo file in the caller's transaction: refused outside a demo installation;
/// creates the parties unless they are loaded, then fills the caller's blank draft cutoffs.
pub(crate) fn load(
    ctx: &mut CommandContext<'_>,
    kind: InstallationKind,
) -> Result<DemoDataLoaded, DemoError> {
    ctx.require(Capability::ManageConfiguration)?;
    // Proposed REQ-OPS-016: demo records never enter a laboratory's database.
    if kind != InstallationKind::Demo {
        return Err(DemoError::NotDemo);
    }
    let (file, digest) = DemoFile::embedded()?;
    let mut loaded = DemoDataLoaded {
        data_version: file.data_version.clone(),
        notice: file.notice.clone(),
        ..DemoDataLoaded::default()
    };
    match parties(ctx.tx(), &file)? {
        Parties::Absent => load_parties(ctx, &file, &mut loaded)?,
        Parties::Loaded => {}
        Parties::Partial => return Err(DemoError::PartlyLoaded),
    }
    fill_cutoffs(ctx, &file.demo_values(), &mut loaded)?;
    ctx.audit(
        AuditEvent::new(Stream::Clinical, LOAD_DEMO_DATA, Outcome::Succeeded)
            .detail(
                "data_version",
                AuditDetail::machine_token(&file.data_version)?,
            )
            .detail("file_digest", AuditDetail::sha256(&digest)?)
            .detail("clients", AuditDetail::count(loaded.clients))
            .detail("patients", AuditDetail::count(loaded.patients))
            .detail(
                "demo_cutoffs_filled",
                AuditDetail::count(loaded.demo_cutoffs_filled),
            ),
    )?;
    Ok(loaded)
}

fn load_parties(
    ctx: &mut CommandContext<'_>,
    file: &DemoFile,
    loaded: &mut DemoDataLoaded,
) -> Result<(), DemoError> {
    if lis_identity::current_laboratory_profile(ctx.tx())?.is_none() {
        lis_identity::set_laboratory_profile(ctx, file.laboratory_profile.clone())?;
        loaded.laboratory_profile_set = true;
    }
    for client in &file.clients {
        load_client(ctx, client, loaded)?;
    }
    for practitioner in &file.practitioners {
        lis_identity::register_practitioner(ctx, practitioner.clone())?;
        loaded.practitioners += 1;
    }
    load_patients(ctx, &file.patients, loaded)
}

fn load_client(
    ctx: &mut CommandContext<'_>,
    client: &DemoClient,
    loaded: &mut DemoDataLoaded,
) -> Result<(), DemoError> {
    let client_id = lis_identity::create_client(ctx, client.input())?;
    loaded.clients += 1;
    let mut locations: BTreeMap<&str, ClientLocationId> = BTreeMap::new();
    for location in &client.locations {
        let input = AddClientLocation {
            client_id,
            code: location.code.clone(),
            name: location.name.clone(),
        };
        locations.insert(
            &location.code,
            lis_identity::add_client_location(ctx, input)?,
        );
        loaded.client_locations += 1;
    }
    for destination in &client.destinations {
        let client_location_id = match destination.location_code.as_deref() {
            Some(code) => Some(
                *locations
                    .get(code)
                    .ok_or(DemoFileError("destination location codes"))?,
            ),
            None => None,
        };
        let input = CreateDeliveryDestinationInput {
            client_id,
            client_location_id,
            channel: destination.channel,
            address: destination.address.clone(),
            label: destination.label.clone(),
        };
        lis_reporting::create_delivery_destination(ctx, input)?;
        loaded.delivery_destinations += 1;
    }
    Ok(())
}

/// Registers the patients in file order; `acknowledgeCandidates` names earlier patients,
/// whose subjects are the candidates the registration must acknowledge (REQ-IDN-001).
fn load_patients(
    ctx: &mut CommandContext<'_>,
    patients: &[DemoPatient],
    loaded: &mut DemoDataLoaded,
) -> Result<(), DemoError> {
    let mut subjects: BTreeMap<&str, SubjectId> = BTreeMap::new();
    for patient in patients {
        let acknowledged_candidates = patient
            .acknowledge_candidates
            .iter()
            .map(|key| {
                subjects
                    .get(key.as_str())
                    .copied()
                    .ok_or(DemoFileError("patient keys"))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let registered = lis_identity::register_subject(
            ctx,
            RegisterSubject {
                demographics: patient.demographics.clone(),
                source: patient.source.clone(),
                identifiers: patient.identifiers.clone(),
                acknowledged_candidates,
            },
        )?;
        loaded.patients += 1;
        loaded.identity_holds += usize::from(registered.identity_hold_id.is_some());
        subjects.insert(&patient.key, registered.subject_id);
    }
    Ok(())
}
