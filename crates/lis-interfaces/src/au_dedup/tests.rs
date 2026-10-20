//! Correcting an AU wire profile must not suppress a valid resend.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;

use lis_au::{AuConfig, FrameEnd, TransmissionClass, encode_frame};
use lis_core::{Capability, CorrelationId, FixedClock, InstrumentId, Principal};
use lis_store::Database;
use lis_store::testing::{principal, test_start};

use crate::commands::{begin_receipt, complete_receipt, persist_frame};
use crate::{
    AuConnectionSettings, ConnectionSettings, DataBits, Instrument, InstrumentTransport,
    InterfaceError, Parity, Receipt, RegisterInstrument, SCHEMA, StopBits,
    UpdateInstrumentConnection, register_instrument, repo, update_instrument_connection,
};

fn protocol(width: u8) -> AuConfig {
    let mut config = crate::au_session::tests::settings(TransmissionClass::B).protocol;
    config.format.unit_no = false;
    config.format.sample_id_digits = width;
    config
}

fn connection(width: u8, tcp: bool) -> ConnectionSettings {
    let au = AuConnectionSettings {
        protocol: protocol(width),
        frame_timeout_ms: 2000,
        block_timeout_ms: 2000,
    };
    if tcp {
        return ConnectionSettings::AuTcp {
            bind_address: Ipv4Addr::LOCALHOST.into(),
            port: 31234,
            allowed_peers: vec![Ipv4Addr::LOCALHOST.into()],
            au,
        };
    }
    ConnectionSettings::AuSerial {
        port_path: "synthetic-unused-port".to_owned(),
        baud_rate: 9600,
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::One,
        au,
    }
}

fn registered(database: &Database) -> Instrument {
    database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            |ctx| {
                register_instrument(
                    ctx,
                    RegisterInstrument {
                        code: "SYN-AU-DEDUP".to_owned(),
                        name: "Synthetic AU duplicate check".to_owned(),
                        connection: connection(8, false),
                        enabled: true,
                    },
                )
            },
        )
        .unwrap()
}

fn fixture() -> (tempfile::TempDir, Database, Instrument) {
    let dir = tempfile::tempdir().unwrap();
    let database = Database::open(
        dir.path(),
        &[SCHEMA],
        Arc::new(FixedClock::new(test_start())),
    )
    .unwrap();
    let instrument = registered(&database);
    (dir, database, instrument)
}

fn change(database: &Database, instrument: &Instrument, tcp: bool) -> Instrument {
    let id = instrument.id;
    let revision = instrument.revision;
    database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id: id,
                        expected_revision: revision,
                        connection: connection(9, tcp),
                    },
                )
            },
        )
        .unwrap()
}

fn receive(database: &Database, instrument: &Instrument, bytes: &[u8]) -> Receipt {
    let id = instrument.id;
    let transport = instrument.connection.transport();
    let peer = transport
        .is_tcp()
        .then(|| SocketAddr::from((Ipv4Addr::LOCALHOST, 42000)));
    let bytes = bytes.to_vec();
    let artifact = database.artifacts().put(&bytes).unwrap();
    database
        .write_blocking(
            Principal::instrument(id),
            CorrelationId::generate(),
            move |ctx| {
                let receipt = begin_receipt(ctx, id, transport, peer)?;
                persist_frame(ctx, receipt.id, 0, bytes)?;
                complete_receipt(ctx, receipt.id, artifact)
            },
        )
        .unwrap()
}

fn imports(database: &Database) -> u32 {
    database
        .read_blocking(|conn| {
            Ok::<_, InterfaceError>(conn.query_row(
                "SELECT COUNT(*) FROM store_outbox WHERE topic='ifc.receipt.completed'",
                [],
                |row| row.get(0),
            )?)
        })
        .unwrap()
}

#[test]
fn corrected_profile_resend_imports_and_same_profile_retries_still_deduplicate() {
    let (_dir, database, instrument) = fixture();
    let bytes = encode_frame(
        &protocol(9),
        b"D 000101U0001SYN000001    E01 12.50  ",
        FrameEnd::Text,
    )
    .unwrap();
    let wrong_frame = crate::au_evidence::frame(&protocol(8), &bytes).unwrap();
    assert!(lis_au::parse_message(&protocol(8), &wrong_frame).is_err());
    let corrected_frame = crate::au_evidence::frame(&protocol(9), &bytes).unwrap();
    assert!(lis_au::parse_message(&protocol(9), &corrected_frame).is_ok());
    let malformed = receive(&database, &instrument, &bytes);
    assert_eq!(malformed.duplicate_of, None);
    assert_eq!(imports(&database), 1);

    let instrument = change(&database, &instrument, false);
    let corrected = receive(&database, &instrument, &bytes);
    assert_eq!(corrected.duplicate_of, None);
    assert_eq!(corrected.message_sha256, malformed.message_sha256);
    assert_eq!(imports(&database), 2);
    assert_eq!(
        receive(&database, &instrument, &bytes).duplicate_of,
        Some(corrected.id)
    );

    // TCP and serial carry the same AU protocol, so changing the adapter is not a new result.
    let instrument = change(&database, &instrument, true);
    assert_eq!(
        receive(&database, &instrument, &bytes).duplicate_of,
        Some(corrected.id)
    );
    assert_eq!(imports(&database), 2);
}

#[test]
fn au_and_non_au_receipts_are_different_duplicate_families() {
    let (_dir, database, instrument) = fixture();
    let bytes = encode_frame(
        &protocol(8),
        b"D 000101U0001SYN00001    E01 12.50  ",
        FrameEnd::Text,
    )
    .unwrap();
    let received = receive(&database, &instrument, &bytes);
    let id: InstrumentId = instrument.id;
    database
        .write_blocking(
            principal(&[Capability::InstrumentIntake]),
            CorrelationId::generate(),
            move |ctx| {
                let conn = ctx.tx();
                let digest = received.message_sha256.as_deref().unwrap();
                assert_eq!(repo::first_duplicate(conn, id, digest)?, None);
                let mut legacy = received.clone();
                legacy.id = crate::ReceiptId::generate();
                legacy.transport = InstrumentTransport::AstmSerial;
                repo::insert_receipt(conn, &legacy)?;
                repo::complete_receipt(conn, legacy.id, test_start(), digest, None)?;
                assert_eq!(repo::first_duplicate(conn, id, digest)?, Some(legacy.id));
                assert_eq!(
                    repo::first_au_duplicate(conn, &received, digest)?,
                    Some(received.id)
                );
                Ok::<_, InterfaceError>(())
            },
        )
        .unwrap();
}
