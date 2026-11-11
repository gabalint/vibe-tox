//! Identifiers, command inputs and their validation. Pure: no database.

use lis_core::{Actor, Id, Phi, Principal};
use serde::{Deserialize, Serialize};
use ts_rs::TS;
use unicode_normalization::UnicodeNormalization;

use crate::error::{IdentityError, invalid};

/// Marker for [`SubjectId`].
pub enum Subject {}
/// A person the laboratory holds records for.
pub type SubjectId = Id<Subject>;
/// Marker for [`IdentityAssertionId`].
pub enum IdentityAssertion {}
/// One demographic assertion about a subject.
pub type IdentityAssertionId = Id<IdentityAssertion>;
/// Marker for [`SubjectIdentifierId`].
pub enum SubjectIdentifier {}
/// One identifier row of a subject.
pub type SubjectIdentifierId = Id<SubjectIdentifier>;
/// Marker for [`IdentityHoldId`].
pub enum IdentityHold {}
/// A review task for an unresolved identity question.
pub type IdentityHoldId = Id<IdentityHold>;
/// Marker for [`ClientId`].
pub enum Client {}
/// An ordering client organization.
pub type ClientId = Id<Client>;
/// Marker for [`ClientLocationId`].
pub enum ClientLocation {}
/// A location of a client organization.
pub type ClientLocationId = Id<ClientLocation>;
/// Marker for [`PractitionerId`].
pub enum Practitioner {}
/// An ordering practitioner.
pub type PractitionerId = Id<Practitioner>;
/// Marker for [`LaboratoryProfileId`].
pub enum LaboratoryProfile {}
/// One version of the performing laboratory's profile.
pub type LaboratoryProfileId = Id<LaboratoryProfile>;

/// The assigning authority of every practitioner identifier.
pub const NPI_AUTHORITY: &str = "NPI";

pub(crate) const NAME_MAX: usize = 200;
pub(crate) const CODE_MAX: usize = 64;
pub(crate) const REASON_MAX: usize = 1000;

/// [`lis_core::required_text`], as this module's error.
pub(crate) fn required_text(field: &str, value: &str, max: usize) -> Result<String, IdentityError> {
    lis_core::required_text(field, value, max).map_err(IdentityError::Invalid)
}

/// [`lis_core::optional_text`], as this module's error.
pub(crate) fn optional_text(
    field: &str,
    value: Option<&str>,
    max: usize,
) -> Result<Option<String>, IdentityError> {
    lis_core::optional_text(field, value, max).map_err(IdentityError::Invalid)
}

/// Demographics as provided by a source.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct DemographicsInput {
    /// Family name as provided.
    pub family_name: Phi<String>,
    /// Given names as provided; absent for a single-name person.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub given_names: Option<Phi<String>>,
    /// Birth date as an ISO 8601 calendar date (`YYYY-MM-DD`).
    pub birth_date: Phi<String>,
    /// Administrative sex code as provided; never mapped or inferred.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub administrative_sex: Option<Phi<String>>,
}

/// Validated demographics.
#[derive(Debug, Clone)]
pub(crate) struct Demographics {
    pub(crate) family_name: Phi<String>,
    pub(crate) given_names: Option<Phi<String>>,
    pub(crate) birth_date: Phi<String>,
    pub(crate) administrative_sex: Option<Phi<String>>,
}

impl Demographics {
    pub(crate) fn parse(input: &DemographicsInput) -> Result<Self, IdentityError> {
        let family_name = required_text("familyName", input.family_name.expose(), NAME_MAX)?;
        let given_names = optional_text(
            "givenNames",
            input
                .given_names
                .as_ref()
                .map(|name| name.expose().as_str()),
            NAME_MAX,
        )?;
        let birth_date = birth_date(input.birth_date.expose())?;
        let administrative_sex = optional_text(
            "administrativeSex",
            input
                .administrative_sex
                .as_ref()
                .map(|sex| sex.expose().as_str()),
            CODE_MAX,
        )?;
        Ok(Self {
            family_name: Phi::new(family_name),
            given_names: given_names.map(Phi::new),
            birth_date: Phi::new(birth_date.to_owned()),
            administrative_sex: administrative_sex.map(Phi::new),
        })
    }

    /// The deterministic candidate key: family name, given names and birth
    /// date, trimmed, lowercased and normalized to NFC. Equal keys list
    /// subjects as possible duplicates; they never link them.
    pub(crate) fn match_key(&self) -> Phi<String> {
        let given = self.given_names.as_ref().map_or("", |name| name.expose());
        Phi::new(format!(
            "{}{MATCH_KEY_SEPARATOR}{}{MATCH_KEY_SEPARATOR}{}",
            name_key(self.family_name.expose()),
            name_key(given),
            self.birth_date.expose()
        ))
    }
}

/// Separates the parts of a match key.
pub(crate) const MATCH_KEY_SEPARATOR: char = '\u{1f}';

/// A name part of the match key: lowercased and normalized to NFC.
pub(crate) fn name_key(name: &str) -> String {
    name.to_lowercase().nfc().collect()
}

/// The trimmed birth date when it is `YYYY-MM-DD` naming a real calendar day.
pub(crate) fn birth_date(text: &str) -> Result<&str, IdentityError> {
    let text = text.trim();
    if text
        .parse::<jiff::civil::Date>()
        .is_ok_and(|date| date.to_string() == text)
    {
        Ok(text)
    } else {
        Err(invalid(
            "birthDate",
            "invalid_date",
            "Enter the birth date as YYYY-MM-DD.",
        ))
    }
}

/// An identifier as provided, scoped by its assigning authority and type.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct IdentifierInput {
    /// The authority that assigned the value, such as a client's MRN namespace.
    pub assigning_authority: String,
    /// The identifier type code, such as `MR`.
    pub identifier_type: String,
    /// The identifier value.
    pub value: Phi<String>,
}

/// A validated identifier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Identifier {
    pub(crate) assigning_authority: String,
    pub(crate) identifier_type: String,
    pub(crate) value: Phi<String>,
}

impl Identifier {
    /// Authority, type and value are compared exactly after trimming: a
    /// looser comparison could link two different people.
    pub(crate) fn parse(field: &str, input: &IdentifierInput) -> Result<Self, IdentityError> {
        Ok(Self {
            assigning_authority: required_text(
                &format!("{field}.assigningAuthority"),
                &input.assigning_authority,
                CODE_MAX,
            )?,
            identifier_type: required_text(
                &format!("{field}.identifierType"),
                &input.identifier_type,
                CODE_MAX,
            )?,
            value: Phi::new(required_text(
                &format!("{field}.value"),
                input.value.expose(),
                CODE_MAX,
            )?),
        })
    }
}

/// Validates identifiers and refuses the same identifier twice in one request.
pub(crate) fn parse_identifiers(
    inputs: &[IdentifierInput],
) -> Result<Vec<Identifier>, IdentityError> {
    let mut identifiers: Vec<Identifier> = Vec::with_capacity(inputs.len());
    for (index, input) in inputs.iter().enumerate() {
        let field = format!("identifiers[{index}]");
        let identifier = Identifier::parse(&field, input)?;
        if identifiers.contains(&identifier) {
            return Err(invalid(
                &field,
                "duplicate",
                "This identifier is listed twice.",
            ));
        }
        identifiers.push(identifier);
    }
    Ok(identifiers)
}

/// A National Provider Identifier: 10 digits whose last digit is the Luhn
/// check digit computed with the `80840` prefix.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct Npi(String);

impl Npi {
    /// Validates length, digits and the check digit, with a separate message for each so a
    /// ten-digit typo is not reported as a length problem.
    pub fn parse(text: &str) -> Result<Self, IdentityError> {
        Self::parse_field("npi", text)
    }

    /// [`Npi::parse`], reporting a refusal against `field`.
    pub(crate) fn parse_field(field: &str, text: &str) -> Result<Self, IdentityError> {
        let text = text.trim();
        let digits: Option<Vec<u32>> = text.chars().map(|c| c.to_digit(10)).collect();
        match digits {
            Some(digits) if digits.len() == 10 => {
                if luhn_valid_with_prefix(&digits) {
                    Ok(Self(text.to_owned()))
                } else {
                    Err(invalid(
                        field,
                        "npi_check_digit",
                        "This NPI is not valid. Please check the number for a typo.",
                    ))
                }
            }
            Some(_) | None => Err(invalid(
                field,
                "npi_length",
                "The NPI must be exactly 10 digits.",
            )),
        }
    }

    /// The ten digits.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Rebuilds a stored NPI, which was validated before it was written.
    pub(crate) fn from_stored(text: String) -> Self {
        Self(text)
    }
}

/// The Luhn check over the NPI with the `80840` health-industry prefix.
fn luhn_valid_with_prefix(digits: &[u32]) -> bool {
    const PREFIX: [u32; 5] = [8, 0, 8, 4, 0];
    let sum: u32 = PREFIX
        .iter()
        .chain(digits)
        .rev()
        .enumerate()
        .map(|(position, &digit)| match (position % 2 == 1, digit * 2) {
            (true, doubled) if doubled > 9 => doubled - 9,
            (true, doubled) => doubled,
            (false, _) => digit,
        })
        .sum();
    sum.is_multiple_of(10)
}

/// Who made an assertion or decision, as stored with the record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ActorRef {
    pub(crate) kind: &'static str,
    pub(crate) id: String,
}

impl ActorRef {
    pub(crate) fn of(principal: &Principal) -> Self {
        let actor = principal.actor();
        Self {
            kind: actor.kind(),
            id: actor.id_text(),
        }
    }

    pub(crate) fn user(principal: &Principal) -> Result<Self, IdentityError> {
        let actor = principal.actor();
        match actor {
            Actor::User { .. } => Ok(Self {
                kind: actor.kind(),
                id: actor.id_text(),
            }),
            Actor::Instrument { .. } | Actor::System { .. } => Err(IdentityError::UserRequired),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn demographics(family: &str, given: Option<&str>, birth: &str) -> DemographicsInput {
        DemographicsInput {
            family_name: Phi::new(family.to_owned()),
            given_names: given.map(|name| Phi::new(name.to_owned())),
            birth_date: Phi::new(birth.to_owned()),
            administrative_sex: None,
        }
    }

    // Synthetic numbers with check digits computed by hand; 1234567893 is the
    // example in the CMS NPI check-digit description.
    #[test]
    fn npi_accepts_valid_check_digits() {
        for npi in ["1234567893", "1000000004", "2000000002", "1245319599"] {
            assert_eq!(Npi::parse(npi).unwrap().as_str(), npi, "{npi}");
        }
    }

    fn npi_refusal(npi: &str) -> String {
        match Npi::parse(npi) {
            Err(IdentityError::Invalid(field)) => field.code,
            other => panic!("{npi}: {other:?}"),
        }
    }

    #[test]
    fn npi_refuses_a_wrong_check_digit() {
        for npi in ["1234567890", "1234567894", "1000000005", "1908763221"] {
            assert_eq!(npi_refusal(npi), "npi_check_digit", "{npi}");
        }
    }

    #[test]
    fn npi_refuses_wrong_length_or_characters() {
        for npi in [
            "123456789",
            "12345678930",
            "12345678a3",
            "",
            "１２３４５６７８９３",
        ] {
            assert_eq!(npi_refusal(npi), "npi_length", "{npi}");
        }
    }

    #[test]
    fn birth_date_must_be_a_real_calendar_day() {
        assert!(Demographics::parse(&demographics("Testperson", None, "2000-02-29")).is_ok());
        for date in [
            "2001-02-29",
            "2000-2-1",
            "01/02/2000",
            "2000-01-01T00:00",
            "2000",
        ] {
            let error = Demographics::parse(&demographics("Testperson", None, date)).unwrap_err();
            assert!(
                matches!(&error, IdentityError::Invalid(field) if field.field == "birthDate"),
                "{date}"
            );
        }
    }

    #[test]
    fn match_key_ignores_case_space_and_unicode_composition() {
        let a = Demographics::parse(&demographics(" TESTPERSON ", Some("Alpha"), "2000-01-01"));
        let b = Demographics::parse(&demographics("testperson", Some("alpha "), "2000-01-01"));
        let composed = Demographics::parse(&demographics("García", Some("Alpha"), "2000-01-01"));
        let decomposed =
            Demographics::parse(&demographics("Garci\u{301}a", Some("Alpha"), "2000-01-01"));
        let c = Demographics::parse(&demographics("testperson", Some("alpha"), "2000-01-02"));
        let key = |d: Result<Demographics, IdentityError>| d.unwrap().match_key().into_inner();
        let a = key(a);
        assert_eq!(a, key(b));
        assert_eq!(key(composed), key(decomposed));
        assert_ne!(a, key(c));
    }

    #[test]
    fn errors_never_echo_the_value() {
        let error = Demographics::parse(&demographics("SYNTHETIC-CANARY", None, "CANARY-DATE"))
            .unwrap_err();
        assert!(!format!("{error} {error:?}").contains("CANARY"));
    }

    #[test]
    fn text_is_trimmed_and_bounded() {
        assert_eq!(required_text("f", "  a b ", 10).unwrap(), "a b");
        assert!(required_text("f", "   ", 10).is_err());
        assert!(required_text("f", &"x".repeat(11), 10).is_err());
        assert!(required_text("f", "a\u{0}b", 10).is_err());
        assert_eq!(optional_text("f", Some("  "), 10).unwrap(), None);
    }
}
