//! The acceptance test cases of outgoing connections (ticket #4555 step 5a; REQ-INT-001),
//! on synthetic data only, with nothing sent. An HL7 Results connection's cases build ORU
//! messages with its partner and approved code mappings and read them back: header, the
//! patient's sex code, codes and units, a censored value, no interpretation, a correction and a withdrawal, and how
//! it travels. A file connection's case makes a file name with its pattern. Pure but for
//! reading the mappings.

use rusqlite::Connection;

use super::contract_model::AcceptanceCase;
use super::error::PartnerError;
use super::hl7::{Coded, Observation, ObservationGroup, ObservationValue, Organization, Oru};
use super::hl7_read::{Reader, Segment, find, message_text, segments, separators};
use super::hl7_sex::TABLE_0001;
use super::mapping_model::{ApprovedMappings, MappingKind, MappingValues};
use super::mappings::approved_mappings;
use super::model::{PartnerConnectionId, PartnerConnectionSettings, PartnerMessageType};
use super::repo::load_connection;
use super::settings::{FileNameParts, file_name};

/// The cases of an outgoing connection; empty for HL7 Orders, which the engine tests.
pub fn outgoing_cases(
    conn: &Connection,
    connection_id: PartnerConnectionId,
) -> Result<Vec<AcceptanceCase>, PartnerError> {
    let row = load_connection(conn, connection_id)?.ok_or(PartnerError::NotFound("connection"))?;
    let settings = &row.settings;
    let mut cases = Vec::new();
    if settings.message_type == PartnerMessageType::Hl7Oru {
        let mappings = approved_mappings(conn, row.id)?;
        cases.extend(hl7_cases(settings, mappings.as_ref()));
    }
    if !settings.message_type.incoming() && !settings.over_mllp() {
        cases.push(file_name_case(settings));
    }
    Ok(cases)
}

fn case(code: &str, problem: Option<&str>) -> AcceptanceCase {
    AcceptanceCase {
        code: code.to_owned(),
        passed: problem.is_none(),
        problem: problem.map(str::to_owned),
    }
}

/// The first test and analyte mappings, which the synthetic messages carry.
fn first(mappings: Option<&ApprovedMappings>, kind: MappingKind) -> Option<&MappingValues> {
    mappings?
        .mappings
        .iter()
        .find(|mapping| mapping.kind == kind)
        .map(|mapping| &mapping.values)
}

fn hl7_cases(
    settings: &PartnerConnectionSettings,
    mappings: Option<&ApprovedMappings>,
) -> Vec<AcceptanceCase> {
    let test = first(mappings, MappingKind::Test);
    let analyte = first(mappings, MappingKind::Analyte);
    let unitless = mappings.is_some_and(|mappings| {
        mappings.mappings.iter().any(|mapping| {
            mapping.kind == MappingKind::Analyte && mapping.values.ucum_unit.is_none()
        })
    });
    let complete = match (test, analyte) {
        _ if mappings.is_none() => Some("no_approved_mappings"),
        (None, _) => Some("no_test_mapping"),
        (_, None) => Some("no_analyte_mapping"),
        _ if unitless => Some("analyte_without_unit"),
        _ => None,
    };
    let message = Synthetic {
        partner: &settings.partner,
        test,
        analyte,
        acknowledged: settings.mllp.is_some(),
    };
    let mut cases = vec![
        case("mappings_complete", complete),
        message.header_case(),
        message.patient_sex_case(),
        message.codes_case(),
        message.censored_case(),
        message.statuses_case("correction", ("C", "C"), "C"),
        message.statuses_case("withdrawal", ("C", "W"), "W"),
    ];
    if settings.mllp.is_some() {
        cases.push(message.acknowledgement_case());
    }
    cases
}

/// The connection's file name pattern for a synthetic report or handoff.
fn file_name_case(settings: &PartnerConnectionSettings) -> AcceptanceCase {
    let name = file_name(
        &settings.file_name_pattern,
        &FileNameParts {
            record: "TEST00001-U",
            revision: "1",
            date: "20260101",
            time: "120000",
        },
    );
    let extension = match settings.message_type {
        PartnerMessageType::Hl7Oru => ".hl7",
        PartnerMessageType::ReportPdf => ".pdf",
        PartnerMessageType::BillingJson | PartnerMessageType::Hl7Orders => ".json",
    };
    let plain = !name.contains(['/', '\\', '{', '}']) && !name.starts_with('.');
    let problem = (!plain || !name.ends_with(extension)).then_some("file_name_invalid");
    case("file_name", problem)
}

/// The synthetic ORU of one connection.
struct Synthetic<'a> {
    partner: &'a str,
    test: Option<&'a MappingValues>,
    analyte: Option<&'a MappingValues>,
    acknowledged: bool,
}

impl Synthetic<'_> {
    /// The message with `value` and the group's and observation's statuses.
    fn oru(
        &self,
        value: ObservationValue,
        (group, observation): (&'static str, &'static str),
    ) -> Oru {
        let coded = |values: Option<&MappingValues>, code: &str| Coded {
            code: values.map_or_else(|| code.to_owned(), |values| values.partner_code.clone()),
            text: "Synthetic".to_owned(),
            loinc: values.and_then(|values| values.loinc.clone()),
        };
        let lab = Organization {
            name: "Vibe Tox LIS".to_owned(),
            id: String::new(),
        };
        let group = ObservationGroup {
            filler_order: "TEST-1".to_owned(),
            service: coded(self.test, "VIBETOX-TEST"),
            status: group,
            observations: vec![Observation {
                code: coded(self.analyte, "VIBETOX-ANALYTE"),
                sub_id: "1".to_owned(),
                value,
                unit: self.analyte.and_then(|values| values.ucum_unit.clone()),
                reference_range: None,
                status: observation,
                performer: lab.clone(),
            }],
        };
        let at = "20260101120000+0000";
        Oru {
            acknowledged: self.acknowledged,
            ..Oru::synthetic(
                "ACCEPTANCE-1".to_owned(),
                at,
                lab,
                self.partner,
                vec![group],
            )
        }
    }

    fn final_oru(&self) -> Oru {
        self.oru(ObservationValue::Number("12.5".to_owned()), ("F", "F"))
    }

    /// MSH-6 names the partner, MSH-9 the message, MSH-12 the version.
    fn header_case(&self) -> AcceptanceCase {
        let problem = read(&self.final_oru().encode(), |reader, segments| {
            let msh = find(segments, "MSH")?;
            let ok = reader.component(msh.field(6), 1) == self.partner
                && reader.component(msh.field(9), 1) == "ORU"
                && reader.component(msh.field(9), 2) == "R01"
                && reader.text(msh.field(12)) == "2.5.1";
            ok.then_some(())
        });
        case(
            "message_header",
            problem.is_none().then_some("header_mismatch"),
        )
    }

    /// PID-8 is a code from HL7 table 0001, not the text recorded on the patient.
    fn patient_sex_case(&self) -> AcceptanceCase {
        let found = read(&self.final_oru().encode(), |reader, segments| {
            let sex = reader.text(find(segments, "PID")?.field(8));
            TABLE_0001
                .iter()
                .any(|(code, _)| *code == sex)
                .then_some(())
        });
        case("patient_sex", found.is_none().then_some("sex_not_coded"))
    }

    /// OBR-4 and OBX-3 carry the partner's codes, OBX-6 the mapping's UCUM unit.
    fn codes_case(&self) -> AcceptanceCase {
        let (Some(test), Some(analyte)) = (self.test, self.analyte) else {
            return case("mapped_codes", Some("no_approved_mappings"));
        };
        let found = read(&self.final_oru().encode(), |reader, segments| {
            let obr = find(segments, "OBR")?;
            let obx = find(segments, "OBX")?;
            let ok = reader.component(obr.field(4), 1) == test.partner_code
                && reader.component(obx.field(3), 1) == analyte.partner_code
                && reader.component(obx.field(6), 1)
                    == analyte.ucum_unit.clone().unwrap_or_default();
            ok.then_some(())
        });
        case("mapped_codes", found.is_none().then_some("codes_not_sent"))
    }

    /// A censored value goes as SN with its comparator, never as an exact number, and no
    /// abnormal flag or interpretation is sent.
    fn censored_case(&self) -> AcceptanceCase {
        let value = ObservationValue::Censored {
            comparator: "<".to_owned(),
            number: "0.50".to_owned(),
        };
        let found = read(&self.oru(value, ("F", "F")).encode(), |reader, segments| {
            let obx = find(segments, "OBX")?;
            let ok = reader.text(obx.field(2)) == "SN"
                && reader.component(obx.field(5), 1) == "<"
                && reader.component(obx.field(5), 2) == "0.50"
                && obx.field(8).is_empty();
            ok.then_some(())
        });
        case(
            "censored_value",
            found.is_none().then_some("censored_not_sn"),
        )
    }

    /// OBR-25 and OBX-11 carry the corrected or withdrawn status.
    fn statuses_case(
        &self,
        code: &str,
        statuses: (&'static str, &'static str),
        expected: &str,
    ) -> AcceptanceCase {
        let bytes = self
            .oru(ObservationValue::Number("12.5".to_owned()), statuses)
            .encode();
        let found = read(&bytes, |reader, segments| {
            let obr = find(segments, "OBR")?;
            let obx = find(segments, "OBX")?;
            let ok = reader.text(obr.field(25)) == "C" && reader.text(obx.field(11)) == expected;
            ok.then_some(())
        });
        case(code, found.is_none().then_some("status_mismatch"))
    }

    /// Over MLLP, MSH-15 and MSH-16 are empty, so the partner answers on the connection.
    fn acknowledgement_case(&self) -> AcceptanceCase {
        let found = read(&self.final_oru().encode(), |_, segments| {
            let msh = find(segments, "MSH")?;
            (msh.field(15).is_empty() && msh.field(16).is_empty()).then_some(())
        });
        case(
            "acknowledgement_mode",
            found.is_none().then_some("acknowledgement_not_requested"),
        )
    }
}

/// `check` on the message read back; `None` when it is unreadable or the check fails.
fn read(bytes: &[u8], check: impl Fn(&Reader, &[Segment<'_>]) -> Option<()>) -> Option<()> {
    let text = message_text(bytes)?;
    let separators = separators(text)?;
    let segments = segments(text, separators);
    check(&Reader { separators }, &segments)
}

#[cfg(test)]
#[path = "acceptance_cases_tests.rs"]
mod tests;
