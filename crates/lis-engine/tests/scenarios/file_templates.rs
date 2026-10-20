//! The file template page through the engine (result files brief §7, §13;
//! REQ-ANA-003, REQ-INT-005): a sample file is read, previewed and checked
//! for its session only, by configuration authors or approvers, without
//! writing anything; the file source is registered as a draft and approved
//! by another person. Synthetic data only.

use std::path::PathBuf;

use lis_analytical::{
    ApproveSourceInput, EndOfData, FileColumns, FileDelimiter, FileEncoding, FileShape,
    RegistrationState, SampleIdField, SourceTestCodeInput,
};
use lis_auth::{RoleCode, UnlockSessionInput};
use lis_core::{CommandError, ErrorCode, InstrumentId, UserId};
use lis_engine::{
    CheckFileLayoutInput, DialogCommand, Engine, FileLayout, LayoutSampleHandle,
    PreviewLayoutSampleInput, RegisterFileSourceInput,
};
use lis_interfaces::{ConnectionSettings, MAX_RESULT_FILE_BYTES, RegisterInstrument};
use tempfile::TempDir;

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of, secret};

const SCIEX: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/files/sciex_long.csv"
));
const UTF16: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../fixtures/files/utf16le_bom.txt"
));
const SCIEX_HEADER: [&str; 8] = [
    "Sample Name",
    "Sample ID",
    "Sample Type",
    "Component Name",
    "Calculated Concentration",
    "Conc. Units",
    "Dilution Factor",
    "Ion Ratio",
];

/// Calls the file template command `name` with plausible input; `None` for other areas.
pub(crate) async fn file_templates_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let sample_handle = LayoutSampleHandle::generate();
    Some(match name {
        "open_layout_sample" => engine.open_layout_sample(None).await.map(drop),
        "preview_layout_sample" => engine
            .preview_layout_sample(preview(sample_handle))
            .await
            .map(drop),
        "check_file_layout" => engine
            .check_file_layout(check(sample_handle, layout()))
            .await
            .map(drop),
        "register_file_source" => engine
            .register_file_source(registration(InstrumentId::generate(), Vec::new()))
            .await
            .map(drop),
        _ => return None,
    })
}

fn layout() -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![SCIEX_HEADER.map(str::to_owned).to_vec()],
        shape: FileShape::Long {
            analyte_column: 3,
            value_column: 4,
        },
        columns: FileColumns {
            sample_label: 0,
            units: Some(5),
            sample_type: Some(2),
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: Some(6),
        },
        end_of_data: EndOfData::EndOfFile,
        sample_types: Vec::new(),
        value_words: Vec::new(),
        not_results: vec!["Morphine-D3".to_owned()],
        declared_unit: None,
    }
}

fn preview(sample_handle: LayoutSampleHandle) -> PreviewLayoutSampleInput {
    PreviewLayoutSampleInput {
        sample_handle,
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
    }
}

fn check(sample_handle: LayoutSampleHandle, layout: FileLayout) -> CheckFileLayoutInput {
    CheckFileLayoutInput {
        sample_handle,
        layout,
        test_codes: Vec::new(),
    }
}

fn registration(
    instrument_id: InstrumentId,
    test_codes: Vec<SourceTestCodeInput>,
) -> RegisterFileSourceInput {
    RegisterFileSourceInput {
        instrument_id,
        software_name: "SynthQuant".to_owned(),
        software_version: "3.4".to_owned(),
        reports_final_concentration: false,
        layout: layout(),
        test_codes,
    }
}

/// `bytes` written to a file in `dir`, as the shell's dialog would return it.
fn sample_file(dir: &TempDir, name: &str, bytes: &[u8]) -> Option<PathBuf> {
    let path = dir.path().join(name);
    std::fs::write(&path, bytes).unwrap();
    Some(path)
}

/// The administrator creates `username` with `roles`, who then signs in for
/// the first time; returns their TOTP secret.
async fn user(t: &TestEngine, admin_secret: &str, username: &str, roles: &[RoleCode]) -> String {
    t.sign_in_with(ADMIN, PASSWORD, admin_secret).await;
    t.create_user(username, roles).await;
    t.first_sign_in(username).await.1
}

/// `(field, code)` of the first field error, as the UI receives it.
fn field_error(error: &CommandError) -> (String, String) {
    let json = serde_json::to_value(error).unwrap();
    let first = &json["fieldErrors"][0];
    (
        first["field"].as_str().unwrap().to_owned(),
        first["code"].as_str().unwrap().to_owned(),
    )
}

// Brief §7: the approver runs the same check, so `manage_configuration` or
// `approve_configuration` may open, preview and check; anyone else is refused,
// before any dialog opens, with only the denial written.
#[tokio::test]
async fn template_reads_need_manage_or_approve_configuration() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    let approver = user(&t, &admin, "cfg2", &[RoleCode::Administrator]).await;
    let quality = user(&t, &admin, "qm1", &[RoleCode::QualityManager]).await;
    user(&t, &admin, "analyst1", &[RoleCode::Analyst]).await;
    for name in [
        "open_layout_sample",
        "preview_layout_sample",
        "check_file_layout",
    ] {
        let before = t.fingerprint();
        let refused = file_templates_call(&t.engine, name, UserId::generate()).await;
        assert_eq!(code_of(refused.unwrap()), ErrorCode::Forbidden, "{name}");
        let after = t.fingerprint();
        let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
        assert_eq!(added.len(), 1, "{name}: {added:?}");
        let (action, outcome) = t.events("security").pop().unwrap();
        assert_eq!(
            (action, outcome),
            (format!("analytical.{name}"), "denied".to_owned())
        );
    }
    assert_eq!(
        code_of(
            t.engine
                .preflight_dialog(DialogCommand::OpenLayoutSample)
                .await
        ),
        ErrorCode::Forbidden
    );
    t.sign_in_with("qm1", NEW_PASSWORD, &quality).await;
    assert_eq!(
        code_of(
            t.engine
                .preflight_dialog(DialogCommand::OpenLayoutSample)
                .await
        ),
        ErrorCode::Forbidden
    );
    for (username, password, secret) in [
        (ADMIN, PASSWORD, admin.as_str()),
        ("cfg2", NEW_PASSWORD, approver.as_str()),
    ] {
        t.sign_in_with(username, password, secret).await;
        t.engine
            .preflight_dialog(DialogCommand::OpenLayoutSample)
            .await
            .unwrap();
        assert_eq!(t.engine.open_layout_sample(None).await.unwrap(), None);
        let unknown = t
            .engine
            .preview_layout_sample(preview(LayoutSampleHandle::generate()))
            .await;
        assert_eq!(code_of(unknown), ErrorCode::NotFound, "{username}");
    }
}

#[tokio::test]
async fn a_sample_is_read_previewed_and_checked_without_writing_anything() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let dir = tempfile::tempdir().unwrap();
    let before = t.fingerprint();
    let opened = t
        .engine
        .open_layout_sample(sample_file(&dir, "batch.csv", SCIEX))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(opened.bom_encoding, None);
    assert!(opened.valid_utf8);
    assert_eq!(opened.lines.len(), 9);
    assert_eq!(opened.lines[0], SCIEX_HEADER.join(","));
    let lines = t
        .engine
        .preview_layout_sample(preview(opened.sample_handle))
        .await
        .unwrap();
    assert_eq!(lines[0].line, 1);
    assert_eq!(lines[0].cells, SCIEX_HEADER);
    let checked = t
        .engine
        .check_file_layout(check(opened.sample_handle, layout()))
        .await
        .unwrap();
    assert_eq!(checked.failure, None);
    assert_eq!((checked.counts.rows, checked.counts.not_results), (8, 1));
    // No LIS record is read, so no access event or anything else is written.
    assert_eq!(t.fingerprint(), before);

    let mut wrong = layout();
    wrong.columns.units = Some(9);
    let invalid = t
        .engine
        .check_file_layout(check(opened.sample_handle, wrong))
        .await
        .unwrap_err();
    assert_eq!(invalid.code(), ErrorCode::Validation);
    assert_eq!(field_error(&invalid).0, "layout.columns.units");
    let mut renamed = layout();
    renamed.header[0][4] = "Final Concentration".to_owned();
    let mismatch = t
        .engine
        .check_file_layout(check(opened.sample_handle, renamed))
        .await
        .unwrap();
    assert_eq!(
        mismatch.failure,
        Some(lis_results::delimited::FileExtractFailure::LayoutMismatch)
    );
}

#[tokio::test]
async fn unreadable_or_oversized_samples_are_refused_and_not_kept() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let dir = tempfile::tempdir().unwrap();
    let utf16 = t
        .engine
        .open_layout_sample(sample_file(&dir, "unicode.txt", UTF16))
        .await
        .unwrap_err();
    assert_eq!(utf16.code(), ErrorCode::Validation);
    assert_eq!(field_error(&utf16).1, "undecodable_text");
    let size = usize::try_from(MAX_RESULT_FILE_BYTES).unwrap();
    let large = t
        .engine
        .open_layout_sample(sample_file(&dir, "large.csv", &vec![b'a'; size + 1]))
        .await;
    assert_eq!(code_of(large), ErrorCode::Validation);
    let missing = t
        .engine
        .open_layout_sample(Some(dir.path().join("missing.csv")))
        .await;
    assert_eq!(code_of(missing), ErrorCode::Validation);
    let opened = t
        .engine
        .open_layout_sample(sample_file(&dir, "latin.csv", b"Units\r\n\xb5g/L\r\n"))
        .await
        .unwrap()
        .unwrap();
    assert!(!opened.valid_utf8);
    let strict = t
        .engine
        .preview_layout_sample(preview(opened.sample_handle))
        .await
        .unwrap_err();
    assert_eq!(field_error(&strict).1, "undecodable_text");
    let mut latin = preview(opened.sample_handle);
    latin.encoding = FileEncoding::Windows1252;
    let lines = t.engine.preview_layout_sample(latin).await.unwrap();
    assert_eq!(lines[1].cells, ["\u{b5}g/L"]);
}

// Brief §7: a sample stays in memory for its session only, two at most, and
// is dropped when the session locks or ends.
#[tokio::test]
async fn samples_are_dropped_at_lock_and_sign_out_and_kept_two_at_a_time() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    let dir = tempfile::tempdir().unwrap();
    let engine = &t.engine;
    let open = |name: &'static str| {
        let file = sample_file(&dir, name, SCIEX);
        async move {
            engine
                .open_layout_sample(file)
                .await
                .unwrap()
                .unwrap()
                .sample_handle
        }
    };
    let kept = |handle| async move { engine.preview_layout_sample(preview(handle)).await.is_ok() };
    let (first, second, third) = (
        open("a.csv").await,
        open("b.csv").await,
        open("c.csv").await,
    );
    assert_eq!(
        (kept(first).await, kept(second).await, kept(third).await),
        (false, true, true)
    );

    t.engine.lock_session().await.unwrap();
    t.engine
        .unlock_session(UnlockSessionInput {
            password: secret(PASSWORD),
        })
        .await
        .unwrap();
    assert!(!kept(third).await);

    let after_unlock = open("d.csv").await;
    t.advance(16 * 60);
    assert_eq!(
        code_of(engine.preview_layout_sample(preview(after_unlock)).await),
        ErrorCode::SessionLocked
    );
    t.engine
        .unlock_session(UnlockSessionInput {
            password: secret(PASSWORD),
        })
        .await
        .unwrap();
    assert!(!kept(after_unlock).await, "dropped at the idle lock");

    let before_sign_out = open("e.csv").await;
    t.engine.sign_out().await.unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &admin).await;
    assert!(!kept(before_sign_out).await);
}

#[tokio::test]
async fn a_file_source_registered_through_the_engine_is_approved_by_another_person() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    let approver = user(&t, &admin, "cfg2", &[RoleCode::Administrator]).await;
    let quality = user(&t, &admin, "qm1", &[RoleCode::QualityManager]).await;
    t.sign_in_with(ADMIN, PASSWORD, &admin).await;
    let morphine = crate::m2_config::analyte(&t.engine, "SYN-MOR").await;
    let instrument = t
        .engine
        .register_instrument(RegisterInstrument {
            code: "syn-files".to_owned(),
            name: "Synthetic result files".to_owned(),
            connection: ConnectionSettings::ResultFile {},
            enabled: true,
        })
        .await
        .unwrap()
        .instrument
        .id;
    let codes = vec![SourceTestCodeInput {
        source_test_code: "Morphine".to_owned(),
        analyte_id: morphine,
    }];
    let draft = t
        .engine
        .register_file_source(registration(instrument, codes))
        .await
        .unwrap();
    assert_eq!(draft.state, RegistrationState::Draft);
    assert_eq!(draft.sample_id_field, SampleIdField::SpecimenId);
    assert_eq!(draft.file_layout, Some(layout()));
    assert_eq!(
        t.events("clinical").pop().unwrap().0,
        "analytical.register_source"
    );
    let approve = ApproveSourceInput {
        registration_id: draft.registration_id,
        expected_revision: 1,
        self_approval: false,
    };
    assert_eq!(
        code_of(t.engine.approve_source(approve.clone()).await),
        ErrorCode::GateBlocked
    );
    t.sign_in_with("qm1", NEW_PASSWORD, &quality).await;
    assert_eq!(
        code_of(t.engine.approve_source(approve.clone()).await),
        ErrorCode::Forbidden
    );
    t.sign_in_with("cfg2", NEW_PASSWORD, &approver).await;
    let active = t.engine.approve_source(approve).await.unwrap();
    assert_eq!(active.state, RegistrationState::Active);
}
