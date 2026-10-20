//! AU socket → durable receipt → outbox → authorized source → specimen result.
//! Synthetic values and setup only; no hardware/assay qualification is implied.

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::time::Duration;

use lis_analytical::{
    AuSourceProfile, AuTestDefinition, RevisionIndicator, SampleIdField, SourceTestCodeInput,
};
use lis_au::{AuConfig, FormatConfig, FramingConfig, TransmissionClass};
use lis_engine::{ApproveSourceInput, RegisterSourceInput};
use lis_interfaces::{
    AuConnectionSettings, ConnectionSettings, ReceiptId, UpdateInstrumentConnection,
};
use lis_results::ImportOutcome;

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::line;

fn protocol() -> AuConfig {
    AuConfig {
        framing: FramingConfig {
            start: vec![2],
            end: vec![3],
            bcc: true,
            use_etb: false,
            max_wire_len: 256,
            class: TransmissionClass::B,
        },
        format: FormatConfig {
            unit_no: false,
            rack_digits: 0,
            sample_id_digits: 9,
            sex: false,
            age: false,
            month: false,
            patient_info_digits: [0; 6],
            result_digits: 6,
        },
    }
}

async fn configure(lab: &Lab) {
    lab.sign_in(Role::Admin).await;
    let instrument = lab
        .engine()
        .list_instruments()
        .await
        .unwrap()
        .into_iter()
        .find(|instrument| instrument.id == lab.instrument)
        .unwrap();
    let changed = lab
        .engine()
        .update_instrument_connection(UpdateInstrumentConnection {
            instrument_id: lab.instrument,
            expected_revision: instrument.revision,
            connection: ConnectionSettings::AuTcp {
                bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                port: lab.port,
                allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
                au: AuConnectionSettings {
                    protocol: protocol(),
                    frame_timeout_ms: 2000,
                    block_timeout_ms: 3000,
                },
            },
        })
        .await
        .unwrap();
    assert!(changed.listeners_restarted);
    let source = lab
        .engine()
        .register_source(RegisterSourceInput {
            instrument_id: lab.instrument,
            sender_name: "SYN-AU".to_owned(),
            sender_version: "SYN-1".to_owned(),
            sample_id_field: SampleIdField::SpecimenId,
            revision_indicator: RevisionIndicator::None,
            reports_final_concentration: false,
            test_codes: vec![SourceTestCodeInput {
                source_test_code: "01".to_owned(),
                analyte_id: lab.analytes.mor,
            }],
            au_profile: Some(AuSourceProfile {
                protocol: protocol(),
                tests: vec![AuTestDefinition {
                    test_code: "01".to_owned(),
                    unit: Some("ng/mL".to_owned()),
                }],
            }),
        })
        .await
        .unwrap();
    lab.sign_in(Role::Quality).await;
    lab.engine()
        .approve_source(ApproveSourceInput {
            registration_id: source.registration_id,
            expected_revision: source.revision,
            self_approval: false,
        })
        .await
        .unwrap();
}

#[tokio::test]
async fn au_tcp_reaches_specimen_result_and_replay_keeps_one_import() {
    let lab = Lab::start(RevisionIndicator::None).await;
    configure(&lab).await;
    // D-space; typeU, sequence0001, barcode9, reserved4, final E;
    // test01, exact six-byte value, two blank flags. Independent wire fixture.
    let text = format!("D U0001{}    E01  75.0  ", lab.samples[0].label_code);
    let mut wire = vec![2];
    wire.extend_from_slice(text.as_bytes());
    wire.push(3);
    wire.push(wire[1..].iter().fold(0_u8, |bcc, byte| bcc ^ byte));
    assert_eq!(
        lis_results::au::extract_artifact(&wire, &protocol())
            .unwrap()
            .rows
            .len(),
        1
    );
    let port = lab.port;
    tokio::task::spawn_blocking(move || {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        for _ in 0..2 {
            stream.write_all(&wire).unwrap();
            let mut response = [0];
            stream.read_exact(&mut response).unwrap();
            assert_eq!(response, [6]);
        }
    })
    .await
    .unwrap();
    let receipts: Vec<(ReceiptId, Option<ReceiptId>)> = lab
        .t()
        .conn()
        .prepare("SELECT id, duplicate_of FROM ifc_receipts WHERE transport = 'au_tcp' ORDER BY id")
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(receipts.len(), 2);
    assert_eq!(receipts[1].1, Some(receipts[0].0));
    let attempt = lab.attempt(receipts[0].0, 1).await;
    let detail = lab
        .engine()
        .get_import_attempt(lis_engine::GetImportAttemptInput {
            import_attempt_id: attempt.import_attempt_id,
        })
        .await
        .unwrap();
    assert_eq!(attempt.outcome, ImportOutcome::Processed, "{detail:?}");
    assert_eq!(attempt.rows_new, 1);
    let results = lab.results(0).await;
    let result = line(&results, "SYN-MOR").current.as_ref().unwrap();
    assert_eq!(result.units.as_deref(), Some("ng/mL"));
    assert_eq!(result.magnitude.as_deref(), Some("75.0"));
    assert_eq!(lab.attempts().await.len(), 1);
    let raw: (String, Option<String>) = lab
        .t()
        .conn()
        .query_row(
            "SELECT value_text, units_text FROM res_measurements",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(raw, ("  75.0".to_owned(), None));
}
