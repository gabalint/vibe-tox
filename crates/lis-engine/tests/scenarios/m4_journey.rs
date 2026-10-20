//! M4 exit scenario 1, restore onto a new computer (M4 design brief §10 and §3.7–§3.8;
//! REQ-OPS-002, REQ-OPS-007, REQ-SPC-004, NFR-04, NFR-30, ADR 0008), end to end through the
//! engine's commands. The old computer runs the M3 journey to a released report, backs up
//! to a folder target and exports the recovery kit. A fresh data folder with an empty key
//! store then runs the restore wizard with the kit's key line (begin, choose the folder,
//! stage, confirm), stops, and starts again, which applies the restore before the database
//! opens and records it after. Expected outcomes come from the brief, never from the
//! implementation. Synthetic data only.

use std::fs;
use std::path::Path;
use std::time::Duration;

use lis_audit::Stream;
use lis_auth::{DataKey, RoleCode, SignInInput, SignInStep, VerifyMfaInput};
use lis_core::{Clock, CommandError, ErrorCode};
use lis_engine::{
    AcceptOrderInput, AccessionDetail, AddBackupTargetInput, BackupRunKind, BeginRestoreInput,
    CandidateView, ChecklistItem, CloseRecoveryInput, ConfirmRestoreInput, Engine, EngineConfig,
    ExportRecoveryKitInput, GetAccessionInput, KeyProvider, ListBackupRunsInput, MemoryKeyStore,
    PrepareReportCandidateInput, PrintLabelsInput, PrinterDestinations, ReleaseReportInput,
    RequestBackupInput, RestoreSessionInput, RevisionSummary, StageRestoreInput, TargetKind,
    TargetPurpose,
};
use lis_ops::backup::Manifest;
use lis_ops::format::BackupKey;
use lis_reporting::ReportCompleteness;
use lis_specimens::{LabelJobKind, LabelOutcome};
use lis_store::{ARTIFACTS_DIR, ArtifactStore, MAX_ARTIFACT_BYTES};
use rusqlite::Connection;
use tempfile::TempDir;

use crate::fixture::{NEW_PASSWORD, TestEngine, WEBVIEW, challenge_id, secret};
use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::m4_backup::target_root;
use crate::m4_runtime::Host;
use crate::specimens::{accession_input, printer_input, test_printer};

/// The operations lead on both computers: administrator, quality manager, releaser and
/// accessioner. Not the technical reviewer, so the two-person rule holds for release.
const LEAD: &str = "lead1";
/// The bench's fixed installation key (`fixture.rs`).
const KEY: [u8; 32] = [7; 32];

/// The old computer after its backup.
struct Old {
    bench: Bench,
    lead_totp: String,
    folder: TempDir,
    kit_key_line: String,
    manifest: Manifest,
    released_artifact: String,
    highest_accession: String,
    _kits: TempDir,
}

impl Old {
    /// The M3 journey to a released report, a second specimen approved and waiting for
    /// release, then a backup to a folder target and the recovery kit.
    async fn backed_up() -> Self {
        let bench = Bench::ready(Variant::Panel).await;
        bench.sign_in(Staff::Releaser).await;
        let candidate = bench.prepare(None).await.unwrap();
        let released = bench.release(&candidate).await.unwrap();
        let lab = bench.desk().await;
        lab.approve_all(1).await.unwrap();
        lab.sign_in(Role::Admin).await;
        let roles = [
            RoleCode::Administrator,
            RoleCode::QualityManager,
            RoleCode::Releaser,
            RoleCode::Accessioner,
        ];
        lab.t().create_user(LEAD, &roles).await;
        let (_, lead_totp) = lab.t().first_sign_in(LEAD).await;
        let folder = tempfile::tempdir().unwrap();
        let kits = tempfile::tempdir().unwrap();
        let kit = kits.path().join("synthetic-recovery-kit.txt");
        let manifest = back_up(lab.engine(), folder.path(), &kit, lab.t()).await;
        Self {
            highest_accession: highest_accession(&lab.t().conn()),
            kit_key_line: kit_key_line(&kit),
            bench,
            lead_totp,
            folder,
            manifest,
            released_artifact: released.artifact_sha256,
            _kits: kits,
        }
    }
}

/// Adds the folder target, exports the kit to `kit`, runs one manual backup and returns
/// its manifest.
async fn back_up(e: &Engine, folder: &Path, kit: &Path, test: &TestEngine) -> Manifest {
    let target = e
        .add_backup_target(
            AddBackupTargetInput {
                label: "Synthetic backup folder".to_owned(),
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
            password: secrecy::SecretString::from(NEW_PASSWORD).into(),
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
    let runs = e
        .list_backup_runs(ListBackupRunsInput {
            before_run_id: None,
            run_id: None,
            limit: 1,
            before_requested_at: None,
        })
        .await
        .unwrap();
    assert_eq!(runs[0].state, "succeeded", "{runs:?}");
    let root = target_root(test, target.target_id);
    let key = BackupKey::derive(&DataKey::from_bytes(KEY));
    let mut sets = lis_ops::backup::list_sets(&root, &key).unwrap();
    assert_eq!(sets.len(), 1);
    sets.remove(0).manifest.unwrap()
}

/// The value of the kit's `Installation key:` line, as the laboratory would type it.
fn kit_key_line(kit: &Path) -> String {
    let text = fs::read_to_string(kit).unwrap();
    text.lines()
        .find_map(|line| line.strip_prefix("Installation key: "))
        .unwrap()
        .to_owned()
}

fn highest_accession(conn: &Connection) -> String {
    conn.query_row(
        "SELECT max(accession_number) FROM spc_accessions",
        [],
        |row| row.get(0),
    )
    .unwrap()
}

/// The new computer: a fresh data folder, an empty key store, a clock a day later, and
/// the test printer policy so label delivery can be watched.
struct New {
    host: Host,
    store: MemoryKeyStore,
}

impl New {
    fn after(old: &Old) -> Self {
        let host = Host::new();
        let later = old
            .bench
            .lab()
            .t()
            .clock
            .now()
            .checked_add(Duration::from_secs(86_400));
        host.clock.set(later.unwrap());
        Self {
            host,
            store: MemoryKeyStore::new(None),
        }
    }

    /// Starts the engine, waiting for an earlier engine's database lock to go.
    async fn start(&self) -> Engine {
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        loop {
            let started = Engine::start(EngineConfig {
                data_dir: self.host.dir.path().to_owned(),
                clock: self.host.clock.clone(),
                keys: KeyProvider::Memory(self.store.clone()),
                printer_destinations: PrinterDestinations::LabNetworkAndLoopback,
                webview: Some(WEBVIEW.to_owned()),
                outbox_handlers: Vec::new(),
                installation_kind: lis_engine::InstallationKind::Laboratory,
            })
            .await;
            match started {
                Ok(engine) => return engine,
                Err(error) => assert!(
                    tokio::time::Instant::now() < deadline,
                    "the engine did not start: {error}"
                ),
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    /// The restore wizard through the engine's commands, with the folder chosen
    /// explicitly; the restore is applied at the next start.
    async fn restore(&self, old: &Old) {
        let e = self.start().await;
        assert!(e.get_restore_availability().await.unwrap().available);
        let session = e
            .begin_restore(BeginRestoreInput {
                installation_key: old.kit_key_line.clone(),
            })
            .await
            .unwrap();
        let restore_session_id = session.restore_session_id;
        let source = e
            .choose_restore_source(
                RestoreSessionInput { restore_session_id },
                Some(old.folder.path().to_owned()),
            )
            .await
            .unwrap()
            .unwrap();
        assert_eq!(source.installation_id, old.manifest.installation_id);
        assert_eq!(source.sets.len(), 1);
        let staged = e
            .stage_restore(StageRestoreInput {
                restore_session_id,
                set_id: source.sets[0].set_id.clone(),
            })
            .await
            .unwrap();
        assert!(staged.can_confirm && staged.integrity_ok, "{staged:?}");
        assert_eq!(staged.chains.len(), 3, "{staged:?}");
        assert!(
            staged
                .chains
                .iter()
                .all(|c| c.intact && c.head_matches_manifest),
            "{staged:?}"
        );
        // Anchors written after the backup are the expected lost interval, nothing else.
        assert!(
            staged.findings.iter().all(|f| f.code == "lost_interval"),
            "{staged:?}"
        );
        e.confirm_restore(ConfirmRestoreInput {
            restore_session_id,
            confirmation: "RESTORE".to_owned(),
        })
        .await
        .unwrap();
        e.shutdown().await;
    }

    /// Signs the lead in with their authenticator, half a minute after the last code.
    async fn sign_in_lead(&self, e: &Engine, totp: &str) {
        self.host.advance(30);
        let step = e
            .sign_in(SignInInput {
                username: LEAD.into(),
                password: secret(NEW_PASSWORD),
            })
            .await
            .unwrap();
        assert!(matches!(step, SignInStep::MfaRequired { .. }), "{step:?}");
        e.verify_mfa(VerifyMfaInput {
            challenge_id: challenge_id(&step),
            code: secret(&self.host.code(totp)),
        })
        .await
        .unwrap();
    }
}

#[tokio::test]
async fn a_backup_restores_onto_a_new_computer_with_its_kit() {
    let old = Old::backed_up().await;
    let new = New::after(&old);
    new.restore(&old).await;
    let e = new.start().await;
    assert_eq!(
        new.store.key().map(|key| *key.expose()),
        Some(KEY),
        "the kit's key is the new computer's installation key"
    );
    new.sign_in_lead(&e, &old.lead_totp).await;
    let conn = new.host.conn();
    assert_chains_extend_the_manifest(&conn, &old.manifest);
    assert_referenced_artifacts_verify(&conn, new.host.dir.path(), &old.released_artifact);

    let accession = accession_above_the_floor(&e, &old).await;
    let (port, printer) = test_printer(1);
    let labels = held_label(&e, &accession, port).await;
    let release = ReleaseAttempt::prepare(&e, &old).await;
    let refused = release.attempt(&e).await.unwrap_err();
    let json = serde_json::to_value(&refused).unwrap();
    assert_eq!(refused.code(), ErrorCode::GateBlocked, "{json}");
    assert_eq!(
        json["gateReasons"][0]["code"],
        lis_ops::recovery::GATE_CODE,
        "{json}"
    );

    close_reconciliation(&e).await;
    let printed = after_label_outcome(&e, &accession).await;
    assert_eq!(printed, LabelOutcome::HandedOff);
    let label = String::from_utf8(printer.join().unwrap().remove(0)).unwrap();
    assert!(label.contains(&labels), "{label}");
    release.attempt(&e).await.unwrap();
    e.shutdown().await;
}

/// Every chain is intact, and each reaches the manifest's head unchanged: the restored
/// database is the backed-up one plus the events recorded since.
fn assert_chains_extend_the_manifest(conn: &Connection, manifest: &Manifest) {
    for stream in [Stream::Clinical, Stream::Security, Stream::Access] {
        let report = lis_audit::verify(conn, stream).unwrap();
        assert!(report.is_intact(), "{stream:?}: {report:?}");
    }
    assert_eq!(manifest.heads.len(), 3);
    for head in &manifest.heads {
        let hash: String = conn
            .query_row(
                "SELECT hash FROM audit_events WHERE stream = ?1 AND seq = ?2",
                rusqlite::params![head.stream, head.seq],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(hash, head.hash, "{}", head.stream);
    }
}

/// Every artifact the restored database references reads back with its digest.
fn assert_referenced_artifacts_verify(conn: &Connection, data_dir: &Path, released: &str) {
    let digests: Vec<String> = conn
        .prepare(
            "SELECT artifact_sha256 FROM rpt_candidates WHERE artifact_sha256 IS NOT NULL \
             UNION SELECT artifact_digest FROM ifc_receipts \
             UNION SELECT artifact_sha256 FROM bil_handoffs",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert!(
        digests.iter().any(|digest| digest == released),
        "{digests:?}"
    );
    let store = ArtifactStore::open(data_dir.join(ARTIFACTS_DIR), MAX_ARTIFACT_BYTES).unwrap();
    for digest in &digests {
        store.read(&digest.parse().unwrap()).unwrap();
    }
}

/// A new accession: its number is above the restored highest plus 1,000, so no number
/// the lost interval may have put on a physical label is issued again.
async fn accession_above_the_floor(e: &Engine, old: &Old) -> AccessionDetail {
    let lab = old.bench.lab();
    let order = e
        .register_order(lab.order_input(lab.orderables.panel.1, None))
        .await
        .unwrap();
    let accepted = e
        .accept_order(AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        })
        .await
        .unwrap();
    let accession = e
        .create_accession(accession_input(
            accepted.order_id,
            vec![accepted.items[0].item_id],
        ))
        .await
        .unwrap();
    let number = |text: &str| text[3..].parse::<u64>().unwrap();
    assert_eq!(accession.accession_number[..3], old.highest_accession[..3]);
    assert!(
        number(&accession.accession_number) > number(&old.highest_accession) + 1_000,
        "{} after {}",
        accession.accession_number,
        old.highest_accession
    );
    accession
}

/// Configures the loopback test printer and prints the accession's label: the job waits,
/// held, while the reconciliation is open. Returns the label code.
async fn held_label(e: &Engine, accession: &AccessionDetail, port: u16) -> String {
    let printer_id = e
        .configure_printer(printer_input(port))
        .await
        .unwrap()
        .printer_id;
    let container = &accession.specimens[0].containers[0];
    e.print_labels(PrintLabelsInput {
        container_ids: vec![container.container_id],
        printer_id,
        kind: LabelJobKind::Print,
        reason: None,
    })
    .await
    .unwrap();
    // Longer than the dispatcher's poll interval: a printable job would be handed off.
    tokio::time::sleep(Duration::from_secs(3)).await;
    let detail = e
        .get_accession(GetAccessionInput {
            accession_id: accession.accession_id,
        })
        .await
        .unwrap();
    assert_eq!(
        detail.specimens[0].containers[0].label_jobs[0].outcome,
        None
    );
    let reconciliation = e.get_recovery_reconciliation().await.unwrap().unwrap();
    assert_eq!(reconciliation.held_label_jobs, 1);
    container.label_code.clone()
}

/// The quality manager attests every item and closes the reconciliation.
async fn close_reconciliation(e: &Engine) {
    let reconciliation = e.get_recovery_reconciliation().await.unwrap().unwrap();
    assert!(reconciliation.closed.is_none());
    e.close_recovery_reconciliation(CloseRecoveryInput {
        restore_id: reconciliation.restore_id,
        attested_items: ChecklistItem::ALL.to_vec(),
        note: "Synthetic reconciliation of the lost interval".to_owned(),
        password: secrecy::SecretString::from(NEW_PASSWORD).into(),
    })
    .await
    .unwrap();
}

async fn after_label_outcome(e: &Engine, accession: &AccessionDetail) -> LabelOutcome {
    let input = GetAccessionInput {
        accession_id: accession.accession_id,
    };
    for _ in 0..400 {
        let detail = e.get_accession(input).await.unwrap();
        if let Some(outcome) = detail.specimens[0].containers[0].label_jobs[0].outcome {
            return outcome;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("the label worker recorded no outcome");
}

/// The release of the approved second specimen, prepared on the new computer.
struct ReleaseAttempt(CandidateView);

impl ReleaseAttempt {
    async fn prepare(e: &Engine, old: &Old) -> Self {
        let candidate = e
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id: old.bench.lab().samples[1].specimen_id,
                amendment_reason: None,
            })
            .await
            .unwrap();
        Self(candidate)
    }

    /// Releases the candidate as shown, with the lead's password.
    async fn attempt(&self, e: &Engine) -> Result<RevisionSummary, CommandError> {
        let candidate = &self.0;
        e.release_report(ReleaseReportInput {
            candidate_id: candidate.candidate_id,
            expected_revision: candidate.revision,
            content_sha256: candidate.content_sha256.clone(),
            acknowledge_partial: candidate.completeness == ReportCompleteness::Partial,
            password: secret(NEW_PASSWORD).into(),
            self_approval: false,
        })
        .await
    }
}
