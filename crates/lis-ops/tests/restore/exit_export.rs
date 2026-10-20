//! The exit export (REQ-OPS-010, REQ-OPS-003 export part, NFR-12; M4 design brief §5,
//! exit scenario 4's package): every manifest digest verifies, the package holds what
//! the format document lists, a failure leaves no package, and patient data never
//! reaches a file name or the README.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Capability, CorrelationId, ErrorCode};
use lis_ops::backup::{self, BackupError};
use lis_ops::exit_export::{
    EXIT_FORMAT, ExitExportId, ExitExportJob, ExitExportReport, ExitManifest, ExportFailure,
    ExportState, list_exit_exports, request_exit_export, run_exit_export,
};
use lis_store::testing::principal;
use lis_store::{ArtifactDigest, StoreError};

use crate::support::{ARTIFACT_COLUMNS, Fixture, MODULES, admin, system};

/// A value that stands for patient data in this test.
const CANARY: &str = "SYNTHETIC-CANARY-7731";

fn chosen_folder(f: &Fixture) -> PathBuf {
    let folder = f.folders.path().join("encrypted-usb");
    fs::create_dir_all(&folder).unwrap();
    folder
}

fn request(f: &Fixture, folder: &Path, reason: &str) -> Result<ExitExportId, BackupError> {
    let data_dir = f.test.data_dir().to_path_buf();
    let folder = folder.to_path_buf();
    let reason = reason.to_owned();
    f.test
        .db
        .write_blocking(admin(), CorrelationId::generate(), move |ctx| {
            request_exit_export(ctx, &data_dir, &folder, &reason)
        })
}

fn export(f: &Fixture, folder: &Path, cancel: bool) -> ExitExportReport {
    let export_id = request(f, folder, "Laboratory leaves the application").unwrap();
    let job = ExitExportJob {
        db: &f.test.db,
        data_dir: f.test.data_dir(),
        export_id,
        folder,
        modules: &MODULES,
        artifact_columns: ARTIFACT_COLUMNS,
        cancel: &AtomicBool::new(cancel),
    };
    run_exit_export(&job).unwrap()
}

/// A database with a report artifact, a canary in clinical text, and sealed streams.
fn with_data() -> Fixture {
    let f = Fixture::new();
    f.report(format!("synthetic report for {CANARY}").as_bytes());
    f.report(b"synthetic second report");
    f.write(system("test"), |ctx| {
        ctx.audit(
            AuditEvent::new(Stream::Clinical, "test.note", Outcome::Succeeded)
                .reason(format!("note about {CANARY}")),
        )
    });
    f.export_anchors();
    f
}

fn walk(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        found.push(path.clone());
        if path.is_dir() {
            walk(&path, found);
        }
    }
}

#[test]
fn the_package_holds_every_documented_file_and_every_digest_verifies() {
    let f = with_data();
    let folder = chosen_folder(&f);
    let report = export(&f, &folder, false);
    assert_eq!(report.state, ExportState::Succeeded, "{:?}", report.failure);
    let name = report.package_name.clone().unwrap();
    let installation = f
        .test
        .db
        .read_blocking(lis_ops::installation_id)
        .unwrap()
        .unwrap();
    assert!(name.starts_with(&format!("vibe-tox-exit-{installation}-")));
    let package = folder.join(&name);
    let manifest_bytes = fs::read(package.join("manifest.json")).unwrap();
    let (manifest_digest, _) = ArtifactDigest::of_reader(manifest_bytes.as_slice()).unwrap();
    assert_eq!(
        report.manifest_sha256.as_deref(),
        Some(manifest_digest.to_string().as_str())
    );
    let manifest: ExitManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    assert_eq!(manifest.format, EXIT_FORMAT);
    for file in &manifest.files {
        let path = file
            .path
            .split('/')
            .fold(package.clone(), |path, part| path.join(part));
        let (digest, bytes) = ArtifactDigest::of_reader(fs::File::open(path).unwrap()).unwrap();
        assert_eq!(
            (digest.to_string(), bytes),
            (file.sha256.clone(), file.bytes),
            "{}",
            file.path
        );
    }
    let listed: Vec<&str> = manifest
        .files
        .iter()
        .map(|file| file.path.as_str())
        .collect();
    for expected in [
        "README.txt",
        "database/lis.sqlite3",
        "audit/anchors.jsonl",
        "schema/schema.sql",
        "schema/data-dictionary.json",
        "schema/data-dictionary.md",
        "schema/migrations/audit/0001_audit_events.sql",
        "schema/migrations/store/0001_store_outbox.sql",
        "schema/migrations/ops/0003_restore.sql",
        "schema/migrations/demo/0001_demo_reports.sql",
    ] {
        assert!(
            listed.contains(&expected),
            "{expected} missing from {listed:?}"
        );
    }
    let artifacts = listed
        .iter()
        .filter(|path| path.starts_with("artifacts/"))
        .count();
    assert_eq!(artifacts, 2);
    assert!(manifest.artifact_problems.is_empty());
    check_verifier_digest(&package, &report);
    assert_eq!(manifest.artifact_references[0].table, "demo_reports");
    check_anchors(&package, &manifest);
    check_dictionary(&package);
    let mut all = Vec::new();
    walk(&folder, &mut all);
    assert_eq!(
        all.len(),
        manifest.files.len() + 1 + directories(&all),
        "nothing unlisted"
    );
}

fn directories(paths: &[PathBuf]) -> usize {
    paths.iter().filter(|path| path.is_dir()).count()
}

fn check_verifier_digest(package: &Path, report: &ExitExportReport) {
    let verifier = fs::read(package.join("tools").join("verify_export.py")).unwrap();
    let verifier_digest = ArtifactDigest::of_reader(verifier.as_slice())
        .unwrap()
        .0
        .to_string();
    assert_eq!(
        report.verifier_sha256.as_deref(),
        Some(verifier_digest.as_str())
    );
    assert!(
        fs::read_to_string(package.join("README.txt"))
            .unwrap()
            .contains(&format!("Embedded verifier SHA-256: {verifier_digest}"))
    );
}

fn check_anchors(package: &Path, manifest: &ExitManifest) {
    let text = fs::read_to_string(package.join("audit").join("anchors.jsonl")).unwrap();
    let parsed = text
        .lines()
        .map(|line| lis_audit::parse_line(line).unwrap());
    assert_eq!(parsed.count(), manifest.seal_count);
    assert!(
        manifest.seal_count >= 2,
        "the export sealed every stream with events"
    );
}

fn check_dictionary(package: &Path) {
    let json: serde_json::Value = serde_json::from_slice(
        &fs::read(package.join("schema").join("data-dictionary.json")).unwrap(),
    )
    .unwrap();
    let table = |name: &str| {
        json["tables"]
            .as_array()
            .unwrap()
            .iter()
            .find(|table| table["name"] == name)
            .unwrap()
            .clone()
    };
    let reports = table("demo_reports");
    assert_eq!(reports["module"], "demo");
    let artifact = reports["columns"]
        .as_array()
        .unwrap()
        .iter()
        .find(|column| column["name"] == "artifact_sha256")
        .unwrap();
    assert_eq!(artifact["artifact"], true);
    assert_eq!(table("ops_restores")["module"], "ops");
    assert_eq!(table("ops_anchor_exports")["module"], "ops_anchors");
    assert_eq!(table("audit_events")["module"], "audit");
    assert!(
        table("ops_restores")["ddl"]
            .as_str()
            .unwrap()
            .contains("CHECK")
    );
}

#[test]
fn patient_data_never_reaches_a_file_name_the_readme_or_the_record() {
    let f = with_data();
    let folder = chosen_folder(&f);
    let report = export(&f, &folder, false);
    let package = folder.join(report.package_name.unwrap());
    let mut all = Vec::new();
    walk(&folder, &mut all);
    for path in &all {
        assert!(
            !path.to_string_lossy().contains(CANARY),
            "{}",
            path.display()
        );
    }
    let readme = fs::read_to_string(package.join("README.txt")).unwrap();
    assert!(
        readme.starts_with("## Exit export"),
        "the format document's section"
    );
    assert!(!readme.contains(CANARY));
    let database = fs::read(package.join("database").join("lis.sqlite3")).unwrap();
    let held = database
        .windows(CANARY.len())
        .any(|window| window == CANARY.as_bytes());
    assert!(held, "the package itself does hold the data");
    let details: Vec<String> = f
        .test
        .db
        .read_blocking(|conn| -> Result<_, StoreError> {
            let mut statement = conn
                .prepare("SELECT details FROM audit_events WHERE action LIKE 'ops.%exit_export'")?;
            let rows = statement.query_map([], |row| row.get(0))?;
            Ok(rows.collect::<rusqlite::Result<Vec<String>>>()?)
        })
        .unwrap();
    assert_eq!(details.len(), 2);
    assert!(
        details
            .iter()
            .all(|text| !text.contains(CANARY) && !text.contains("encrypted-usb"))
    );
}

#[test]
fn an_export_stopped_part_way_leaves_no_package() {
    let f = with_data();
    let folder = chosen_folder(&f);
    let report = export(&f, &folder, true);
    assert_eq!(report.state, ExportState::Interrupted);
    assert_eq!(
        fs::read_dir(&folder).unwrap().count(),
        0,
        "the package folder was removed"
    );
    let records = f
        .test
        .db
        .read_blocking(|conn| list_exit_exports(conn, 5))
        .unwrap();
    assert_eq!(records[0].state, ExportState::Interrupted);
    assert!(records[0].package_name.is_none());
}

#[test]
fn a_declared_artifact_reference_without_copied_bytes_fails_the_export() {
    let f = Fixture::new();
    f.reference(&"ab".repeat(32));
    let folder = chosen_folder(&f);
    let report = export(&f, &folder, false);

    assert_eq!(report.state, ExportState::Failed);
    assert_eq!(report.failure, Some(ExportFailure::ArtifactListingFailed));
    assert!(report.package_name.is_none());
    assert_eq!(fs::read_dir(folder).unwrap().count(), 0);
}

#[test]
fn a_package_folder_the_app_did_not_create_is_never_removed() {
    let f = with_data();
    let folder = chosen_folder(&f);
    let first = export(&f, &folder, false);
    let name = first.package_name.unwrap();
    // A second export in the same second finds the name taken.
    let second = export(&f, &folder, false);
    assert_eq!(second.state, ExportState::Failed);
    assert_eq!(second.failure, Some(ExportFailure::FolderUnusable));
    assert!(folder.join(&name).join("manifest.json").exists());
    let first_page = f
        .test
        .db
        .read_blocking(|conn| list_exit_exports(conn, 1))
        .unwrap();
    assert_eq!(
        first_page.len(),
        2,
        "the extra row proves more exports exist"
    );
}

#[test]
fn a_request_needs_export_a_reason_an_outside_folder_and_no_running_export() {
    let f = with_data();
    let folder = chosen_folder(&f);
    let data_dir = f.test.data_dir().to_path_buf();
    let as_user = |capabilities: &[Capability], folder: PathBuf, reason: &str| {
        let data_dir = data_dir.clone();
        let reason = reason.to_owned();
        f.test.db.write_blocking(
            principal(capabilities),
            CorrelationId::generate(),
            move |ctx| request_exit_export(ctx, &data_dir, &folder, &reason),
        )
    };
    let forbidden = as_user(&[Capability::BackupRestore], folder.clone(), "Leaving");
    assert_eq!(forbidden.unwrap_err().code(), ErrorCode::Forbidden);
    let no_reason = as_user(&[Capability::Export], folder.clone(), " ").unwrap_err();
    assert_eq!(no_reason.code(), ErrorCode::Validation);
    let inside = as_user(&[Capability::Export], data_dir.join("artifacts"), "Leaving");
    assert!(matches!(
        inside.unwrap_err(),
        BackupError::ExportFolderUnusable
    ));
    let missing = as_user(&[Capability::Export], folder.join("absent"), "Leaving");
    assert!(matches!(
        missing.unwrap_err(),
        BackupError::ExportFolderUnusable
    ));
    request(&f, &folder, "Leaving").unwrap();
    let running = request(&f, &folder, "Leaving again").unwrap_err();
    assert!(matches!(running, BackupError::ExitExportRunning));

    let interrupted = f.write(system("test"), backup::interrupt_abandoned);
    assert_eq!(interrupted, 1);
    let records = f
        .test
        .db
        .read_blocking(|conn| list_exit_exports(conn, 5))
        .unwrap();
    assert_eq!(records[0].state, ExportState::Interrupted);
    assert_eq!(records[0].folder_label, "encrypted-usb");
    request(&f, &folder, "Leaving, after the restart").unwrap();
}
