use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, SocketAddr, TcpStream};
use std::str::FromStr;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use lis_au::{
    AuConfig, FormatConfig, Frame, FrameEnd, FramingConfig, StreamDecoder, TransmissionClass,
    WireEvent, encode_frame,
};
use lis_core::{Capability, CorrelationId, FixedClock, Principal};
use lis_store::testing::{local_port, principal, test_start};
use lis_store::{ArtifactDigest, Database};
use tempfile::TempDir;

use super::AuSession;
use crate::commands::{begin_receipt, persist_frame};
use crate::instrument_commands::{register_instrument, update_instrument_connection};
use crate::listener::StatusMap;
use crate::queries::{get_receipt, list_receipt_frames, receipt_au_profile};
use crate::session::StreamContext;
use crate::{
    AbortCode, AuConnectionSettings, ConnectionSettings, Instrument, InstrumentTransport,
    InterfaceError, ListenerDetailCode, ListenerErrorCode, ListenerSupervisor, Receipt,
    ReceiptState, RegisterInstrument, SCHEMA, UpdateInstrumentConnection, reconcile_receipts,
};

struct Fixture {
    database: Arc<Database>,
    statuses: StatusMap,
    stop: AtomicBool,
    instrument: Instrument,
    settings: AuConnectionSettings,
    _dir: TempDir,
}

impl Fixture {
    fn open(class: TransmissionClass) -> Self {
        let dir = tempfile::tempdir().expect("synthetic data directory");
        let database = Arc::new(
            Database::open(
                dir.path(),
                &[SCHEMA],
                Arc::new(FixedClock::new(test_start())),
            )
            .expect("database opens"),
        );
        let settings = settings(class);
        let connection = connection(31234, settings.clone());
        let instrument = database
            .write_blocking(
                principal(&[Capability::ManageConfiguration]),
                CorrelationId::generate(),
                move |ctx| {
                    register_instrument(
                        ctx,
                        RegisterInstrument {
                            code: "SYN-AU".to_owned(),
                            name: "Synthetic AU analyzer".to_owned(),
                            connection,
                            enabled: true,
                        },
                    )
                },
            )
            .expect("instrument registers");
        Self {
            database,
            statuses: Arc::new(Mutex::new(BTreeMap::new())),
            stop: AtomicBool::new(false),
            instrument,
            settings,
            _dir: dir,
        }
    }
    fn context(&self) -> StreamContext<'_> {
        StreamContext {
            database: &self.database,
            statuses: &self.statuses,
            instrument_id: self.instrument.id,
            transport: InstrumentTransport::AuTcp,
            peer: Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 42000))),
            stop: &self.stop,
            establishment_timeout: Duration::from_secs(5),
        }
    }
    fn receipts(&self) -> Vec<Receipt> {
        self.database
            .read_blocking(|conn| {
                let ids = conn
                    .prepare("SELECT id FROM ifc_receipts ORDER BY rowid")?
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<Vec<_>, _>>()?;
                ids.into_iter()
                    .map(|id| {
                        get_receipt(
                            conn,
                            &Principal::system("synthetic-reader", [Capability::InstrumentIntake]),
                            id,
                        )
                    })
                    .collect::<Result<Vec<_>, _>>()
            })
            .expect("receipts read")
    }
    fn frames(&self, receipt: &Receipt) -> Vec<Vec<u8>> {
        let id = receipt.id;
        self.database
            .read_blocking(move |conn| {
                list_receipt_frames(
                    conn,
                    &Principal::system("synthetic-reader", [Capability::InstrumentIntake]),
                    id,
                )
            })
            .expect("frames read")
            .into_iter()
            .map(|frame| frame.bytes)
            .collect()
    }
    fn imports(&self) -> u32 {
        self.database
            .read_blocking(|conn| {
                Ok::<_, InterfaceError>(conn.query_row(
                    "SELECT COUNT(*) FROM store_outbox WHERE topic='ifc.receipt.completed'",
                    [],
                    |row| row.get(0),
                )?)
            })
            .expect("outbox count reads")
    }
    fn fail_sql(&self, sql: &'static str) {
        self.database
            .write_blocking(
                principal(&[Capability::ManageConfiguration]),
                CorrelationId::generate(),
                move |ctx| {
                    ctx.tx().execute_batch(sql)?;
                    Ok::<_, InterfaceError>(())
                },
            )
            .expect("synthetic failure trigger installs");
    }
}

pub(crate) fn settings(class: TransmissionClass) -> AuConnectionSettings {
    AuConnectionSettings {
        protocol: AuConfig {
            framing: FramingConfig {
                start: vec![2],
                end: vec![3],
                bcc: true,
                use_etb: true,
                max_wire_len: 256,
                class,
            },
            format: FormatConfig {
                unit_no: true,
                rack_digits: 4,
                sample_id_digits: 8,
                sex: false,
                age: false,
                month: false,
                patient_info_digits: [0; 6],
                result_digits: 6,
            },
        },
        frame_timeout_ms: 1000,
        block_timeout_ms: 1000,
    }
}
fn connection(port: u16, au: AuConnectionSettings) -> ConnectionSettings {
    ConnectionSettings::AuTcp {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        au,
    }
}
fn frame(config: &AuConfig, block: u8) -> Frame {
    let mut text = b"D 01000101U0001SYN00001    E01012.34  ".to_vec();
    text[27] = block;
    text_frame(
        config,
        &text,
        if block == b'E' {
            FrameEnd::Text
        } else {
            FrameEnd::Block
        },
    )
}
fn text_frame(config: &AuConfig, text: &[u8], end: FrameEnd) -> Frame {
    let wire = encode_frame(config, text, end).expect("synthetic frame encodes");
    let event = StreamDecoder::new(config.clone())
        .expect("decoder opens")
        .push(&wire)
        .remove(0)
        .expect("frame decodes");
    let WireEvent::Frame(frame) = event else {
        panic!("expected frame");
    };
    frame
}
fn due_ack(session: &mut AuSession, now: Instant, context: &StreamContext<'_>) -> Vec<u8> {
    let mut output = Vec::new();
    session
        .tick(now + Duration::from_millis(499), &mut output, context)
        .expect("early tick");
    assert!(output.is_empty());
    session
        .tick(now + Duration::from_millis(500), &mut output, context)
        .expect("due tick");
    output
}

fn started_partial(fixture: &Fixture, now: Instant) -> AuSession {
    let context = fixture.context();
    let mut session = AuSession::new(&context, &fixture.settings);
    session
        .receive(frame(&fixture.settings.protocol, b'0'), now, &context)
        .expect("first block persists");
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Receiving);
    assert_eq!(fixture.imports(), 0);
    session
}

#[test]
fn class_b_ack_requires_exact_durable_frame_complete_and_turnaround() {
    let fixture = Fixture::open(TransmissionClass::B);
    let context = fixture.context();
    let now = Instant::now();
    let mut session = AuSession::new(&context, &fixture.settings);
    let frame = frame(&fixture.settings.protocol, b'E');
    let wire = frame.wire.expose().clone();
    session
        .receive(frame, now, &context)
        .expect("sample receives");
    let receipt = fixture.receipts().remove(0);
    assert_eq!(receipt.state, ReceiptState::Complete);
    assert_eq!(fixture.frames(&receipt), vec![wire.clone()]);
    assert_eq!(fixture.imports(), 1);
    let digest =
        ArtifactDigest::from_str(receipt.artifact_digest.as_ref().expect("artifact digest"))
            .expect("digest parses");
    assert_eq!(
        fixture
            .database
            .artifacts()
            .read(&digest)
            .expect("artifact reads"),
        wire
    );
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
}

#[test]
fn class_a_retains_results_without_control_response() {
    let fixture = Fixture::open(TransmissionClass::A);
    let context = fixture.context();
    let now = Instant::now();
    let mut session = AuSession::new(&context, &fixture.settings);
    session
        .receive(frame(&fixture.settings.protocol, b'E'), now, &context)
        .expect("sample receives");
    let mut output = Vec::new();
    session
        .tick(now + Duration::from_secs(5), &mut output, &context)
        .expect("tick");
    assert!(output.is_empty());
    assert_eq!(fixture.imports(), 1);
    session
        .receive(frame(&fixture.settings.protocol, b'0'), now, &context)
        .unwrap();
    session
        .tick(now + Duration::from_millis(1000), &mut output, &context)
        .unwrap();
    assert_eq!(
        fixture.receipts()[1].abort_reason,
        Some(AbortCode::ReceiveTimeout)
    );
    assert!(output.is_empty());
}

#[test]
fn multiblock_retry_is_not_persisted_or_appended_twice() {
    let fixture = Fixture::open(TransmissionClass::B);
    let context = fixture.context();
    let mut now = Instant::now();
    let mut session = AuSession::new(&context, &fixture.settings);
    let first = frame(&fixture.settings.protocol, b'0');
    let first_wire = first.wire.expose().clone();
    session
        .receive(first.clone(), now, &context)
        .expect("first block");
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Receiving);
    assert_eq!(fixture.imports(), 0);
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
    now += Duration::from_millis(501);
    session.receive(first, now, &context).expect("retry");
    assert_eq!(fixture.receipts()[0].frame_count, 1);
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
    now += Duration::from_millis(501);
    let final_frame = frame(&fixture.settings.protocol, b'E');
    let final_wire = final_frame.wire.expose().clone();
    session.receive(final_frame, now, &context).expect("final");
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
    let receipt = fixture.receipts().remove(0);
    assert_eq!(receipt.frame_count, 2);
    assert_eq!(fixture.frames(&receipt), vec![first_wire, final_wire]);
    assert_eq!(receipt.state, ReceiptState::Complete);
    assert_eq!(fixture.imports(), 1);
}

#[test]
fn skipped_or_conflicting_blocks_abort_and_never_import() {
    for conflict in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let now = Instant::now();
        let mut session = started_partial(&fixture, now);
        let next = now + Duration::from_millis(501);
        let block = if conflict {
            text_frame(
                &fixture.settings.protocol,
                b"D 01000101U0001SYN00001    001999999  ",
                FrameEnd::Block,
            )
        } else {
            frame(&fixture.settings.protocol, b'2')
        };
        session
            .receive(block, next, &context)
            .expect("invalid block held");
        assert_eq!(due_ack(&mut session, next, &context), vec![21]);
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Aborted);
        assert_eq!(fixture.receipts()[0].frame_count, 2);
        assert_eq!(fixture.imports(), 0);
    }
}

#[test]
fn timeout_and_session_interruption_do_not_import_partial_samples() {
    for interrupt in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let now = Instant::now();
        let mut session = started_partial(&fixture, now);
        if interrupt {
            session
                .receive(
                    text_frame(&fixture.settings.protocol, b"DE01", FrameEnd::Text),
                    now + Duration::from_millis(501),
                    &context,
                )
                .expect("session interruption");
        } else {
            session
                .tick(now + Duration::from_millis(1500), &mut Vec::new(), &context)
                .expect("timeout");
        }
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Aborted);
        assert_eq!(
            fixture.receipts()[0].abort_reason,
            Some(if interrupt {
                AbortCode::IncompleteRecord
            } else {
                AbortCode::ReceiveTimeout
            })
        );
        assert_eq!(fixture.imports(), 0);
        let mut response = Vec::new();
        session
            .tick(now + Duration::from_millis(1600), &mut response, &context)
            .expect("tick");
        session
            .receive(
                frame(&fixture.settings.protocol, b'E'),
                now + Duration::from_millis(1601),
                &context,
            )
            .expect("late tail rejected");
        assert_eq!(fixture.imports(), 0);
        assert_eq!(
            due_ack(&mut session, now + Duration::from_millis(1601), &context),
            vec![21]
        );
    }
}

#[test]
fn query_is_retained_with_unsupported_status_without_import_or_inferred_tri() {
    let fixture = Fixture::open(TransmissionClass::B);
    let context = fixture.context();
    let now = Instant::now();
    let mut session = AuSession::new(&context, &fixture.settings);
    session
        .receive(
            text_frame(
                &fixture.settings.protocol,
                b"R 01000101U0001SYN00001",
                FrameEnd::Text,
            ),
            now,
            &context,
        )
        .expect("query retains");
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Complete);
    assert_eq!(fixture.imports(), 0);
    let status = fixture
        .statuses
        .lock()
        .expect("statuses")
        .get(&fixture.instrument.id)
        .cloned()
        .expect("query status");
    assert_eq!(
        status.detail_code,
        Some(ListenerDetailCode::AuQueryUnsupported)
    );
    assert_eq!(due_ack(&mut session, now, &context), vec![6]);
}

#[test]
fn frame_or_completion_storage_failure_never_schedules_ack_or_import() {
    for completion in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        fixture.fail_sql(if completion {"CREATE TRIGGER synthetic_reject_complete BEFORE UPDATE OF state ON ifc_receipts WHEN NEW.state='complete' BEGIN SELECT RAISE(ABORT,'synthetic storage failure'); END;"}else{"CREATE TRIGGER synthetic_reject_frame BEFORE INSERT ON ifc_receipt_frames BEGIN SELECT RAISE(ABORT,'synthetic storage failure'); END;"});
        let context = fixture.context();
        let now = Instant::now();
        let mut session = AuSession::new(&context, &fixture.settings);
        assert_eq!(
            session.receive(frame(&fixture.settings.protocol, b'E'), now, &context),
            Err(ListenerErrorCode::PersistenceFailed)
        );
        let mut output = Vec::new();
        session
            .tick(now + Duration::from_secs(2), &mut output, &context)
            .expect("tick");
        assert!(output.is_empty());
        assert_eq!(fixture.imports(), 0);
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Receiving);
    }
}

#[test]
fn complete_receipt_recovers_using_frozen_profile_after_instrument_changes() {
    let fixture = Fixture::open(TransmissionClass::B);
    let instrument_id = fixture.instrument.id;
    let profile = fixture.settings.protocol.clone();
    let wire = frame(&profile, b'E').wire.into_inner();
    let receipt = fixture
        .database
        .write_blocking(
            Principal::instrument(instrument_id),
            CorrelationId::generate(),
            move |ctx| {
                begin_receipt(
                    ctx,
                    instrument_id,
                    InstrumentTransport::AuTcp,
                    Some(SocketAddr::from((Ipv4Addr::LOCALHOST, 42000))),
                )
            },
        )
        .expect("receipt begins");
    let receipt_id = receipt.id;
    fixture
        .database
        .write_blocking(
            Principal::instrument(instrument_id),
            CorrelationId::generate(),
            move |ctx| persist_frame(ctx, receipt_id, 0, wire),
        )
        .expect("frame persists");
    let mut changed = fixture.settings.clone();
    changed.protocol.format.sample_id_digits = 9;
    fixture
        .database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id,
                        expected_revision: 1,
                        connection: connection(31234, changed),
                    },
                )
            },
        )
        .expect("current instrument profile changes");
    let frozen = fixture
        .database
        .read_blocking(move |conn| {
            receipt_au_profile(
                conn,
                &Principal::system("synthetic-reader", [Capability::InstrumentIntake]),
                receipt_id,
            )
        })
        .expect("frozen profile reads");
    assert_eq!(frozen, Some(profile));
    reconcile_receipts(&fixture.database).expect("startup reconciles");
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Complete);
    assert_eq!(fixture.imports(), 1);
}

#[test]
fn tcp_supervisor_accepts_fragmented_native_au_frame_without_astm_enq() {
    let fixture = Fixture::open(TransmissionClass::B);
    let port = local_port();
    let instrument_id = fixture.instrument.id;
    let settings = fixture.settings.clone();
    fixture
        .database
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                update_instrument_connection(
                    ctx,
                    UpdateInstrumentConnection {
                        instrument_id,
                        expected_revision: 1,
                        connection: connection(port, settings),
                    },
                )
            },
        )
        .expect("port changes");
    let supervisor = ListenerSupervisor::new(Arc::clone(&fixture.database));
    supervisor.start_all().expect("listener starts");
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).expect("simulator connects");
    stream
        .set_read_timeout(Some(Duration::from_secs(3)))
        .expect("read timeout");
    let wire = frame(&fixture.settings.protocol, b'E').wire.into_inner();
    for chunk in wire.chunks(3) {
        stream.write_all(chunk).expect("fragment sends");
    }
    let mut response = [0];
    stream
        .read_exact(&mut response)
        .expect("delayed ACK receives");
    assert_eq!(response, [6]);
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Complete);
    assert_eq!(fixture.imports(), 1);
    assert_eq!(fixture.frames(&fixture.receipts()[0]), vec![wire]);
    drop(stream);
    supervisor.stop_all().expect("listener stops");
}

#[test]
fn final_frame_ambiguity_keeps_single_block_receipt_evidence_and_holds_multiblock_tail() {
    for multiblock in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let mut now = Instant::now();
        let mut session = AuSession::new(&context, &fixture.settings);
        if multiblock {
            session
                .receive(frame(&fixture.settings.protocol, b'0'), now, &context)
                .expect("first block");
            assert_eq!(due_ack(&mut session, now, &context), vec![6]);
            now += Duration::from_millis(501);
        }
        let final_frame = frame(&fixture.settings.protocol, b'E');
        session
            .receive(final_frame.clone(), now, &context)
            .expect("completed sample");
        assert_eq!(due_ack(&mut session, now, &context), vec![6]);
        now += Duration::from_millis(501);
        session
            .receive(final_frame, now, &context)
            .expect("repeated final");
        assert_eq!(due_ack(&mut session, now, &context), vec![6]);
        let receipts = fixture.receipts();
        assert_eq!(fixture.imports(), 1);
        if multiblock {
            assert_eq!(receipts.len(), 1);
            assert_eq!(receipts[0].frame_count, 2);
        } else {
            assert_eq!(receipts.len(), 2);
            assert_eq!(receipts[1].duplicate_of, Some(receipts[0].id));
            assert_eq!(receipts[1].state, ReceiptState::Complete);
        }
    }
}

fn feed_wire_error(
    session: &mut AuSession,
    config: &AuConfig,
    block: u8,
    now: Instant,
    context: &StreamContext<'_>,
) -> Result<(), ListenerErrorCode> {
    let mut wire = frame(config, block).wire.into_inner();
    *wire.last_mut().expect("BCC exists") ^= 0x80;
    let events = StreamDecoder::new(config.clone())
        .expect("decoder opens")
        .push(&wire);
    assert_eq!(events, vec![Err(lis_au::AuError::Checksum)]);
    super::stream::handle_event(
        session,
        events.into_iter().next().expect("checksum event"),
        now,
        context,
    )
}

#[test]
fn corrupt_middle_and_final_blocks_retry_without_losing_durable_prefix() {
    let fixture = Fixture::open(TransmissionClass::B);
    let context = fixture.context();
    let mut now = Instant::now();
    let mut session = started_partial(&fixture, now);
    let mut expected_wire = vec![frame(&fixture.settings.protocol, b'0').wire.into_inner()];
    for block in *b"1E" {
        now += Duration::from_millis(501);
        feed_wire_error(
            &mut session,
            &fixture.settings.protocol,
            block,
            now,
            &context,
        )
        .expect("wire failure requests retry");
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Receiving);
        assert_eq!(
            fixture.receipts()[0].frame_count as usize,
            expected_wire.len()
        );
        assert_eq!(fixture.imports(), 0);
        assert_eq!(due_ack(&mut session, now, &context), vec![21]);
        now += Duration::from_millis(501);
        let valid = frame(&fixture.settings.protocol, block);
        expected_wire.push(valid.wire.expose().clone());
        super::stream::handle_event(&mut session, Ok(WireEvent::Frame(valid)), now, &context)
            .expect("valid retransmission accepts");
        assert_eq!(due_ack(&mut session, now, &context), vec![6]);
    }
    let receipts = fixture.receipts();
    assert_eq!(receipts.len(), 1);
    assert_eq!(receipts[0].state, ReceiptState::Complete);
    assert_eq!(receipts[0].frame_count, 3);
    assert_eq!(fixture.frames(&receipts[0]), expected_wire);
    assert_eq!(fixture.imports(), 1);
}

#[test]
fn wire_retry_timeout_or_exhaustion_aborts_without_importing_prefix() {
    for exhaustion in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let mut now = Instant::now();
        let mut session = started_partial(&fixture, now);
        for _ in 0..if exhaustion { 4 } else { 1 } {
            now += Duration::from_millis(501);
            feed_wire_error(
                &mut session,
                &fixture.settings.protocol,
                b'E',
                now,
                &context,
            )
            .expect("bounded retry");
            assert_eq!(due_ack(&mut session, now, &context), vec![21]);
        }
        if exhaustion {
            now += Duration::from_millis(501);
            assert_eq!(
                feed_wire_error(
                    &mut session,
                    &fixture.settings.protocol,
                    b'E',
                    now,
                    &context
                ),
                Err(ListenerErrorCode::ProtocolFailed)
            );
        } else {
            now += Duration::from_millis(1500);
            session
                .tick(now, &mut Vec::new(), &context)
                .expect("retry deadline expires");
        }
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Aborted);
        assert_eq!(
            fixture.receipts()[0].abort_reason,
            Some(if exhaustion {
                AbortCode::RetryLimitExceeded
            } else {
                AbortCode::ReceiveTimeout
            })
        );
        assert_eq!(fixture.receipts()[0].frame_count, 1);
        assert_eq!(fixture.imports(), 0);
    }
}

#[test]
fn class_a_wire_error_aborts_without_a_retry_response() {
    let fixture = Fixture::open(TransmissionClass::A);
    let context = fixture.context();
    let now = Instant::now();
    let mut session = AuSession::new(&context, &fixture.settings);
    session
        .receive(frame(&fixture.settings.protocol, b'0'), now, &context)
        .expect("first block persists");
    feed_wire_error(
        &mut session,
        &fixture.settings.protocol,
        b'E',
        now + Duration::from_millis(501),
        &context,
    )
    .expect("class A rejects");
    let mut response = Vec::new();
    session
        .tick(now + Duration::from_secs(2), &mut response, &context)
        .expect("tick");
    assert!(response.is_empty());
    assert_eq!(fixture.receipts()[0].state, ReceiptState::Aborted);
    assert_eq!(fixture.imports(), 0);
}

#[test]
fn persistent_partial_guard_refuses_tail_after_reconnect_or_startup_reconciliation() {
    for startup in [false, true] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let now = Instant::now();
        let mut original = started_partial(&fixture, now);
        if startup {
            drop(original);
            reconcile_receipts(&fixture.database).expect("startup reconciles incomplete prefix");
        } else {
            original
                .abort(&context, AbortCode::ConnectionLost)
                .expect("connection aborts");
        }
        let mut reconnected = AuSession::new(&context, &fixture.settings);
        let retry_time = now + Duration::from_millis(501);
        reconnected
            .receive(
                frame(&fixture.settings.protocol, b'E'),
                retry_time,
                &context,
            )
            .expect("late final is rejected");
        assert_eq!(due_ack(&mut reconnected, retry_time, &context), vec![21]);
        assert_eq!(fixture.receipts().len(), 1);
        assert_eq!(fixture.imports(), 0);
        let restart_time = retry_time + Duration::from_millis(501);
        reconnected
            .receive(
                frame(&fixture.settings.protocol, b'0'),
                restart_time,
                &context,
            )
            .expect("explicit zero restarts");
        assert_eq!(due_ack(&mut reconnected, restart_time, &context), vec![6]);
        let finish_time = restart_time + Duration::from_millis(501);
        reconnected
            .receive(
                frame(&fixture.settings.protocol, b'E'),
                finish_time,
                &context,
            )
            .expect("restarted full sample completes");
        assert_eq!(due_ack(&mut reconnected, finish_time, &context), vec![6]);
        assert_eq!(fixture.receipts().len(), 2);
        assert_eq!(fixture.receipts()[1].state, ReceiptState::Complete);
        assert_eq!(fixture.receipts()[1].frame_count, 2);
        assert_eq!(fixture.imports(), 1);
    }
}

#[test]
fn persistent_completed_multiblock_guard_refuses_lost_ack_tail_on_new_session() {
    let fixture = Fixture::open(TransmissionClass::B);
    let context = fixture.context();
    let now = Instant::now();
    let mut original = started_partial(&fixture, now);
    let finish_time = now + Duration::from_millis(501);
    original
        .receive(
            frame(&fixture.settings.protocol, b'E'),
            finish_time,
            &context,
        )
        .expect("sample completes before final ACK");
    // The transport disappears before the queued ACK is sent.
    original
        .abort(&context, AbortCode::ConnectionLost)
        .expect("connection ends");
    let mut reconnected = AuSession::new(&context, &fixture.settings);
    let retry_time = finish_time + Duration::from_millis(501);
    reconnected
        .receive(
            frame(&fixture.settings.protocol, b'E'),
            retry_time,
            &context,
        )
        .expect("ambiguous tail held");
    assert_eq!(due_ack(&mut reconnected, retry_time, &context), vec![21]);
    assert_eq!(fixture.receipts().len(), 1);
    assert_eq!(fixture.receipts()[0].frame_count, 2);
    assert_eq!(fixture.imports(), 1);
}

#[test]
fn framing_and_size_errors_request_retry_without_resetting_sample() {
    for error in [lis_au::AuError::Framing, lis_au::AuError::FrameTooLong] {
        let fixture = Fixture::open(TransmissionClass::B);
        let context = fixture.context();
        let now = Instant::now();
        let mut session = started_partial(&fixture, now);
        let failure_time = now + Duration::from_millis(501);
        super::stream::handle_event(&mut session, Err(error), failure_time, &context)
            .expect("wire retry schedules");
        assert_eq!(due_ack(&mut session, failure_time, &context), vec![21]);
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Receiving);
        assert_eq!(fixture.receipts()[0].frame_count, 1);
        let retry_time = failure_time + Duration::from_millis(501);
        session
            .receive(
                frame(&fixture.settings.protocol, b'E'),
                retry_time,
                &context,
            )
            .expect("valid final retries");
        assert_eq!(due_ack(&mut session, retry_time, &context), vec![6]);
        assert_eq!(fixture.receipts()[0].state, ReceiptState::Complete);
        assert_eq!(fixture.receipts()[0].frame_count, 2);
        assert_eq!(fixture.imports(), 1);
    }
}
