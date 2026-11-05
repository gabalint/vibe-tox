//! The order reader on synthetic messages only.

use super::*;

/// A synthetic OML^O21 from a clinic's EMR: one requisition with two tests.
fn oml() -> String {
    [
        "MSH|^~\\&|SYN-EMR|CLINIC-A|VIBETOX LIS|LAB|20261104093000||OML^O21^OML_O21|MSG-0001|P|2.5.1",
        "PID|1||MRN-00042^^^CLINIC-A^MR~SYN-123^^^SYNPAYER^MB||Doe^Jane^Q||19800101|F",
        "PV1|1|O|CLINIC-A-MAIN",
        "ORC|NW|ORD-000124^SYN-EMR||||||||||1234567893^Test^Doctor|||||||||CLINIC-A",
        "OBR|1|ORD-000124^SYN-EMR||SYN-OPI^Synthetic opiates^L|||20261104081500",
        "DG1|1||Z79.891^Long term opioid use^I10",
        "ORC|NW|ORD-000125^SYN-EMR",
        "OBR|2|ORD-000125^SYN-EMR||SYN-AMP^Synthetic amphetamines^L",
    ]
    .join("\r")
}

#[test]
fn an_oml_message_is_read_as_supplied() {
    let text = |value: &str| value.to_owned();
    let some = |value: &str| Some(value.to_owned());
    assert_eq!(
        read_order_message(oml().as_bytes()).unwrap(),
        OrderMessage {
            message_type: text("OML^O21"),
            control_id: text("MSG-0001"),
            sending_facility: text("CLINIC-A"),
            patient: OrderPatient {
                identifiers: vec![
                    (text("MRN-00042"), some("CLINIC-A"), some("MR")),
                    (text("SYN-123"), some("SYNPAYER"), some("MB")),
                ],
                family_name: some("Doe"),
                given_name: some("Jane"),
                birth_date: some("19800101"),
                sex: some("F"),
            },
            location: some("CLINIC-A-MAIN"),
            placer_order: text("ORD-000124"),
            placer_authority: some("SYN-EMR"),
            provider_id: some("1234567893"),
            tests: vec![text("SYN-OPI"), text("SYN-AMP")],
            collected_at: some("20261104081500"),
            diagnoses: vec![(text("Z79.891"), some("I10"))],
        }
    );
}

#[test]
fn an_orm_message_with_line_feeds_and_framing_is_read() {
    let orm = format!(
        "\u{b}{}\u{1c}\r",
        oml()
            .replace("OML^O21^OML_O21", "ORM^O01")
            .replace('\r', "\n")
    );
    let message = read_order_message(orm.as_bytes()).unwrap();
    assert_eq!(message.message_type, "ORM^O01");
    assert_eq!(message.tests.len(), 2);
}

#[test]
fn the_placer_group_number_names_the_requisition() {
    let grouped = oml().replace(
        "ORC|NW|ORD-000124^SYN-EMR|||",
        "ORC|NW|ORD-000124^SYN-EMR||REQ-77^SYN-EMR|",
    );
    let message = read_order_message(grouped.as_bytes()).unwrap();
    assert_eq!(message.placer_order, "REQ-77");
}

#[test]
fn escapes_are_read_and_cannot_split_a_value() {
    let escaped = oml().replace("Doe^Jane^Q", "O\\S\\Brien\\F\\Doe^Jane");
    let message = read_order_message(escaped.as_bytes()).unwrap();
    assert_eq!(message.patient.family_name.as_deref(), Some("O^Brien|Doe"));
}

#[test]
fn what_is_not_understood_is_refused_with_a_code() {
    let cases = [
        (oml().replace("MSH|", "XXX|"), ParseProblem::NotHl7),
        (
            oml().replace("OML^O21^OML_O21", "ADT^A01"),
            ParseProblem::UnsupportedMessage,
        ),
        (oml().replace("|MSG-0001|", "||"), ParseProblem::NoControlId),
        (
            oml().replace("MRN-00042^^^CLINIC-A^MR~SYN-123^^^SYNPAYER^MB", ""),
            ParseProblem::NoPatient,
        ),
        (
            oml().replace("ORC|NW|ORD-000125", "ORC|CA|ORD-000125"),
            ParseProblem::UnsupportedControl,
        ),
        (
            oml().replace("SYN-AMP^Synthetic amphetamines^L", ""),
            ParseProblem::NoTest,
        ),
    ];
    for (message, problem) in cases {
        assert_eq!(read_order_message(message.as_bytes()), Err(problem));
    }
    assert_eq!(
        read_order_message(&[0xff, 0xfe, 0x00]),
        Err(ParseProblem::NotText)
    );
    let no_orders: String = oml()
        .split('\r')
        .filter(|line| !line.starts_with("ORC") && !line.starts_with("OBR"))
        .collect::<Vec<_>>()
        .join("\r");
    assert_eq!(
        read_order_message(no_orders.as_bytes()),
        Err(ParseProblem::NoOrder)
    );
}
