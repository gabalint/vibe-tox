//! HL7 orders received from a clinic's EMR through a folder (ticket #4555 step 3c;
//! REQ-INT-002, REQ-INT-003, REQ-ORD-001): a message waits until its patient is registered
//! with the identifier it carries, then becomes an order in Received status; an exact
//! replay has no effect; a message from another sending facility moves to Failed. Every
//! read file moves to Processed or Failed › year › month. Synthetic data only.

use std::fs;
use std::path::{Path, PathBuf};

use lis_core::Phi;
use lis_engine::{
    CreatePartnerConnectionInput, GetOrderInput, OrderStatus, PartnerConnectionDetail, SourceKind,
};
use lis_identity::{DemographicsInput, IdentifierInput, RegisterSubject};
use lis_interfaces::partner::{
    ApprovePartnerMappings, DraftPartnerMapping, IntakeSettings, ListPartnerMessages, MappingKind,
    MappingValues, MessageView, PartnerConnectionId, PartnerConnectionSettings, PartnerMessageType,
    PartnerProtocol, ReceivedState, RecordPartnerDestinationCheck, Schedule, TransportOutcome,
    UnknownPatient,
};
use tempfile::TempDir;

use crate::m2_lab::Role;
use crate::m3_fixture::{Bench, Staff, Variant};
use crate::partner_contract::{approve_and_test, assert_passed, draft_contract};

pub(super) const SENDER: &str = "SYN-EMR-FAC";

/// A synthetic OML^O21 for the fixture's practitioner and the panel's partner code.
pub(super) fn oml(sender: &str, control_id: &str) -> String {
    [
        format!("MSH|^~\\&|SYN-EMR|{sender}|VIBETOX LIS|LAB|20261104093000||OML^O21|{control_id}|P|2.5.1"),
        format!("PID|1||MRN-00042^^^{SENDER}^MR||Doe^Jane||19800101|F"),
        "ORC|NW|ORD-0001^SYN-EMR||||||||||1234567893^Synthetic".to_owned(),
        "OBR|1|ORD-0001^SYN-EMR||SYN-PANEL^Synthetic panel^L".to_owned(),
    ]
    .join("\r")
}

pub(super) struct Orders {
    pub(super) bench: Bench,
    pub(super) input: TempDir,
    processed: TempDir,
    failed: TempDir,
    pub(super) connection: PartnerConnectionDetail,
}

impl Orders {
    /// A connection that reads: checked, with its contract approved and tests passed.
    pub(super) async fn start() -> Self {
        let mut orders = Self::checked().await;
        orders.connection = accept(&orders.bench, orders.connection.connection.id).await;
        orders
    }

    /// A connection mapped and checked, without an interface contract yet.
    pub(super) async fn checked() -> Self {
        let bench = Bench::ready(Variant::Panel).await;
        let (input, processed, failed) = (folder(), folder(), folder());
        bench.sign_in(Staff::Desk(Role::Admin)).await;
        let engine = bench.lab().engine();
        let mut handles = Vec::new();
        for chosen in [&input, &processed, &failed] {
            let picked = engine
                .choose_partner_folder(Some(chosen.path().to_owned()))
                .await
                .unwrap()
                .unwrap();
            handles.push(picked.folder_choice);
        }
        let create = CreatePartnerConnectionInput {
            settings: settings(),
            folder_choice: Some(handles[0]),
            processed_folder_choice: Some(handles[1]),
            failed_folder_choice: Some(handles[2]),
            password: None,
            active: true,
        };
        let connection = checked(&bench, create).await;
        Self {
            bench,
            input,
            processed,
            failed,
            connection,
        }
    }

    /// Reads the folder once its `check every` has passed.
    pub(super) async fn read(&self) {
        self.bench.lab().t().advance(30);
        self.bench.lab().engine().read_partner_orders().await;
    }

    pub(super) fn states(&self) -> Vec<(String, String, Option<String>)> {
        let conn = self.bench.lab().t().conn();
        let mut statement = conn
            .prepare(
                "SELECT file_name, state, order_id FROM ifc_partner_received \
                 ORDER BY received_at, file_name",
            )
            .unwrap();
        statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))
            .unwrap()
            .collect::<Result<_, _>>()
            .unwrap()
    }
}

/// A connection that reads: [`checked`], then [`accept`]ed.
pub(super) async fn connect(
    bench: &Bench,
    create: CreatePartnerConnectionInput,
) -> PartnerConnectionDetail {
    let id = checked(bench, create).await.connection.id;
    accept(bench, id).await
}

/// The administrator adds the connection and maps the clinic's `SYN-PANEL` to the panel,
/// first line; a second administrator approves the mapping and records the destination
/// check. The second administrator stays signed in.
pub(super) async fn checked(
    bench: &Bench,
    create: CreatePartnerConnectionInput,
) -> PartnerConnectionDetail {
    let created = bench
        .lab()
        .engine()
        .create_partner_connection(create)
        .await
        .unwrap();
    let id = created.connection.id;
    bench
        .lab()
        .engine()
        .draft_partner_mapping(DraftPartnerMapping {
            connection_id: id,
            kind: MappingKind::Test,
            lis_id: bench.lab().orderables.panel.0.cast(),
            mapping: Some(MappingValues {
                partner_code: "SYN-PANEL".to_owned(),
                loinc: None,
                ucum_unit: None,
                sequence_role: Some("first_line".to_owned()),
            }),
        })
        .await
        .unwrap();
    bench.sign_in(Staff::Racer).await;
    let engine = bench.lab().engine();
    engine
        .approve_partner_mappings(ApprovePartnerMappings {
            connection_id: id,
            expected_revision: revision(bench, id).await,
            reason: "Synthetic: order codes confirmed with the clinic".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap();
    engine
        .record_partner_destination_check(RecordPartnerDestinationCheck {
            connection_id: id,
            expected_revision: revision(bench, id).await,
            method: "Synthetic call-back to the clinic's IT".to_owned(),
            self_approval: false,
        })
        .await
        .unwrap()
}

/// The administrator drafts the interface contract; a second administrator approves it and
/// runs the acceptance tests, which pass.
pub(super) async fn accept(bench: &Bench, id: PartnerConnectionId) -> PartnerConnectionDetail {
    bench.sign_in(Staff::Desk(Role::Admin)).await;
    draft_contract(bench.lab().engine(), id).await;
    bench.sign_in(Staff::Racer).await;
    assert_passed(&approve_and_test(bench.lab().engine(), id).await);
    bench
        .lab()
        .engine()
        .get_partner_connection(lis_engine::PartnerConnectionInput { connection_id: id })
        .await
        .unwrap()
}

pub(super) async fn revision(bench: &Bench, connection_id: PartnerConnectionId) -> i64 {
    let input = lis_engine::PartnerConnectionInput { connection_id };
    let engine = bench.lab().engine();
    engine
        .get_partner_connection(input)
        .await
        .unwrap()
        .connection
        .revision
}

fn folder() -> TempDir {
    tempfile::tempdir().unwrap()
}

pub(super) fn settings() -> PartnerConnectionSettings {
    PartnerConnectionSettings {
        name: "HL7 Orders - Synthetic clinic".to_owned(),
        partner: "SYN-CLIENT".to_owned(),
        message_type: PartnerMessageType::Hl7Orders,
        protocol: PartnerProtocol::Folder,
        rules: Vec::new(),
        schedule: Schedule::Immediately,
        file_name_pattern: String::new(),
        sftp: None,
        intake: Some(IntakeSettings {
            check_every_seconds: 10,
            accept_from: SENDER.to_owned(),
            unknown_patient: UnknownPatient::Hold,
            remote_processed_folder: None,
            remote_failed_folder: None,
        }),
        mllp: None,
        mllp_listener: None,
    }
}

/// Every file under `root`, in its year and month subfolders.
fn moved(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for year in fs::read_dir(root).unwrap() {
        for month in fs::read_dir(year.unwrap().path()).unwrap() {
            for file in fs::read_dir(month.unwrap().path()).unwrap() {
                files.push(file.unwrap().path());
            }
        }
    }
    files.sort();
    files
}

#[tokio::test]
async fn an_emr_order_waits_for_its_patient_then_arrives_as_received() {
    let orders = Orders::start().await;
    let input = orders.input.path();
    fs::write(input.join("clinic_0001.hl7"), oml(SENDER, "MSG-1")).unwrap();
    fs::write(input.join("clinic_0002.hl7.tmp"), "not complete").unwrap();
    orders.read().await;
    assert_eq!(
        orders.states(),
        [(
            "clinic_0001.hl7".to_owned(),
            "waiting_patient".to_owned(),
            None
        )]
    );
    // The file still written as .tmp stays; the read one moved to Processed › year › month.
    assert!(input.join("clinic_0002.hl7.tmp").exists());
    assert!(!input.join("clinic_0001.hl7").exists());
    assert_eq!(moved(orders.processed.path()).len(), 1);
    logs_show_it_waiting(&orders).await;

    // An exact replay has no effect, and moves to Processed too.
    fs::write(input.join("clinic_0001_again.hl7"), oml(SENDER, "MSG-1")).unwrap();
    // Another sending facility is refused.
    fs::write(input.join("other.hl7"), oml("OTHER-FAC", "MSG-9")).unwrap();
    orders.read().await;
    let states = orders.states();
    assert!(states.contains(&(
        "clinic_0001_again.hl7".to_owned(),
        "duplicate".to_owned(),
        None
    )));
    assert!(states.contains(&("other.hl7".to_owned(), "rejected".to_owned(), None)));
    assert_eq!(moved(orders.processed.path()).len(), 2);
    assert_eq!(moved(orders.failed.path()).len(), 1);

    // Once the patient is registered with the identifier, the next reading makes the order.
    register_patient(&orders).await;
    orders.read().await;
    let created = orders.states()[0].clone();
    assert_eq!(created.1, "order_created");
    let order_id = created.2.unwrap().parse().unwrap();
    orders.bench.sign_in(Staff::Desk(Role::Accessioner)).await;
    let order = orders
        .bench
        .lab()
        .engine()
        .get_order(GetOrderInput { order_id })
        .await
        .unwrap();
    assert_eq!(order.status, OrderStatus::Received);
    assert_eq!(order.source_kind, SourceKind::Hl7Interface);
    assert_eq!(order.placer_order_id.as_deref(), Some("ORD-0001"));
    assert_eq!(order.items.len(), 1);
}

/// Logs list the message as received, waiting for its patient, with where the file moved;
/// the connection counts it as waiting.
async fn logs_show_it_waiting(orders: &Orders) {
    orders.bench.sign_in(Staff::Desk(Role::Admin)).await;
    let engine = orders.bench.lab().engine();
    let page = engine
        .list_partner_messages(ListPartnerMessages {
            since: None,
            connection_id: Some(orders.connection.connection.id),
            view: MessageView::All,
            search: None,
            hide_tests: false,
            row_ids: Vec::new(),
        })
        .await
        .unwrap();
    let [row] = page.rows.as_slice() else {
        panic!("one row expected");
    };
    assert_eq!(row.transport, TransportOutcome::Received);
    assert_eq!(row.received, Some(ReceivedState::WaitingPatient));
    assert_eq!(row.source_label.as_deref(), Some("ORD-0001"));
    assert!(
        row.written_path
            .as_deref()
            .is_some_and(|path| path.ends_with("clinic_0001.hl7"))
    );
    let listed = engine.list_partner_connections().await.unwrap();
    assert_eq!(listed[0].waiting, 1);
}

pub(super) async fn register_patient(orders: &Orders) {
    orders.bench.sign_in(Staff::Desk(Role::Accessioner)).await;
    orders
        .bench
        .lab()
        .engine()
        .register_subject(RegisterSubject {
            demographics: DemographicsInput {
                family_name: Phi::new("Doe".to_owned()),
                given_names: Some(Phi::new("Jane".to_owned())),
                birth_date: Phi::new("1980-01-01".to_owned()),
                administrative_sex: None,
            },
            source: "Synthetic clinic registration".to_owned(),
            identifiers: vec![IdentifierInput {
                assigning_authority: SENDER.to_owned(),
                identifier_type: "MR".to_owned(),
                value: Phi::new("MRN-00042".to_owned()),
            }],
            acknowledged_candidates: Vec::new(),
        })
        .await
        .unwrap();
}
