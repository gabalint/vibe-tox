//! An MLLP listener's settings, on synthetic addresses only.

use super::*;
use crate::partner::model::{CharacterSet, MllpAcknowledgement};

fn partner(address: &str, note: &str) -> AllowedPartner {
    AllowedPartner {
        address: address.to_owned(),
        note: note.to_owned(),
    }
}

fn listener() -> MllpListener {
    MllpListener {
        address: " 192.168.1.20 ".to_owned(),
        port: 6660,
        allowed_partners: vec![partner("192.168.1.50", " Hospital B interface engine ")],
        acknowledgement: MllpAcknowledgement::Application,
        character_set: CharacterSet::Utf8,
        idle_timeout_seconds: 300,
    }
}

fn field_of(error: PartnerError) -> String {
    error.field_errors()[0].field.clone()
}

#[test]
fn a_listener_on_the_lab_network_is_kept_with_its_partners() {
    let valid = valid_mllp_listener(listener()).unwrap();
    assert_eq!(valid.address, "192.168.1.20");
    assert_eq!(
        valid.allowed_partners[0].note,
        "Hospital B interface engine"
    );
    assert!(valid.allows("192.168.1.50".parse().unwrap()));
    assert!(!valid.allows("192.168.1.51".parse().unwrap()));
    assert_eq!(
        listener_destination(&valid),
        "mllp-listen://192.168.1.20:6660 from 192.168.1.50"
    );
}

#[test]
fn every_address_or_an_internet_address_is_refused() {
    for (address, field) in [
        ("0.0.0.0", "settings.mllpListener.address"),
        ("8.8.8.8", "settings.mllpListener.address"),
        ("lis.lab.example", "settings.mllpListener.address"),
    ] {
        let refused = valid_mllp_listener(MllpListener {
            address: address.to_owned(),
            ..listener()
        });
        assert_eq!(field_of(refused.unwrap_err()), field, "{address}");
    }
}

#[test]
fn the_partners_must_be_on_the_lab_network_distinct_and_bounded() {
    let with = |partners: Vec<AllowedPartner>| {
        valid_mllp_listener(MllpListener {
            allowed_partners: partners,
            ..listener()
        })
        .map_err(field_of)
    };
    // The dialog edits the list as one field: every problem lands on it.
    let field = "settings.mllpListener.allowedPartners";
    let many = (1..=21)
        .map(|n| partner(&format!("10.0.0.{n}"), ""))
        .collect();
    for partners in [
        Vec::new(),
        vec![partner("192.168.1.50", ""), partner("8.8.4.4", "")],
        vec![partner("192.168.1.50", ""), partner(" 192.168.1.50", "")],
        vec![partner("192.168.1.50", &"x".repeat(81))],
        many,
    ] {
        assert_eq!(with(partners).unwrap_err(), field);
    }
}

#[test]
fn the_port_and_idle_timeout_are_bounded() {
    for (broken, field) in [
        (
            MllpListener {
                port: 0,
                ..listener()
            },
            "settings.mllpListener.port",
        ),
        (
            MllpListener {
                idle_timeout_seconds: 5,
                ..listener()
            },
            "settings.mllpListener.idleTimeoutSeconds",
        ),
    ] {
        assert_eq!(field_of(valid_mllp_listener(broken).unwrap_err()), field);
    }
}

#[test]
fn the_lis_listens_over_mllp_for_hl7_orders_only() {
    let check = |message_type, protocol, settings| valid_listener(message_type, protocol, settings);
    assert!(
        check(
            PartnerMessageType::Hl7Orders,
            PartnerProtocol::MllpListen,
            Some(listener())
        )
        .unwrap()
        .is_some()
    );
    let refused = check(
        PartnerMessageType::Hl7Oru,
        PartnerProtocol::MllpListen,
        Some(listener()),
    );
    assert_eq!(field_of(refused.unwrap_err()), "settings.protocol");
    assert!(
        check(
            PartnerMessageType::Hl7Orders,
            PartnerProtocol::Folder,
            Some(listener())
        )
        .is_err()
    );
    assert_eq!(
        check(PartnerMessageType::Hl7Orders, PartnerProtocol::Folder, None).unwrap(),
        None
    );
}
