//! Transaction-bound result-import regressions on production migrations.
//! Synthetic configuration only.

use std::net::{IpAddr, Ipv4Addr};

use lis_analytical::{
    ApproveSourceInput, RegisterSourceInput, RevisionIndicator, SampleIdField, SourceTestCodeInput,
};
use lis_catalog::CreateAnalyte;
use lis_core::{Capability, CorrelationId};
use lis_interfaces::{ConnectionSettings, RegisterInstrument};
use lis_results::astm::ExtractFailure;
use lis_results::{ImportReceiptInput, ImportSource, ResultError, import_receipt};
use lis_store::testing::{TestDatabase, principal};

#[test]
fn import_refuses_a_registration_different_from_the_parsing_profile()
-> Result<(), Box<dyn std::error::Error>> {
    let t = TestDatabase::open(&[
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
        lis_results::SCHEMA,
    ])?;
    let (instrument, first) = rotate_registration(&t)?;
    let intake = principal(&[Capability::InstrumentIntake]);
    let refused =
        t.db.write_blocking(intake, CorrelationId::generate(), move |ctx| {
            import_receipt(
                ctx,
                ImportReceiptInput {
                    receipt_id: lis_interfaces::ReceiptId::generate(),
                    attempt_no: 1,
                    instrument_id: instrument,
                    registration: Some(first),
                    source: ImportSource::Malformed(ExtractFailure::NotAResultMessage),
                },
            )
        });
    assert!(matches!(refused, Err(ResultError::RevisionConflict)));
    let attempts: i64 = t.db.read_blocking(|conn| {
        conn.query_row("SELECT count(*) FROM res_import_attempts", [], |row| {
            row.get(0)
        })
        .map_err(ResultError::from)
    })?;
    assert_eq!(attempts, 0);
    Ok(())
}

fn rotate_registration(
    t: &TestDatabase,
) -> Result<
    (
        lis_core::InstrumentId,
        lis_analytical::SourceRegistrationView,
    ),
    Box<dyn std::error::Error>,
> {
    let author = principal(&[Capability::ManageConfiguration]);
    let approver = principal(&[Capability::ApproveConfiguration]);
    let instrument =
        t.db.write_blocking(author.clone(), CorrelationId::generate(), |ctx| {
            lis_interfaces::register_instrument(
                ctx,
                RegisterInstrument {
                    code: "SYN-REG-RACE".to_owned(),
                    name: "Synthetic registration race analyzer".to_owned(),
                    connection: ConnectionSettings::AstmTcp {
                        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
                        port: 62001,
                        allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
                    },
                    enabled: false,
                },
            )
        })?;
    let analyte =
        t.db.write_blocking(author.clone(), CorrelationId::generate(), |ctx| {
            lis_catalog::create_analyte(
                ctx,
                CreateAnalyte {
                    code: "SYN-REG-A".to_owned(),
                    name: "Synthetic registration analyte".to_owned(),
                    loinc_code: None,
                    drug_class_code: None,
                },
            )
        })?;
    let first = register(
        t,
        author.clone(),
        instrument.id,
        analyte,
        SampleIdField::SpecimenId,
    )?;
    let first =
        t.db.write_blocking(approver.clone(), CorrelationId::generate(), move |ctx| {
            lis_analytical::approve_source(
                ctx,
                ApproveSourceInput {
                    registration_id: first.registration_id,
                    expected_revision: first.revision,
                    self_approval: false,
                },
            )
        })?;
    let second = register(
        t,
        author,
        instrument.id,
        analyte,
        SampleIdField::InstrumentSpecimenId,
    )?;
    t.db.write_blocking(approver, CorrelationId::generate(), move |ctx| {
        lis_analytical::approve_source(
            ctx,
            ApproveSourceInput {
                registration_id: second.registration_id,
                expected_revision: second.revision,
                self_approval: false,
            },
        )
    })?;
    Ok((instrument.id, first))
}

fn register(
    t: &TestDatabase,
    author: lis_core::Principal,
    instrument_id: lis_core::InstrumentId,
    analyte_id: lis_catalog::AnalyteId,
    sample_id_field: SampleIdField,
) -> Result<lis_analytical::SourceRegistrationView, lis_analytical::AnalyticalError> {
    t.db.write_blocking(author, CorrelationId::generate(), move |ctx| {
        lis_analytical::register_source(
            ctx,
            RegisterSourceInput {
                au_profile: None,
                instrument_id,
                sender_name: "SYNTH-REG".to_owned(),
                sender_version: "1.0".to_owned(),
                sample_id_field,
                revision_indicator: RevisionIndicator::None,
                reports_final_concentration: false,
                test_codes: vec![SourceTestCodeInput {
                    source_test_code: "SYN-A".to_owned(),
                    analyte_id,
                }],
            },
        )
    })
}
