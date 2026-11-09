//! Acceptance cases of outgoing connections, on synthetic mappings only.

use super::*;
use crate::partner::mapping_model::{CodeMapping, LisRecordId};
use crate::partner::model::{MllpAcknowledgement, MllpPartner, PartnerProtocol, Schedule};

fn settings(message_type: PartnerMessageType, pattern: &str) -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "HL7 Results - Clinic A".to_owned(),
        partner: "CLINIC-A".to_owned(),
        message_type,
        protocol: PartnerProtocol::Folder,
        rules: Vec::new(),
        schedule: Schedule::Immediately,
        file_name_pattern: pattern.to_owned(),
        sftp: None,
        intake: None,
        mllp: None,
        mllp_listener: None,
    }
}

fn mapping(kind: MappingKind, code: &str, unit: Option<&str>) -> CodeMapping {
    CodeMapping {
        kind,
        lis_id: LisRecordId::generate(),
        values: MappingValues {
            partner_code: code.to_owned(),
            loinc: None,
            ucum_unit: unit.map(str::to_owned),
            sequence_role: None,
        },
    }
}

fn problems(cases: &[AcceptanceCase]) -> Vec<(&str, Option<&str>)> {
    cases
        .iter()
        .map(|case| (case.code.as_str(), case.problem.as_deref()))
        .collect()
}

#[test]
fn an_hl7_connection_with_complete_mappings_passes_every_case() {
    let mappings = ApprovedMappings {
        version: 2,
        mappings: vec![
            mapping(MappingKind::Test, "CA-AMPH-CONF", None),
            mapping(MappingKind::Analyte, "CA-AMP", Some("ng/mL")),
        ],
    };
    let hl7 = settings(PartnerMessageType::Hl7Oru, "{accession}_r{revision}.hl7");
    let mut cases = hl7_cases(&hl7, Some(&mappings));
    cases.push(file_name_case(&hl7));
    assert!(cases.iter().all(|case| case.passed), "{cases:?}");
    assert_eq!(
        problems(&cases)
            .iter()
            .map(|(code, _)| *code)
            .collect::<Vec<_>>(),
        [
            "mappings_complete",
            "message_header",
            "patient_identifier",
            "patient_sex",
            "mapped_codes",
            "censored_value",
            "correction",
            "withdrawal",
            "file_name"
        ]
    );
}

#[test]
fn missing_mappings_or_units_fail_and_say_why() {
    let hl7 = settings(PartnerMessageType::Hl7Oru, "{accession}_r{revision}.hl7");
    let none = hl7_cases(&hl7, None);
    assert_eq!(
        problems(&none)[0],
        ("mappings_complete", Some("no_approved_mappings"))
    );
    assert_eq!(
        problems(&none)[4],
        ("mapped_codes", Some("no_approved_mappings"))
    );
    let unitless = ApprovedMappings {
        version: 1,
        mappings: vec![
            mapping(MappingKind::Test, "CA-AMPH-CONF", None),
            mapping(MappingKind::Analyte, "CA-AMP", None),
        ],
    };
    let cases = hl7_cases(&hl7, Some(&unitless));
    assert_eq!(
        problems(&cases)[0],
        ("mappings_complete", Some("analyte_without_unit"))
    );
}

#[test]
fn over_mllp_the_partner_is_asked_to_answer_and_no_file_is_named() {
    let mut hl7 = settings(PartnerMessageType::Hl7Oru, "");
    hl7.protocol = PartnerProtocol::MllpConnect;
    hl7.mllp = Some(MllpPartner {
        address: "10.20.0.15".to_owned(),
        port: 6661,
        acknowledgement: MllpAcknowledgement::Application,
        ack_timeout_seconds: 30,
    });
    let cases = hl7_cases(&hl7, None);
    assert_eq!(
        problems(&cases).last(),
        Some(&("acknowledgement_mode", None))
    );
}

#[test]
fn a_file_name_must_keep_its_extension() {
    let pdf = settings(PartnerMessageType::ReportPdf, "{accession}_r{revision}.pdf");
    assert!(file_name_case(&pdf).passed);
    let wrong = settings(PartnerMessageType::ReportPdf, "{accession}_r{revision}.txt");
    assert_eq!(
        file_name_case(&wrong).problem.as_deref(),
        Some("file_name_invalid")
    );
}
