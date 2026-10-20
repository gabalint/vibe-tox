//! Synthetic AU receipt imports on the production SQLite schemas. The receipt
//! seed represents already-committed listener evidence; transport is tested by
//! lis-interfaces, while these tests exercise semantic authorization and replay.

#![cfg(test)]

use std::net::{IpAddr, Ipv4Addr};

use lis_analytical::{
    ApproveSourceInput, AuSourceProfile, AuTestDefinition, RegisterSourceInput, RevisionIndicator,
    SampleIdField, SourceTestCodeInput,
};
use lis_au::{AuConfig, FormatConfig, FrameEnd, FramingConfig, TransmissionClass, encode_frame};
use lis_catalog::CreateAnalyte;
use lis_core::{Capability, CorrelationId, InstrumentId};
use lis_interfaces::{AuConnectionSettings, ConnectionSettings, ReceiptId, RegisterInstrument};
use lis_results::{ImportFailureCode, ImportReceiptInput, ImportSource, ResultError};
use lis_store::testing::{TestDatabase, principal};

#[path = "support/au_receipt.rs"]
mod receipt;

struct Fixture {
    database: TestDatabase,
    instrument: InstrumentId,
    registration: lis_analytical::SourceRegistrationView,
    profile: AuConfig,
    receipt: ReceiptId,
    bytes: Vec<u8>,
}

fn profile() -> AuConfig {
    AuConfig {
        framing: FramingConfig {
            start: vec![2],
            end: vec![3],
            bcc: true,
            use_etb: true,
            max_wire_len: 256,
            class: TransmissionClass::B,
        },
        format: FormatConfig {
            unit_no: false,
            rack_digits: 4,
            sample_id_digits: 12,
            sex: false,
            age: false,
            month: false,
            patient_info_digits: [0; 6],
            result_digits: 6,
        },
    }
}

fn fixture() -> Fixture {
    let database = TestDatabase::open(&[
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
        lis_results::SCHEMA,
    ])
    .unwrap();
    let profile = profile();
    let admin = principal(&[Capability::ManageConfiguration]);
    let protocol = profile.clone();
    let instrument = database
        .db
        .write_blocking(admin.clone(), CorrelationId::generate(), move |ctx| {
            lis_interfaces::register_instrument(
                ctx,
                RegisterInstrument {
                    code: "SYN-AU".to_owned(),
                    name: "Synthetic AU analyzer".to_owned(),
                    enabled: true,
                    connection: ConnectionSettings::AuTcp {
                        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                        port: 62002,
                        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
                        au: AuConnectionSettings {
                            protocol,
                            frame_timeout_ms: 2000,
                            block_timeout_ms: 2000,
                        },
                    },
                },
            )
        })
        .unwrap()
        .id;
    let analyte = database
        .db
        .write_blocking(admin, CorrelationId::generate(), |ctx| {
            lis_catalog::create_analyte(
                ctx,
                CreateAnalyte {
                    code: "SYN-AU-A".to_owned(),
                    name: "Synthetic AU analyte".to_owned(),
                    loinc_code: None,
                    drug_class_code: None,
                },
            )
        })
        .unwrap();
    let registration = register(&database, instrument, analyte, profile.clone());
    let bytes = encode_frame(
        &profile,
        b"D 000101U0001SYN-UNPLACED    E01 12.50  ",
        FrameEnd::Text,
    )
    .unwrap();
    let receipt = receipt::seed(&database, instrument, &profile, &bytes);
    Fixture {
        database,
        instrument,
        registration,
        profile,
        receipt,
        bytes,
    }
}

fn register(
    database: &TestDatabase,
    instrument: InstrumentId,
    analyte: lis_catalog::AnalyteId,
    protocol: AuConfig,
) -> lis_analytical::SourceRegistrationView {
    let draft = database
        .db
        .write_blocking(
            principal(&[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                lis_analytical::register_source(
                    ctx,
                    RegisterSourceInput {
                        instrument_id: instrument,
                        sender_name: "Synthetic declared AU software".to_owned(),
                        sender_version: "1.0".to_owned(),
                        sample_id_field: SampleIdField::SpecimenId,
                        revision_indicator: RevisionIndicator::None,
                        reports_final_concentration: true,
                        test_codes: vec![SourceTestCodeInput {
                            source_test_code: "01".to_owned(),
                            analyte_id: analyte,
                        }],
                        au_profile: Some(AuSourceProfile {
                            protocol,
                            tests: vec![AuTestDefinition {
                                test_code: "01".to_owned(),
                                unit: Some("ng/mL".to_owned()),
                            }],
                        }),
                    },
                )
            },
        )
        .unwrap();
    database
        .db
        .write_blocking(
            principal(&[Capability::ApproveConfiguration]),
            CorrelationId::generate(),
            move |ctx| {
                lis_analytical::approve_source(
                    ctx,
                    ApproveSourceInput {
                        registration_id: draft.registration_id,
                        expected_revision: draft.revision,
                        self_approval: false,
                    },
                )
            },
        )
        .unwrap()
}

impl Fixture {
    fn input(&self, profile: AuConfig) -> ImportReceiptInput {
        ImportReceiptInput {
            receipt_id: self.receipt,
            attempt_no: 1,
            instrument_id: self.instrument,
            registration: Some(self.registration.clone()),
            source: ImportSource::Au {
                extracted: lis_results::au::extract_artifact(&self.bytes, &profile),
                profile,
            },
        }
    }
    fn counts(&self) -> (i64, i64, i64) {
        self.database
            .db
            .read_blocking(|conn| {
                Ok::<_, ResultError>(conn.query_row(
                    "SELECT (SELECT count(*) FROM res_import_attempts),
                (SELECT count(*) FROM res_quarantine_items),
                (SELECT count(*) FROM audit_events WHERE action = 'result.import')",
                    [],
                    |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
                )?)
            })
            .unwrap()
    }
}

#[test]
fn au_unknown_sample_is_quarantined_once_on_replay_with_exact_evidence() {
    let f = fixture();
    let import = || {
        let input = f.input(f.profile.clone());
        f.database
            .db
            .write_blocking(
                principal(&[Capability::InstrumentIntake]),
                CorrelationId::generate(),
                move |ctx| lis_results::import_receipt(ctx, input),
            )
            .unwrap()
    };
    let first = import();
    assert_eq!(first.rows_quarantined, 1);
    assert_eq!(import().import_attempt_id, first.import_attempt_id);
    assert_eq!(f.counts(), (1, 1, 1));
    f.database.db.read_blocking(|conn| {
        let row: (String, String, Option<String>, String, Option<String>) = conn.query_row(
            "SELECT value_text, flags_text, units_text, reason, status_text FROM res_quarantine_items", [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, row.get(4)?)))?;
        assert_eq!(row, (" 12.50".to_owned(), "  ".to_owned(), None, "unknown_sample".to_owned(), None));
        let parser: String = conn.query_row("SELECT parser_version FROM res_import_attempts", [], |row| row.get(0))?;
        assert_eq!(parser, "au-result-v1");
        Ok::<_, ResultError>(())
    }).unwrap();
}

#[test]
fn au_profile_mismatch_records_whole_attempt_failure_without_measurements() {
    let f = fixture();
    let mut protocol = f.profile.clone();
    protocol.framing.class = TransmissionClass::A;
    let input = f.input(protocol);
    let outcome = f
        .database
        .db
        .write_blocking(
            principal(&[Capability::InstrumentIntake]),
            CorrelationId::generate(),
            move |ctx| lis_results::import_receipt(ctx, input),
        )
        .unwrap();
    assert_eq!(
        outcome.failure_code,
        Some(ImportFailureCode::LayoutMismatch)
    );
    assert_eq!(f.counts(), (1, 0, 1));
}

#[test]
fn au_import_denies_missing_capability_without_semantic_writes() {
    let f = fixture();
    let input = f.input(f.profile.clone());
    let denied =
        f.database
            .db
            .write_blocking(principal(&[]), CorrelationId::generate(), move |ctx| {
                lis_results::import_receipt(ctx, input)
            });
    assert!(denied.is_err());
    assert_eq!(f.counts(), (0, 0, 0));
}
