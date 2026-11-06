use super::*;

/// The tables and published views the list reads, without their owners' constraints.
const SCHEMA: &str = "CREATE TABLE res_import_attempts(id TEXT PRIMARY KEY,receipt_id TEXT,attempt_no INTEGER,instrument_id TEXT,outcome TEXT,failure_code TEXT,rows_total INTEGER,rows_new INTEGER,rows_duplicate INTEGER,rows_reprocessed INTEGER,rows_qc INTEGER,rows_quarantined INTEGER,rows_not_result INTEGER,rows_set_aside INTEGER,rows_retained INTEGER,processed_at TEXT);
    CREATE TABLE ifc_v_receipt_origins(receipt_id TEXT,origin TEXT);
    CREATE TABLE ifc_v_receipt_arrivals(receipt_id TEXT,received_at TEXT,file_name TEXT);
    CREATE TABLE res_measurements(import_attempt_id TEXT,position_id TEXT);
    CREATE TABLE res_quarantine_items(import_attempt_id TEXT,sample_label TEXT,position_id TEXT);
    CREATE TABLE res_set_aside_rows(import_attempt_id TEXT,sample_label TEXT);
    CREATE TABLE ana_v_positions(position_id TEXT,sample_label TEXT,container_id TEXT);
    CREATE TABLE spc_v_containers(container_id TEXT,label_code TEXT,accession_number TEXT);
    CREATE TABLE ana_v_qc_receipt_positions(receipt_id TEXT,position_id TEXT);";

const NOON: &str = "2026-11-05T12:00:00.000000000Z";

fn attempt(
    conn: &Connection,
    instrument: InstrumentId,
    outcome: &str,
    received: &str,
    processed: &str,
) -> Result<(ImportAttemptId, ReceiptId), rusqlite::Error> {
    let (id, receipt) = (ImportAttemptId::generate(), ReceiptId::generate());
    conn.execute(
        "INSERT INTO res_import_attempts VALUES(?1,?2,1,?3,?4,NULL,1,1,0,0,0,0,0,0,0,?5)",
        params![id, receipt, instrument, outcome, processed],
    )?;
    conn.execute(
        "INSERT INTO ifc_v_receipt_arrivals VALUES(?1,?2,NULL)",
        params![receipt, received],
    )?;
    Ok((id, receipt))
}

fn ids(page: &[ImportAttemptSummary]) -> Vec<ImportAttemptId> {
    let mut found: Vec<_> = page.iter().map(|row| row.import_attempt_id).collect();
    found.sort();
    found
}

#[test]
fn import_pages_reach_equal_times_and_clock_rollback_and_find_old_receipts()
-> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    let instrument = InstrumentId::generate();
    let mut old_receipt = None;
    let mut expected = Vec::new();
    for index in 0..503 {
        // The last persisted import follows a rolled-back clock, and is outside page 1.
        let time = if index == 502 {
            "2020-01-01T09:00:00.000000000Z"
        } else {
            "2026-10-14T09:00:00.000000000Z"
        };
        let (id, receipt) = attempt(&conn, instrument, "processed", time, time)?;
        if index == 502 {
            old_receipt = Some(receipt);
        }
        expected.push((time.to_owned(), id));
    }
    expected.sort_by(|a, b| b.cmp(a));
    let filter = ImportAttemptFilter {
        instrument_id: Some(instrument),
        ..ImportAttemptFilter::default()
    };
    let mut cursor = None;
    let mut seen = Vec::new();
    loop {
        let page = list_import_attempts_page(&conn, &filter, 200, None, cursor)?;
        seen.extend(page.iter().take(200).map(|row| row.import_attempt_id));
        if page.len() <= 200 {
            break;
        }
        cursor = Some(page[199].import_attempt_id);
    }
    assert_eq!(seen, expected.iter().map(|(_, id)| *id).collect::<Vec<_>>());
    let old_receipt = old_receipt.expect("old receipt was written");
    let exact =
        latest_import_attempt_for_receipt(&conn, old_receipt)?.expect("old receipt is reachable");
    assert_eq!(exact.import_attempt_id, *seen.last().unwrap());
    assert!(latest_import_attempt_for_receipt(&conn, ReceiptId::generate())?.is_none());
    let any = ImportAttemptFilter::default();
    assert!(list_import_attempts_page(&conn, &any, 0, None, None).is_err());
    assert!(list_import_attempts_page(&conn, &any, 501, None, None).is_err());
    Ok(())
}

#[test]
fn outcome_and_received_period_filter_before_the_page_is_cut()
-> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    let instrument = InstrumentId::generate();
    let day = "2026-11-05T00:00:00.000000000Z";
    let next_day = "2026-11-06T00:00:00.000000000Z";
    let (held, _) = attempt(&conn, instrument, "quarantined", NOON, NOON)?;
    let (done, _) = attempt(&conn, instrument, "processed", NOON, NOON)?;
    // Received the day before, retried on the day: the received day decides.
    let (earlier, _) = attempt(
        &conn,
        instrument,
        "quarantined",
        "2026-11-04T23:59:59.999999999Z",
        NOON,
    )?;
    let page =
        |filter: ImportAttemptFilter| list_import_attempts_page(&conn, &filter, 10, None, None);
    let quarantined = page(ImportAttemptFilter {
        outcome: Some(ImportOutcome::Quarantined),
        ..ImportAttemptFilter::default()
    })?;
    assert_eq!(ids(&quarantined), ids_of(&[held, earlier]));
    let on_the_day = page(ImportAttemptFilter {
        received_from: Some(UtcTimestamp::parse(day)?),
        received_before: Some(UtcTimestamp::parse(next_day)?),
        ..ImportAttemptFilter::default()
    })?;
    assert_eq!(ids(&on_the_day), ids_of(&[held, done]));
    assert_eq!(on_the_day[0].received_at, UtcTimestamp::parse(NOON)?);
    let both = page(ImportAttemptFilter {
        instrument_id: Some(instrument),
        outcome: Some(ImportOutcome::Quarantined),
        received_from: Some(UtcTimestamp::parse(day)?),
        ..ImportAttemptFilter::default()
    })?;
    assert_eq!(ids(&both), ids_of(&[held]));
    let backwards = page(ImportAttemptFilter {
        received_from: Some(UtcTimestamp::parse(next_day)?),
        received_before: Some(UtcTimestamp::parse(day)?),
        ..ImportAttemptFilter::default()
    });
    assert!(
        matches!(backwards, Err(ResultError::Invalid(error)) if error.field == "receivedBefore")
    );
    Ok(())
}

fn ids_of(list: &[ImportAttemptId]) -> Vec<ImportAttemptId> {
    let mut found = list.to_vec();
    found.sort();
    found
}

#[test]
fn search_finds_labels_accessions_and_file_names_in_any_row_kind()
-> Result<(), Box<dyn std::error::Error>> {
    let conn = Connection::open_in_memory()?;
    conn.execute_batch(SCHEMA)?;
    let instrument = InstrumentId::generate();
    let mut made = Vec::new();
    for _ in 0..6 {
        made.push(attempt(&conn, instrument, "processed", NOON, NOON)?);
    }
    let [measured, quarantined, set_aside, qc, file, _] = made.as_slice() else {
        unreachable!("six attempts were made");
    };
    conn.execute_batch(
        "INSERT INTO ana_v_positions VALUES('p1','C00000009','c1'),('p2','QC-LCMS-1',NULL);
         INSERT INTO spc_v_containers VALUES('c1','C00000009','A-2026-0042');",
    )?;
    conn.execute("INSERT INTO res_measurements VALUES(?1,'p1')", [measured.0])?;
    conn.execute(
        "INSERT INTO res_quarantine_items VALUES(?1,'DUPTEST01',NULL)",
        [quarantined.0],
    )?;
    conn.execute(
        "INSERT INTO res_set_aside_rows VALUES(?1,'Blank-7')",
        [set_aside.0],
    )?;
    conn.execute(
        "INSERT INTO ana_v_qc_receipt_positions VALUES(?1,'p2')",
        [qc.1],
    )?;
    conn.execute(
        "UPDATE ifc_v_receipt_arrivals SET file_name = 'Batch 12.csv' WHERE receipt_id = ?1",
        [file.1],
    )?;
    let found = |text: &str| -> Result<Vec<ImportAttemptId>, ResultError> {
        let filter = ImportAttemptFilter {
            search: Some(text.to_owned()),
            ..ImportAttemptFilter::default()
        };
        Ok(ids(&list_import_attempts_page(
            &conn, &filter, 10, None, None,
        )?))
    };
    assert_eq!(found("c00000009")?, ids_of(&[measured.0]));
    assert_eq!(found(" A-2026-0042 ")?, ids_of(&[measured.0]));
    assert_eq!(found("duptest")?, ids_of(&[quarantined.0]));
    assert_eq!(found("BLANK-7")?, ids_of(&[set_aside.0]));
    assert_eq!(found("QC-LCMS-1")?, ids_of(&[qc.0]));
    assert_eq!(found("batch 12")?, ids_of(&[file.0]));
    assert!(found("nothing like it")?.is_empty());
    assert_eq!(found("   ")?.len(), 6, "blank text is no search");
    assert!(found(&"x".repeat(101)).is_err());
    Ok(())
}
