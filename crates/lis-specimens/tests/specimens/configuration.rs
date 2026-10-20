//! Storage locations and label printers.

use lis_core::{Capability, ErrorCode};
use lis_labels::PrinterDestinations;
use lis_specimens::{
    ConfigureLabelPrinterInput, ConfigureStorageLocationInput, PrinterTransport, SpecimenError,
};

use crate::{code_of, counts, fixture, last_event, read, run};

const CONFIGURATION: &[Capability] = &[Capability::ManageConfiguration];

fn printer_input() -> ConfigureLabelPrinterInput {
    ConfigureLabelPrinterInput {
        printer_id: None,
        expected_revision: None,
        code: "bench-1".to_owned(),
        name: "Synthetic bench printer".to_owned(),
        transport: PrinterTransport::Tcp,
        ip_address: Some("192.168.0.10".to_owned()),
        port: None,
        queue: None,
        active: true,
    }
}

fn configure(
    f: &crate::Fixture,
    input: ConfigureLabelPrinterInput,
) -> Result<lis_specimens::LabelPrinterView, SpecimenError> {
    run(&f.t, CONFIGURATION, move |ctx| {
        lis_specimens::configure_printer(ctx, input, PrinterDestinations::LabNetwork)
    })
}

#[test]
fn printers_are_addressed_by_ip_or_checked_queue_name() {
    let f = fixture();
    let created = configure(&f, printer_input()).unwrap();
    assert_eq!(created.port, Some(9100));
    assert_eq!(created.revision, 1);
    assert_eq!(last_event(&f.t).0, "specimen.configure_printer");
    let mut host_name = printer_input();
    host_name.code = "bench-2".to_owned();
    host_name.ip_address = Some("printer.lab.example".to_owned());
    assert_eq!(code_of(configure(&f, host_name)), ErrorCode::Validation);
    let mut option_queue = printer_input();
    option_queue.code = "bench-3".to_owned();
    option_queue.transport = PrinterTransport::Cups;
    option_queue.ip_address = None;
    option_queue.queue = Some("-o evil".to_owned());
    assert_eq!(
        code_of(configure(&f, option_queue.clone())),
        ErrorCode::Validation
    );
    option_queue.queue = Some("zebra_gk420d".to_owned());
    assert_eq!(configure(&f, option_queue).unwrap().ip_address, None);
    assert_eq!(code_of(configure(&f, printer_input())), ErrorCode::Conflict);
    let printers = read(&f.t, lis_specimens::list_printers);
    assert_eq!(printers.len(), 2);
}

/// The `details` of the newest clinical audit event.
fn last_details(f: &crate::Fixture) -> serde_json::Value {
    let details: String = read(&f.t, |conn| {
        Ok(conn.query_row(
            "SELECT details FROM audit_events WHERE stream = 'clinical' ORDER BY seq DESC LIMIT 1",
            [],
            |row| row.get(0),
        )?)
    });
    serde_json::from_str(&details).unwrap()
}

#[test]
fn tcp_printers_must_be_on_the_laboratory_network() {
    let f = fixture();
    for (index, allowed) in [
        "10.1.2.3",
        "172.16.5.5",
        "192.168.40.2",
        "169.254.1.1",
        "fd00::5",
        "fe80::5",
        "::ffff:192.168.40.3",
    ]
    .into_iter()
    .enumerate()
    {
        let mut input = printer_input();
        input.code = format!("lan-{index}");
        input.ip_address = Some(allowed.to_owned());
        assert!(configure(&f, input).is_ok(), "{allowed}");
    }
    for refused in [
        "127.0.0.1",
        "::1",
        "0.0.0.0",
        "::",
        "224.0.0.1",
        "255.255.255.255",
        "8.8.8.8",
        "2001:4860:4860::8888",
        "100.64.0.1",
        "192.0.2.10",
        "2001:db8::10",
        "::ffff:8.8.8.8",
    ] {
        let mut input = printer_input();
        input.code = "off-lan".to_owned();
        input.ip_address = Some(refused.to_owned());
        let before = counts(&f.t);
        let Err(SpecimenError::Invalid(error)) = configure(&f, input) else {
            panic!("{refused} was accepted");
        };
        assert_eq!(
            (error.field.as_str(), error.code.as_str()),
            ("ipAddress", "not_lab_network"),
            "{refused}"
        );
        assert_eq!(counts(&f.t), before, "{refused}");
    }
}

#[test]
fn updating_a_printer_to_a_public_address_is_refused() {
    let f = fixture();
    let created = configure(&f, printer_input()).unwrap();
    let mut update = printer_input();
    update.printer_id = Some(created.printer_id);
    update.expected_revision = Some(1);
    update.ip_address = Some("203.0.113.7".to_owned());
    assert_eq!(code_of(configure(&f, update)), ErrorCode::Validation);
    let printers = read(&f.t, lis_specimens::list_printers);
    assert_eq!(printers[0].ip_address.as_deref(), Some("192.168.0.10"));
}

#[test]
fn the_configuration_audit_records_the_full_destination() {
    let f = fixture();
    let mut tcp = printer_input();
    tcp.ip_address = Some("FD00::A".to_owned());
    tcp.port = Some(6101);
    let created = configure(&f, tcp).unwrap();
    assert_eq!(
        (created.ip_address.as_deref(), created.port),
        (Some("fd00::a"), Some(6101))
    );
    let details = last_details(&f);
    assert_eq!(details["ip_address"], "fd00::a");
    assert_eq!(details["port"], 6101);
    assert_eq!(details["transport"], "tcp");
    assert!(details.get("queue").is_none());

    let mut cups = printer_input();
    cups.code = "bench-cups".to_owned();
    cups.transport = PrinterTransport::Cups;
    cups.ip_address = None;
    cups.queue = Some("Zebra_GK420d".to_owned());
    let created = configure(&f, cups).unwrap();
    assert_eq!(created.queue.as_deref(), Some("Zebra_GK420d"));
    let details = last_details(&f);
    assert_eq!(details["queue"], "zebra_gk420d");
    assert!(details.get("ip_address").is_none());
    assert!(details.get("port").is_none());
}

#[test]
fn updates_need_the_current_revision() {
    let f = fixture();
    let created = configure(&f, printer_input()).unwrap();
    let mut update = printer_input();
    update.printer_id = Some(created.printer_id);
    update.active = false;
    assert_eq!(code_of(configure(&f, update.clone())), ErrorCode::Conflict);
    update.expected_revision = Some(1);
    let updated = configure(&f, update.clone()).unwrap();
    assert_eq!((updated.revision, updated.active), (2, false));
    assert_eq!(code_of(configure(&f, update)), ErrorCode::Conflict);
}

#[test]
fn location_codes_are_unique() {
    let f = fixture();
    let input = ConfigureStorageLocationInput {
        location_id: None,
        expected_revision: None,
        code: "freezer-1".to_owned(),
        name: "Synthetic freezer".to_owned(),
        storage_condition: Some("-20 C".to_owned()),
        active: true,
    };
    let again = input.clone();
    run(&f.t, CONFIGURATION, move |ctx| {
        lis_specimens::configure_location(ctx, input)
    })
    .unwrap();
    assert_eq!(
        code_of(run(&f.t, CONFIGURATION, move |ctx| {
            lis_specimens::configure_location(ctx, again)
        })),
        ErrorCode::Conflict
    );
    let locations = read(&f.t, lis_specimens::list_locations);
    assert_eq!(locations[0].code, "freezer-1");
}
