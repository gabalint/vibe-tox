//! The demo laboratory (proposed REQ-OPS-016; starter data brief, decisions 1, 1a, 3 and
//! 6): an installation's kind is fixed, a demo backup never restores into a laboratory,
//! and the demo loader creates exactly the demo file's records, only in a demo
//! installation, once. Expected counts come from the demo file's README. Synthetic data
//! only.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use lis_auth::RoleCode;
use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::{
    AddBackupTargetInput, BackupRunKind, BeginRestoreInput, DemoStatus, Engine,
    ExportRecoveryKitInput, GetMethodVersionInput, InstallationKind, ListMethodVersionsInput,
    LoadStandardCatalog, RequestBackupInput, RestoreSessionInput, StageRestoreInput, TargetKind,
    TargetPurpose,
};

use crate::fixture::{PASSWORD, TestEngine, code_of};
use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::specimens::{printer_input, test_printer};

/// The test dispatcher's calls for the demo commands.
pub(crate) async fn demo_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "demo_status" => engine.demo_status().await.map(drop),
        "load_demo_data" => engine.load_demo_data().await.map(drop),
        "complete_demo_setup" => engine.complete_demo_setup().await,
        _ => return None,
    })
}

/// A started installation of `kind` with a signed-in user holding `roles`.
async fn signed_in(kind: InstallationKind, roles: &[RoleCode]) -> TestEngine {
    let t = TestEngine::start_as(kind).await;
    t.admin_session().await;
    t.create_user("demo1", roles).await;
    t.first_sign_in("demo1").await;
    t
}

/// The roles the loader needs: configuration, and order entry for the patients.
const LOADER_ROLES: [RoleCode; 2] = [RoleCode::Administrator, RoleCode::Accessioner];

fn message(error: &CommandError) -> String {
    serde_json::to_value(error).unwrap()["message"]
        .as_str()
        .unwrap()
        .to_owned()
}

fn count(t: &TestEngine, table: &str) -> i64 {
    t.conn()
        .query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
            row.get(0)
        })
        .unwrap()
}

#[tokio::test]
async fn a_database_never_opens_as_the_other_installation_kind() {
    for (created, other) in [
        (InstallationKind::Demo, InstallationKind::Laboratory),
        (InstallationKind::Laboratory, InstallationKind::Demo),
    ] {
        let t = TestEngine::start_as(created).await;
        let refused = t.reopen_as(other).await.err().expect("refused");
        assert_eq!(refused.code(), "installation_kind_mismatch");
    }
}

#[tokio::test]
async fn demo_status_reports_the_installation_kind() {
    let lab = signed_in(InstallationKind::Laboratory, &[]).await;
    assert_eq!(
        lab.engine.demo_status().await.unwrap(),
        DemoStatus {
            is_demo: false,
            demo_data_loaded: false
        }
    );
    let demo = signed_in(InstallationKind::Demo, &LOADER_ROLES).await;
    let status = demo.engine.demo_status().await.unwrap();
    assert!(status.is_demo && !status.demo_data_loaded);
    demo.engine.load_demo_data().await.unwrap();
    assert!(demo.engine.demo_status().await.unwrap().demo_data_loaded);
}

// Proposed REQ-OPS-016: demo records never enter a laboratory's database.
#[tokio::test]
async fn the_loader_is_refused_in_a_laboratory_installation() {
    let t = signed_in(InstallationKind::Laboratory, &LOADER_ROLES).await;
    let before = t.fingerprint();
    let refused = t.engine.load_demo_data().await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    assert!(message(&refused).contains("demo laboratory"), "{refused:?}");
    assert_eq!(t.fingerprint(), before, "nothing was written");
}

#[tokio::test]
async fn the_loader_creates_exactly_the_files_records_once() {
    let t = signed_in(InstallationKind::Demo, &LOADER_ROLES).await;
    let loaded = t.engine.load_demo_data().await.unwrap();
    assert!(loaded.laboratory_profile_set);
    assert_eq!(
        (
            loaded.clients,
            loaded.client_locations,
            loaded.delivery_destinations,
            loaded.practitioners,
            loaded.patients,
            loaded.identity_holds,
        ),
        (4, 8, 15, 8, 12, 1)
    );
    for (table, rows) in [
        ("idn_client_organizations", 4),
        ("idn_client_locations", 8),
        ("rpt_destinations", 15),
        ("idn_practitioners", 8),
        ("idn_subjects", 12),
        ("idn_laboratory_profiles", 1),
        ("idn_identity_holds", 1),
    ] {
        assert_eq!(count(&t, table), rows, "{table}");
    }
    let profile = t.engine.get_laboratory_profile().await.unwrap().unwrap();
    assert_eq!(profile.name, "Demo Toxicology Laboratory");
    assert_eq!(
        t.events("clinical")
            .iter()
            .filter(|(action, _)| action == "demo.load_demo_data")
            .count(),
        1
    );

    let before = t.fingerprint();
    let again = t.engine.load_demo_data().await.unwrap();
    assert_eq!((again.clients, again.patients), (0, 0));
    assert!(!again.laboratory_profile_set);
    let after = t.fingerprint();
    let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
    assert_eq!(added.len(), 1, "only the load's audit event: {added:?}");
    assert!(added[0].contains("demo.load_demo_data"), "{added:?}");
}

// REQ-IDN-001: P08 shares P07's name and birth date; registering P08 acknowledges P07 as
// the only candidate and opens a possible-duplicate hold naming it.
#[tokio::test]
async fn the_identity_hold_case_acknowledges_its_candidate() {
    let t = signed_in(InstallationKind::Demo, &LOADER_ROLES).await;
    t.engine.load_demo_data().await.unwrap();
    let pairs: Vec<(String, String)> = t
        .conn()
        .prepare(
            "SELECT h.subject_id, c.subject_id FROM idn_identity_holds h \
             JOIN idn_identity_hold_candidates c ON c.hold_id = h.id",
        )
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let mrn_of = |subject: &str| -> String {
        t.conn()
            .query_row(
                "SELECT value FROM idn_subject_identifiers WHERE subject_id = ?1",
                [subject],
                |row| row.get(0),
            )
            .unwrap()
    };
    assert_eq!(pairs.len(), 1);
    assert_eq!(
        (mrn_of(&pairs[0].0), mrn_of(&pairs[0].1)),
        ("DEMO-400002".to_owned(), "DEMO-100003".to_owned()),
        "P08 held against P07"
    );
}

// The loader runs through the domain commands, so each needs its own capability.
#[tokio::test]
async fn the_loader_needs_order_entry_for_the_patients_and_writes_nothing_without_it() {
    let t = signed_in(InstallationKind::Demo, &[RoleCode::Administrator]).await;
    assert_eq!(
        code_of(t.engine.load_demo_data().await),
        ErrorCode::Forbidden
    );
    assert_eq!(count(&t, "idn_client_organizations"), 0);
    let (action, outcome) = t.events("security").pop().unwrap();
    assert_eq!(
        (action.as_str(), outcome.as_str()),
        ("demo.load_demo_data", "denied")
    );
}

/// Oral-fluid screen and benzodiazepine drafts, each with one blank drug cutoff.
async fn load_two_methods(e: &Engine) {
    e.load_standard_catalog(LoadStandardCatalog {
        catalog_version: "1.1.0".to_owned(),
        orderable_codes: Vec::new(),
        method_codes: vec!["OF-IA-SCREEN".to_owned(), "OF-LCMSMS-BENZO".to_owned()],
        omitted_analytes: Vec::new(),
        disclaimer_acknowledged: true,
    })
    .await
    .unwrap();
}

/// The cutoffs of a method's current draft, by analyte code.
async fn draft_cutoffs(e: &Engine, code: &str) -> BTreeMap<String, Option<String>> {
    let method = e
        .list_methods()
        .await
        .unwrap()
        .into_iter()
        .find(|method| method.code == code)
        .unwrap();
    let versions = e
        .list_method_versions(ListMethodVersionsInput {
            method_id: method.id,
        })
        .await
        .unwrap();
    let draft = e
        .get_method_version(GetMethodVersionInput {
            method_version_id: versions.last().unwrap().id,
        })
        .await
        .unwrap();
    draft
        .analytes
        .into_iter()
        .map(|a| {
            (
                a.analyte_code,
                a.limits.cutoff.map(|c| c.as_str().to_owned()),
            )
        })
        .collect()
}

// Decision 3: demo values fill only blank cutoffs of the caller's drafts; published values
// stay, and a second load changes nothing.
#[tokio::test]
async fn demo_cutoffs_fill_the_blanks_of_the_authors_drafts() {
    let t = signed_in(InstallationKind::Demo, &LOADER_ROLES).await;
    load_two_methods(&t.engine).await;
    let loaded = t.engine.load_demo_data().await.unwrap();
    assert_eq!(
        (loaded.demo_cutoffs_filled, loaded.demo_cutoffs_not_filled),
        (2, 0)
    );
    let benzo = draft_cutoffs(&t.engine, "OF-LCMSMS-BENZO").await;
    assert_eq!(benzo["OF-7ACLO"].as_deref(), Some("5"));
    assert_eq!(
        benzo["OF-ALP"].as_deref(),
        Some("0.5"),
        "published value kept"
    );
    let screen = draft_cutoffs(&t.engine, "OF-IA-SCREEN").await;
    assert_eq!(
        screen["OF-AMP-IA"].as_deref(),
        Some("20"),
        "published value kept"
    );
    assert_eq!(screen["OF-MDMA-IA"].as_deref(), Some("10"));
    let again = t.engine.load_demo_data().await.unwrap();
    assert_eq!(
        (again.demo_cutoffs_filled, again.demo_cutoffs_not_filled),
        (0, 0)
    );
}

#[tokio::test]
async fn drafts_of_another_author_are_counted_not_edited() {
    let t = TestEngine::start_as(InstallationKind::Demo).await;
    t.admin_session().await;
    t.create_user("demo1", &LOADER_ROLES).await;
    t.create_user("demo2", &LOADER_ROLES).await;
    t.first_sign_in("demo1").await;
    load_two_methods(&t.engine).await;
    t.first_sign_in("demo2").await;
    let loaded = t.engine.load_demo_data().await.unwrap();
    assert_eq!(
        (loaded.demo_cutoffs_filled, loaded.demo_cutoffs_not_filled),
        (0, 2)
    );
    let benzo = draft_cutoffs(&t.engine, "OF-LCMSMS-BENZO").await;
    assert!(benzo["OF-7ACLO"].is_none(), "{benzo:?}");
    assert_eq!(benzo["OF-ALP"].as_deref(), Some("0.5"));
    let screen = draft_cutoffs(&t.engine, "OF-IA-SCREEN").await;
    assert!(screen["OF-MDMA-IA"].is_none(), "{screen:?}");
    assert_eq!(screen["OF-AMP-IA"].as_deref(), Some("20"));
}

/// The kit's `Installation key:` line, as the laboratory would type it.
fn kit_key_line(kit: &Path) -> String {
    fs::read_to_string(kit)
        .unwrap()
        .lines()
        .find_map(|line| line.strip_prefix("Installation key: "))
        .unwrap()
        .to_owned()
}

/// Backs the demo installation up to `folder`; returns the kit's key line.
async fn back_up(e: &Engine, folder: &Path, kit: &Path) -> String {
    e.add_backup_target(
        AddBackupTargetInput {
            label: "Synthetic demo backup folder".to_owned(),
            kind: TargetKind::LocalDisk,
            purpose: TargetPurpose::BackupAndAnchors,
        },
        Some(folder.to_owned()),
    )
    .await
    .unwrap()
    .unwrap();
    e.export_recovery_kit(
        ExportRecoveryKitInput {
            disk_encryption_reference: "Synthetic disk encryption".to_owned(),
            password: secrecy::SecretString::from(PASSWORD).into(),
        },
        Some(kit.to_owned()),
    )
    .await
    .unwrap()
    .unwrap();
    e.request_backup(RequestBackupInput {
        kind: BackupRunKind::Manual,
    })
    .await
    .unwrap();
    e.run_operations_pass().await;
    kit_key_line(kit)
}

// Proposed REQ-OPS-016: a laboratory installation refuses a demo backup at staging, so it
// can never be confirmed.
#[tokio::test]
async fn a_laboratory_installation_refuses_a_demo_backup() {
    let demo = TestEngine::start_as(InstallationKind::Demo).await;
    demo.admin_session().await;
    let (folder, kits) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
    let key_line = back_up(&demo.engine, folder.path(), &kits.path().join("kit.txt")).await;

    let lab = TestEngine::start().await;
    let session = lab
        .engine
        .begin_restore(BeginRestoreInput {
            installation_key: key_line,
        })
        .await
        .unwrap();
    let restore_session_id = session.restore_session_id;
    let source = lab
        .engine
        .choose_restore_source(
            RestoreSessionInput { restore_session_id },
            Some(folder.path().to_owned()),
        )
        .await
        .unwrap()
        .unwrap();
    let refused = lab
        .engine
        .stage_restore(StageRestoreInput {
            restore_session_id,
            set_id: source.sets[0].set_id.clone(),
        })
        .await
        .unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict);
    assert!(
        message(&refused).contains("demo installation"),
        "{refused:?}"
    );
}

/// The bytes of a stored artifact.
fn artifact(t: &TestEngine, digest: &str) -> Vec<u8> {
    let store = lis_store::ArtifactStore::open(
        t.data_dir().join(lis_store::ARTIFACTS_DIR),
        lis_store::MAX_ARTIFACT_BYTES,
    )
    .unwrap();
    store.read(&digest.parse().unwrap()).unwrap()
}

fn contains(bytes: &[u8], text: &str) -> bool {
    bytes
        .windows(text.len())
        .any(|window| window == text.as_bytes())
}

/// What an installation of `kind` stores and prints for the M3 bench: the released
/// report's PDF, a container label's ZPL and a Catalog Preview's PDF.
async fn stored_output(kind: InstallationKind) -> [Vec<u8>; 3] {
    let bench = Bench::ready_as(Variant::Panel, kind).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let released = bench.release(&candidate).await.unwrap();
    let lab = bench.desk().await;
    let (port, printer) = test_printer(1);
    let printer_id = lab
        .engine()
        .configure_printer(printer_input(port))
        .await
        .unwrap()
        .printer_id;
    let preview = lab.engine().generate_catalog_preview().await.unwrap();
    lab.sign_in(Role::Accessioner).await;
    lab.engine()
        .print_labels(lis_engine::PrintLabelsInput {
            container_ids: vec![bench.sample().container_id],
            printer_id,
            kind: lis_specimens::LabelJobKind::Print,
            reason: None,
        })
        .await
        .unwrap();
    let label = tokio::task::spawn_blocking(move || printer.join().unwrap().remove(0))
        .await
        .unwrap();
    let preview_digest: String = lab
        .t()
        .conn()
        .query_row(
            "SELECT artifact_digest FROM cat_catalog_previews WHERE id = ?1",
            [preview.to_string()],
            |row| row.get(0),
        )
        .unwrap();
    [
        artifact(lab.t(), &released.artifact_sha256),
        label,
        artifact(lab.t(), &preview_digest),
    ]
}

// Proposed REQ-OPS-016: everything the demo laboratory stores or prints is marked; the same
// steps in a laboratory installation are not.
#[tokio::test]
async fn stored_reports_labels_and_previews_are_marked_only_in_the_demo() {
    for (kind, marked) in [
        (InstallationKind::Demo, true),
        (InstallationKind::Laboratory, false),
    ] {
        let [report, label, preview] = stored_output(kind).await;
        assert_eq!(
            contains(&report, lis_report::METADATA_MARK),
            marked,
            "{kind:?} report"
        );
        assert_eq!(
            contains(&preview, lis_report::METADATA_MARK),
            marked,
            "{kind:?} preview"
        );
        assert_eq!(contains(&label, "^FDDEMO^FS"), marked, "{kind:?} label");
        assert!(report.starts_with(b"%PDF") && preview.starts_with(b"%PDF"));
    }
}
