//! Label jobs: print, reprint and delivery (REQ-SPC-001).
//!
//! Printers must sit on the laboratory network, so these tests never reach a
//! printer: delivery is checked against a printer row whose address was
//! changed behind the store's back to loopback, which the transport refuses
//! before connecting. Exact bytes over TCP are covered by `lis-labels`.

use lis_core::{Capability, ErrorCode, Principal};
use lis_labels::{LabelTemplateRef, PrinterDestinations};
use lis_specimens::{
    ConfigureLabelPrinterInput, ContainerId, LabelDelivery, LabelJobId, LabelJobKind, LabelJobView,
    LabelOutcome, PrintLabelsInput, PrinterId, PrinterTransport, SpecimenError,
};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::{Fixture, STAFF, code_of, counts, fixture, last_event, read, run, run_as};

/// A printer at 192.168.0.9:`port`; nothing in these tests connects to it.
fn printer(f: &Fixture, port: u16, active: bool) -> PrinterId {
    let input = ConfigureLabelPrinterInput {
        printer_id: None,
        expected_revision: None,
        code: format!("bench-{port}"),
        name: "Synthetic bench printer".to_owned(),
        transport: PrinterTransport::Tcp,
        ip_address: Some("192.168.0.9".to_owned()),
        port: Some(port),
        queue: None,
        active,
    };
    run(&f.t, &[Capability::ManageConfiguration], move |ctx| {
        lis_specimens::configure_printer(ctx, input, PrinterDestinations::LabNetwork)
    })
    .unwrap()
    .printer_id
}

fn print(
    f: &Fixture,
    containers: Vec<ContainerId>,
    printer_id: PrinterId,
    kind: LabelJobKind,
    reason: Option<&str>,
) -> Result<Vec<LabelJobView>, SpecimenError> {
    let input = PrintLabelsInput {
        container_ids: containers,
        printer_id,
        kind,
        reason: reason.map(str::to_owned),
    };
    run(&f.t, STAFF, move |ctx| {
        lis_specimens::print_labels(ctx, input)
    })
}

/// What the label worker does for one job: read, deliver, record.
fn work(f: &Fixture, job: &LabelJobView) -> Option<LabelDelivery> {
    let job_id = job.job_id;
    let request = read(&f.t, move |conn| lis_specimens::label_request(conn, job_id))?;
    let delivery = lis_specimens::deliver_label(
        &request,
        PrinterDestinations::LabNetwork,
        lis_labels::Marking::Unmarked,
    );
    let recorded = delivery.clone();
    run_as(
        &f.t,
        Principal::system("label-printer", [Capability::Accession]),
        move |ctx| lis_specimens::record_label_outcome(ctx, recorded),
    )
    .unwrap();
    Some(delivery)
}

/// A printer whose stored address was set to loopback outside the store,
/// as a row saved before the destination rule existed would be.
fn loopback_printer(f: &Fixture) -> PrinterId {
    let printer_id = printer(f, 9100, true);
    let changed = crate::raw(f)
        .execute(
            "UPDATE spc_printers SET ip_address = '127.0.0.1' WHERE id = ?1",
            [printer_id.to_string()],
        )
        .unwrap();
    assert_eq!(changed, 1);
    printer_id
}

#[test]
fn a_label_is_queued_then_rendered_with_exact_bytes() {
    let f = fixture();
    let detail = f.accession(1);
    let container = &detail.specimens[0].containers[0];
    let printer_id = loopback_printer(&f);
    let jobs = print(
        &f,
        vec![container.container_id],
        printer_id,
        LabelJobKind::Print,
        None,
    )
    .unwrap();
    assert_eq!(jobs[0].outcome, None);
    let job_id = jobs[0].job_id;
    let outbox: (String, String, String) = read(&f.t, |conn| {
        Ok(conn.query_row(
            "SELECT topic, dedup_key, payload FROM store_outbox",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    });
    assert_eq!(outbox.0, lis_specimens::LABEL_PRINT_TOPIC);
    assert_eq!(outbox.1, job_id.to_string());
    let payload: serde_json::Value = serde_json::from_str(&outbox.2).unwrap();
    assert_eq!(payload, json!({ "labelJobId": job_id.to_string() }));
    assert_eq!(lis_specimens::label_job_id(&payload), Some(job_id));

    let delivery = work(&f, &jobs[0]).unwrap();
    assert_eq!(delivery.truncated, Some(false));
    let expected = lis_labels::render(
        &LabelTemplateRef {
            id: "specimen".to_owned(),
            version: 1,
        },
        &[
            ("container_id", "C00000001"),
            ("subject_name", "SYNTHETIC, Patient Alpha"),
            ("birth_date", "1980-01-01"),
            ("accession", "A26000001"),
            ("matrix", "syn-ur"),
        ],
    )
    .unwrap();
    assert_eq!(
        delivery.label_sha256,
        Some(hex::encode(Sha256::digest(&expected)))
    );
    assert_eq!(last_event(&f.t).0, "specimen.record_label_outcome");
    // A redelivered outbox item finds the outcome and sends nothing.
    assert!(work(&f, &jobs[0]).is_none());
}

#[test]
fn only_the_label_printer_system_component_records_outcomes() {
    let f = fixture();
    let delivery = LabelDelivery {
        job_id: LabelJobId::generate(),
        outcome: LabelOutcome::Failed,
        error_code: Some("synthetic_failure"),
        label_sha256: None,
        truncated: None,
    };
    for principal in [
        lis_store::testing::principal(&[Capability::Accession]),
        Principal::system("outbox-worker", [Capability::Accession]),
    ] {
        let before = counts(&f.t);
        let attempt = delivery.clone();
        let error = run_as(&f.t, principal, move |ctx| {
            lis_specimens::record_label_outcome(ctx, attempt)
        })
        .unwrap_err();
        assert_eq!(error.code(), ErrorCode::Forbidden);
        assert_eq!(counts(&f.t), before);
    }
}

#[test]
fn a_second_label_is_a_reprint_with_a_reason_and_the_same_code() {
    let f = fixture();
    let container = f.accession(1).specimens[0].containers[0].container_id;
    let printer_id = printer(&f, 9100, true);
    print(&f, vec![container], printer_id, LabelJobKind::Print, None).unwrap();
    let before = counts(&f.t);
    assert_eq!(
        code_of(print(
            &f,
            vec![container],
            printer_id,
            LabelJobKind::Print,
            None
        )),
        ErrorCode::Conflict
    );
    assert_eq!(
        code_of(print(
            &f,
            vec![container],
            printer_id,
            LabelJobKind::Reprint,
            None
        )),
        ErrorCode::Validation
    );
    assert_eq!(counts(&f.t), before);
    let reprint = print(
        &f,
        vec![container],
        printer_id,
        LabelJobKind::Reprint,
        Some("Label smudged"),
    )
    .unwrap();
    assert_eq!(reprint[0].kind, LabelJobKind::Reprint);
    let (action, record, reason) = last_event(&f.t);
    assert_eq!(action, "specimen.print_label");
    assert_eq!(record, Some(reprint[0].job_id.to_string()));
    assert_eq!(reason.as_deref(), Some("Label smudged"));
    let label_codes: Vec<String> = read(&f.t, move |conn| {
        Ok(conn
            .prepare(
                "SELECT v.label_code FROM spc_label_jobs j \
                 JOIN spc_v_containers v ON v.container_id = j.container_id ORDER BY j.id",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?)
    });
    assert_eq!(label_codes, ["C00000001", "C00000001"]);
}

#[test]
fn an_inactive_printer_is_refused() {
    let f = fixture();
    let container = f.accession(1).specimens[0].containers[0].container_id;
    let printer_id = printer(&f, 9100, false);
    assert_eq!(
        code_of(print(
            &f,
            vec![container],
            printer_id,
            LabelJobKind::Print,
            None
        )),
        ErrorCode::Conflict
    );
}

#[test]
fn a_printer_off_the_laboratory_network_is_a_recorded_failure_at_print_time() {
    let f = fixture();
    let container = f.accession(1).specimens[0].containers[0].container_id;
    let printer_id = loopback_printer(&f);
    let jobs = print(&f, vec![container], printer_id, LabelJobKind::Print, None).unwrap();
    let delivery = work(&f, &jobs[0]).unwrap();
    assert_eq!(delivery.outcome, LabelOutcome::Failed);
    assert_eq!(delivery.error_code, Some("destination_refused"));
    let recorded: (String, String) = read(&f.t, |conn| {
        Ok(conn.query_row(
            "SELECT outcome, error_code FROM spc_label_job_outcomes",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    assert_eq!(
        recorded,
        ("failed".to_owned(), "destination_refused".to_owned())
    );
    // One outcome per job.
    let again = delivery;
    assert!(matches!(
        run_as(
            &f.t,
            Principal::system("label-printer", [Capability::Accession]),
            move |ctx| lis_specimens::record_label_outcome(ctx, again),
        ),
        Err(SpecimenError::OutcomeRecorded)
    ));
}

#[test]
fn a_failed_label_stays_queued_until_a_reprint_is_requested() {
    let f = fixture();
    let container = f.accession(1).specimens[0].containers[0].container_id;
    let printer_id = loopback_printer(&f);
    let jobs = print(&f, vec![container], printer_id, LabelJobKind::Print, None).unwrap();
    work(&f, &jobs[0]).unwrap();
    let failed = read(&f.t, |conn| lis_specimens::failed_label_jobs(conn, 10));
    assert_eq!(failed.len(), 1);
    assert_eq!(
        (failed[0].job_id, failed[0].container_id),
        (jobs[0].job_id, container)
    );
    assert_eq!(failed[0].label_code, "C00000001");
    assert_eq!(failed[0].error_code, "destination_refused");
    print(
        &f,
        vec![container],
        printer_id,
        LabelJobKind::Reprint,
        Some("Synthetic: printer was off"),
    )
    .unwrap();
    assert!(read(&f.t, |conn| lis_specimens::failed_label_jobs(conn, 10)).is_empty());
}
