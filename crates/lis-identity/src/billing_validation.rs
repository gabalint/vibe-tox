//! Shared validation of supplied financial facts; no payer or clinical decision.

use std::collections::BTreeSet;

use lis_core::{Capability, Phi, Principal, UtcTimestamp};
use lis_store::StoreError;

use crate::billing_model::{
    BillingContact, BillingParty, FinancialEvidence, FinancialEvidenceKind, PatientBillingContent,
};
use crate::error::{IdentityError, invalid};
use crate::model::{Npi, optional_text, required_text};

/// Both registration staff and billing staff may maintain supplied financial facts.
/// Return the successful capability so mutations record the authority in their context.
/// Preserve the store error so each calling domain retains its auditable denial variant.
pub fn require_financial_access(principal: &Principal) -> Result<Capability, StoreError> {
    principal
        .require(Capability::EnterOrders)
        .map(|()| Capability::EnterOrders)
        .or_else(|_| {
            principal
                .require(Capability::Billing)
                .map(|()| Capability::Billing)
        })
        .map_err(StoreError::from)
}

pub(crate) fn validate_content(
    content: &PatientBillingContent,
    now: UtcTimestamp,
) -> Result<(), IdentityError> {
    validate_contact(&content.contact)?;
    if let Some(party) = &content.guarantor {
        validate_party(party)?;
    }
    if content.coverages.len() > 32 {
        return Err(invalid(
            "coverages",
            "too_many",
            "Use at most 32 coverage records.",
        ));
    }
    let mut keys = BTreeSet::new();
    for coverage in &content.coverages {
        validate_key(&coverage.key)?;
        if !keys.insert(coverage.key.as_str()) {
            return Err(invalid(
                "coverages",
                "duplicate_key",
                "Coverage keys must be unique.",
            ));
        }
        required_text("payerName", &coverage.payer_name, 200)?;
        for value in [&coverage.payer_id, &coverage.plan_name] {
            optional_text("payer", value.as_deref(), 200)?;
        }
        for value in [&coverage.member_id, &coverage.group_number] {
            optional_phi("coverage", value, 200)?;
        }
        required_text("coverageSource", coverage.source.expose(), 1000)?;
        if let Some(party) = &coverage.subscriber {
            validate_party(party)?;
        }
        validate_period(&coverage.effective_from, &coverage.effective_until)?;
        if let Some(date) = &coverage.verified_on {
            validate_date(date)?;
        }
    }
    validate_financial_evidence(&content.financial_evidence, now)?;
    for evidence in &content.financial_evidence {
        if evidence
            .coverage_key
            .as_ref()
            .is_some_and(|key| !keys.contains(key.as_str()))
        {
            return Err(invalid(
                "coverageKey",
                "not_found",
                "Select a coverage in this profile.",
            ));
        }
    }
    Ok(())
}

fn validate_contact(contact: &BillingContact) -> Result<(), IdentityError> {
    if contact.address_lines.len() > 3 {
        return Err(invalid(
            "addressLines",
            "too_many",
            "Use at most three address lines.",
        ));
    }
    for line in &contact.address_lines {
        required_text("addressLines", line.expose(), 200)?;
    }
    for value in [
        &contact.city,
        &contact.region,
        &contact.postal_code,
        &contact.phone,
        &contact.email,
    ] {
        optional_phi("contact", value, 200)?;
    }
    optional_text("country", contact.country.as_deref(), 100)?;
    Ok(())
}

fn validate_party(party: &BillingParty) -> Result<(), IdentityError> {
    required_text("partyName", party.name.expose(), 200)?;
    if let Some(date) = &party.birth_date {
        validate_date(date.expose())?;
    }
    validate_contact(&party.contact)
}

/// Validate manual evidence uniformly in patient and order contexts (BILL-G07).
pub fn validate_financial_evidence(
    items: &[FinancialEvidence],
    now: UtcTimestamp,
) -> Result<(), IdentityError> {
    if items.len() > 64 {
        return Err(invalid(
            "financialEvidence",
            "too_many",
            "Use at most 64 evidence records.",
        ));
    }
    let mut keys = BTreeSet::new();
    for item in items {
        validate_key(&item.key)?;
        if !keys.insert(&item.key) {
            return Err(invalid(
                "financialEvidence",
                "duplicate_key",
                "Evidence keys must be unique.",
            ));
        }
        required_text("sourceRef", item.source_ref.expose(), 1000)?;
        required_text("scope", item.scope.expose(), 2000)?;
        optional_phi("referenceNumber", &item.reference_number, 200)?;
        optional_phi("notes", &item.notes, 2000)?;
        if let Some(key) = &item.coverage_key {
            validate_key(key)?;
        }
        if let Some(npi) = &item.provider_npi {
            Npi::parse(npi)?;
        }
        if item.kind == FinancialEvidenceKind::ProviderEnrollment && item.provider_npi.is_none() {
            return Err(invalid(
                "providerNpi",
                "required",
                "Identify the provider whose enrollment was checked.",
            ));
        }
        if item.units == Some(0) {
            return Err(invalid(
                "units",
                "positive",
                "Units must be positive when supplied.",
            ));
        }
        validate_period(&item.valid_from, &item.valid_until)?;
        if item.occurred_at > now {
            return Err(invalid(
                "occurredAt",
                "future",
                "Record when the evidence actually occurred.",
            ));
        }
        validate_notice(item, now)?;
    }
    Ok(())
}

fn validate_notice(item: &FinancialEvidence, now: UtcTimestamp) -> Result<(), IdentityError> {
    let Some(notice) = &item.notice else {
        return Ok(());
    };
    if !matches!(
        item.kind,
        FinancialEvidenceKind::AdvanceNotice | FinancialEvidenceKind::GoodFaithEstimate
    ) {
        return Err(invalid(
            "notice",
            "not_applicable",
            "Notice details belong to a notice or estimate.",
        ));
    }
    required_text("noticeReason", notice.reason.expose(), 2000)?;
    optional_text("formVersion", notice.form_version.as_deref(), 100)?;
    optional_phi("choice", &notice.choice, 1000)?;
    optional_phi("signer", &notice.signer, 200)?;
    if notice.estimated_amount.is_some() != notice.currency.is_some() {
        return Err(invalid(
            "currency",
            "amount_currency",
            "Record the amount and currency together.",
        ));
    }
    if let Some(currency) = &notice.currency
        && (currency.len() != 3 || !currency.bytes().all(|byte| byte.is_ascii_uppercase()))
    {
        return Err(invalid(
            "currency",
            "invalid_currency",
            "Use the supplied three-letter currency code.",
        ));
    }
    if notice
        .estimated_amount
        .as_ref()
        .is_some_and(|amount| amount.as_str().starts_with('-'))
    {
        return Err(invalid(
            "estimatedAmount",
            "nonnegative",
            "An estimate amount cannot be negative.",
        ));
    }
    if notice.provided_at.is_some_and(|time| time > now)
        || notice.signed_at.is_some_and(|time| time > now)
    {
        return Err(invalid(
            "noticeTime",
            "future",
            "Record actual notice and signature times.",
        ));
    }
    Ok(())
}

fn optional_phi(field: &str, value: &Option<Phi<String>>, max: usize) -> Result<(), IdentityError> {
    if let Some(value) = value {
        required_text(field, value.expose(), max)?;
    }
    Ok(())
}

fn validate_key(key: &str) -> Result<(), IdentityError> {
    if key.is_empty()
        || key.len() > 64
        || !key
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_'))
    {
        return Err(invalid(
            "key",
            "invalid_key",
            "Use a key of at most 64 letters, digits, hyphens or underscores.",
        ));
    }
    Ok(())
}

fn validate_period(from: &Option<String>, until: &Option<String>) -> Result<(), IdentityError> {
    for date in [from, until].into_iter().flatten() {
        validate_date(date)?;
    }
    if matches!((from, until), (Some(from), Some(until)) if from > until) {
        return Err(invalid(
            "period",
            "reversed",
            "The end date must not precede the start date.",
        ));
    }
    Ok(())
}

fn validate_date(text: &str) -> Result<(), IdentityError> {
    if text
        .parse::<jiff::civil::Date>()
        .is_ok_and(|date| date.to_string() == text)
    {
        Ok(())
    } else {
        Err(invalid(
            "date",
            "invalid_date",
            "Use a real date in YYYY-MM-DD format.",
        ))
    }
}
