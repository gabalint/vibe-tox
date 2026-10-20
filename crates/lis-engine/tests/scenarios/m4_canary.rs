//! M4 exit scenario 9: the secret canary of `no_secrets` extended to M4 (NFR-32, M4 brief
//! §0 ground rules). The installation key, in every form the app writes it, the recovery
//! kit's key line, target folder paths and target labels never appear in the diagnostic
//! log, in errors returned to the UI, or in audit details. The kit is exported with
//! `export_recovery_kit`; the backup runs through `lis_ops` (see `m4_anchors`). Synthetic
//! data only.

use std::fs;
use std::sync::Arc;

use lis_core::{Clock, CommandError};
use lis_engine::ExportRecoveryKitInput;
use lis_ops::kit;
use lis_ops::targets::TargetKind;

use crate::fixture::NEW_PASSWORD;
use crate::m4_anchors::{Station, backup_now, installation_key};
use crate::no_secrets::logged;

const PATH_CANARY: &str = "SYNTHPATHCANARY";
const LABEL_CANARY: &str = "SYNTHLABELCANARY";
/// The installation key `[7; 32]` in lowercase hexadecimal and base64.
const KEY_HEX: &str = "0707070707070707070707070707070707070707070707070707070707070707";
const KEY_BASE64: &str = "BwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwcHBwc";

#[tokio::test]
async fn keys_and_target_paths_never_reach_logs_errors_or_audit_details() {
    logged();
    let mut station = Station::new().await;
    let target = station
        .add_target(
            &format!("{PATH_CANARY}-usb"),
            &format!("{LABEL_CANARY} USB"),
            TargetKind::Removable,
        )
        .await;
    let kit_folder = station.folders.path().join(format!("{PATH_CANARY}-kit"));
    fs::create_dir_all(&kit_folder).unwrap();
    let kit_file = kit_folder.join("vibe-tox-recovery-kit.txt");
    station
        .engine()
        .export_recovery_kit(
            ExportRecoveryKitInput {
                disk_encryption_reference: "Synthetic sealed envelope".to_owned(),
                password: secrecy::SecretString::from(NEW_PASSWORD).into(),
            },
            Some(kit_file.clone()),
        )
        .await
        .unwrap()
        .unwrap();
    let key_line = kit::key_text(&installation_key()).to_string();
    let kit_text = fs::read_to_string(&kit_file).unwrap();
    assert!(kit_text.contains(&key_line), "the kit holds the key line");
    station.stop().await;
    station.changes(2, "synthetic change").await;
    let clock: Arc<dyn Clock> = station.clock.clone();
    station
        .offline(move |db, dir| backup_now(db, dir, clock.as_ref()))
        .await;

    station.start().await;
    station.export_anchors().await;
    station.verify_targets().await;
    let mut errors: Vec<CommandError> = Vec::new();
    let missing = station
        .folders
        .path()
        .join(format!("{PATH_CANARY}-missing"));
    errors.push(
        station
            .engine()
            .verify_audit_anchors(Some(missing))
            .await
            .unwrap_err(),
    );
    fs::rename(&target.path, station.folders.path().join("unplugged")).unwrap();
    errors.push(station.engine().export_anchors_now().await.unwrap_err());

    let logs = logged();
    assert!(logs.contains("command completed"), "the capture works");
    let errors = serde_json::to_string(&errors).unwrap();
    let audit: String = station
        .raw()
        .query_row(
            "SELECT group_concat(concat_ws('|', principal_id, action, record_type, record_id, \
             authority, details), char(10)) FROM audit_events",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let folders = station.folders.path().display().to_string();
    for (name, text) in [("log", &logs), ("error", &errors), ("audit", &audit)] {
        for forbidden in [
            KEY_HEX,
            KEY_BASE64,
            key_line.as_str(),
            PATH_CANARY,
            LABEL_CANARY,
            folders.as_str(),
        ] {
            assert!(!text.contains(forbidden), "{name} contains {forbidden:?}");
        }
    }
}
