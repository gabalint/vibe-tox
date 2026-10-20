//! Whole-file outcomes (result files brief §5): nothing in the file is
//! imported, the attempt records why, and the bytes stay in the receipt.

use lis_interfaces::{ConnectionSettings, RegisterInstrument};
use lis_results::{ImportFailureCode, ImportOutcome, ImportReceiptInput, ImportSource};

use crate::rows::row;
use crate::*;

/// Rows written by an attempt: measurements, quarantine and set-aside rows.
fn written(f: &Fixture) -> i64 {
    f.read(|conn| {
        Ok(conn.query_row(
            "SELECT (SELECT count(*) FROM res_measurements) \
             + (SELECT count(*) FROM res_quarantine_items) \
             + (SELECT count(*) FROM res_set_aside_rows)",
            [],
            |row| row.get(0),
        )?)
    })
}

#[test]
fn a_file_that_cannot_be_read_fails_as_a_whole_with_its_code() {
    let f = fixture();
    let s = f.sample();
    let mut too_wide = csv(&[]);
    too_wide.extend_from_slice(format!("{}\n", vec!["x"; 513].join(",")).as_bytes());
    let renamed = String::from_utf8(csv(&[row(&s.label, "Unknown", "Morphine", "1", UNIT, "1")]))
        .unwrap()
        .replace("Final Conc", "Final Concentration");
    let cases: [(Vec<u8>, ImportFailureCode); 3] = [
        (
            b"\xff\xfeS\x00a\x00".to_vec(),
            ImportFailureCode::UndecodableText,
        ),
        (renamed.into_bytes(), ImportFailureCode::LayoutMismatch),
        (too_wide, ImportFailureCode::LimitsExceeded),
    ];
    let registration = f.registration().map(|item| item.registration_id);
    for (bytes, code) in cases {
        let attempt = f.import_bytes(&bytes);
        assert_eq!(attempt.failure_code, Some(code), "{code:?}");
        assert_eq!(
            (attempt.outcome, attempt.rows_total),
            (ImportOutcome::Quarantined, 0)
        );
        let detail = f.detail(&attempt);
        assert_eq!(detail.registration_id, registration);
        assert!(detail.file.is_some());
    }
    assert_eq!(written(&f), 0);
    let failures: Vec<(String, String)> = f.read(|conn| {
        Ok(conn
            .prepare("SELECT parser_version, failure_code FROM res_import_attempts ORDER BY rowid")?
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<_, _>>()?)
    });
    assert_eq!(
        failures,
        [
            ("delimited-v1", "undecodable_text"),
            ("delimited-v1", "layout_mismatch"),
            ("delimited-v1", "limits_exceeded"),
        ]
        .map(|(parser, code)| (parser.to_owned(), code.to_owned()))
    );
}

// Brief §5: no active file registration for the instrument is
// `unregistered_source`; the attempt binds no registration.
#[test]
fn a_file_from_an_instrument_without_a_file_template_is_unregistered() {
    let f = fixture();
    let other = f
        .setup(|ctx| {
            lis_interfaces::register_instrument(
                ctx,
                RegisterInstrument {
                    code: "syn-lcms-2".to_owned(),
                    name: "Synthetic LC-MS/MS 2".to_owned(),
                    connection: ConnectionSettings::ResultFile {},
                    enabled: true,
                },
            )
        })
        .id;
    let digest =
        f.t.db
            .artifacts()
            .put(b"anything")
            .unwrap()
            .digest
            .to_string();
    let input = lis_interfaces::ReceiveResultFileInput {
        file: lis_interfaces::FileFacts {
            instrument_id: other,
            source: lis_interfaces::FileSource::Upload,
            file_name: Phi::new("other.csv".to_owned()),
            byte_len: 8,
            modified_at: test_start(),
        },
        sha256: digest.clone(),
        artifact_digest: digest,
    };
    let receipt_id = f
        .setup(|ctx| lis_interfaces::receive_result_file(ctx, input))
        .id;
    let intake = || Principal::system("result-import", [Capability::InstrumentIntake]);
    let import = move || ImportReceiptInput {
        receipt_id,
        attempt_no: 1,
        instrument_id: other,
        registration: None,
        source: ImportSource::UnregisteredFile,
    };
    let attempt = f
        .run_as(intake(), move |ctx| {
            lis_results::import_receipt(ctx, import())
        })
        .unwrap();
    assert_eq!(
        attempt.failure_code,
        Some(ImportFailureCode::UnregisteredSource)
    );
    assert_eq!(f.detail(&attempt).registration_id, None);
    // NFR-02: the same attempt again is the same attempt, with no new event.
    let events = |f: &Fixture| -> i64 {
        f.read(|conn| {
            Ok(conn.query_row(
                "SELECT count(*) FROM audit_events WHERE action = 'result.import'",
                [],
                |row| row.get(0),
            )?)
        })
    };
    let before = events(&f);
    let again = f
        .run_as(intake(), move |ctx| {
            lis_results::import_receipt(ctx, import())
        })
        .unwrap();
    assert_eq!(again, attempt);
    assert_eq!(events(&f), before);
}
