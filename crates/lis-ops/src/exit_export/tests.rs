//! The package carries the independent verifier (M4 design brief §5): `tools/verify_export.py`
//! is a byte-identical copy of `scripts/verify_exit_export.py`, listed in the manifest, and
//! verifies its own package (run when `VIBE_TOX_RUN_PY_VERIFIER=1`, as CI does on Linux).

use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::atomic::AtomicBool;

use lis_core::{Capability, CorrelationId};
use lis_store::ArtifactDigest;
use lis_store::testing::{TestDatabase, principal};

use super::{ExitExportJob, ExitManifest, ExportState, request_exit_export, run_exit_export};
use crate::backup::ArtifactProblem;

fn verifier_succeeds(script: &Path, package: &Path) -> bool {
    Command::new("python3")
        .arg("-I")
        .arg(script)
        .arg(package)
        .output()
        .expect("python3 runs")
        .status
        .success()
}

fn write_manifest(package: &Path, manifest: &ExitManifest) {
    let mut bytes = serde_json::to_vec_pretty(manifest).unwrap();
    bytes.push(b'\n');
    fs::write(package.join("manifest.json"), bytes).unwrap();
}

fn refresh_file(package: &Path, manifest: &mut ExitManifest, relative: &str) {
    let bytes = fs::read(
        relative
            .split('/')
            .fold(package.to_path_buf(), |path, part| path.join(part)),
    )
    .unwrap();
    let (digest, length) = ArtifactDigest::of_reader(bytes.as_slice()).unwrap();
    let entry = manifest
        .files
        .iter_mut()
        .find(|entry| entry.path == relative)
        .unwrap();
    entry.sha256 = digest.to_string();
    entry.bytes = length;
    write_manifest(package, manifest);
}

fn check_tampered_variants(
    script: &Path,
    package: &Path,
    manifest: &ExitManifest,
    manifest_bytes: &[u8],
) {
    assert!(verifier_succeeds(script, package));
    let restore_manifest = || fs::write(package.join("manifest.json"), manifest_bytes).unwrap();

    let mut changed = manifest.clone();
    changed.artifact_problems.push(ArtifactProblem {
        sha256: None,
        problem: "unexpected_entry".to_owned(),
    });
    write_manifest(package, &changed);
    assert!(!verifier_succeeds(script, package));
    restore_manifest();

    let readme = fs::read(package.join("README.txt")).unwrap();
    fs::remove_file(package.join("README.txt")).unwrap();
    assert!(!verifier_succeeds(script, package));
    fs::write(package.join("README.txt"), readme).unwrap();

    check_semantic_tampering(script, package, manifest, manifest_bytes);
    fs::remove_file(package.join("tools").join("verify_export.py")).unwrap();
    assert!(!verifier_succeeds(script, package));
}

fn check_semantic_tampering(
    script: &Path,
    package: &Path,
    manifest: &ExitManifest,
    manifest_bytes: &[u8],
) {
    let schema_path = package.join("schema").join("schema.sql");
    let schema = fs::read(&schema_path).unwrap();
    fs::write(&schema_path, b"-- synthetically altered schema\n").unwrap();
    let mut changed = manifest.clone();
    refresh_file(package, &mut changed, "schema/schema.sql");
    assert!(!verifier_succeeds(script, package));
    fs::write(&schema_path, schema).unwrap();
    fs::write(package.join("manifest.json"), manifest_bytes).unwrap();

    let dictionary_path = package.join("schema").join("data-dictionary.json");
    let dictionary = fs::read(&dictionary_path).unwrap();
    let mut changed_dictionary: serde_json::Value = serde_json::from_slice(&dictionary).unwrap();
    changed_dictionary["tables"][0]["ddl"] = serde_json::Value::String("changed".to_owned());
    fs::write(
        &dictionary_path,
        serde_json::to_vec_pretty(&changed_dictionary).unwrap(),
    )
    .unwrap();
    let mut changed = manifest.clone();
    refresh_file(package, &mut changed, "schema/data-dictionary.json");
    assert!(!verifier_succeeds(script, package));
    fs::write(&dictionary_path, dictionary).unwrap();
    fs::write(package.join("manifest.json"), manifest_bytes).unwrap();
}

#[test]
fn the_package_lists_a_byte_identical_copy_of_the_verifier() {
    let modules = [crate::SCHEMA, crate::ANCHORS_SCHEMA];
    let test = TestDatabase::open(&modules).unwrap();
    let who = principal(&[Capability::Export]);
    test.db
        .write_blocking(who.clone(), CorrelationId::generate(), |ctx| {
            crate::ensure_installation(ctx)
        })
        .unwrap();
    let folder = tempfile::tempdir().unwrap();
    let data_dir = test.data_dir().to_path_buf();
    let chosen = folder.path().to_path_buf();
    let export_id = test
        .db
        .write_blocking(who, CorrelationId::generate(), move |ctx| {
            request_exit_export(ctx, &data_dir, &chosen, "Synthetic exit")
        })
        .unwrap();
    let report = run_exit_export(&ExitExportJob {
        db: &test.db,
        data_dir: test.data_dir(),
        export_id,
        folder: folder.path(),
        modules: &modules,
        artifact_columns: &[],
        cancel: &AtomicBool::new(false),
    })
    .unwrap();
    assert_eq!(report.state, ExportState::Succeeded, "{:?}", report.failure);
    let package = folder.path().join(report.package_name.unwrap());

    let script = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../scripts/verify_exit_export.py");
    let original = fs::read(&script).unwrap();
    let copy = fs::read(package.join("tools").join("verify_export.py")).unwrap();
    assert_eq!(copy, original, "the copy is byte-identical");

    let manifest_bytes = fs::read(package.join("manifest.json")).unwrap();
    let manifest: ExitManifest = serde_json::from_slice(&manifest_bytes).unwrap();
    let listed = manifest
        .files
        .iter()
        .find(|file| file.path == "tools/verify_export.py")
        .expect("the verifier is listed in the manifest");
    let (digest, bytes) = ArtifactDigest::of_reader(original.as_slice()).unwrap();
    assert_eq!(
        (listed.sha256.as_str(), listed.bytes),
        (digest.to_string().as_str(), bytes)
    );

    if std::env::var("VIBE_TOX_RUN_PY_VERIFIER").as_deref() == Ok("1") {
        check_tampered_variants(&script, &package, &manifest, &manifest_bytes);
    }
}
