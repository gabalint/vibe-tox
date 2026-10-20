//! Instrument readiness and analyzer source registrations (REQ-ANA-003
//! registration part, REQ-ANA-007).

use std::collections::BTreeSet;

use lis_audit::{AuditDetail, AuditEvent};
use lis_auth::Duty;
use lis_catalog::AnalyteId;
use lis_core::{Capability, InstrumentId};
use lis_interfaces::InstrumentTransport;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::{
    CODE_MAX, NAME_MAX, actor, au_sources, clinical, instrument_name, reason, require_user,
};
use crate::actions;
use crate::error::{AnalyticalError, ensure_revision, invalid};
use crate::model::{
    InstrumentStatus, RegistrationState, RevisionIndicator, SampleIdField, SourceRegistrationId,
};
use crate::queries;
use crate::repo::readiness::{self as repo, NewRegistration};
use crate::views::{InstrumentReadinessView, SourceRegistrationView};

/// Input of [`set_instrument_status`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SetInstrumentStatusInput {
    /// The instrument.
    pub instrument_id: InstrumentId,
    /// In service or out of service.
    pub status: InstrumentStatus,
    /// Why, as entered.
    pub reason: String,
}

/// Records an instrument status event (REQ-ANA-007). Runs need the latest
/// status to be in service; an instrument with no event is not ready.
pub fn set_instrument_status(
    ctx: &mut CommandContext<'_>,
    input: SetInstrumentStatusInput,
) -> Result<InstrumentReadinessView, AnalyticalError> {
    ctx.require(Capability::ManageQuality)?;
    let reason = reason(&input.reason)?;
    let name = instrument_name(ctx.tx(), input.instrument_id)?;
    let by = actor(ctx);
    let event = repo::insert_status_event(
        ctx.tx(),
        input.instrument_id,
        input.status,
        &reason,
        &by,
        ctx.now(),
    )?;
    ctx.audit(
        clinical(actions::SET_INSTRUMENT_STATUS)
            .record("instrument", input.instrument_id)
            .detail("status_event", AuditDetail::id(event))
            .detail("status", AuditDetail::code(input.status.as_str()))
            .reason(reason),
    )?;
    queries::instrument_readiness(ctx.tx(), input.instrument_id, name)
}

/// One test code of [`RegisterSourceInput`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SourceTestCodeInput {
    /// The manufacturer's local test code (R.3), exactly as the source sends it.
    pub source_test_code: String,
    /// The catalog analyte it reports.
    pub analyte_id: AnalyteId,
}

/// Input of [`register_source`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterSourceInput {
    /// The sending instrument.
    pub instrument_id: InstrumentId,
    /// ASTM H.5 sender name, or the declared device identity for an AU source.
    pub sender_name: String,
    /// ASTM header software version, or the declared AU software version.
    pub sender_version: String,
    /// Which order field carries the sample label.
    pub sample_id_field: SampleIdField,
    /// How the source marks corrected re-exports.
    pub revision_indicator: RevisionIndicator,
    /// Whether the source reports final (dilution-corrected) concentrations.
    pub reports_final_concentration: bool,
    /// The test-code map; at least one code.
    pub test_codes: Vec<SourceTestCodeInput>,
    /// Exact AU wire settings and declared numeric units; absent for ASTM.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub au_profile: Option<crate::AuSourceProfile>,
}

/// Records a draft ASTM or AU source registration with its test-code map
/// (REQ-ANA-003). It takes effect only when approved ([`approve_source`]). A
/// result-file instrument is refused: its source is a file template
/// (`register_file_source`), and an ASTM registration would leave its files
/// without a template to read them with.
pub fn register_source(
    ctx: &mut CommandContext<'_>,
    input: RegisterSourceInput,
) -> Result<SourceRegistrationView, AnalyticalError> {
    ctx.require(Capability::ManageConfiguration)?;
    let sender_name = lis_core::required_text("senderName", &input.sender_name, NAME_MAX)?;
    let sender_version = lis_core::required_text("senderVersion", &input.sender_version, NAME_MAX)?;
    let codes = test_codes(ctx, &input.test_codes)?;
    au_sources::validate_profile(
        input.au_profile.as_ref(),
        &codes,
        input.sample_id_field,
        input.revision_indicator,
    )?;
    au_sources::validate_connection(ctx, input.instrument_id, input.au_profile.as_ref(), false)?;
    let (id, mut event) = insert_draft(
        ctx,
        &Draft {
            instrument_id: input.instrument_id,
            sender_name: &sender_name,
            sender_version: &sender_version,
            sample_id_field: input.sample_id_field,
            revision_indicator: input.revision_indicator,
            reports_final_concentration: input.reports_final_concentration,
            codes: &codes,
        },
    )?;
    if let Some(profile) = &input.au_profile {
        crate::repo::au_profiles::insert(ctx.tx(), id, profile)?;
        event = event.detail("format", AuditDetail::code("au"));
    }
    ctx.audit(event)?;
    queries::source_registration(ctx.tx(), id)
}

/// The connection type of a configured instrument.
pub(super) fn instrument_transport(
    ctx: &CommandContext<'_>,
    instrument_id: InstrumentId,
) -> Result<InstrumentTransport, AnalyticalError> {
    Ok(
        lis_interfaces::get_instrument(ctx.tx(), ctx.principal(), instrument_id)?
            .connection
            .transport(),
    )
}

/// A draft registration to record, with its validated test-code map.
pub(super) struct Draft<'a> {
    pub(super) instrument_id: InstrumentId,
    pub(super) sender_name: &'a str,
    pub(super) sender_version: &'a str,
    pub(super) sample_id_field: SampleIdField,
    pub(super) revision_indicator: RevisionIndicator,
    pub(super) reports_final_concentration: bool,
    pub(super) codes: &'a [(String, AnalyteId)],
}

/// Writes a draft registration and its test codes. Returns its ID and the
/// `register_source` audit event, which the caller may extend and appends.
pub(super) fn insert_draft(
    ctx: &CommandContext<'_>,
    draft: &Draft<'_>,
) -> Result<(SourceRegistrationId, AuditEvent), AnalyticalError> {
    let id = SourceRegistrationId::generate();
    let by = actor(ctx);
    repo::insert_registration(
        ctx.tx(),
        &NewRegistration {
            id,
            instrument_id: draft.instrument_id,
            sender_name: draft.sender_name,
            sender_version: draft.sender_version,
            sample_id_field: draft.sample_id_field,
            revision_indicator: draft.revision_indicator,
            reports_final_concentration: draft.reports_final_concentration,
            by: &by,
            at: ctx.now(),
        },
    )?;
    for (code, analyte_id) in draft.codes {
        repo::insert_test_code(ctx.tx(), id, code, *analyte_id)?;
    }
    let event = clinical(actions::REGISTER_SOURCE)
        .record("source_registration", id)
        .new_revision(1)
        .detail("instrument", AuditDetail::id(draft.instrument_id))
        .detail("test_codes", AuditDetail::count(draft.codes.len()));
    Ok((id, event))
}

/// Validates the test-code map: at least one, unique, exact codes for known analytes.
pub(super) fn test_codes(
    ctx: &CommandContext<'_>,
    input: &[SourceTestCodeInput],
) -> Result<Vec<(String, AnalyteId)>, AnalyticalError> {
    if input.is_empty() {
        return Err(invalid(
            "testCodes",
            "required",
            "Map at least one source test code.",
        ));
    }
    let known: BTreeSet<AnalyteId> = lis_catalog::list_analytes(ctx.tx())?
        .into_iter()
        .map(|analyte| analyte.id)
        .collect();
    let mut codes = Vec::new();
    for (index, entry) in input.iter().enumerate() {
        let field = format!("testCodes[{index}].sourceTestCode");
        // The code is matched exactly as received, so it is not trimmed.
        let code = lis_core::required_text(&field, &entry.source_test_code, CODE_MAX)?;
        if code != entry.source_test_code {
            return Err(invalid(&field, "untrimmed", "Remove surrounding spaces."));
        }
        if codes.iter().any(|(seen, _)| *seen == code) {
            return Err(invalid(&field, "duplicate", "Each test code appears once."));
        }
        if !known.contains(&entry.analyte_id) {
            return Err(AnalyticalError::NotFound("analyte"));
        }
        codes.push((code, entry.analyte_id));
    }
    Ok(codes)
}

/// Input of [`approve_source`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ApproveSourceInput {
    /// The draft registration.
    pub registration_id: SourceRegistrationId,
    /// The revision shown.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// The approver confirms they wrote the draft and approve it with no second
    /// person, as the laboratory's separation-of-duties policy allows (ADR 0015);
    /// needed only when they wrote it.
    pub self_approval: bool,
}

/// Approves a draft registration (REQ-ANA-003): the approver must be a
/// signed-in user other than its author, or its author as the laboratory's
/// separation-of-duties policy allows, with `self_approval` confirmed
/// (ADR 0015). The instrument's previous active registration is retired in
/// the same transaction.
pub fn approve_source(
    ctx: &mut CommandContext<'_>,
    input: ApproveSourceInput,
) -> Result<SourceRegistrationView, AnalyticalError> {
    ctx.require(Capability::ApproveConfiguration)?;
    let approver = require_user(ctx)?;
    let draft = queries::source_registration(ctx.tx(), input.registration_id)?;
    ensure_revision(draft.revision, input.expected_revision)?;
    if draft.state != RegistrationState::Draft {
        return Err(AnalyticalError::WrongState("source registration"));
    }
    au_sources::validate_connection(
        ctx,
        draft.instrument_id,
        draft.au_profile.as_ref(),
        draft.file_layout.is_some(),
    )?;
    let self_approval = draft.authored_by == approver;
    if self_approval {
        // REQ-ANA-003, ADR 0015: the policy in force, read in this transaction.
        lis_auth::current_separation_policy(ctx.tx())?
            .authorize_self(Duty::ConfigurationApproval, input.self_approval)?;
    }
    let previous = repo::registrations(
        ctx.tx(),
        draft.instrument_id,
        Some(RegistrationState::Active),
    )?;
    let mut event = clinical(actions::APPROVE_SOURCE)
        .record("source_registration", draft.registration_id)
        .old_revision(draft.revision)
        .new_revision(draft.revision + 1);
    for retired in &previous {
        repo::retire(
            ctx.tx(),
            retired.registration_id,
            retired.revision + 1,
            ctx.now(),
        )?;
        event = event.detail(
            "retired_registration",
            AuditDetail::id(retired.registration_id),
        );
    }
    if self_approval {
        event = event.self_approval();
    }
    repo::activate(
        ctx.tx(),
        draft.registration_id,
        draft.revision + 1,
        &approver,
        ctx.now(),
    )?;
    ctx.audit(event)?;
    queries::source_registration(ctx.tx(), draft.registration_id)
}
