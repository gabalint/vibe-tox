//! Label printers that fail (REQ-SPC-004, REQ-INT-009; VAL-51): a printer that accepts
//! a connection and never reads, and one that is switched off, while an analyzer sends.
//! Expected outcomes: the validation plan's VAL-51 ("an offline printer leaves a visible
//! failed job, never a silent loss"; a reprint is attributable), the label outcome
//! vocabulary (`handed_off` is not proof of a printed label) and the outbox contract
//! (bounded work per item, so other topics keep moving).

use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use lis_analytical::RevisionIndicator;
use lis_core::ErrorCode;
use lis_engine::{GetAccessionInput, LabelJobView, PrintLabelsInput};
use lis_specimens::{LabelJobKind, LabelOutcome, PrinterId, PrinterTransport};

use crate::m2_lab::{Lab, Role};
use crate::m2_steps::JOURNEY;

/// A printer that accepts connections and never reads from them, like one whose buffer
/// is stuck; the connections stay open until the test ends.
fn silent_printer() -> (u16, mpsc::Sender<()>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let (stop, stopped) = mpsc::channel::<()>();
    thread::spawn(move || {
        let mut held: Vec<TcpStream> = Vec::new();
        listener.set_nonblocking(true).unwrap();
        while stopped.try_recv().is_err() {
            if let Ok((connection, _)) = listener.accept() {
                held.push(connection);
            }
            thread::sleep(Duration::from_millis(10));
        }
    });
    (port, stop)
}

/// A local port with nothing listening on it: a printer that is switched off.
fn switched_off_printer() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

async fn configure(lab: &Lab, code: &str, port: u16) -> PrinterId {
    lab.sign_in(Role::Admin).await;
    let input = lis_engine::ConfigureLabelPrinterInput {
        printer_id: None,
        expected_revision: None,
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        transport: PrinterTransport::Tcp,
        ip_address: Some("127.0.0.1".to_owned()),
        port: Some(port),
        queue: None,
        active: true,
    };
    lab.engine()
        .configure_printer(input)
        .await
        .unwrap()
        .printer_id
}

async fn print(lab: &Lab, index: usize, printer_id: PrinterId, kind: LabelJobKind) {
    lab.sign_in(Role::Accessioner).await;
    let input = PrintLabelsInput {
        container_ids: vec![lab.samples[index].container_id],
        printer_id,
        kind,
        reason: (kind == LabelJobKind::Reprint).then(|| "Synthetic jammed label".to_owned()),
    };
    lab.engine().print_labels(input).await.unwrap();
}

/// The label jobs of sample `index` once each has an outcome, waiting at most 30 s,
/// longer than the transport's 5 s connect and 10 s write limits.
async fn settled_jobs(lab: &Lab, index: usize) -> Vec<LabelJobView> {
    lab.sign_in(Role::Accessioner).await;
    let label_code = lab.samples[index].label_code.clone();
    let found = lab
        .engine()
        .find_container(lis_engine::FindContainerInput { label_code })
        .await
        .unwrap();
    let input = GetAccessionInput {
        accession_id: found.accession_id,
    };
    for _ in 0..1_200 {
        let detail = lab.engine().get_accession(input).await.unwrap();
        let jobs = detail.specimens[0].containers[0].label_jobs.clone();
        if jobs.iter().all(|job| job.outcome.is_some()) {
            return jobs;
        }
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    panic!("a label job never got an outcome");
}

/// A stuck printer and a switched-off printer each get a bounded, visible outcome; the
/// analyzer's message is imported meanwhile; the failed label is reprinted with a reason
/// and stays in the container's history.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn failing_printers_leave_visible_jobs_and_never_hold_up_analyzer_results() {
    let lab = Lab::start(RevisionIndicator::None).await;
    let (silent_port, stop_silent) = silent_printer();
    let silent = configure(&lab, "stuck", silent_port).await;
    let off = configure(&lab, "off", switched_off_printer()).await;

    print(&lab, 0, silent, LabelJobKind::Print).await;
    print(&lab, 1, off, LabelJobKind::Print).await;
    let imported = lab.import(JOURNEY).await;

    assert_eq!(imported.rows_new, 6, "{imported:?}");
    let stuck = settled_jobs(&lab, 0).await;
    assert_eq!(stuck[0].outcome, Some(LabelOutcome::HandedOff), "{stuck:?}");
    let failed = settled_jobs(&lab, 1).await;
    assert_eq!(failed[0].outcome, Some(LabelOutcome::Failed));
    assert_eq!(failed[0].error_code.as_deref(), Some("connection_refused"));

    lab.sign_in(Role::Accessioner).await;
    let unexplained = lab
        .engine()
        .print_labels(PrintLabelsInput {
            container_ids: vec![lab.samples[1].container_id],
            printer_id: silent,
            kind: LabelJobKind::Reprint,
            reason: None,
        })
        .await
        .unwrap_err();
    assert_eq!(unexplained.code(), ErrorCode::Validation);
    print(&lab, 1, silent, LabelJobKind::Reprint).await;
    let history = settled_jobs(&lab, 1).await;
    let kinds: Vec<_> = history.iter().map(|job| (job.kind, job.outcome)).collect();
    assert_eq!(
        kinds,
        [
            (LabelJobKind::Print, Some(LabelOutcome::Failed)),
            (LabelJobKind::Reprint, Some(LabelOutcome::HandedOff)),
        ]
    );
    stop_silent.send(()).unwrap();
}
