//! The synthetic test file of each message type: it says what it is, names the
//! connection and the time, and holds no patient data.

use jiff::tz::TimeZone;
use lis_core::UtcTimestamp;
use lis_identity::LaboratoryProfileView;
use lis_interfaces::partner::hl7::{
    Coded, Observation, ObservationGroup, ObservationValue, Order, Organization, Oru, Patient,
    Specimen,
};
use lis_interfaces::partner::{ConnectionTarget, PartnerConnectionId, PartnerMessageType};

use crate::partner_delivery::hl7_message::dtm;

const NOTE: &str = "Synthetic test message from the Vibe Tox LIS. It contains no patient data.";

/// The test file's bytes and extension for the connection's message type; `None` for an
/// incoming type, to which the LIS sends nothing.
pub(super) fn test_file(
    connection_id: PartnerConnectionId,
    target: &ConnectionTarget,
    laboratory: Option<&LaboratoryProfileView>,
    sent_at: UtcTimestamp,
) -> Option<(Vec<u8>, &'static str)> {
    Some(match target.message_type {
        PartnerMessageType::BillingJson => (test_message(connection_id, sent_at), "json"),
        PartnerMessageType::ReportPdf => (test_pdf(connection_id, sent_at), "pdf"),
        PartnerMessageType::Hl7Oru => (
            Oru {
                // Over MLLP the partner answers the test message too.
                acknowledged: target.destination.mllp.is_some(),
                ..test_oru(connection_id, &target.partner, laboratory, sent_at)
            }
            .encode(),
            "hl7",
        ),
        PartnerMessageType::Hl7Orders => return None,
    })
}

/// The synthetic test file: a fixed format, the connection and the time, no patient data.
fn test_message(connection_id: PartnerConnectionId, sent_at: UtcTimestamp) -> Vec<u8> {
    let mut bytes = serde_json::to_vec_pretty(&serde_json::json!({
        "format": "vibetox-test-v1",
        "connectionId": connection_id.to_string(),
        "sentAt": sent_at.to_string(),
        "note": NOTE,
    }))
    .unwrap_or_default();
    bytes.push(b'\n');
    bytes
}

/// The synthetic test file of a Report PDF connection: a one-page PDF.
fn test_pdf(connection_id: PartnerConnectionId, sent_at: UtcTimestamp) -> Vec<u8> {
    let lines = [
        "Vibe Tox LIS - synthetic test message".to_owned(),
        "It contains no patient data.".to_owned(),
        format!("Connection: {connection_id}"),
        format!("Sent at: {sent_at}"),
    ];
    let text: String = lines
        .iter()
        .map(|line| format!("({line}) Tj 0 -18 Td "))
        .collect();
    let content = format!("BT /F1 12 Tf 72 720 Td {text}ET");
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R >>".to_owned(),
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_owned(),
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] /Contents 4 0 R \
         /Resources << /Font << /F1 5 0 R >> >> >>"
            .to_owned(),
        format!(
            "<< /Length {} >>\nstream\n{content}\nendstream",
            content.len()
        ),
        "<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_owned(),
    ];
    let mut pdf = String::from("%PDF-1.4\n");
    let mut offsets = Vec::with_capacity(objects.len());
    for (index, object) in objects.iter().enumerate() {
        offsets.push(pdf.len());
        pdf.push_str(&format!("{} 0 obj\n{object}\nendobj\n", index + 1));
    }
    let xref = pdf.len();
    pdf.push_str(&format!(
        "xref\n0 {}\n0000000000 65535 f \n",
        objects.len() + 1
    ));
    for offset in offsets {
        pdf.push_str(&format!("{offset:010} 00000 n \n"));
    }
    pdf.push_str(&format!(
        "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
        objects.len() + 1
    ));
    pdf.into_bytes()
}

/// The synthetic test file of an HL7 Results connection: an ORU^R01 with processing ID
/// `T`, a synthetic patient and one text result saying what it is.
fn test_oru(
    connection_id: PartnerConnectionId,
    partner: &str,
    laboratory: Option<&LaboratoryProfileView>,
    sent_at: UtcTimestamp,
) -> Oru {
    let zone = laboratory
        .and_then(|profile| TimeZone::get(&profile.time_zone).ok())
        .unwrap_or(TimeZone::UTC);
    let lab = Organization {
        name: laboratory.map_or_else(|| "Vibe Tox LIS".to_owned(), |profile| profile.name.clone()),
        id: laboratory
            .map(|profile| profile.clia_id.clone())
            .unwrap_or_default(),
    };
    let now = dtm(sent_at, &zone);
    let test = |code: &str, text: &str| Coded {
        code: code.to_owned(),
        text: text.to_owned(),
        loinc: None,
    };
    Oru {
        control_id: format!("TEST-{connection_id}"),
        created_at: now.clone(),
        processing_id: "T",
        laboratory: lab.clone(),
        receiving_facility: partner.to_owned(),
        acknowledged: false,
        patient: Patient {
            identifiers: vec![(
                "TEST-00000".to_owned(),
                "VIBETOX".to_owned(),
                "MR".to_owned(),
            )],
            family_name: "TEST".to_owned(),
            given_names: Some("SYNTHETIC".to_owned()),
            birth_date: "19000101".to_owned(),
            sex: None,
        },
        location: None,
        order: Order {
            placer_order: None,
            provider: ("0000000000".to_owned(), "TEST".to_owned(), None),
            facility: Organization {
                name: partner.to_owned(),
                id: String::new(),
            },
            collected_at: now.clone(),
            released_at: now.clone(),
        },
        groups: vec![ObservationGroup {
            filler_order: "TEST-1".to_owned(),
            service: test("VIBETOX-TEST", "Synthetic test message"),
            status: "F",
            observations: vec![Observation {
                code: test("VIBETOX-NOTE", "Note"),
                sub_id: "1".to_owned(),
                value: ObservationValue::Text(NOTE.to_owned()),
                unit: None,
                reference_range: None,
                status: "F",
                performer: lab,
            }],
        }],
        specimen: Specimen {
            id: "TEST-1".to_owned(),
            kind: ("TEST".to_owned(), "Synthetic".to_owned()),
            collected_at: now.clone(),
            received_at: now,
        },
    }
}
