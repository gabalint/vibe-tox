//! Synthetic remediation capacity evidence (UI-12), never clinical validation.
//! Run the ignored test with VIBE_TOX_CAPACITY_ARTIFACT_DIR to retain picker payload.
use super::*;
use lis_core::Phi;
use lis_identity::{DemographicsInput, Npi, RegisterPractitioner, RegisterSubject, SubjectId};

const HISTORY: usize = 25_000;
const PEAK_PATIENTS: usize = 250;
const ANALYTES: usize = 12;
const REPEATS: usize = 25;
const PRACTITIONERS: usize = 10_000;

pub(super) async fn register_patient(engine: &Engine) -> SubjectId {
    // Unique synthetic name avoids accidental duplicate candidates; no real identities.
    let token = SubjectId::generate();
    engine
        .register_subject(RegisterSubject {
            demographics: DemographicsInput {
                family_name: Phi::new(format!("SYN-{token}")),
                given_names: Some(Phi::new("Capacity fixture".to_owned())),
                birth_date: Phi::new("1980-01-01".to_owned()),
                administrative_sex: None,
            },
            source: "Synthetic capacity measurement".to_owned(),
            identifiers: Vec::new(),
            acknowledged_candidates: Vec::new(),
        })
        .await
        .unwrap()
        .subject_id
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
#[ignore = "synthetic capacity measurement; run with --ignored --nocapture"]
async fn remediation_history_peak_repeats_and_directory_payload() {
    let mut perf = Perf::start(Panel::Wide(ANALYTES)).await;
    perf.distinct_patients = true;
    println!(
        "{}; history={HISTORY} accessions; peak={PEAK_PATIENTS} distinct new patients; panel={ANALYTES}; repeats={REPEATS}",
        host_line()
    );
    let retained = RetainedDatabase(perf.lab.t().data_dir().to_owned());
    perf.grow(HISTORY, Approval::Unread).await;
    retained.snapshot("history");
    perf.approve_pending().await;
    let (run, specimens, artifact) = perf.batch(PEAK_PATIENTS).await;
    let imported = perf.import(&artifact).await;
    perf.accept_qc(run).await;
    close_run(perf.lab.engine(), run).await;
    println!(
        "peak import: {} bytes, {} patient analyte observations, {:.3} s",
        artifact.len(),
        PEAK_PATIENTS * ANALYTES,
        imported.as_secs_f64()
    );
    let started = Instant::now();
    let queue = perf
        .lab
        .engine()
        .list_review_queue(ListReviewQueueInput {
            filter: ReviewQueueFilter::Ready,
            limit: 50,
            after: None,
        })
        .await
        .unwrap();
    println!(
        "ready queue: first-page {} rows of at least {PEAK_PATIENTS} pending specimens, {:.3} ms",
        queue.items.len(),
        started.elapsed().as_secs_f64() * 1_000.0
    );
    assert_eq!(
        queue.items.len(),
        50,
        "public review page respects the requested item limit"
    );
    assert!(
        queue.next.is_some(),
        "remaining peak specimens stay reachable"
    );
    measure_composed_queue(perf.lab.engine()).await;
    repeat(&perf, run).await;
    perf.approve(Approval::Unread, specimens).await;
    directory(perf.lab.engine(), 1).await;
    counts(&perf);
    retained.snapshot("complete");
}

async fn repeat(perf: &Perf, parent_run: RunId) {
    let engine = perf.lab.engine();
    let parents = engine
        .get_run(GetRunInput { run_id: parent_run })
        .await
        .unwrap();
    let repeated = engine
        .create_run(CreateRunInput {
            instrument_id: perf.lab.instrument,
            method_version_id: perf.lab.methods.drugs,
            lot_ids: perf.lots.clone(),
        })
        .await
        .unwrap();
    for (index, lot) in perf.lots.iter().enumerate() {
        engine
            .add_qc_position(AddQcPositionInput {
                run_id: repeated.run_id,
                position_label: format!("Q{index}"),
                role: QcRole::Control,
                lot_id: Some(*lot),
                sample_label: None,
            })
            .await
            .unwrap();
    }
    let patients: Vec<_> = parents
        .positions
        .iter()
        .filter(|p| p.execution.is_some())
        .take(REPEATS)
        .collect();
    assert_eq!(patients.len(), REPEATS);
    for parent in &patients {
        let mut input = position(repeated.run_id, &parent.sample_label);
        input.attempt_kind = AttemptKind::Rerun;
        input.related_execution_id = parent.execution.as_ref().map(|e| e.execution_id);
        input.reason = Some("Synthetic capacity repeat".to_owned());
        engine.add_patient_position(input).await.unwrap();
    }
    let labels: Vec<_> = patients.iter().map(|p| p.sample_label.as_str()).collect();
    let elapsed = perf.import(&batch_message(&labels, &perf.panel)).await;
    perf.accept_qc(repeated.run_id).await;
    close_run(engine, repeated.run_id).await;
    println!(
        "repeat import: {REPEATS} executions, {} analyte observations, {:.3} s",
        REPEATS * ANALYTES,
        elapsed.as_secs_f64()
    );
}

async fn directory(engine: &Engine, existing: usize) {
    for index in 0..PRACTITIONERS {
        let prefix = format!("19{index:07}");
        let npi = (0..10)
            .map(|digit| format!("{prefix}{digit}"))
            .find(|npi| Npi::parse(npi).is_ok())
            .unwrap();
        engine
            .register_practitioner(RegisterPractitioner {
                npi,
                family_name: format!("SYN-PRACTITIONER-{index:05}"),
                given_names: Some("Capacity fixture".to_owned()),
            })
            .await
            .unwrap();
    }
    let mut measurements = Vec::new();
    let mut payload = Vec::new();
    for _ in 0..20 {
        let started = Instant::now();
        let records = engine.list_practitioners().await.unwrap();
        payload = serde_json::to_vec(&records).unwrap();
        assert_eq!(records.len(), PRACTITIONERS + existing);
        measurements.push(started.elapsed());
    }
    println!(
        "practitioner full read + JSON: {} records; {} bytes; n,p50/p95/max ms {}",
        PRACTITIONERS + existing,
        payload.len(),
        summary(&measurements)
    );
    if let Some(path) = std::env::var_os("VIBE_TOX_CAPACITY_ARTIFACT_DIR") {
        fs::create_dir_all(&path).unwrap();
        fs::write(
            Path::new(&path).join("practitioner-picker-payload.json"),
            payload,
        )
        .unwrap();
    }
}

fn counts(perf: &Perf) {
    let conn = perf.lab.t().conn();
    for (label, sql) in [
        ("patients", "SELECT count(*) FROM idn_subjects"),
        ("accessions", "SELECT count(*) FROM spc_accessions"),
        ("specimens", "SELECT count(*) FROM spc_specimens"),
        ("patient executions", "SELECT count(*) FROM ana_executions"),
        (
            "patient analyte observations",
            "SELECT count(*) FROM res_measurements",
        ),
        (
            "QC observations",
            "SELECT count(*) FROM ana_qc_observations",
        ),
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).unwrap();
        println!("{label}: {count}");
    }
}

/// A separate current-contract remeasurement avoids repeating the accumulated-history seed
/// when directory metadata changes while the long history measurement is running.
#[tokio::test]
#[ignore = "synthetic directory measurement; run with --ignored --nocapture"]
async fn remediation_current_directory_contract_payload() {
    let t = crate::fixture::TestEngine::start().await;
    t.admin_session().await;
    directory(&t.engine, 0).await;
}

// SQLite produces a transactionally consistent standalone snapshot, including committed WAL
// content, at each completed phase and on unwind. A failed measurement keeps its evidence.
struct RetainedDatabase(std::path::PathBuf);
impl RetainedDatabase {
    fn snapshot(&self, phase: &str) {
        let Some(root) = std::env::var_os("VIBE_TOX_CAPACITY_ARTIFACT_DIR") else {
            return;
        };
        let destination = Path::new(&root).join(format!("capacity-{phase}.sqlite"));
        fs::create_dir_all(&root).unwrap();
        if destination.exists() {
            fs::remove_file(&destination).unwrap();
        }
        let conn = rusqlite::Connection::open(self.0.join(DATABASE_FILE)).unwrap();
        conn.execute("VACUUM INTO ?1", [destination.to_str().unwrap()])
            .unwrap();
        println!(
            "retained synthetic SQLite snapshot: {}",
            destination.display()
        );
    }
}
impl Drop for RetainedDatabase {
    fn drop(&mut self) {
        if std::thread::panicking() {
            self.snapshot("failed");
        }
    }
}
async fn measure_composed_queue(engine: &Engine) {
    let started = Instant::now();
    let mut cursor = None;
    let mut count = 0;
    loop {
        let rows = engine
            .list_work_queue_items(lis_engine::ListWorkQueueItemsInput {
                queue: "review_ready".to_owned(),
                limit: 100,
                after: cursor,
            })
            .await
            .unwrap();
        count += rows.items.len();
        cursor = rows.next;
        if cursor.is_none() {
            break;
        }
    }
    let elapsed = started.elapsed();
    println!(
        "composed technical-review queue: {count} items across all pages, {}.{:03} ms",
        elapsed.as_millis(),
        elapsed.subsec_micros() % 1_000
    );
    assert_eq!(count, PEAK_PATIENTS);
    println!(
        "billing list not qualified: historical fixture has technical reviews but no clinical releases"
    );
}
