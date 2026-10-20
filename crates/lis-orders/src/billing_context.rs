//! Explicit order financial selection and attribution, with no clinical gating.

use std::collections::BTreeSet;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Actor, Phi};
use lis_identity::{
    get_patient_billing_profile, require_financial_access, validate_financial_evidence,
};
use lis_store::CommandContext;
use rusqlite::{Connection, OptionalExtension, params};

use crate::billing_model::{
    OrderBillingContext, OrderBillingSelection, SetOrderBillingContextInput,
};
use crate::error::{OrderError, invalid};
use crate::{BillingResponsibility, OrderId};

/// Records/corrects financial selection, preserving every earlier order context.
pub fn set_order_billing_context(
    ctx: &mut CommandContext<'_>,
    input: SetOrderBillingContextInput,
) -> Result<OrderBillingContext, OrderError> {
    ctx.require(require_financial_access(ctx.principal())?)?;
    let actor = financial_actor(ctx)?;
    let (subject_id, revision, version): (lis_identity::SubjectId, i64, i64) = ctx
        .tx()
        .query_row(
            "SELECT subject_id, revision, version FROM ord_orders WHERE id = ?1",
            [input.order_id],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .optional()?
        .ok_or(OrderError::NotFound("order"))?;
    let previous =
        get_order_billing_context(ctx.tx(), input.order_id)?.map(|context| context.version);
    if revision != input.expected_revision || previous != input.expected_billing_version {
        return Err(OrderError::RevisionConflict);
    }
    validate_source("source", &input.source)?;
    validate_source("reason", &input.reason)?;
    let selection = select_profile(ctx.tx(), subject_id, &input)?;
    validate_financial_evidence(&input.financial_evidence, ctx.now())?;
    validate_order_evidence(ctx.tx(), &input, &selection)?;
    let context = OrderBillingContext {
        order_id: input.order_id,
        subject_id,
        version: previous.unwrap_or(0) + 1,
        order_version: version + 1,
        selection,
        financial_evidence: input.financial_evidence,
        source: input.source,
        reason: input.reason,
        recorded_by: actor,
        recorded_at: ctx.now(),
    };
    insert_context(ctx.tx(), &context)?;
    ctx.tx().execute("UPDATE ord_orders SET billing_responsibility = ?2, version = version + 1, revision = revision + 1 WHERE id = ?1",
        params![context.order_id, context.selection.responsibility])?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "order.set_billing_context",
            Outcome::Succeeded,
        )
        .record("order", context.order_id)
        .old_revision(revision)
        .new_revision(revision + 1)
        .reason(context.reason.expose().clone()),
    )?;
    Ok(context)
}

fn select_profile(
    conn: &Connection,
    subject_id: lis_identity::SubjectId,
    input: &SetOrderBillingContextInput,
) -> Result<OrderBillingSelection, OrderError> {
    validate_selection(input)?;
    if let Some(client_id) = input.client_id {
        lis_identity::get_client(conn, client_id)?;
    }
    let profile = input
        .profile_version
        .map(|version| {
            get_patient_billing_profile(conn, subject_id, Some(version))?
                .ok_or(OrderError::NotFound("patient_billing_profile"))
        })
        .transpose()?;
    let coverages = selected_coverages(profile.as_ref(), &input.coverage_keys)?;
    let guarantor = if input.use_profile_guarantor {
        Some(
            profile
                .as_ref()
                .and_then(|profile| profile.content.guarantor.clone())
                .ok_or_else(|| {
                    invalid(
                        "useProfileGuarantor",
                        "not_found",
                        "This profile has no guarantor.",
                    )
                })?,
        )
    } else {
        None
    };
    Ok(OrderBillingSelection {
        responsibility: input.responsibility,
        client_id: input.client_id,
        profile_version: input.profile_version,
        contact: profile.map(|profile| profile.content.contact),
        guarantor,
        coverages,
    })
}

fn validate_selection(input: &SetOrderBillingContextInput) -> Result<(), OrderError> {
    if input.coverage_keys.len() > 8 {
        return Err(invalid(
            "coverageKeys",
            "too_many",
            "Select at most eight coverages.",
        ));
    }
    if input.responsibility != BillingResponsibility::ThirdParty && !input.coverage_keys.is_empty()
    {
        return Err(invalid(
            "coverageKeys",
            "not_applicable",
            "Select coverage only for third-party billing.",
        ));
    }
    if input.responsibility != BillingResponsibility::Client && input.client_id.is_some() {
        return Err(invalid(
            "clientId",
            "not_applicable",
            "Select a bill-to client only for client billing.",
        ));
    }
    if input.profile_version.is_none()
        && (!input.coverage_keys.is_empty() || input.use_profile_guarantor)
    {
        return Err(invalid(
            "profileVersion",
            "required",
            "Select a patient billing profile first.",
        ));
    }
    Ok(())
}

fn selected_coverages(
    profile: Option<&lis_identity::PatientBillingProfile>,
    selected_keys: &[String],
) -> Result<Vec<lis_identity::PatientCoverage>, OrderError> {
    let mut coverages = Vec::new();
    let mut keys = BTreeSet::new();
    for key in selected_keys {
        if !keys.insert(key) {
            return Err(invalid(
                "coverageKeys",
                "duplicate",
                "Select each coverage once.",
            ));
        }
        let coverage = profile
            .and_then(|profile| {
                profile
                    .content
                    .coverages
                    .iter()
                    .find(|coverage| coverage.key == *key)
            })
            .ok_or_else(|| {
                invalid(
                    "coverageKeys",
                    "not_found",
                    "Selected coverage is absent from this profile version.",
                )
            })?;
        coverages.push(coverage.clone());
    }
    Ok(coverages)
}

fn validate_order_evidence(
    conn: &Connection,
    input: &SetOrderBillingContextInput,
    selection: &OrderBillingSelection,
) -> Result<(), OrderError> {
    let practitioner_id = conn.query_row(
        "SELECT practitioner_id FROM ord_orders WHERE id = ?1",
        [input.order_id],
        |row| row.get(0),
    )?;
    let practitioner = lis_identity::get_practitioner(conn, practitioner_id)?;
    for evidence in &input.financial_evidence {
        if evidence.coverage_key.as_ref().is_some_and(|key| {
            !selection
                .coverages
                .iter()
                .any(|coverage| &coverage.key == key)
        }) {
            return Err(invalid(
                "coverageKey",
                "not_selected",
                "Evidence must reference a selected coverage.",
            ));
        }
        if evidence.kind == lis_identity::FinancialEvidenceKind::ProviderEnrollment
            && evidence.provider_npi.as_deref() != Some(practitioner.npi.as_str())
        {
            return Err(invalid(
                "providerNpi",
                "orderer_mismatch",
                "Enrollment evidence must identify this order's practitioner.",
            ));
        }
    }
    Ok(())
}

fn insert_context(conn: &Connection, context: &OrderBillingContext) -> Result<(), OrderError> {
    let json = serde_json::to_string(context).map_err(|_| OrderError::InvalidBillingRecord)?;
    if json.len() > 262_144 {
        return Err(invalid(
            "context",
            "too_large",
            "Billing context exceeds its size limit.",
        ));
    }
    conn.execute("INSERT INTO ord_billing_contexts (order_id, version, order_version, subject_id, profile_version, client_id, responsibility, context_json, recorded_by, recorded_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10)",
        params![context.order_id, context.version, context.order_version, context.subject_id,
            context.selection.profile_version, context.selection.client_id, context.selection.responsibility,
            json, context.recorded_by, context.recorded_at])?;
    Ok(())
}

/// Latest immutable financial snapshot; caller authorizes/audits UI reads.
pub fn get_order_billing_context(
    conn: &Connection,
    order_id: OrderId,
) -> Result<Option<OrderBillingContext>, OrderError> {
    let json: Option<String> = conn.query_row("SELECT context_json FROM ord_billing_contexts WHERE order_id = ?1 ORDER BY version DESC LIMIT 1", [order_id], |row| row.get(0)).optional()?;
    json.map(|json| serde_json::from_str(&json).map_err(|_| OrderError::InvalidBillingRecord))
        .transpose()
}

pub(crate) fn financial_actor(ctx: &CommandContext<'_>) -> Result<String, OrderError> {
    match ctx.principal().actor() {
        Actor::User { user_id, .. } => Ok(user_id.to_string()),
        Actor::Instrument { .. } | Actor::System { .. } => {
            Err(lis_identity::IdentityError::UserRequired.into())
        }
    }
}

pub(crate) fn validate_source(field: &str, value: &Phi<String>) -> Result<(), OrderError> {
    lis_core::required_text(field, value.expose(), 1000).map_err(OrderError::Invalid)?;
    Ok(())
}
