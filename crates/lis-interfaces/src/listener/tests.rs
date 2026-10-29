use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, Shutdown, SocketAddr, TcpListener, TcpStream};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use lis_astm::{ACK, CR, ENQ, EOT, FrameEnd, NAK, ReceiveEvent, encode_frame, encode_records};
use lis_core::{Capability, CorrelationId, FixedClock, Principal};
use lis_store::testing::{TestDatabase, local_port, principal, test_start};
use lis_store::{ArtifactDigest, Database, StoreError, lint_schema};
use tempfile::TempDir;

use super::*;
use crate::commands::{begin_receipt, complete_receipt, persist_frame};
use crate::instrument_commands::{
    register_instrument, set_instrument_enabled, update_instrument_connection,
};
use crate::model::{
    ConnectionSettings, DataBits, Parity, RegisterInstrument, SetInstrumentEnabled, StopBits,
    UpdateInstrumentConnection,
};
use crate::peer_rejections::{MAX_TRACKED_PEERS, RejectionTally};
use crate::queries::{get_receipt, list_instruments, list_receipt_frames};
use crate::reconcile_receipts;
use crate::session::{Session, assembled_message};
use crate::{
    AbortCode, InstrumentTransport, InterfaceError, Receipt, ReceiptId, ReceiptState, SCHEMA,
};

const RECORDS: [&[u8]; 2] = [b"H|\\^&|||SYNTHETIC", b"L|1|N"];

struct Fixture {
    database: Arc<Database>,
    _dir: TempDir,
}

impl Fixture {
    fn open() -> Self {
        let dir = tempfile::tempdir().expect("temporary data directory");
        let clock = Arc::new(FixedClock::new(test_start()));
        let database = Arc::new(
            Database::open(dir.path(), &[SCHEMA], clock).expect("interface database opens"),
        );
        Self {
            database,
            _dir: dir,
        }
    }

    fn register(&self, connection: ConnectionSettings, enabled: bool) -> Instrument {
        register_on(&self.database, connection, enabled)
    }

    fn receipt(&self, receipt_id: ReceiptId) -> Receipt {
        self.database
            .read_blocking(move |conn| {
                get_receipt(
                    conn,
                    &Principal::system("test-reader", [Capability::InstrumentIntake]),
                    receipt_id,
                )
            })
            .expect("receipt reads")
    }

    fn receipt_count(&self) -> u32 {
        self.database
            .read_blocking(|conn| {
                let count = conn.query_row("SELECT COUNT(*) FROM ifc_receipts", [], |row| {
                    row.get::<_, u32>(0)
                })?;
                Ok::<_, InterfaceError>(count)
            })
            .expect("receipt count reads")
    }

    fn latest_receipt(&self) -> Receipt {
        let receipt_id = self
            .database
            .read_blocking(|conn| {
                let id = conn.query_row(
                    "SELECT id FROM ifc_receipts ORDER BY rowid DESC LIMIT 1",
                    [],
                    |row| row.get(0),
                )?;
                Ok::<_, InterfaceError>(id)
            })
            .expect("latest receipt id reads");
        self.receipt(receipt_id)
    }

    fn wait_for_state(&self, count: u32, state: ReceiptState) -> Receipt {
        wait_until(|| self.receipt_count() >= count && self.latest_receipt().state == state);
        self.latest_receipt()
    }
}

#[test]
fn schema_obeys_module_rules() {
    assert!(lint_schema(&[SCHEMA]).expect("schema lints").is_empty());
}

#[test]
fn register_instrument_refuses_missing_capability_and_writes_audit() {
    let fixture = Fixture::open();
    let connection = tcp_settings(local_port());
    let denied =
        fixture
            .database
            .write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
                register_instrument(
                    ctx,
                    RegisterInstrument {
                        code: "DENIED".to_owned(),
                        name: "Denied analyzer".to_owned(),
                        connection,
                        enabled: false,
                    },
                )
            });
    assert!(matches!(
        denied,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));

    fixture.register(tcp_settings(local_port()), false);
    let audit_count = fixture
        .database
        .read_blocking(|conn| {
            let count = conn.query_row(
                "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.register_instrument'",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            Ok::<_, InterfaceError>(count)
        })
        .expect("audit count reads");
    assert_eq!(audit_count, 1);
}

#[test]
fn update_and_enable_refuse_missing_capability_and_stale_revision() {
    let fixture = Fixture::open();
    let instrument = fixture.register(tcp_settings(local_port()), false);
    let update_id = instrument.id;
    let update_denied =
        fixture
            .database
            .write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
                update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id: update_id,
                        expected_revision: 1,
                        connection: tcp_settings(local_port()),
                    },
                )
            });
    assert!(matches!(
        update_denied,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));

    let enable_id = instrument.id;
    let enable_denied =
        fixture
            .database
            .write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
                set_instrument_enabled(
                    ctx,
                    SetInstrumentEnabled {
                        instrument_id: enable_id,
                        expected_revision: 1,
                        enabled: true,
                    },
                )
            });
    assert!(matches!(
        enable_denied,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));

    let stale_id = instrument.id;
    let stale = fixture.database.write_blocking(
        principal(&[Capability::ManageConfiguration]),
        CorrelationId::generate(),
        move |ctx| {
            set_instrument_enabled(
                ctx,
                SetInstrumentEnabled {
                    instrument_id: stale_id,
                    expected_revision: 2,
                    enabled: true,
                },
            )
        },
    );
    assert!(matches!(stale, Err(InterfaceError::RevisionConflict)));
}

#[test]
fn update_and_enable_increment_revision_and_write_audit() {
    let fixture = Fixture::open();
    let instrument = fixture.register(tcp_settings(local_port()), false);
    let update_id = instrument.id;
    let updated = fixture
        .database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id: update_id,
                        expected_revision: 1,
                        connection: tcp_settings(local_port()),
                    },
                )
            },
        )
        .expect("connection updates");
    assert_eq!(updated.revision, 2);
    let enable_id = instrument.id;
    let enabled = fixture
        .database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                set_instrument_enabled(
                    ctx,
                    SetInstrumentEnabled {
                        instrument_id: enable_id,
                        expected_revision: 2,
                        enabled: true,
                    },
                )
            },
        )
        .expect("instrument enables");
    assert_eq!(enabled.revision, 3);
    assert!(enabled.enabled);
    let audit_count = fixture
        .database
        .read_blocking(|conn| {
            let count = conn.query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE action IN ('interface.update_connection', 'interface.set_enabled')",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            Ok::<_, InterfaceError>(count)
        })
        .expect("audit count reads");
    assert_eq!(audit_count, 2);
}

#[test]
fn duplicate_message_links_receipt_without_second_outbox_item() {
    let fixture = Fixture::open();
    let instrument = fixture.register(tcp_settings(local_port()), true);
    let message = assembled_message(records());
    let artifact = fixture
        .database
        .artifacts()
        .put(&message)
        .expect("artifact stores");
    let first = complete_direct_receipt(&fixture, instrument.id, artifact.clone());
    let second = complete_direct_receipt(&fixture, instrument.id, artifact);
    assert_eq!(second.duplicate_of, Some(first.id));
    let outbox_count = fixture
        .database
        .read_blocking(|conn| {
            let count = conn.query_row(
                "SELECT COUNT(*) FROM store_outbox WHERE topic = 'ifc.receipt.completed'",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            Ok::<_, InterfaceError>(count)
        })
        .expect("outbox count reads");
    assert_eq!(outbox_count, 1);
}

#[test]
fn receipt_guards_refuse_wrong_source_frame_order_and_artifact() {
    let fixture = Fixture::open();
    let instrument = fixture.register(tcp_settings(local_port()), true);
    let other = fixture.register(tcp_settings(local_port()), true);
    let receipt = begin_direct_receipt(&fixture, instrument.id);
    let receipt_id = receipt.id;
    let frame = encode_records(&records()).expect("records encode")[0].clone();
    let wrong_source = fixture.database.write_blocking(
        Principal::instrument(other.id),
        CorrelationId::generate(),
        move |ctx| persist_frame(ctx, receipt_id, 0, frame),
    );
    assert!(matches!(
        wrong_source,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));

    let invalid_id = receipt.id;
    let invalid = fixture.database.write_blocking(
        Principal::instrument(instrument.id),
        CorrelationId::generate(),
        move |ctx| persist_frame(ctx, invalid_id, 0, vec![0x02, 0x31]),
    );
    assert!(matches!(invalid, Err(InterfaceError::InvalidFrame)));

    let ordered_id = receipt.id;
    let frame = encode_records(&records()).expect("records encode")[0].clone();
    let out_of_order = fixture.database.write_blocking(
        Principal::instrument(instrument.id),
        CorrelationId::generate(),
        move |ctx| persist_frame(ctx, ordered_id, 1, frame),
    );
    assert!(matches!(
        out_of_order,
        Err(InterfaceError::FrameOutOfSequence)
    ));

    let frame = encode_records(&records()).expect("records encode")[0].clone();
    let persist_id = receipt.id;
    fixture
        .database
        .write_blocking(
            Principal::instrument(instrument.id),
            CorrelationId::generate(),
            move |ctx| persist_frame(ctx, persist_id, 0, frame),
        )
        .expect("valid frame persists");
    let wrong_artifact = fixture
        .database
        .artifacts()
        .put(b"unrelated bytes")
        .expect("wrong artifact stores");
    let complete_id = receipt.id;
    let mismatch = fixture.database.write_blocking(
        Principal::instrument(instrument.id),
        CorrelationId::generate(),
        move |ctx| complete_receipt(ctx, complete_id, wrong_artifact),
    );
    assert!(matches!(mismatch, Err(InterfaceError::ArtifactMismatch)));
    assert_eq!(fixture.receipt(receipt.id).state, ReceiptState::Receiving);
}

#[test]
fn public_tcp_bind_is_refused_before_configuration_write() {
    let fixture = Fixture::open();
    let refused = fixture.database.write_blocking(
        principal(&[Capability::ManageConfiguration]),
        CorrelationId::generate(),
        move |ctx| {
            register_instrument(
                ctx,
                RegisterInstrument {
                    code: "PUBLIC".to_owned(),
                    name: "Public listener".to_owned(),
                    connection: ConnectionSettings::AstmTcp {
                        bind_address: "8.8.8.8".parse().expect("address parses"),
                        port: 5000,
                        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
                    },
                    enabled: true,
                },
            )
        },
    );
    assert!(matches!(
        refused,
        Err(InterfaceError::InvalidTcpBindAddress)
    ));
}

#[test]
fn tcp_configuration_requires_a_nonempty_peer_allowlist() {
    let fixture = Fixture::open();
    let refused = fixture.database.write_blocking(
        principal(&[Capability::ManageConfiguration]),
        CorrelationId::generate(),
        move |ctx| {
            register_instrument(
                ctx,
                RegisterInstrument {
                    code: "NO-PEER".to_owned(),
                    name: "No peer analyzer".to_owned(),
                    connection: ConnectionSettings::AstmTcp {
                        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                        port: local_port(),
                        allowed_peers: Vec::new(),
                    },
                    enabled: true,
                },
            )
        },
    );
    assert!(matches!(
        refused,
        Err(InterfaceError::InvalidTcpAllowedPeers)
    ));
    let configured = fixture
        .database
        .read_blocking(|conn| {
            list_instruments(
                conn,
                &Principal::system("test-reader", [Capability::ManageConfiguration]),
            )
        })
        .expect("instrument list reads");
    assert!(configured.is_empty());

    let instrument = fixture.register(tcp_settings(local_port()), false);
    let instrument_id = instrument.id;
    let revision = instrument.revision;
    let update = fixture.database.write_blocking(
        principal(&[Capability::ManageConfiguration]),
        CorrelationId::generate(),
        move |ctx| {
            update_instrument_connection(
                ctx,
                UpdateInstrumentConnection {
                    instrument_id,
                    expected_revision: revision,
                    connection: ConnectionSettings::AstmTcp {
                        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                        port: local_port(),
                        allowed_peers: Vec::new(),
                    },
                },
            )
        },
    );
    assert!(matches!(
        update,
        Err(InterfaceError::InvalidTcpAllowedPeers)
    ));
    let unchanged = fixture
        .database
        .read_blocking(move |conn| {
            crate::get_instrument(
                conn,
                &Principal::system("test-reader", [Capability::ManageConfiguration]),
                instrument_id,
            )
        })
        .expect("instrument reads");
    assert_eq!(unchanged.revision, 1);
}

#[test]
fn configuration_and_receipt_queries_refuse_missing_capabilities() {
    let fixture = Fixture::open();
    let denied_instruments = fixture
        .database
        .read_blocking(|conn| list_instruments(conn, &principal(&[])));
    assert!(matches!(
        denied_instruments,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));
    let receipt_id = ReceiptId::generate();
    let denied_receipt = fixture
        .database
        .read_blocking(move |conn| get_receipt(conn, &principal(&[]), receipt_id));
    assert!(matches!(
        denied_receipt,
        Err(InterfaceError::Store(StoreError::Denied(_)))
    ));
}

#[test]
fn startup_reconciliation_marks_receipt_interrupted_with_audit() {
    let test_db = TestDatabase::open(&[SCHEMA]).expect("test database opens");
    let instrument = register_on(&test_db.db, tcp_settings(local_port()), true);
    let instrument_id = instrument.id;
    let receipt = test_db
        .db
        .write_blocking(
            Principal::instrument(instrument_id),
            CorrelationId::generate(),
            move |ctx| {
                begin_receipt(
                    ctx,
                    instrument_id,
                    InstrumentTransport::AstmTcp,
                    Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 41000))),
                )
            },
        )
        .expect("receipt begins");
    let test_db = test_db.reopen().expect("database reopens");
    let summary = reconcile_receipts(&test_db.db).expect("reconciliation succeeds");
    assert_eq!(summary.interrupted, 1);
    assert_eq!(summary.recovered, 0);
    let receipt_id = receipt.id;
    let (state, audit_count) = test_db
        .db
        .read_blocking(move |conn| {
            let state: String = conn.query_row(
                "SELECT state FROM ifc_receipts WHERE id = ?",
                [receipt_id],
                |row| row.get(0),
            )?;
            let audit_count = conn.query_row(
                "SELECT COUNT(*) FROM audit_events WHERE action = 'interface.interrupt_receipt'",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            Ok::<_, InterfaceError>((state, audit_count))
        })
        .expect("reconciled receipt reads");
    assert_eq!(state, "interrupted");
    assert_eq!(audit_count, 1);
}

#[test]
fn startup_recovers_complete_frames_committed_before_eot() {
    let test_db = TestDatabase::open(&[SCHEMA]).expect("test database opens");
    let instrument = register_on(&test_db.db, tcp_settings(local_port()), true);
    let receipt_id = persist_recoverable_transfer_without_eot(&test_db.db, instrument.id);

    let test_db = test_db
        .reopen()
        .expect("database reopens after simulated crash");
    let summary = reconcile_receipts(&test_db.db).expect("reconciliation succeeds");
    assert_eq!(summary.recovered, 1);
    assert_eq!(summary.interrupted, 0);
    let expected = assembled_message(recoverable_records());
    let (receipt, outbox_count, recovery_audit_count) = test_db
        .db
        .read_blocking(move |conn| {
            let receipt = get_receipt(
                conn,
                &Principal::system("test-reader", [Capability::InstrumentIntake]),
                receipt_id,
            )?;
            let outbox_count = conn.query_row(
                "SELECT COUNT(*) FROM store_outbox WHERE topic = 'ifc.receipt.completed'",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            let recovery_audit_count = conn.query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE action = 'interface.complete_receipt'
                 AND details LIKE '%\"recovered_at_startup\":true%'",
                [],
                |row| row.get::<_, u32>(0),
            )?;
            Ok::<_, InterfaceError>((receipt, outbox_count, recovery_audit_count))
        })
        .expect("recovered receipt reads");
    assert_eq!(receipt.state, ReceiptState::Complete);
    let digest = ArtifactDigest::from_str(
        receipt
            .artifact_digest
            .as_deref()
            .expect("recovered receipt has artifact"),
    )
    .expect("artifact digest parses");
    assert_eq!(
        test_db
            .db
            .artifacts()
            .read(&digest)
            .expect("recovered artifact reads"),
        expected
    );
    assert_eq!(outbox_count, 1);
    assert_eq!(recovery_audit_count, 1);
}

fn persist_recoverable_transfer_without_eot(
    database: &Database,
    instrument_id: InstrumentId,
) -> ReceiptId {
    let statuses = Arc::new(Mutex::new(BTreeMap::new()));
    let mut session = Session::new(
        instrument_id,
        InstrumentTransport::AstmTcp,
        Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 41001))),
    );
    let mut output = Vec::new();
    session
        .handle_event(
            ReceiveEvent::Bytes(vec![ENQ]),
            &mut output,
            database,
            &statuses,
        )
        .expect("ENQ is handled");
    assert_eq!(output, vec![ACK]);
    for frame in encode_records(&recoverable_records()).expect("records encode") {
        output.clear();
        session
            .handle_event(ReceiveEvent::Bytes(frame), &mut output, database, &statuses)
            .expect("frame is durably handled");
        assert_eq!(output, vec![ACK]);
    }
    database
        .read_blocking(|conn| {
            conn.query_row(
                "SELECT id FROM ifc_receipts WHERE state = 'receiving'",
                [],
                |row| row.get::<_, ReceiptId>(0),
            )
            .map_err(InterfaceError::from)
        })
        .expect("receiving receipt reads")
}

/// Starts an enabled TCP listener on localhost whose only allowed peer is
/// 127.0.0.2, so every local connection is an unexpected peer.
fn start_listener_rejecting_localhost(fixture: &Fixture) -> (u16, Instrument, ListenerSupervisor) {
    let port = local_port();
    let connection = ConnectionSettings::AstmTcp {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers: vec![IpAddr::V4(Ipv4Addr::new(127, 0, 0, 2))],
    };
    let instrument = fixture.register(connection, true);
    let supervisor = ListenerSupervisor::new(Arc::clone(&fixture.database));
    supervisor.start_all().expect("listener starts");
    (port, instrument, supervisor)
}

#[test]
fn unexpected_tcp_peer_is_rejected_before_receipt_and_security_audited() {
    let fixture = Fixture::open();
    let (port, instrument, supervisor) = start_listener_rejecting_localhost(&fixture);

    let mut peer = connect(port);
    peer.write_all(&[ENQ]).expect("unexpected peer sends ENQ");
    let mut response = [0_u8; 1];
    match peer.read(&mut response) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            ) => {}
        outcome => panic!("unexpected peer was not closed: {outcome:?}"),
    }
    wait_until(|| {
        fixture
            .database
            .read_blocking(|conn| {
                let count = conn.query_row(
                    "SELECT COUNT(*) FROM audit_events
                     WHERE action = 'interface.reject_peer'
                     AND stream = 'security' AND principal_kind = 'system'",
                    [],
                    |row| row.get::<_, u32>(0),
                )?;
                Ok::<_, InterfaceError>(count)
            })
            .is_ok_and(|count| count == 1)
    });
    assert_eq!(fixture.receipt_count(), 0);
    let status = supervisor.status_snapshot();
    assert_eq!(status[0].instrument_id, instrument.id);
    assert_eq!(
        status[0].detail_code,
        Some(ListenerDetailCode::UnexpectedPeerRejected)
    );
    supervisor.stop_all().expect("listener stops");
}

#[test]
fn idle_tcp_client_is_closed_so_next_analyzer_can_establish() {
    let fixture = Fixture::open();
    let port = local_port();
    fixture.register(tcp_settings(port), true);
    let supervisor = ListenerSupervisor::with_establishment_timeout(
        Arc::clone(&fixture.database),
        Duration::from_millis(100),
    );
    supervisor.start_all().expect("listener starts");

    let _idle = connect(port);
    let mut analyzer = connect(port);
    begin_transfer(&mut analyzer);
    for frame in encode_records(&records()).expect("records encode") {
        analyzer.write_all(&frame).expect("frame writes");
        assert_eq!(read_control(&mut analyzer), ACK);
    }
    analyzer.write_all(&[EOT]).expect("EOT writes");
    fixture.wait_for_state(1, ReceiptState::Complete);
    supervisor.stop_all().expect("listener stops");
}

#[test]
fn tcp_simulator_commits_before_ack_and_handles_retransmission_and_checksum() {
    let (fixture, port, instrument, supervisor) = started_tcp_listener();

    let mut simulator = connect(port);
    begin_transfer(&mut simulator);
    let frames = encode_records(&records()).expect("records encode");
    for (index, frame) in frames.iter().enumerate() {
        simulator.write_all(frame).expect("frame writes");
        assert_eq!(read_control(&mut simulator), ACK);
        let expected = u32::try_from(index + 1).expect("small frame count");
        let receipt = fixture.latest_receipt();
        let committed = fixture
            .database
            .read_blocking(move |conn| {
                Ok::<_, InterfaceError>(
                    list_receipt_frames(
                        conn,
                        &Principal::system("test-reader", [Capability::InstrumentIntake]),
                        receipt.id,
                    )?
                    .len(),
                )
            })
            .expect("frames read");
        assert_eq!(committed, expected as usize);
        if index == 0 {
            simulator.write_all(frame).expect("retransmission writes");
            assert_eq!(read_control(&mut simulator), ACK);
            assert_eq!(fixture.latest_receipt().frame_count, expected);
        }
    }
    simulator.write_all(&[EOT]).expect("EOT writes");
    let complete = fixture.wait_for_state(1, ReceiptState::Complete);
    let digest = ArtifactDigest::from_str(
        complete
            .artifact_digest
            .as_deref()
            .expect("complete receipt has artifact"),
    )
    .expect("artifact digest parses");
    assert_eq!(
        fixture
            .database
            .artifacts()
            .read(&digest)
            .expect("artifact reads"),
        assembled_message(records())
    );
    drop(simulator);

    let mut simulator = connect(port);
    begin_transfer(&mut simulator);
    let frame = encode_records(&records()).expect("records encode")[0].clone();
    let mut corrupted = frame.clone();
    let checksum_index = corrupted.len() - 4;
    corrupted[checksum_index] = if corrupted[checksum_index] == b'0' {
        b'1'
    } else {
        b'0'
    };
    simulator.write_all(&corrupted).expect("bad frame writes");
    assert_eq!(read_control(&mut simulator), NAK);
    simulator.write_all(&frame).expect("valid retry writes");
    assert_eq!(read_control(&mut simulator), ACK);
    simulator.write_all(&[EOT]).expect("EOT writes");
    fixture.wait_for_state(2, ReceiptState::Complete);
    drop(simulator);

    supervisor.stop_all().expect("listener stops");
    assert_eq!(instrument.id, supervisor.status_snapshot()[0].instrument_id);
}

#[test]
fn tcp_mid_record_eot_and_connection_drop_abort_without_artifact() {
    let (fixture, port, _, supervisor) = started_tcp_listener();

    let mut simulator = connect(port);
    begin_transfer(&mut simulator);
    let frame = encode_frame(1, b"H|unfinished", FrameEnd::Intermediate)
        .expect("intermediate frame encodes");
    simulator.write_all(&frame).expect("frame writes");
    assert_eq!(read_control(&mut simulator), ACK);
    simulator.write_all(&[EOT]).expect("EOT writes");
    let aborted = fixture.wait_for_state(1, ReceiptState::Aborted);
    assert_eq!(aborted.abort_reason, Some(AbortCode::IncompleteRecord));
    assert!(aborted.artifact_digest.is_none());
    drop(simulator);

    let mut simulator = connect(port);
    begin_transfer(&mut simulator);
    let frame = encode_records(&records()).expect("records encode")[0].clone();
    simulator.write_all(&frame).expect("frame writes");
    assert_eq!(read_control(&mut simulator), ACK);
    simulator
        .shutdown(Shutdown::Both)
        .expect("connection shuts down");
    drop(simulator);
    let aborted = fixture.wait_for_state(2, ReceiptState::Aborted);
    assert_eq!(aborted.abort_reason, Some(AbortCode::ConnectionLost));
    assert!(aborted.artifact_digest.is_none());

    supervisor.stop_all().expect("listener stops");
}

#[test]
fn repeated_bad_checksum_reaches_aborted_terminal_state() {
    let (fixture, port, _, supervisor) = started_tcp_listener();
    let mut simulator = connect(port);
    begin_transfer(&mut simulator);
    let mut corrupted = encode_records(&records()).expect("records encode")[0].clone();
    let checksum_index = corrupted.len() - 4;
    corrupted[checksum_index] = if corrupted[checksum_index] == b'0' {
        b'1'
    } else {
        b'0'
    };
    for _ in 0..6 {
        simulator.write_all(&corrupted).expect("bad frame writes");
        assert_eq!(read_control(&mut simulator), NAK);
    }
    let aborted = fixture.wait_for_state(1, ReceiptState::Aborted);
    assert_eq!(aborted.abort_reason, Some(AbortCode::RetryLimitExceeded));
    assert!(aborted.artifact_digest.is_none());
    supervisor.stop_all().expect("listener stops");
}

#[test]
fn timer_expiry_aborts_receiving_receipt() {
    let fixture = Fixture::open();
    let instrument = fixture.register(serial_settings("synthetic-timeout"), true);
    let statuses = Arc::new(Mutex::new(BTreeMap::new()));
    let mut session = Session::new(instrument.id, InstrumentTransport::AstmSerial, None);
    let mut output = Vec::new();
    session
        .handle_event(
            ReceiveEvent::Bytes(vec![ENQ]),
            &mut output,
            &fixture.database,
            &statuses,
        )
        .expect("ENQ is handled");
    assert_eq!(output, vec![ACK]);
    let timer_id = session.current_timer().expect("timer starts");
    session
        .handle_event(
            ReceiveEvent::TimerExpired(timer_id),
            &mut output,
            &fixture.database,
            &statuses,
        )
        .expect("timeout is handled");
    let receipt = fixture.latest_receipt();
    assert_eq!(receipt.state, ReceiptState::Aborted);
    assert_eq!(receipt.abort_reason, Some(AbortCode::ReceiveTimeout));
}

#[cfg(unix)]
#[test]
fn serial_pseudoterminal_completes_same_durable_flow() {
    use std::sync::atomic::Ordering;

    use serialport::{SerialPort, TTYPort};

    use crate::session::{StreamContext, run_stream};

    let fixture = Fixture::open();
    let instrument = fixture.register(serial_settings("synthetic-pty"), true);
    let (mut simulator, mut receiver) = TTYPort::pair().expect("pseudo-terminal pair opens");
    receiver
        .set_timeout(IO_POLL)
        .expect("receiver timeout sets");
    simulator
        .set_timeout(Duration::from_secs(2))
        .expect("simulator timeout sets");
    let stop = Arc::new(AtomicBool::new(false));
    let thread_stop = Arc::clone(&stop);
    let database = Arc::clone(&fixture.database);
    let statuses = Arc::new(Mutex::new(BTreeMap::new()));
    let thread_statuses = Arc::clone(&statuses);
    let instrument_id = instrument.id;
    let worker = thread::spawn(move || {
        let context = StreamContext {
            database: &database,
            statuses: &thread_statuses,
            instrument_id,
            transport: InstrumentTransport::AstmSerial,
            peer: None,
            stop: &thread_stop,
            establishment_timeout: DEFAULT_ESTABLISHMENT_TIMEOUT,
        };
        run_stream(&mut receiver, &context)
    });

    begin_transfer(&mut simulator);
    for frame in encode_records(&records()).expect("records encode") {
        simulator.write_all(&frame).expect("frame writes");
        assert_eq!(read_control(&mut simulator), ACK);
    }
    simulator.write_all(&[EOT]).expect("EOT writes");
    let complete = fixture.wait_for_state(1, ReceiptState::Complete);
    assert_eq!(complete.transport, InstrumentTransport::AstmSerial);
    assert!(complete.peer.is_none());
    stop.store(true, Ordering::Release);
    assert!(worker.join().expect("listener joins").is_ok());
}

fn register_on(database: &Database, connection: ConnectionSettings, enabled: bool) -> Instrument {
    database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                register_instrument(
                    ctx,
                    RegisterInstrument {
                        code: format!("SYNTH-{}", CorrelationId::generate()),
                        name: "Synthetic analyzer".to_owned(),
                        connection,
                        enabled,
                    },
                )
            },
        )
        .expect("instrument registers")
}

fn started_tcp_listener() -> (Fixture, u16, Instrument, ListenerSupervisor) {
    let fixture = Fixture::open();
    let port = local_port();
    let instrument = fixture.register(tcp_settings(port), true);
    let supervisor = ListenerSupervisor::new(Arc::clone(&fixture.database));
    supervisor.start_all().expect("listener starts");
    (fixture, port, instrument, supervisor)
}

fn complete_direct_receipt(
    fixture: &Fixture,
    instrument_id: InstrumentId,
    artifact: lis_store::StoredArtifact,
) -> Receipt {
    let receipt = begin_direct_receipt(fixture, instrument_id);
    let receipt_id = receipt.id;
    for (ordinal, frame) in encode_records(&records())
        .expect("records encode")
        .into_iter()
        .enumerate()
    {
        let ordinal = u32::try_from(ordinal).expect("small frame count");
        fixture
            .database
            .write_blocking(
                Principal::instrument(instrument_id),
                CorrelationId::generate(),
                move |ctx| persist_frame(ctx, receipt_id, ordinal, frame),
            )
            .expect("frame persists");
    }
    fixture
        .database
        .write_blocking(
            Principal::instrument(instrument_id),
            CorrelationId::generate(),
            move |ctx| complete_receipt(ctx, receipt_id, artifact),
        )
        .expect("receipt completes")
}

fn begin_direct_receipt(fixture: &Fixture, instrument_id: InstrumentId) -> Receipt {
    fixture
        .database
        .write_blocking(
            Principal::instrument(instrument_id),
            CorrelationId::generate(),
            move |ctx| {
                begin_receipt(
                    ctx,
                    instrument_id,
                    InstrumentTransport::AstmTcp,
                    Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 42000))),
                )
            },
        )
        .expect("receipt begins")
}

fn records() -> Vec<Vec<u8>> {
    RECORDS.iter().map(|record| record.to_vec()).collect()
}

fn recoverable_records() -> Vec<Vec<u8>> {
    [
        b"H|\\^&|||SYNTH-ANALYZER^1.0|||||||P|LIS2-A2|20261012120000".as_slice(),
        b"P|1|SYN-PRACTICE|SYN-LAB".as_slice(),
        b"O|1|SYN-0001|SYN-INST-0001|^^^SYN-TEST|R||||||N||||URINE".as_slice(),
        b"R|1|^^^SYN-TEST|<0.0100|ng/mL|0.0000-1.0000|L||F||SYN-OP||20261012115959|SYN-INST"
            .as_slice(),
        b"L|1|N".as_slice(),
    ]
    .into_iter()
    .map(<[u8]>::to_vec)
    .collect()
}

fn tcp_settings(port: u16) -> ConnectionSettings {
    ConnectionSettings::AstmTcp {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
    }
}

fn serial_settings(port_path: &str) -> ConnectionSettings {
    ConnectionSettings::AstmSerial {
        port_path: port_path.to_owned(),
        baud_rate: 9_600,
        data_bits: DataBits::Eight,
        parity: Parity::None,
        stop_bits: StopBits::One,
    }
}

fn connect(port: u16) -> TcpStream {
    let stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("simulator connects");
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("read timeout sets");
    stream
}

fn begin_transfer(stream: &mut impl WriteRead) {
    stream.write_all(&[ENQ]).expect("ENQ writes");
    assert_eq!(read_control(stream), ACK);
}

trait WriteRead: Write + Read {}
impl<T: Write + Read> WriteRead for T {}

fn read_control(stream: &mut impl Read) -> u8 {
    let mut response = [0_u8; 1];
    stream
        .read_exact(&mut response)
        .expect("control byte reads");
    response[0]
}

fn wait_until(mut condition: impl FnMut() -> bool) {
    let deadline = Instant::now() + Duration::from_secs(2);
    while !condition() {
        assert!(Instant::now() < deadline, "condition did not become true");
        thread::yield_now();
    }
}

#[test]
fn assembled_message_restores_record_terminators() {
    assert_eq!(
        assembled_message(vec![b"H|x".to_vec(), b"L|1".to_vec()]),
        b"H|x\rL|1\r"
    );
    assert_eq!(assembled_message(vec![vec![]]), vec![CR]);
}

fn listener_state(
    supervisor: &ListenerSupervisor,
) -> Option<(ListenerState, Option<ListenerDetailCode>)> {
    let status = supervisor.status_snapshot();
    status
        .first()
        .map(|status| (status.state, status.detail_code))
}

fn change_settings(
    fixture: &Fixture,
    instrument_id: InstrumentId,
    expected_revision: i64,
    change: Option<ConnectionSettings>,
) {
    fixture
        .database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| match change {
                Some(connection) => update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id,
                        expected_revision,
                        connection,
                    },
                ),
                None => set_instrument_enabled(
                    ctx,
                    SetInstrumentEnabled {
                        instrument_id,
                        expected_revision,
                        enabled: false,
                    },
                ),
            },
        )
        .expect("configuration changes");
}

// REQ-INT-009: a configuration change takes effect when the listeners restart.
#[test]
fn restart_applies_changed_connection_and_disabled_state() {
    let (fixture, old_port, instrument, supervisor) = started_tcp_listener();
    wait_until(|| listener_state(&supervisor) == Some((ListenerState::Listening, None)));
    let new_port = local_port();
    change_settings(&fixture, instrument.id, 1, Some(tcp_settings(new_port)));
    supervisor.restart_all().expect("listeners restart");
    wait_until(|| listener_state(&supervisor) == Some((ListenerState::Listening, None)));
    let mut stream = connect(new_port);
    begin_transfer(&mut stream);
    drop(stream);
    assert!(
        TcpStream::connect((Ipv4Addr::LOCALHOST, old_port)).is_err(),
        "the old port is closed"
    );

    change_settings(&fixture, instrument.id, 2, None);
    supervisor.restart_all().expect("listeners restart");
    assert_eq!(
        listener_state(&supervisor),
        Some((
            ListenerState::Disabled,
            Some(ListenerDetailCode::ConfigurationDisabled)
        ))
    );
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, new_port)).is_err());
    supervisor.stop_all().expect("listeners stop");
}

#[test]
fn connection_settings_use_camel_case_and_refuse_unknown_fields() {
    let json = serde_json::json!({
        "kind": "astm_tcp",
        "bindAddress": "127.0.0.1",
        "port": 4100,
        "allowedPeers": ["127.0.0.1"],
    });
    let settings: ConnectionSettings = serde_json::from_value(json.clone()).expect("parses");
    assert_eq!(settings.transport(), InstrumentTransport::AstmTcp);
    let mut extra = json;
    extra["acceptAnyPeer"] = serde_json::json!(true);
    assert!(serde_json::from_value::<ConnectionSettings>(extra).is_err());
}

#[test]
fn configuration_errors_name_their_field() {
    let error = InterfaceError::InvalidTcpAllowedPeers;
    assert_eq!(error.code(), lis_core::ErrorCode::Validation);
    assert_eq!(error.field_errors()[0].field, "connection.allowedPeers");
    assert!(InterfaceError::RevisionConflict.field_errors().is_empty());
    assert_eq!(
        InterfaceError::RevisionConflict.code(),
        lis_core::ErrorCode::Conflict
    );
}

/// Rejected-connection counts of the `interface.reject_peer` events, in order.
fn rejection_audit_counts(fixture: &Fixture) -> Vec<i64> {
    fixture
        .database
        .read_blocking(|conn| {
            let mut statement = conn.prepare(
                "SELECT json_extract(details, '$.rejected_connections') FROM audit_events
                 WHERE action = 'interface.reject_peer' ORDER BY seq",
            )?;
            let counts = statement
                .query_map([], |row| row.get::<_, i64>(0))?
                .collect::<Result<Vec<_>, _>>()?;
            Ok::<_, InterfaceError>(counts)
        })
        .expect("rejection audits read")
}

fn wait_closed(stream: &mut TcpStream) {
    let mut response = [0_u8; 1];
    match stream.read(&mut response) {
        Ok(0) => {}
        Err(error)
            if matches!(
                error.kind(),
                std::io::ErrorKind::ConnectionReset | std::io::ErrorKind::BrokenPipe
            ) => {}
        outcome => panic!("rejected peer was not closed: {outcome:?}"),
    }
}

// REQ-INT-009, NFR-08: a connection flood from an unexpected peer is closed
// without one database write per connection; the counts reach the audit.
#[test]
fn rejected_peer_flood_writes_one_audit_per_window_with_its_count() {
    let fixture = Fixture::open();
    let (port, _, supervisor) = start_listener_rejecting_localhost(&fixture);

    for _ in 0..10 {
        let mut batch: Vec<TcpStream> = (0..100).map(|_| connect(port)).collect();
        batch.iter_mut().for_each(wait_closed);
    }
    assert_eq!(rejection_audit_counts(&fixture), [1]);
    assert_eq!(fixture.receipt_count(), 0);

    supervisor.stop_all().expect("listener stops");
    assert_eq!(rejection_audit_counts(&fixture), [1, 999]);
}

// REQ-INT-009: the tally never delays or refuses a configured analyzer.
#[test]
fn configured_analyzer_is_served_amid_rejected_peer_flood() {
    let fixture = Fixture::open();
    let source = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("test listener binds");
    let port = source.local_addr().expect("local address reads").port();
    let instrument = fixture.register(tcp_settings(port), true);
    let context = TcpListenerConfig {
        au: None,
        database: Arc::clone(&fixture.database),
        statuses: Arc::new(Mutex::new(BTreeMap::new())),
        instrument_id: instrument.id,
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        stop: Arc::new(AtomicBool::new(false)),
        establishment_timeout: DEFAULT_ESTABLISHMENT_TIMEOUT,
    };
    let mut tally = RejectionTally::default();
    flood(&source, &context, &mut tally, 0..500);

    let analyzer = thread::spawn(move || {
        let mut stream = connect(port);
        begin_transfer(&mut stream);
        for frame in encode_records(&records()).expect("records encode") {
            stream.write_all(&frame).expect("frame writes");
            assert_eq!(read_control(&mut stream), ACK);
        }
        stream.write_all(&[EOT]).expect("EOT writes");
    });
    let (stream, peer) = source.accept().expect("analyzer accepts");
    crate::tcp::handle_connection(stream, peer, &context, &mut tally);
    analyzer.join().expect("analyzer finishes");
    flood(&source, &context, &mut tally, 500..1_000);

    assert_eq!(fixture.latest_receipt().state, ReceiptState::Complete);
    assert!(rejection_audit_counts(&fixture).is_empty());
    let due = tally.take_due(Instant::now());
    assert_eq!(due.len(), MAX_TRACKED_PEERS + 1);
    assert_eq!(
        due.iter()
            .map(|item| item.rejected_connections)
            .sum::<usize>(),
        1_000
    );
}

/// Feeds real localhost streams to the accept handler under synthetic,
/// unconfigured private source addresses.
fn flood(
    source: &TcpListener,
    context: &TcpListenerConfig,
    tally: &mut RejectionTally,
    indexes: std::ops::Range<u16>,
) {
    let port = source.local_addr().expect("local address reads").port();
    for index in indexes {
        let _client = connect(port);
        let (stream, _) = source.accept().expect("flood connection accepts");
        let [high, low] = index.to_be_bytes();
        let peer = SocketAddr::from((Ipv4Addr::new(10, 9, high, low), 40_000));
        crate::tcp::handle_connection(stream, peer, context, tally);
    }
}
