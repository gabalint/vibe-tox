//! The offline day (REQ-OPS-002, REQ-OPS-007, REQ-OPS-009, NFR-23; M4 design brief §10
//! scenario 4): a fixed clock from 07:00 to 18:00, backups to a "network" and a
//! "removable" folder, the removable one unplugged at midday and catching up later, a
//! lunch close and restart whose listening gap is recorded and acknowledged and whose
//! missed backup runs at once. Everything stays on this computer: the targets are local
//! folders and no cloud service is configured.

use std::path::{Path, PathBuf};

use lis_auth::RoleCode;
use lis_core::{Clock, UtcTimestamp};
use lis_engine::{
    AcknowledgeListeningGapInput, AddBackupTargetInput, AttestHealthCheckInput, BackupRunView,
    Engine, HealthCheckView, HealthState, KeyProvider, ListAppRunsInput, ListBackupRunsInput,
    ListWorkQueueItemsInput, TargetKind, TargetPurpose, TargetView,
};
use lis_ops::health::CheckCode;
use lis_ops::policy::{DEFAULT_SETTINGS, SEAL_INTERVAL_MINUTES};
use tempfile::TempDir;

use crate::fixture::{NEW_PASSWORD, PASSWORD};
use crate::m4_runtime::Host;

fn at(time: &str) -> UtcTimestamp {
    UtcTimestamp::parse(&format!("2026-10-13T{time}:00.000000000Z")).unwrap()
}

async fn add_target(engine: &Engine, label: &str, kind: TargetKind, folder: &Path) -> TargetView {
    engine
        .add_backup_target(
            AddBackupTargetInput {
                label: label.to_owned(),
                kind,
                purpose: TargetPurpose::BackupAndAnchors,
            },
            Some(folder.to_owned()),
        )
        .await
        .unwrap()
        .expect("a chosen folder")
}

/// Someone signs in and attests a check: audited changes for the next copy.
async fn work(host: &Host, engine: &Engine, totp: &str) {
    host.admin_sign_in(engine, totp).await.unwrap();
    engine
        .attest_health_check(AttestHealthCheckInput {
            check: CheckCode::Ups,
            note: "Synthetic UPS check".to_owned(),
        })
        .await
        .unwrap();
}

async fn runs(engine: &Engine) -> Vec<BackupRunView> {
    engine
        .list_backup_runs(ListBackupRunsInput {
            before_run_id: None,
            run_id: None,
            limit: 200,
            before_requested_at: None,
        })
        .await
        .unwrap()
}

/// The labels of the targets that received a verified copy in the newest run.
async fn newest_copies(engine: &Engine) -> Vec<String> {
    let runs = runs(engine).await;
    let newest = runs.first().expect("a backup run");
    assert_eq!(newest.state, "succeeded", "{newest:?}");
    let mut labels: Vec<String> = newest
        .copies
        .iter()
        .filter(|copy| copy.state == "succeeded")
        .map(|copy| copy.target_label.clone())
        .collect();
    labels.sort();
    labels
}

async fn check(engine: &Engine, code: CheckCode) -> HealthCheckView {
    engine
        .get_host_health()
        .await
        .unwrap()
        .checks
        .into_iter()
        .find(|check| check.check == code)
        .unwrap()
}

/// The day's data folder, targets and administrator.
struct Day {
    host: Host,
    totp: String,
    network: TempDir,
    drives: TempDir,
}

impl Day {
    fn key() -> KeyProvider {
        KeyProvider::Fixed(lis_auth::DataKey::from_bytes([5; 32]))
    }

    fn removable(&self) -> PathBuf {
        self.drives.path().join("drive-a")
    }

    fn away(&self) -> PathBuf {
        self.drives.path().join("drive-a-away")
    }

    /// 07:00: the app starts, two targets are added and the first pass copies to both.
    async fn begin() -> (Self, Engine) {
        let host = Host::new();
        host.clock.set(at("07:00"));
        let engine = host.start(Self::key()).await;
        let totp = host.enrol_admin(&engine).await;
        let day = Self {
            host,
            totp,
            network: tempfile::tempdir().unwrap(),
            drives: tempfile::tempdir().unwrap(),
        };
        std::fs::create_dir(day.removable()).unwrap();
        let share = day.network.path();
        add_target(&engine, "Network share", TargetKind::NetworkShare, share).await;
        add_target(
            &engine,
            "USB drive A",
            TargetKind::Removable,
            &day.removable(),
        )
        .await;
        engine.run_operations_pass().await;
        assert_eq!(newest_copies(&engine).await, BOTH);
        (day, engine)
    }

    async fn work(&self, engine: &Engine) {
        work(&self.host, engine, &self.totp).await;
    }

    async fn sign_in(&self, engine: &Engine) {
        self.host.admin_sign_in(engine, &self.totp).await.unwrap();
    }

    /// A change and a copy every backup interval, 08:00 to 10:00; none without a change at
    /// 11:00.
    async fn morning(&self, engine: &Engine) {
        let interval = u64::from(DEFAULT_SETTINGS.backup_interval_minutes) * 60;
        for _ in 0..3 {
            self.host.advance(interval);
            self.work(engine).await;
            engine.run_operations_pass().await;
            assert_eq!(newest_copies(engine).await, BOTH);
        }
        let morning = runs(engine).await.len();
        self.host.advance(interval);
        engine.run_operations_pass().await;
        self.sign_in(engine).await;
        assert_eq!(
            runs(engine).await.len(),
            morning,
            "no copy without a change"
        );
    }

    /// 12:00: the removable drive is unplugged and rotated off the computer; the share
    /// alone takes the next copy, and the drive's set counts as the offline copy.
    async fn midday(&self, engine: &Engine) {
        self.host.clock.set(at("12:00"));
        std::fs::rename(self.removable(), self.away()).unwrap();
        self.work(engine).await;
        engine.run_operations_pass().await;
        assert_eq!(newest_copies(engine).await, ["Network share"]);
        let offline = check(engine, CheckCode::OfflineCopyAge).await;
        assert_eq!(offline.state, HealthState::Ok, "{offline:?}");
    }

    /// 13:30: the app opens again after an hour; the missed backup runs at once and the
    /// listening gap is recorded.
    async fn after_lunch(&self) -> Engine {
        self.host.clock.set(at("13:30"));
        let engine = self.host.start(Self::key()).await;
        self.sign_in(&engine).await;
        let before = runs(&engine).await.len();
        engine.run_operations_pass().await;
        let caught_up = runs(&engine).await;
        assert_eq!(
            caught_up.len(),
            before + 1,
            "the missed backup runs at once"
        );
        assert_eq!(caught_up[0].finished_at, Some(self.host.clock.now()));
        engine
    }

    /// The gap is queued for analysts and one acknowledges it.
    async fn acknowledge_gap(&self, engine: &Engine) {
        let app_runs = engine
            .list_app_runs(ListAppRunsInput {
                run_id: None,
                before_run_id: None,
                limit: 10,
            })
            .await
            .unwrap();
        assert_eq!(app_runs[0].gap_seconds, Some(3600));
        assert!(!app_runs[0].previous_unclean);
        // Granting the analyst role takes account administration and quality management.
        let lead = [RoleCode::Administrator, RoleCode::QualityManager];
        self.host.add_user(engine, "lead1", &lead, PASSWORD).await;
        self.host
            .add_user(engine, "analyst1", &[RoleCode::Analyst], NEW_PASSWORD)
            .await;
        let gaps = listening_gaps(engine).await;
        assert_eq!(gaps.len(), 1);
        assert_eq!(gaps[0].item_id, app_runs[0].run_id.to_string());
        let acknowledged = engine
            .acknowledge_listening_gap(AcknowledgeListeningGapInput {
                run_id: app_runs[0].run_id,
                note: "No analyzer ran over lunch; nothing to resend.".to_owned(),
            })
            .await
            .unwrap();
        assert!(acknowledged.acknowledged.is_some());
        assert!(listening_gaps(engine).await.is_empty());
    }

    /// 14:00 the drive is plugged in again and catches up at once, alone: the share's
    /// 13:30 copy is within the interval. 18:00 the day ends with current copies and no
    /// cloud service configured or needed.
    async fn afternoon(&self, engine: &Engine) {
        self.host.clock.set(at("14:00"));
        std::fs::rename(self.away(), self.removable()).unwrap();
        engine.run_operations_pass().await;
        self.sign_in(engine).await;
        assert_eq!(newest_copies(engine).await, ["USB drive A"]);
        self.host.clock.set(at("18:00"));
        self.work(engine).await;
        engine.run_operations_pass().await;
        assert_eq!(newest_copies(engine).await, BOTH);
        let cloud = check(engine, CheckCode::CloudServices).await;
        assert_eq!(cloud.state, HealthState::Ok);
        assert_eq!(cloud.summary, "No cloud services configured.");
        let backup_age = check(engine, CheckCode::BackupAge).await;
        assert_eq!(backup_age.state, HealthState::Ok, "{backup_age:?}");
    }
}

/// Both targets, as `newest_copies` lists them.
const BOTH: [&str; 2] = ["Network share", "USB drive A"];

async fn listening_gaps(engine: &Engine) -> Vec<lis_engine::WorkQueueItem> {
    engine
        .list_work_queue_items(ListWorkQueueItemsInput {
            after: None,
            queue: "listening_gaps".to_owned(),
            limit: 10,
        })
        .await
        .unwrap()
        .items
}

#[tokio::test]
async fn an_offline_day_backs_up_to_lab_folders_and_catches_up_after_lunch() {
    let (day, engine) = Day::begin().await;
    day.morning(&engine).await;
    day.midday(&engine).await;
    // 12:30: the app is closed for lunch; analyzers cannot deliver meanwhile.
    day.host.clock.set(at("12:30"));
    engine.shutdown().await;
    drop(engine);
    let engine = day.after_lunch().await;
    day.acknowledge_gap(&engine).await;
    day.afternoon(&engine).await;
    engine.shutdown().await;
}

// NFR-18: seals and anchor exports keep their own cadence between hourly backups.
#[tokio::test]
async fn anchors_are_exported_between_hourly_backups() {
    let (day, engine) = Day::begin().await;
    let copies = runs(&engine).await.len();
    day.host.advance(u64::from(SEAL_INTERVAL_MINUTES + 1) * 60);
    day.work(&engine).await;
    engine.run_operations_pass().await;
    assert_eq!(runs(&engine).await.len(), copies, "no backup is due yet");
    let newest_export: Option<UtcTimestamp> = day
        .host
        .conn()
        .query_row(
            "SELECT max(exported_at) FROM ops_anchor_exports",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(newest_export, Some(day.host.clock.now()));
    engine.shutdown().await;
}
