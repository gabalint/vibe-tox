//! The encoder against a hand-written synthetic message.

use super::*;

fn laboratory() -> Organization {
    Organization {
        name: "Synthetic Tox Lab".to_owned(),
        id: "00D0000000".to_owned(),
    }
}

fn message() -> Oru {
    Oru {
        control_id: "01a1a000-0000-7000-8000-000000000011".to_owned(),
        created_at: "20261104102000-0600".to_owned(),
        processing_id: "P",
        laboratory: laboratory(),
        receiving_facility: "CLINIC-A".to_owned(),
        acknowledged: false,
        patient: Patient {
            identifiers: vec![(
                "MRN-00000".to_owned(),
                "CLINIC-A".to_owned(),
                "MR".to_owned(),
            )],
            family_name: "Doe".to_owned(),
            given_names: Some("Jane".to_owned()),
            birth_date: "19800101".to_owned(),
            sex: Some("Female".to_owned()),
        },
        location: Some(("LOC-1".to_owned(), "Main office".to_owned())),
        order: Order {
            placer_order: Some("ORD-000124".to_owned()),
            provider: (
                "1234567893".to_owned(),
                "Test".to_owned(),
                Some("Doctor".to_owned()),
            ),
            facility: Organization {
                name: "Clinic A".to_owned(),
                id: "CLINIC-A".to_owned(),
            },
            collected_at: "20261103083000-0500".to_owned(),
            released_at: "20261104101500-0600".to_owned(),
        },
        groups: vec![ObservationGroup {
            filler_order: "A26000005-UR-1".to_owned(),
            service: Coded {
                code: "CA-AMPH-CONF".to_owned(),
                text: "Amphetamines Panel".to_owned(),
                loinc: None,
            },
            status: "C",
            observations: vec![
                Observation {
                    code: Coded {
                        code: "CA-AMP".to_owned(),
                        text: "Amphetamine".to_owned(),
                        loinc: Some("12345-6".to_owned()),
                    },
                    sub_id: "1".to_owned(),
                    value: ObservationValue::Number("152.40".to_owned()),
                    unit: Some("ng/mL".to_owned()),
                    reference_range: None,
                    status: "C",
                    performer: laboratory(),
                },
                Observation {
                    code: Coded {
                        code: "CA-MAMP".to_owned(),
                        text: "Methamphetamine".to_owned(),
                        loinc: None,
                    },
                    sub_id: "1".to_owned(),
                    value: ObservationValue::Censored {
                        comparator: "<".to_owned(),
                        number: "0.50".to_owned(),
                    },
                    unit: Some("ng/mL".to_owned()),
                    reference_range: None,
                    status: "C",
                    performer: laboratory(),
                },
            ],
        }],
        specimen: Specimen {
            id: "A26000005".to_owned(),
            kind: ("UR".to_owned(), "Urine".to_owned()),
            collected_at: "20261103083000-0500".to_owned(),
            received_at: "20261103120000-0600".to_owned(),
        },
    }
}

#[test]
fn an_oru_has_every_segment_in_order_with_exact_values() {
    let text = String::from_utf8(message().encode()).unwrap();
    let segments: Vec<&str> = text.split('\r').collect();
    assert_eq!(
        segments,
        [
            "MSH|^~\\&|VIBETOX LIS|Synthetic Tox Lab^00D0000000^CLIA||CLINIC-A|20261104102000-0600||ORU^R01^ORU_R01|01a1a000-0000-7000-8000-000000000011|P|2.5.1|||NE|NE||UNICODE UTF-8",
            "PID|1||MRN-00000^^^CLINIC-A^MR||Doe^Jane||19800101|F",
            "PV1|1|U|LOC-1^^^^^^^^Main office",
            "ORC|RE|ORD-000124|A26000005-UR-1^VIBETOX LIS|||||||||1234567893^Test^Doctor^^^^^^^^^^NPI|||||||||Clinic A^^^^^^^^^CLINIC-A",
            "OBR|1|ORD-000124|A26000005-UR-1^VIBETOX LIS|CA-AMPH-CONF^Amphetamines Panel^L|||20261103083000-0500|||||||||1234567893^Test^Doctor^^^^^^^^^^NPI||||||20261104101500-0600|||C",
            "OBX|1|NM|CA-AMP^Amphetamine^L^12345-6^^LN|1|152.40|ng/mL^^UCUM|||||C||||||||||||Synthetic Tox Lab^^^^^^^^^00D0000000",
            "OBX|2|SN|CA-MAMP^Methamphetamine^L|1|<^0.50|ng/mL^^UCUM|||||C||||||||||||Synthetic Tox Lab^^^^^^^^^00D0000000",
            "SPM|1|^A26000005||UR^Urine^L|||||||||||||20261103083000-0500|20261103120000-0600",
            "",
        ]
    );
}

#[test]
fn a_message_sent_over_mllp_asks_for_the_application_ack_in_original_mode() {
    let oru = Oru {
        acknowledged: true,
        ..message()
    };
    let text = String::from_utf8(oru.encode()).unwrap();
    let msh = text.split('\r').next().unwrap();
    // MSH-13 to MSH-17 empty, MSH-15 and MSH-16 among them: the partner answers on the
    // same connection.
    assert!(msh.ends_with("|P|2.5.1||||||UNICODE UTF-8"), "{msh}");
}

#[test]
fn delimiters_in_values_are_escaped_and_cannot_add_fields() {
    assert_eq!(esc("a|b^c&d~e\\f"), "a\\F\\b\\S\\c\\T\\d\\R\\e\\E\\f");
    assert_eq!(esc("line\r\nnext"), "line\\.br\\next");
    let mut oru = message();
    oru.patient.family_name = "O|Brien^X\rPID|2".to_owned();
    let text = String::from_utf8(oru.encode()).unwrap();
    let pid = text.split('\r').nth(1).unwrap();
    assert_eq!(pid.split('|').count(), 9);
    assert_eq!(text.matches("PID|").count(), 1);
}

#[test]
fn a_result_without_a_value_has_no_type() {
    let mut oru = message();
    oru.groups[0].observations[0].value = ObservationValue::None;
    oru.groups[0].observations[0].status = "X";
    let text = String::from_utf8(oru.encode()).unwrap();
    assert!(text.contains("OBX|1||CA-AMP^Amphetamine^L^12345-6^^LN|1||ng/mL^^UCUM|||||X|"));
}
