//! The performing laboratory's profile (REQ-RPT-001, REQ-RPT-008,
//! REQ-BIL-005). Each change appends a new version with a reason (L15).

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::error::{IdentityError, invalid};
use crate::model::{
    ActorRef, LaboratoryProfileId, NAME_MAX, Npi, REASON_MAX, optional_text, required_text,
};
use crate::repo::laboratory as repo;
use crate::views::LaboratoryProfileView;

const ADDRESS_LINES_MAX: usize = 3;
const PHONE_MAX: usize = 40;
const TIME_ZONE_MAX: usize = 64;

/// Input of [`set_laboratory_profile`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SetLaboratoryProfile {
    /// The current version shown, or `null` when no profile exists yet.
    #[ts(type = "number | null")]
    pub expected_version: Option<i64>,
    /// The laboratory's name as printed.
    pub name: String,
    /// One to three address lines.
    pub address_lines: Vec<String>,
    /// CLIA certificate number: two digits, `D`, seven digits.
    pub clia_id: String,
    /// Laboratory director's name as printed.
    pub director: String,
    /// Telephone number as printed.
    pub phone: String,
    /// IANA time zone name, such as `America/Chicago`.
    pub time_zone: String,
    /// Billing NPI.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub billing_npi: Option<String>,
    /// Tax identification number: nine digits, optionally as `NN-NNNNNNN`.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub tax_id: Option<String>,
    /// Why the profile changes; required for every version after the first.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub reason: Option<String>,
}

/// Sets the laboratory profile as a new version (L15: one signed-in user
/// with `manage_configuration` and a reason). The expected version must be
/// the current one, so two edits cannot silently overwrite each other.
pub fn set_laboratory_profile(
    ctx: &mut CommandContext<'_>,
    input: SetLaboratoryProfile,
) -> Result<LaboratoryProfileView, IdentityError> {
    ctx.require(Capability::ManageConfiguration)?;
    let actor = ActorRef::user(ctx.principal())?;
    let current = repo::current_profile(ctx.tx())?.map(|profile| profile.version);
    if current != input.expected_version {
        return Err(IdentityError::RevisionConflict);
    }
    let version = current.unwrap_or(0) + 1;
    let profile = LaboratoryProfileView {
        profile_id: LaboratoryProfileId::generate(),
        version,
        name: required_text("name", &input.name, NAME_MAX)?,
        address_lines: address_lines(&input.address_lines)?,
        clia_id: clia_id(&input.clia_id)?,
        director: required_text("director", &input.director, NAME_MAX)?,
        phone: required_text("phone", &input.phone, PHONE_MAX)?,
        time_zone: time_zone(&input.time_zone)?,
        billing_npi: billing_npi(input.billing_npi.as_deref())?,
        tax_id: tax_id(input.tax_id.as_deref())?,
        reason: change_reason(version, input.reason.as_deref())?,
        set_by: actor.id,
        set_by_display: None,
        set_at: ctx.now(),
    };
    repo::insert_profile(ctx.tx(), &profile)?;
    let mut event = AuditEvent::new(
        Stream::Clinical,
        "identity.set_laboratory_profile",
        Outcome::Succeeded,
    )
    .record("laboratory_profile", profile.profile_id)
    .new_revision(version);
    if let Some(previous) = current {
        event = event.old_revision(previous);
    }
    if let Some(reason) = &profile.reason {
        event = event.reason(reason.clone());
    }
    ctx.audit(event)?;
    Ok(profile)
}

fn address_lines(lines: &[String]) -> Result<Vec<String>, IdentityError> {
    if lines.is_empty() || lines.len() > ADDRESS_LINES_MAX {
        return Err(invalid(
            "addressLines",
            "line_count",
            "Enter one to three address lines.",
        ));
    }
    lines
        .iter()
        .map(|line| required_text("addressLines", line, NAME_MAX))
        .collect()
}

/// A CLIA number is ten characters: two digits, the letter `D`, seven digits.
fn clia_id(text: &str) -> Result<String, IdentityError> {
    let text = text.trim().to_ascii_uppercase();
    let bytes = text.as_bytes();
    let valid = bytes.len() == 10
        && bytes[2] == b'D'
        && bytes
            .iter()
            .enumerate()
            .all(|(index, byte)| index == 2 || byte.is_ascii_digit());
    if valid {
        Ok(text)
    } else {
        Err(invalid(
            "cliaId",
            "invalid_clia",
            "Enter the 10-character CLIA number, such as 12D3456789.",
        ))
    }
}

/// The canonical IANA name; fixed offsets and POSIX rules are refused, so
/// local times follow daylight saving rules (REQ-RPT-008 printed times).
fn time_zone(text: &str) -> Result<String, IdentityError> {
    let refused = || {
        invalid(
            "timeZone",
            "invalid_time_zone",
            "Enter an IANA time zone name, such as America/Chicago.",
        )
    };
    let text = required_text("timeZone", text, TIME_ZONE_MAX)?;
    let zone = jiff::tz::TimeZone::get(&text).map_err(|_| refused())?;
    zone.iana_name().map(str::to_owned).ok_or_else(refused)
}

fn billing_npi(text: Option<&str>) -> Result<Option<String>, IdentityError> {
    let Some(text) = text.filter(|text| !text.trim().is_empty()) else {
        return Ok(None);
    };
    let npi = Npi::parse(text)
        .map_err(|_| invalid("billingNpi", "invalid_npi", "Enter a valid 10-digit NPI."))?;
    Ok(Some(npi.as_str().to_owned()))
}

/// Nine digits, stored without the hyphen of the `NN-NNNNNNN` form.
fn tax_id(text: Option<&str>) -> Result<Option<String>, IdentityError> {
    let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
        return Ok(None);
    };
    let digits = match text.split_once('-') {
        Some((head, tail)) if head.len() == 2 => format!("{head}{tail}"),
        Some(_) => String::new(),
        None => text.to_owned(),
    };
    if digits.len() == 9 && digits.bytes().all(|byte| byte.is_ascii_digit()) {
        Ok(Some(digits))
    } else {
        Err(invalid(
            "taxId",
            "invalid_tax_id",
            "Enter the nine-digit tax identification number.",
        ))
    }
}

/// The first version has no reason; every later version needs one.
fn change_reason(version: i64, reason: Option<&str>) -> Result<Option<String>, IdentityError> {
    let reason = optional_text("reason", reason, REASON_MAX)?;
    match (version == 1, reason) {
        (true, None) => Ok(None),
        (true, Some(_)) => Err(invalid(
            "reason",
            "not_applicable",
            "The first laboratory profile needs no reason.",
        )),
        (false, Some(reason)) => Ok(Some(reason)),
        (false, None) => Err(invalid(
            "reason",
            "required",
            "Enter why the laboratory profile changes.",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_code(error: IdentityError) -> (String, String) {
        let IdentityError::Invalid(field) = error else {
            panic!("unexpected {error:?}");
        };
        (field.field, field.code)
    }

    #[test]
    fn clia_numbers_need_two_digits_a_d_and_seven_digits() {
        assert_eq!(clia_id(" 00d0000000 ").unwrap(), "00D0000000");
        for wrong in [
            "00X0000000",
            "0D00000000",
            "00D000000",
            "00D00000000",
            "AAD0000000",
        ] {
            assert_eq!(
                field_code(clia_id(wrong).unwrap_err()),
                ("cliaId".to_owned(), "invalid_clia".to_owned()),
                "{wrong}"
            );
        }
    }

    #[test]
    fn time_zones_are_canonical_iana_names() {
        assert_eq!(time_zone("America/Chicago").unwrap(), "America/Chicago");
        assert_eq!(time_zone("america/chicago").unwrap(), "America/Chicago");
        assert_eq!(time_zone(" UTC ").unwrap(), "UTC");
        for wrong in [
            "Mars/Olympus_Mons",
            "+05:00",
            "EST5EDT,M3.2.0,M11.1.0",
            "CDT",
        ] {
            assert_eq!(
                field_code(time_zone(wrong).unwrap_err()).1,
                "invalid_time_zone",
                "{wrong}"
            );
        }
        assert_eq!(field_code(time_zone(" ").unwrap_err()).1, "required");
    }

    #[test]
    fn tax_ids_are_nine_digits_with_an_optional_hyphen() {
        assert_eq!(tax_id(Some("12-3456789")).unwrap().unwrap(), "123456789");
        assert_eq!(tax_id(Some("123456789")).unwrap().unwrap(), "123456789");
        assert_eq!(tax_id(Some("  ")).unwrap(), None);
        for wrong in ["123-456789", "12-345678", "12345678X", "12--3456789"] {
            assert_eq!(
                field_code(tax_id(Some(wrong)).unwrap_err()).1,
                "invalid_tax_id"
            );
        }
    }

    #[test]
    fn billing_npi_reports_its_own_field() {
        assert_eq!(
            billing_npi(Some("1234567893")).unwrap().unwrap(),
            "1234567893"
        );
        assert_eq!(billing_npi(None).unwrap(), None);
        assert_eq!(
            field_code(billing_npi(Some("1234567890")).unwrap_err()).0,
            "billingNpi"
        );
    }

    #[test]
    fn only_later_versions_carry_a_reason() {
        assert_eq!(change_reason(1, None).unwrap(), None);
        assert_eq!(change_reason(1, Some(" ")).unwrap(), None);
        assert_eq!(
            field_code(change_reason(1, Some("x")).unwrap_err()).1,
            "not_applicable"
        );
        assert_eq!(
            field_code(change_reason(2, None).unwrap_err()).1,
            "required"
        );
        assert_eq!(change_reason(2, Some(" Moved ")).unwrap().unwrap(), "Moved");
    }

    #[test]
    fn address_needs_one_to_three_lines() {
        let lines = |count: usize| vec!["1 Synthetic Way".to_owned(); count];
        assert_eq!(address_lines(&lines(3)).unwrap().len(), 3);
        assert_eq!(
            field_code(address_lines(&lines(0)).unwrap_err()).1,
            "line_count"
        );
        assert_eq!(
            field_code(address_lines(&lines(4)).unwrap_err()).1,
            "line_count"
        );
        assert_eq!(
            field_code(address_lines(&[" ".to_owned()]).unwrap_err()).1,
            "required"
        );
    }
}
