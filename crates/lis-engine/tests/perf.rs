//! Single-writer measurements for NFR-29 and NFR-05 (milestone M5) on the machine
//! that runs the test, which is not the reference hardware of any OS. Ignored by
//! default; run with
//! `cargo test -p lis-engine --test perf -- --ignored --nocapture`.
//!
//! The workload is the M2 journey laboratory ([`m2_lab`]) grown through the engine's
//! public commands to one month at the NFR baseline (2,000 accessions) and then to a
//! stress size, with each run's results imported over the real ASTM TCP listener,
//! QC-accepted and technically reviewed, so queues hold one day's work. In
//! each measured phase four simulated users (two front desks, a bench and a reviewer)
//! issue routine reads and saves, while an ASTM import burst (10× the batch) arrives and
//! a manual backup runs. The engine exposes no writer-queue hook, so the numbers are
//! end-to-end command latencies, which include the writer-queue wait. The engine has
//! one window session (LAN workstations are Q-28), so the four users share one
//! signed-in account that holds every role they need. The first scenario's panel has two
//! analytes; the volume scenario orders a panel of [`WIDE_ANALYTES`] on a second analyzer
//! and measures database growth, snapshot and backup cost, and the pages one day of work
//! changes (backup incremental design brief). Synthetic data only.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "the journey fixture has helpers this binary does not use"
)]
#[path = "scenarios/fixture.rs"]
mod fixture;
#[expect(
    dead_code,
    reason = "the journey fixture has helpers this binary does not use"
)]
#[path = "scenarios/m2_config.rs"]
mod m2_config;
#[expect(
    dead_code,
    reason = "the journey fixture has helpers this binary does not use"
)]
#[path = "scenarios/m2_lab.rs"]
mod m2_lab;
#[expect(
    dead_code,
    reason = "the journey fixture has helpers this binary does not use"
)]
#[path = "scenarios/m2_steps.rs"]
mod m2_steps;

#[path = "perf/remediation_capacity.rs"]
mod remediation_capacity;

use std::collections::BTreeMap;
use std::fs;
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use lis_analytical::{
    AttemptKind, LotId, MaterialKind, QcOutcome, QcRole, QcScope, RevisionIndicator, RunId,
};
use lis_auth::RoleCode;
use lis_catalog::{AnalyteId, ComponentInput, ComponentPurpose, MethodAnalyte};
use lis_core::{CommandError, DecimalText};
use lis_engine::{
    AcceptOrderInput, AddBackupTargetInput, AddPatientPositionInput, AddQcPositionInput,
    ApproveMethodVersion, AssessQcInput, BackupRunKind, CloseRunInput, CreateAccessionInput,
    CreateRunInput, Engine, GetAccessionInput, GetRunInput, GetRunQcInput, GetSpecimenResultsInput,
    ListBackupRunsInput, ListImportAttemptsInput, ListLotsInput, ListOrdersInput,
    ListReviewQueueInput, ListRunsInput, ListWorklistInput, OrderFilter, OrderId, OrderItemId,
    RecordReceiptDecisionInput, RequestBackupInput, ReviewQueueFilter, SubmitTechnicalReviewInput,
    TargetKind, TargetPurpose,
};
use lis_results::{ResultRevisionId, ReviewDecision};
use lis_specimens::{
    AccessionId, CollectionInput, CollectionTimeInput, ReceiptDecision, SpecimenId, SpecimenInput,
    VolumeKind,
};
use lis_store::DATABASE_FILE;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

use crate::m2_config::URINE;
use crate::m2_lab::{Lab, Role};
use crate::m2_steps::{current_revisions, message};

/// One month at the NFR-05 baseline (about 2,000 accessions a month).
const BASELINE_ACCESSIONS: usize = 2_000;
/// The stress size: ten days at the upper stress case of 1,000 accessions a day, which
/// is also five months of history at the baseline rate.
const STRESS_ACCESSIONS: usize = 10_000;
/// Accessions per run and per imported ASTM batch while the dataset grows.
const BATCH: usize = 100;
/// The import burst: ten times a routine batch of 10 samples.
const BURST: usize = 100;
/// Each user's pause between actions.
const THINK: Duration = Duration::from_millis(100);
/// Each user's minimum number of actions in a phase.
const MIN_ACTIONS: usize = 20;
/// NFR-29: p95 writer-queue wait (here measured as save latency).
const WRITER_BUDGET: Duration = Duration::from_millis(200);
/// NFR-05: p95 routine read and save latency on the host.
const READ_BUDGET: Duration = Duration::from_secs(1);
const SAVE_BUDGET: Duration = Duration::from_secs(2);
/// Analytes per accession in the volume scenario: a mid-size toxicology panel (the
/// planning workload has 50–75).
const WIDE_ANALYTES: usize = 60;
/// Sizes at which the volume scenario reports: a tenth of a year, then a year at about
/// 100 accessions a working day.
const VOLUME_ACCESSIONS: [usize; 2] = [2_500, 25_000];
/// Chunk sizes, in bytes, for which the volume scenario counts the changed chunks.
const CHUNK_BYTES: [usize; 3] = [64 << 10, 256 << 10, 1 << 20];

/// How growth approves each batch's results.
#[derive(Clone, Copy)]
enum Approval {
    /// Read the results through the engine, then approve them, as a reviewer does.
    AfterReading,
    /// Approve the current revisions found in the database without reading them first:
    /// `get_specimen_results` scans every stored measurement once per result line (P-5),
    /// so reading during growth would make a year at 60 analytes take more than a day.
    Unread,
}

/// The drug panel every accession is ordered on and every batch reports.
enum Panel {
    /// The journey's `SYN-MOR` and `SYN-OXY`.
    Journey,
    /// This many synthetic analytes on a second analyzer.
    Wide(usize),
}

/// The journey laboratory with one account holding every role the workload needs.
struct Perf {
    lab: Lab,
    lots: Vec<LotId>,
    /// Imported, QC-accepted specimens not yet reviewed.
    to_review: Mutex<Vec<SpecimenId>>,
    accessions: usize,
    /// The panel's test codes and analytes, in report order.
    panel: Vec<(String, AnalyteId)>,
    backups: TempDir,
    /// The remediation workload registers a distinct synthetic patient per receipt.
    distinct_patients: bool,
}

/// Latencies of one phase, in completion order, by kind and by command.
#[derive(Default)]
struct Latencies {
    reads: Mutex<Vec<Duration>>,
    saves: Mutex<Vec<Duration>>,
    commands: Mutex<BTreeMap<&'static str, Vec<Duration>>>,
}

#[derive(Clone, Copy)]
enum Kind {
    Read,
    Save,
}

/// What one measured phase observed.
struct Phase {
    latencies: Latencies,
    /// From the first ASTM byte of the burst to its recorded import attempt.
    burst: Option<Duration>,
    /// The manual backup during the burst.
    backup: Option<BackupPass>,
}

/// One manual backup through a job-runner pass.
struct BackupPass {
    elapsed: Duration,
    /// The snapshot size the run recorded.
    snapshot_bytes: u64,
    /// The new set's folder on the target: encrypted database and manifest.
    set_bytes: u64,
}

/// The database at one size, after a checkpoint, with a backup of it.
struct Volume {
    /// The volume table's row.
    row: String,
    /// SHA-256 of every page of the database file.
    file_pages: Vec<[u8; 32]>,
    snapshot: Snapshot,
}

/// A `VACUUM INTO` copy with its integrity check, timed separately.
struct Snapshot {
    vacuum: Duration,
    check: Duration,
    bytes: u64,
    /// SHA-256 of every page of the copy.
    pages: Vec<[u8; 32]>,
}

impl Latencies {
    /// Runs `call` and records its latency; any refusal fails the test.
    async fn time<T>(
        &self,
        command: &'static str,
        kind: Kind,
        call: impl Future<Output = Result<T, CommandError>>,
    ) -> T {
        let started = Instant::now();
        let value = call.await.unwrap();
        let elapsed = started.elapsed();
        let list = match kind {
            Kind::Read => &self.reads,
            Kind::Save => &self.saves,
        };
        list.lock().unwrap().push(elapsed);
        let mut commands = self.commands.lock().unwrap();
        commands.entry(command).or_default().push(elapsed);
        value
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measurement; run with --ignored --nocapture"]
async fn writer_wait_and_snapshot_at_baseline_and_stress() {
    let mut perf = Perf::start(Panel::Journey).await;
    println!("{}", host_line());
    println!("{HEADER}");
    let mut commands = Vec::new();
    let quiet = perf.phase(false).await;
    commands.push(perf.report("quiet: 4 users", &quiet));
    perf.grow(BASELINE_ACCESSIONS, Approval::AfterReading).await;
    let busy = perf.phase(false).await;
    commands.push(perf.report("baseline: 4 users", &busy));
    let baseline = perf.phase(true).await;
    commands.push(perf.report("baseline: 4 users, burst, backup", &baseline));
    perf.grow(STRESS_ACCESSIONS, Approval::AfterReading).await;
    let stress = perf.phase(true).await;
    commands.push(perf.report("stress: 4 users, burst, backup", &stress));
    println!("\n| Phase | Command n, p50/p95/max (ms) |\n| --- | --- |");
    for row in commands {
        println!("{row}");
    }
    for (label, phase) in [("baseline", &baseline), ("stress", &stress)] {
        let saves = percentile(&phase.latencies.saves.lock().unwrap(), 95);
        let reads = percentile(&phase.latencies.reads.lock().unwrap(), 95);
        assert!(saves <= WRITER_BUDGET, "{label}: NFR-29 save p95 {saves:?}");
        assert!(saves <= SAVE_BUDGET, "{label}: NFR-05 save p95 {saves:?}");
        assert!(reads <= READ_BUDGET, "{label}: NFR-05 read p95 {reads:?}");
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "measurement; run with --ignored --nocapture"]
async fn backup_volume_with_a_wide_panel() {
    let mut perf = Perf::start(Panel::Wide(WIDE_ANALYTES)).await;
    println!("{}; {WIDE_ANALYTES} analytes per accession", host_line());
    for size in VOLUME_ACCESSIONS {
        perf.grow(size, Approval::Unread).await;
        perf.approve_pending().await;
        let before = perf.volume().await;
        perf.grow(perf.accessions + BATCH, Approval::Unread).await;
        perf.approve_pending().await;
        let after = perf.volume().await;
        println!(
            "{VOLUME_HEADER}\n{}\n{}\n{CHURN_HEADER}",
            before.row, after.row
        );
        print_churn("database file", &before.file_pages, &after.file_pages);
        print_churn(
            "`VACUUM INTO` copy",
            &before.snapshot.pages,
            &after.snapshot.pages,
        );
    }
    print_largest_tables(&perf.lab.t().conn());
    perf.grow(perf.accessions + BATCH, Approval::Unread).await;
    println!("{HEADER}");
    let year = perf.phase(true).await;
    let commands = perf.report("year: 4 users, burst, backup", &year);
    println!("\n| Phase | Command n, p50/p95/max (ms) |\n| --- | --- |\n{commands}");
    let saves = percentile(&year.latencies.saves.lock().unwrap(), 95);
    let reads = percentile(&year.latencies.reads.lock().unwrap(), 95);
    assert!(saves <= WRITER_BUDGET, "NFR-29 save p95 {saves:?}");
    assert!(reads <= READ_BUDGET, "NFR-05 read p95 {reads:?}");
}

impl Perf {
    /// The journey laboratory with `panel`, the workload account signed in, and a folder
    /// backup target.
    async fn start(panel: Panel) -> Self {
        let mut lab = Lab::start(RevisionIndicator::None).await;
        let panel = match panel {
            Panel::Journey => vec![
                ("SYN-MOR".to_owned(), lab.analytes.mor),
                ("SYN-OXY".to_owned(), lab.analytes.oxy),
            ],
            Panel::Wide(count) => widen(&mut lab, count).await,
        };
        lab.sign_in(Role::Admin).await;
        let roles = [
            RoleCode::Accessioner,
            RoleCode::Analyst,
            RoleCode::TechnicalReviewer,
            RoleCode::QualityManager,
            RoleCode::Administrator,
        ];
        lab.t().create_user("perf1", &roles).await;
        lab.t().first_sign_in("perf1").await;
        let e = lab.engine();
        let lots = e
            .list_lots(ListLotsInput { active_only: true })
            .await
            .unwrap()
            .into_iter()
            .filter(|lot| lot.material_kind == MaterialKind::Control)
            .map(|lot| lot.lot_id)
            .collect();
        let backups = tempfile::tempdir().unwrap();
        let target = AddBackupTargetInput {
            label: "Synthetic backup folder".to_owned(),
            kind: TargetKind::LocalDisk,
            purpose: TargetPurpose::BackupAndAnchors,
        };
        e.add_backup_target(target, Some(backups.path().to_owned()))
            .await
            .unwrap()
            .unwrap();
        // Control labels must match one open position, so only each batch's run stays open.
        close_run(e, lab.run).await;
        let accessions = lab.samples.len();
        Self {
            lab,
            lots,
            to_review: Mutex::new(Vec::new()),
            accessions,
            panel,
            backups,
            distinct_patients: false,
        }
    }

    /// Registers and accepts a panel order, accessions one container and accepts the
    /// specimen: four saves. Returns the specimen, its accession and its label code.
    async fn receive(&self, l: &Latencies) -> (SpecimenId, AccessionId, String) {
        let e = self.lab.engine();
        let mut input = self.lab.order_input(self.lab.orderables.panel.1, None);
        if self.distinct_patients {
            input.subject_id = remediation_capacity::register_patient(e).await;
        }
        let order = l
            .time("register_order", Kind::Save, e.register_order(input))
            .await;
        let accept = AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        };
        let order = l
            .time("accept_order", Kind::Save, e.accept_order(accept))
            .await;
        let accession = l
            .time(
                "create_accession",
                Kind::Save,
                e.create_accession(accession_input(order.order_id, order.items[0].item_id)),
            )
            .await;
        let specimen = &accession.specimens[0];
        let decision = RecordReceiptDecisionInput {
            specimen_id: specimen.specimen_id,
            expected_revision: specimen.revision,
            decision: ReceiptDecision::Accepted,
            discrepancies: Vec::new(),
            policy_reference: None,
            scope: None,
            reason: None,
        };
        l.time(
            "record_receipt_decision",
            Kind::Save,
            e.record_receipt_decision(decision),
        )
        .await;
        let label = specimen.containers[0].label_code.clone();
        (specimen.specimen_id, accession.accession_id, label)
    }

    /// Receives `count` specimens and places them on a new run with both controls.
    async fn batch(&self, count: usize) -> (RunId, Vec<SpecimenId>, Vec<u8>) {
        let untimed = Latencies::default();
        let mut received = Vec::with_capacity(count);
        for _ in 0..count {
            received.push(self.receive(&untimed).await);
        }
        let e = self.lab.engine();
        let run = e
            .create_run(CreateRunInput {
                instrument_id: self.lab.instrument,
                method_version_id: self.lab.methods.drugs,
                lot_ids: self.lots.clone(),
            })
            .await
            .unwrap()
            .run_id;
        for (n, lot_id) in self.lots.iter().enumerate() {
            e.add_qc_position(AddQcPositionInput {
                run_id: run,
                position_label: format!("Q{}", n + 1),
                role: QcRole::Control,
                lot_id: Some(*lot_id),
                sample_label: None,
            })
            .await
            .unwrap();
        }
        for (_, _, label) in &received {
            e.add_patient_position(position(run, label)).await.unwrap();
        }
        let labels: Vec<&str> = received
            .iter()
            .map(|(_, _, label)| label.as_str())
            .collect();
        let specimens = received.iter().map(|(specimen, _, _)| *specimen).collect();
        (run, specimens, batch_message(&labels, &self.panel))
    }

    /// Delivers `artifact` through the analyzer's TCP listener and waits for the import
    /// worker's attempt, which must process every row; returns the elapsed time.
    async fn import(&self, artifact: &[u8]) -> Duration {
        let started = Instant::now();
        let receipt = self.lab.deliver(artifact).await.receipt_id;
        loop {
            let outcome: Option<String> = self
                .lab
                .t()
                .conn()
                .query_row(
                    "SELECT outcome FROM res_import_attempts WHERE receipt_id = ?1",
                    [receipt.to_string()],
                    |row| row.get(0),
                )
                .ok();
            if let Some(outcome) = outcome {
                assert_eq!(outcome, "processed", "{:?}", self.quarantined());
                return started.elapsed();
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    }

    /// `(sample label, reason)` of every quarantined row, to explain a failed import.
    fn quarantined(&self) -> Vec<(Option<String>, String)> {
        self.lab
            .t()
            .conn()
            .prepare("SELECT sample_label, reason FROM res_quarantine_items")
            .unwrap()
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }

    /// Accepts QC of every panel analyte on `run` with every current control observation.
    async fn accept_qc(&self, run_id: RunId) {
        let e = self.lab.engine();
        let qc = e.get_run_qc(GetRunQcInput { run_id }).await.unwrap();
        for &(_, analyte) in &self.panel {
            let entry = qc
                .analytes
                .iter()
                .find(|a| a.analyte_id == analyte)
                .unwrap();
            e.assess_qc(AssessQcInput {
                scope: QcScope::RunAnalyte,
                run_id: Some(run_id),
                analyte_id: Some(analyte),
                prep_batch_id: None,
                outcome: QcOutcome::Accepted,
                evidence_observation_ids: entry
                    .observations
                    .iter()
                    .filter(|o| !o.superseded)
                    .map(|o| o.observation_id)
                    .collect(),
                supersedes_assessment_id: None,
                reason: None,
            })
            .await
            .unwrap();
        }
    }

    /// Grows the dataset to `target` accessions, one run and one ASTM batch at a time:
    /// receive, place, import, accept QC, close the run and review the results, as a
    /// laboratory works through each day. The last batch is left for the reviewer.
    async fn grow(&mut self, target: usize, approval: Approval) {
        let started = Instant::now();
        while self.accessions < target {
            let count = BATCH.min(target - self.accessions);
            let (run, specimens, artifact) = self.batch(count).await;
            self.import(&artifact).await;
            self.accept_qc(run).await;
            close_run(self.lab.engine(), run).await;
            self.accessions += count;
            if self.accessions < target {
                self.approve(approval, specimens).await;
            } else {
                self.to_review.lock().unwrap().extend(specimens);
            }
        }
        println!(
            "grew to {} accessions in {:.0} s",
            self.accessions,
            started.elapsed().as_secs_f64()
        );
    }

    /// Four users act until they have done [`MIN_ACTIONS`] each and, under `load`, an
    /// import burst and a manual backup have finished alongside them.
    async fn phase(&self, load: bool) -> Phase {
        let burst = if load {
            Some(self.batch(BURST).await)
        } else {
            None
        };
        let latencies = Latencies::default();
        let done = AtomicBool::new(!load);
        let background = async {
            let Some((run, specimens, artifact)) = &burst else {
                return (None, None);
            };
            let (imported, backup) = tokio::join!(self.import(artifact), self.backup());
            done.store(true, Ordering::SeqCst);
            self.accept_qc(*run).await;
            close_run(self.lab.engine(), *run).await;
            self.to_review.lock().unwrap().extend(specimens);
            (Some(imported), Some(backup))
        };
        let l = &latencies;
        let ((burst, backup), (), (), (), ()) = tokio::join!(
            background,
            self.user(&done, async || self.desk(l).await),
            self.user(&done, async || self.desk(l).await),
            self.user(&done, async || self.bench(l).await),
            self.user(&done, async || self.reviewer(l).await),
        );
        Phase {
            latencies,
            burst,
            backup,
        }
    }

    /// Repeats `action` with think time until the phase is done.
    async fn user(&self, done: &AtomicBool, action: impl AsyncFn()) {
        let mut actions = 0;
        while actions < MIN_ACTIONS || !done.load(Ordering::SeqCst) {
            action().await;
            tokio::time::sleep(THINK).await;
            actions += 1;
        }
    }

    /// Front desk: a new order through receipt, then the order list and the accession.
    async fn desk(&self, l: &Latencies) {
        let (_, accession_id, _) = self.receive(l).await;
        let e = self.lab.engine();
        let orders = ListOrdersInput {
            filter: OrderFilter::All,
            subject_id: None,
            limit: 50,
            before_received_at: None,
            before_order_id: None,
        };
        l.time("list_orders", Kind::Read, e.list_orders(orders))
            .await;
        let accession = GetAccessionInput { accession_id };
        l.time("get_accession", Kind::Read, e.get_accession(accession))
            .await;
    }

    /// Bench: the worklist, the open runs, the journey run and the import attempts.
    async fn bench(&self, l: &Latencies) {
        let e = self.lab.engine();
        let worklist = ListWorklistInput {
            method_version_id: self.lab.methods.drugs,
        };
        l.time("list_worklist", Kind::Read, e.list_worklist(worklist))
            .await;
        let runs = ListRunsInput {
            state: None,
            instrument_id: Some(self.lab.instrument),
            method_version_id: None,
            after_run_id: None,
            limit: None,
        };
        l.time("list_runs", Kind::Read, e.list_runs(runs)).await;
        let run = GetRunInput {
            run_id: self.lab.run,
        };
        l.time("get_run", Kind::Read, e.get_run(run)).await;
        let attempts = ListImportAttemptsInput {
            instrument_id: Some(self.lab.instrument),
            limit: 50,
            before: None,
            after_import_attempt_id: None,
        };
        l.time(
            "list_import_attempts",
            Kind::Read,
            e.list_import_attempts(attempts),
        )
        .await;
    }

    /// Reviewer: the review queue, then one imported specimen's results approved.
    async fn reviewer(&self, l: &Latencies) {
        let e = self.lab.engine();
        let queue = ListReviewQueueInput {
            filter: ReviewQueueFilter::Ready,
            limit: 50,
            after: None,
        };
        l.time("list_review_queue", Kind::Read, e.list_review_queue(queue))
            .await;
        let Some(specimen_id) = self.to_review.lock().unwrap().pop() else {
            return;
        };
        self.review(l, specimen_id).await;
    }

    /// Approves every current revision of each of `specimens`, untimed.
    async fn approve(&self, approval: Approval, specimens: Vec<SpecimenId>) {
        let untimed = Latencies::default();
        let conn = self.lab.t().conn();
        for specimen_id in specimens {
            match approval {
                Approval::AfterReading => self.review(&untimed, specimen_id).await,
                Approval::Unread => {
                    let review = SubmitTechnicalReviewInput {
                        specimen_id,
                        decision: ReviewDecision::Approved,
                        result_revision_ids: current_revision_ids(&conn, specimen_id),
                        reason: None,
                    };
                    let e = self.lab.engine();
                    e.submit_technical_review(review).await.unwrap();
                }
            }
        }
    }

    /// Reads one specimen's results and approves every current revision.
    async fn review(&self, l: &Latencies, specimen_id: SpecimenId) {
        let e = self.lab.engine();
        let read = GetSpecimenResultsInput { specimen_id };
        let results = l
            .time(
                "get_specimen_results",
                Kind::Read,
                e.get_specimen_results(read),
            )
            .await;
        let review = SubmitTechnicalReviewInput {
            specimen_id,
            decision: ReviewDecision::Approved,
            result_revision_ids: current_revisions(&results),
            reason: None,
        };
        l.time(
            "submit_technical_review",
            Kind::Save,
            e.submit_technical_review(review),
        )
        .await;
    }

    /// A manual backup through one job-runner pass.
    async fn backup(&self) -> BackupPass {
        let e = self.lab.engine();
        let request = RequestBackupInput {
            kind: BackupRunKind::Manual,
        };
        e.request_backup(request).await.unwrap();
        let started = Instant::now();
        e.run_operations_pass().await;
        let elapsed = started.elapsed();
        let runs = e
            .list_backup_runs(ListBackupRunsInput {
                limit: 1,
                before_requested_at: None,
                before_run_id: None,
                run_id: None,
            })
            .await
            .unwrap();
        assert_eq!(runs[0].state, "succeeded", "{runs:?}");
        let set_id = runs[0].copies[0].set_id.as_ref().unwrap();
        BackupPass {
            elapsed,
            snapshot_bytes: runs[0].database_bytes.unwrap(),
            set_bytes: folder_bytes(&self.backups.path().join("VibeToxLIS/sets").join(set_id)),
        }
    }

    /// Prints one table row (latencies, burst, backup, and the database's size and
    /// contents) and returns the phase's per-command row for the second table.
    fn report(&self, label: &str, phase: &Phase) -> String {
        let reads = summary(&phase.latencies.reads.lock().unwrap());
        let saves = summary(&phase.latencies.saves.lock().unwrap());
        let burst = phase
            .burst
            .map_or_else(|| "—".to_owned(), |d| format!("{:.2} s", d.as_secs_f64()));
        let backup = phase.backup.as_ref().map_or_else(
            || "—".to_owned(),
            |pass| {
                format!(
                    "{:.2} s, {}",
                    pass.elapsed.as_secs_f64(),
                    mib(pass.snapshot_bytes)
                )
            },
        );
        let snapshot = self.snapshot();
        println!(
            "| {label} | {} | {} | {reads} | {saves} | {burst} | {backup} | {:.2} s, {} | {} |",
            self.accessions,
            self.measurements(),
            (snapshot.vacuum + snapshot.check).as_secs_f64(),
            mib(snapshot.bytes),
            mib(self.database_bytes()),
        );
        let commands = phase.latencies.commands.lock().unwrap();
        let p95: Vec<String> = commands
            .iter()
            .map(|(name, durations)| format!("{name} {}", summary(durations)))
            .collect();
        format!("| {label} | {} |", p95.join("; "))
    }

    /// Approves every specimen left for the reviewer, unread, as at the end of a day.
    async fn approve_pending(&self) {
        let pending = std::mem::take(&mut *self.to_review.lock().unwrap());
        self.approve(Approval::Unread, pending).await;
    }

    /// Checkpoints the log into the database file, digests the file's pages, takes a
    /// timed snapshot and a manual backup, outside any measured phase.
    async fn volume(&self) -> Volume {
        let conn = self.lab.t().conn();
        conn.busy_timeout(Duration::from_secs(60)).unwrap();
        let busy: i64 = conn
            .query_row("PRAGMA wal_checkpoint(TRUNCATE)", [], |row| row.get(0))
            .unwrap();
        assert_eq!(busy, 0, "the checkpoint completed");
        let file = self.lab.t().data_dir().join(DATABASE_FILE);
        let file_bytes = fs::metadata(&file).unwrap().len();
        assert_page_size(&conn);
        let (file_pages, scan) = page_digests(&file);
        let snapshot = self.snapshot();
        let backup = self.backup().await;
        let measurements = self.measurements();
        let accessions = u64::try_from(self.accessions).unwrap();
        let row = format!(
            "| {} | {measurements} | {} | {} KiB | {} B | {:.2} s | {:.2} s | {:.2} s | {} | {:.2} s | {} |",
            self.accessions,
            mib(file_bytes),
            tenths(u128::from(file_bytes / accessions), 1_024),
            file_bytes / u64::try_from(measurements).unwrap(),
            scan.as_secs_f64(),
            snapshot.vacuum.as_secs_f64(),
            snapshot.check.as_secs_f64(),
            mib(snapshot.bytes),
            backup.elapsed.as_secs_f64(),
            mib(backup.set_bytes),
        );
        Volume {
            row,
            file_pages,
            snapshot,
        }
    }

    fn measurements(&self) -> i64 {
        self.lab
            .t()
            .conn()
            .query_row("SELECT count(*) FROM res_measurements", [], |row| {
                row.get(0)
            })
            .unwrap()
    }

    /// `VACUUM INTO` plus `PRAGMA integrity_check` on its own connection, as the
    /// store's snapshot does, outside any measured phase; then the copy's page digests.
    fn snapshot(&self) -> Snapshot {
        let folder = tempfile::tempdir().unwrap();
        let copy = folder.path().join("snapshot.sqlite3");
        let started = Instant::now();
        let conn = self.lab.t().conn();
        conn.execute("VACUUM INTO ?1", [copy.to_str().unwrap()])
            .unwrap();
        let vacuum = started.elapsed();
        let started = Instant::now();
        let check = rusqlite::Connection::open(&copy).unwrap();
        let integrity: String = check
            .query_row("PRAGMA integrity_check", [], |row| row.get(0))
            .unwrap();
        assert_eq!(integrity, "ok");
        Snapshot {
            vacuum,
            check: started.elapsed(),
            bytes: fs::metadata(&copy).unwrap().len(),
            pages: page_digests(&copy).0,
        }
    }

    /// The database file plus its write-ahead log.
    fn database_bytes(&self) -> u64 {
        let file = self.lab.t().data_dir().join(DATABASE_FILE);
        let wal = file.with_extension("sqlite3-wal");
        [file, wal]
            .iter()
            .filter_map(|path| fs::metadata(path).ok())
            .map(|metadata| metadata.len())
            .sum()
    }
}

const HEADER: &str = "| Phase | Accessions | Measurements | Reads n, p50/p95/max (ms) | \
Saves n, p50/p95/max (ms) | Burst import | Backup pass, snapshot | VACUUM INTO + check | \
Database + WAL |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- |";

const VOLUME_HEADER: &str = "| Accessions | Measurements | Database file | Per accession | \
Per measurement | SHA-256 of the file | `VACUUM INTO` | `integrity_check` | Snapshot | \
Backup pass | Set on target |\n| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |";

const CHURN_HEADER: &str = "| After one batch of 100 accessions | Pages | Changed pages | \
Changed 64 KiB chunks | Changed 256 KiB chunks | Changed 1 MiB chunks |\n\
| --- | --- | --- | --- | --- | --- |";

/// Prints how many pages and fixed-size chunks differ between `before` and `after`,
/// counting pages added at the end as changed.
fn print_churn(label: &str, before: &[[u8; 32]], after: &[[u8; 32]]) {
    let changed: Vec<bool> = after
        .iter()
        .enumerate()
        .map(|(index, page)| before.get(index) != Some(page))
        .collect();
    let mut row = format!(
        "| {label} | {} | {} |",
        after.len(),
        changed.iter().filter(|changed| **changed).count()
    );
    for chunk in CHUNK_BYTES {
        let pages = chunk / PAGE_BYTES;
        let chunks = changed.chunks(pages).filter(|c| c.contains(&true)).count();
        let total = after.len().div_ceil(pages);
        row.push_str(&format!(
            " {chunks} of {total} ({}) |",
            mib(u64::try_from(chunks * chunk).unwrap())
        ));
    }
    println!("{row}");
}

/// SQLite's default page size, which the store keeps.
const PAGE_BYTES: usize = 4_096;

fn assert_page_size(conn: &rusqlite::Connection) {
    let size: i64 = conn
        .query_row("PRAGMA page_size", [], |row| row.get(0))
        .unwrap();
    assert_eq!(usize::try_from(size).unwrap(), PAGE_BYTES);
}

/// SHA-256 of every page of the database file at `path`, and the time it took: the cost
/// of finding changed pages by reading the whole file.
fn page_digests(path: &Path) -> (Vec<[u8; 32]>, Duration) {
    let started = Instant::now();
    let mut reader = BufReader::with_capacity(1 << 20, fs::File::open(path).unwrap());
    let mut page = vec![0; PAGE_BYTES];
    let mut digests = Vec::new();
    loop {
        match reader.read_exact(&mut page) {
            Ok(()) => digests.push(Sha256::digest(&page).into()),
            Err(error) if error.kind() == std::io::ErrorKind::UnexpectedEof => break,
            Err(error) => panic!("{error}"),
        }
    }
    (digests, started.elapsed())
}

/// The ten tables and indexes that take the most space, from `dbstat`.
fn print_largest_tables(conn: &rusqlite::Connection) {
    println!("| Table or index | Size |\n| --- | --- |");
    let mut statement = conn
        .prepare("SELECT name, sum(pgsize) FROM dbstat GROUP BY name ORDER BY 2 DESC LIMIT 10")
        .unwrap();
    let rows = statement
        .query_map([], |row| {
            Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?))
        })
        .unwrap();
    for row in rows {
        let (name, bytes) = row.unwrap();
        println!("| `{name}` | {} |", mib(u64::try_from(bytes).unwrap()));
    }
}

/// The specimen's current result revisions, read from the database.
fn current_revision_ids(
    conn: &rusqlite::Connection,
    specimen_id: SpecimenId,
) -> Vec<ResultRevisionId> {
    conn.prepare_cached(
        "SELECT r.current_revision_id FROM res_results r \
         JOIN res_result_revisions rr ON rr.id = r.current_revision_id \
         WHERE rr.specimen_id = ?1 ORDER BY r.id",
    )
    .unwrap()
    .query_map([specimen_id], |row| row.get(0))
    .unwrap()
    .collect::<Result<_, _>>()
    .unwrap()
}

/// The total size of the files in `folder`.
fn folder_bytes(folder: &Path) -> u64 {
    fs::read_dir(folder)
        .unwrap()
        .map(|entry| entry.unwrap().metadata().unwrap().len())
        .sum()
}

/// Replaces the journey's drug panel with `count` synthetic analytes (`SYN-W01`, …) in
/// `ng/mL` with cutoff `50`, on a second analyzer; the drafting administrator and
/// approving administrator are separate fixture users. Returns the panel's test codes.
async fn widen(lab: &mut Lab, count: usize) -> Vec<(String, AnalyteId)> {
    lab.sign_in(Role::Admin).await;
    let e = lab.engine();
    let mut panel = Vec::with_capacity(count);
    for n in 1..=count {
        let code = format!("SYN-W{n:02}");
        let analyte = m2_config::analyte(e, &code).await;
        panel.push((code, analyte));
    }
    let urine = e
        .list_specimen_types()
        .await
        .unwrap()
        .into_iter()
        .find(|specimen_type| specimen_type.code == URINE)
        .unwrap()
        .id;
    let analytes = panel.iter().map(|(_, analyte)| drug(*analyte)).collect();
    let method = m2_config::draft_method(e, "SYN-WIDE-M", urine, analytes).await;
    let codes: Vec<(&str, AnalyteId)> = panel
        .iter()
        .map(|(code, analyte)| (code.as_str(), *analyte))
        .collect();
    let indicator = RevisionIndicator::None;
    let (instrument, port) =
        m2_config::register_analyzer(&lab.desk, "SYNTH-LCMS-02", &codes, indicator).await;
    lab.sign_in(Role::Quality).await;
    e.approve_method_version(ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id: method,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    m2_config::approve_analyzer(&lab.desk, instrument).await;
    let component = ComponentInput {
        method_version_id: method,
        purpose: ComponentPurpose::Analytical,
        analyte_ids: panel.iter().map(|(_, analyte)| *analyte).collect(),
    };
    let orderable = m2_config::orderable(&lab.desk, "SYN-WIDE", vec![component]).await;
    lab.instrument = instrument;
    lab.port = port;
    lab.methods.drugs = method;
    lab.orderables.panel = orderable;
    panel
}

/// A drug analyte as the journey configures its two.
fn drug(analyte_id: AnalyteId) -> MethodAnalyte {
    MethodAnalyte {
        analyte_id,
        unit: Some("ng/mL".to_owned()),
        cutoff: Some(DecimalText::parse("50").unwrap()),
        limit_of_detection: None,
        limit_of_quantitation: None,
        reportable_low: None,
        reportable_high: None,
        display_scale: Some(1),
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
    }
}

fn host_line() -> String {
    let cpus = std::thread::available_parallelism().map_or(0, std::num::NonZero::get);
    format!(
        "host: {} {}, {cpus} logical CPUs; test build (dev profile: opt-level 1, dependencies 2)",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

/// The nearest-rank percentile `p` of `durations`.
fn percentile(durations: &[Duration], p: usize) -> Duration {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    let rank = (sorted.len() * p).div_ceil(100).max(1);
    sorted[rank - 1]
}

/// `n, p50/p95/max` in milliseconds with one decimal.
fn summary(durations: &[Duration]) -> String {
    let ms = |d: Duration| tenths(d.as_micros(), 1_000);
    format!(
        "{}, {}/{}/{}",
        durations.len(),
        ms(percentile(durations, 50)),
        ms(percentile(durations, 95)),
        ms(*durations.iter().max().unwrap()),
    )
}

fn mib(bytes: u64) -> String {
    format!("{} MiB", tenths(u128::from(bytes), 1_048_576))
}

/// `value / unit` with one decimal, in integer arithmetic.
fn tenths(value: u128, unit: u128) -> String {
    let tenths = value * 10 / unit;
    format!("{}.{}", tenths / 10, tenths % 10)
}

/// QC low, the patient containers in `labels`, then QC high: one result per `panel`
/// analyte each, with synthetic values that vary by position.
fn batch_message(labels: &[&str], panel: &[(String, AnalyteId)]) -> Vec<u8> {
    let mut records = Vec::new();
    let mut add = |sample: &str, position: usize, value: String| {
        records.push(format!("P|{position}"));
        records.push(format!("O|1|{sample}|SYN-POS-{position:04}||R"));
        for (n, (code, _)) in panel.iter().enumerate() {
            records.push(format!(
                "R|{}|^^^{code}|{value}|ng/mL||N||F||SYN-TECH-01||20261012091500|SYNTH-LCMS-01",
                n + 1
            ));
        }
    };
    add("QC-LOW-SYN", 1, "37.6".to_owned());
    for (n, label) in labels.iter().enumerate() {
        add(label, n + 2, format!("{}.{}", 10 + n % 90, n % 10));
    }
    add("QC-HIGH-SYN", labels.len() + 2, "187.9".to_owned());
    let records: Vec<&str> = records.iter().map(String::as_str).collect();
    message(&records)
}

async fn close_run(e: &Engine, run_id: RunId) {
    let revision = e.get_run(GetRunInput { run_id }).await.unwrap().revision;
    e.close_run(CloseRunInput {
        run_id,
        expected_revision: revision,
        reason: None,
    })
    .await
    .unwrap();
}

fn position(run_id: RunId, label: &str) -> AddPatientPositionInput {
    AddPatientPositionInput {
        run_id,
        position_label: format!("P-{label}"),
        container_label: label.to_owned(),
        attempt_kind: AttemptKind::Initial,
        related_execution_id: None,
        dilution_factor: None,
        prep_batch_id: None,
        reason: None,
    }
}

fn accession_input(order_id: OrderId, item_id: OrderItemId) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids: vec![item_id],
        collection: CollectionInput {
            time: CollectionTimeInput::Datetime {
                local: "2026-10-12T03:30".to_owned(),
                utc_offset_minutes: -240,
            },
            collector: Some("Synthetic Collector".to_owned()),
            observations: Vec::new(),
        },
        specimens: vec![SpecimenInput {
            specimen_type_code: URINE.to_owned(),
            container_count: 1,
            volume: Some(DecimalText::parse("30").unwrap()),
            volume_unit: Some("mL".to_owned()),
            volume_kind: VolumeKind::Estimated,
        }],
    }
}
