//! Crashes and lost files around a release (`docs/planning/m3-design-brief.md`
//! §3.1, §3.6; NFR-01, NFR-07, REQ-RPT-001). Synthetic data only.

use std::path::Path;
use std::time::Duration;

use lis_core::ErrorCode;
use lis_engine::SetInstrumentEnabled;
use lis_reporting::CandidateState;
use rusqlite::Connection;

use crate::m2_lab::Role;
use crate::m2_steps::gate_codes;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::m3_release_races::released_events;

/// Disables the analyzer: its listener thread would otherwise keep the
/// dropped engine's database open, which a real crash cannot do.
async fn disable_analyzer(bench: &Bench) {
    let lab = bench.desk().await;
    lab.sign_in(Role::Admin).await;
    let e = lab.engine();
    let instruments = e.list_instruments().await.unwrap();
    let analyzer = instruments.iter().find(|i| i.id == lab.instrument).unwrap();
    e.set_instrument_enabled(SetInstrumentEnabled {
        instrument_id: analyzer.id,
        expected_revision: analyzer.revision,
        enabled: false,
    })
    .await
    .unwrap();
}

/// The state of candidate `id`, read from its stored row.
fn stored_state(bench: &Bench, id: &str) -> String {
    bench
        .lab()
        .t()
        .conn()
        .query_row(
            "SELECT state FROM rpt_candidates WHERE id = ?1",
            [id],
            |row| row.get(0),
        )
        .unwrap()
}

/// Waits until a candidate is `rendering`, then holds the write lock so its
/// second transaction cannot commit. Runs on a blocking thread.
fn lock_while_rendering(database: &Path) -> Connection {
    let conn = Connection::open(database).unwrap();
    for _ in 0..60_000 {
        let rendering: i64 = conn
            .query_row(
                "SELECT count(*) FROM rpt_candidates WHERE state = 'rendering'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        if rendering > 0 {
            conn.execute_batch("BEGIN IMMEDIATE").unwrap();
            return conn;
        }
        std::thread::sleep(Duration::from_micros(200));
    }
    panic!("no candidate reached rendering");
}

/// §3.6 crash case 1. T1 commits, the PDF renders and may be stored, but
/// T2 never commits: the state a crash after T1 or after the artifact write
/// leaves. After a restart the candidate is still `rendering`; the next
/// prepare supersedes it and no revision exists.
#[tokio::test]
async fn a_crash_before_the_candidate_is_ready_leaves_no_revision() {
    let bench = Bench::ready(Variant::Panel).await;
    disable_analyzer(&bench).await;
    bench.sign_in(Staff::Releaser).await;
    let database = bench.lab().t().conn().path().unwrap().to_owned();
    let watcher = tokio::task::spawn_blocking(move || lock_while_rendering(Path::new(&database)));
    let (prepared, lock) = tokio::join!(bench.prepare(None), watcher);
    // The engine gave up on T2 and on recording the failure; nothing is pending.
    assert_eq!(prepared.unwrap_err().code(), ErrorCode::Internal);
    let stuck: String = lock
        .unwrap()
        .query_row("SELECT id FROM rpt_candidates", [], |row| row.get(0))
        .unwrap();

    let bench = bench.crash_and_restart().await;
    assert_eq!(stored_state(&bench, &stuck), "rendering");
    bench.sign_in(Staff::Releaser).await;
    let next = bench.prepare(None).await.unwrap();
    assert_eq!(next.state, CandidateState::Ready);
    assert_eq!(stored_state(&bench, &stuck), "superseded");
    assert_eq!(bench.count("rpt_revisions"), 0);
}

/// §3.6 crash case 2: a crash at any point of the final transaction leaves
/// either the revision, its obligation and its `report.release` event, or
/// none of them.
#[tokio::test]
async fn a_crash_during_release_leaves_all_of_it_or_none() {
    for polls in [0, 1, 2, 4, 8, 16, 64] {
        let bench = Bench::ready(Variant::Panel).await;
        disable_analyzer(&bench).await;
        bench.sign_in(Staff::Releaser).await;
        let candidate = bench.prepare(None).await.unwrap();
        tokio::select! {
            biased;
            _ = bench.release(&candidate) => {}
            () = yields(polls) => {}
        }
        // A write queued behind any release work the dropped call left:
        // the restart then never waits on the writer. A release split over
        // two transactions would still show half of it here.
        bench.lab().engine().lock_session().await.unwrap();
        let bench = bench.crash_and_restart().await;
        let stored = (
            bench.count("rpt_revisions"),
            bench.count("rpt_delivery_obligations"),
            released_events(&bench).len(),
        );
        assert!(stored == (0, 0, 0) || stored == (1, 1, 1), "{stored:?}");
    }
}

async fn yields(count: usize) {
    for _ in 0..count {
        tokio::task::yield_now().await;
    }
}

/// §3.4 row 6 and §3.6: an issued file deleted before release refuses the
/// release and records a preservation incident (NFR-07).
#[tokio::test]
async fn a_release_whose_file_is_missing_is_refused_with_an_incident() {
    let bench = Bench::ready(Variant::Panel).await;
    bench.sign_in(Staff::Releaser).await;
    let candidate = bench.prepare(None).await.unwrap();
    let sha256 = candidate.artifact_sha256.clone().unwrap();
    std::fs::remove_file(bench.artifact_path(&sha256)).unwrap();
    let refused = bench.release(&candidate).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Conflict, "{refused:?}");
    assert!(gate_codes(&refused).contains(&"artifact_missing".to_owned()));
    assert_eq!(bench.count("rpt_preservation_incidents"), 1);
    assert_eq!(bench.count("rpt_revisions"), 0);
    assert!(released_events(&bench).is_empty());
}
