//! Read-only measurements against retained synthetic history snapshots.
#![cfg(test)]

use std::fs;
use std::path::Path;
use std::time::{Duration, Instant};

/// Summary costs are independent of the visible ready page. Run once on a retained
/// accumulated-history snapshot before prescribing an optimization or infrastructure.
#[test]
#[ignore = "read-only accumulated-history owner summary measurement"]
fn remediation_retained_history_review_summaries() {
    let path = std::env::var("VIBE_TOX_CAPACITY_DATABASE").unwrap();
    let deadline = Instant::now() + Duration::from_secs(300);
    while !snapshot_ready(&path) {
        assert!(
            Instant::now() < deadline,
            "retained snapshot was not published within five minutes"
        );
        std::thread::sleep(Duration::from_millis(200));
    }
    let conn =
        rusqlite::Connection::open_with_flags(&path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    configure_statement_cache(&conn);
    for (label, sql) in [
        (
            "current-result candidate specimens",
            "SELECT count(DISTINCT r.specimen_id) FROM res_result_revisions r JOIN res_results x ON x.current_revision_id=r.id",
        ),
        (
            "previously-reviewed candidate specimens",
            "SELECT count(DISTINCT r.specimen_id) FROM res_result_revisions r JOIN res_review_items i ON i.result_revision_id=r.id",
        ),
    ] {
        let count: i64 = conn.query_row(sql, [], |row| row.get(0)).unwrap();
        println!("{label}: {count}");
    }
    let stale = timed_owner_summary(&conn, "review_stale", lis_results::review_stale_queue);
    let blocked = timed_owner_summary(&conn, "review_blocked", lis_results::review_blocked_queue);
    assert!(
        stale && blocked,
        "owner summary exceeded deadline or returned an error; inspect measured output"
    );
}

fn timed_owner_summary(
    conn: &rusqlite::Connection,
    name: &str,
    query: fn(
        &rusqlite::Connection,
        u32,
    ) -> Result<Vec<lis_results::ResultReviewQueueItem>, lis_results::ResultError>,
) -> bool {
    let interrupt = conn.get_interrupt_handle();
    let (done, stop) = std::sync::mpsc::channel();
    let deadline = std::thread::spawn(move || {
        if stop.recv_timeout(Duration::from_secs(120)).is_err() {
            interrupt.interrupt();
        }
    });
    let started = Instant::now();
    let rows = query(conn, 501);
    let elapsed = started.elapsed();
    let _ = done.send(());
    deadline.join().unwrap();
    match rows {
        Ok(rows) => {
            println!(
                "readonly owner {name} summary: {} rows; {} ms; native domain query only",
                rows.len(),
                millis(elapsed)
            );
            true
        }
        Err(error) => {
            println!(
                "readonly owner {name} summary FAILED: {error:?}; {} ms; 120 s interrupt deadline",
                millis(elapsed)
            );
            false
        }
    }
}

fn snapshot_ready(path: &str) -> bool {
    if !Path::new(path).exists() {
        return false;
    }
    match std::env::var("VIBE_TOX_CAPACITY_READY_LOG") {
        Ok(log) => fs::read_to_string(log).is_ok_and(|text| {
            text.contains(&format!("retained synthetic SQLite snapshot: {path}"))
        }),
        Err(_) => true,
    }
}

/// Keep the shared Cargo test slot while measuring the preserved pre-fix binary.
#[test]
#[ignore = "requires the explicitly pinned pre-optimization measurement binary"]
fn remediation_pinned_baseline_review_summaries() {
    let binary = std::env::var("VIBE_TOX_CAPACITY_BASELINE_BINARY").unwrap();
    let status = std::process::Command::new(binary)
        .args([
            "remediation_retained_history_review_summaries",
            "--ignored",
            "--nocapture",
        ])
        .status()
        .unwrap();
    assert!(
        status.success(),
        "pinned baseline measurement failed; retain its timings/errors"
    );
}

#[test]
#[ignore = "read-only retained-history bounded ready-page measurement"]
fn remediation_retained_history_review_pages() {
    let path = std::env::var("VIBE_TOX_CAPACITY_DATABASE").unwrap();
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    configure_statement_cache(&conn);
    let mut after = None;
    let mut total = 0;
    loop {
        let started = Instant::now();
        let mut rows = lis_results::review_work_queue_page(
            &conn,
            lis_results::ReviewQueueFilter::Ready,
            50,
            after,
        )
        .unwrap();
        println!(
            "readonly owner ready page: {} rows incl lookahead, {} ms; native domain only",
            rows.len(),
            millis(started.elapsed())
        );
        let more = rows.len() > 50;
        rows.truncate(50);
        total += rows.len();
        if !more {
            break;
        }
        let last = rows.last().unwrap();
        after = Some(lis_results::ReviewQueueCursor {
            since: last.since,
            specimen_id: last.specimen_id,
        });
    }
    println!("readonly owner ready pages reached {total} pending specimens");
    assert_eq!(
        total, 95,
        "the retained history phase has 95 pending specimens"
    );
}

#[test]
#[ignore = "read-only individual result-status and gate profiling"]
fn remediation_retained_history_result_components() {
    let path = std::env::var("VIBE_TOX_CAPACITY_DATABASE").unwrap();
    let mut conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    let tx = conn.transaction().unwrap();
    let revisions: Vec<lis_results::ResultRevisionId> = tx
        .prepare(
            "SELECT result.current_revision_id FROM res_results result \
             JOIN res_result_revisions current ON current.id=result.current_revision_id \
             WHERE current.specimen_id=(SELECT specimen_id FROM res_result_revisions LIMIT 1)",
        )
        .unwrap()
        .query_map([], |row| row.get(0))
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    assert_eq!(revisions.len(), 12);
    for revision in revisions {
        let start = Instant::now();
        let status = lis_results::review_status(&tx, revision).unwrap();
        let status_time = start.elapsed();
        let start = Instant::now();
        let gates = lis_results::review_gates(&tx, revision).unwrap();
        println!(
            "result {revision}: status={status:?}, status {} ms, gates {} / {} ms; fresh public per-result readers in one read-only transaction",
            millis(status_time),
            gates.len(),
            millis(start.elapsed())
        );
    }
}

fn millis(duration: Duration) -> String {
    format!(
        "{}.{:03}",
        duration.as_millis(),
        duration.subsec_micros() % 1_000
    )
}

fn configure_statement_cache(conn: &rusqlite::Connection) {
    if let Ok(value) = std::env::var("VIBE_TOX_CAPACITY_STATEMENT_CACHE") {
        let capacity: usize = value.parse().unwrap();
        conn.set_prepared_statement_cache_capacity(capacity);
        println!("benchmark-only statement cache capacity: {capacity}; unchanged data/gates");
    }
}

#[test]
#[ignore = "read-only bounded-history request latency and exhaustive continuation"]
fn remediation_bounded_history_pages_reach_every_candidate() {
    let path = std::env::var("VIBE_TOX_CAPACITY_DATABASE").unwrap();
    let conn =
        rusqlite::Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
            .unwrap();
    configure_statement_cache(&conn);
    measure_bounded(&conn, lis_results::ReviewQueueFilter::Stale, 24_900);
    measure_bounded(&conn, lis_results::ReviewQueueFilter::Blocked, 24_995);
}

fn measure_bounded(
    conn: &rusqlite::Connection,
    filter: lis_results::ReviewQueueFilter,
    expected: usize,
) {
    let mut after = None;
    let mut cursors = std::collections::BTreeSet::new();
    let mut checked = 0;
    let mut times = Vec::new();
    let total = Instant::now();
    loop {
        let start = Instant::now();
        let page = lis_results::scan_review_work_queue(conn, filter, 50, after).unwrap();
        times.push(start.elapsed());
        assert!(
            page.items.is_empty(),
            "healthy retained history has no matching work"
        );
        assert!(
            page.scanned <= 500,
            "one request must bound candidate checks"
        );
        checked += page.scanned;
        after = page.next;
        if let Some(cursor) = after {
            assert!(page.scanned > 0);
            assert!(cursors.insert((cursor.since, cursor.specimen_id)));
        } else {
            break;
        }
        assert!(times.len() <= expected.div_ceil(500));
    }
    assert_eq!(checked, expected);
    times.sort();
    println!(
        "bounded {filter:?}: {} requests, {checked} candidates, 0 matches; p50 {} ms / p95 {} ms / max {} ms; total {} ms; native domain requests, not full IPC",
        times.len(),
        millis(times[times.len().div_ceil(2) - 1]),
        millis(times[(times.len() * 95).div_ceil(100) - 1]),
        millis(*times.last().unwrap()),
        millis(total.elapsed()),
    );
}
