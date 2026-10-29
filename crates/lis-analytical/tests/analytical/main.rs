//! Integration tests on real SQLite files with the production pragmas. The
//! fixture builds the catalog, an analyzer and accepted, accessioned orders
//! through the owners' public commands; QC rows cite a real durable receipt
//! from the analyzer's TCP listener. All names, codes and identifiers are
//! synthetic.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod au_sources;
mod authorization;
mod continuity;
mod dependencies;
mod dependency_read;
mod deviations;
mod file_sources;
mod imports;
mod positions;
mod qc;
mod readiness;
mod runs;
mod separation;

use std::io::{Read, Write};
use std::net::{IpAddr, Ipv4Addr, TcpStream};
use std::sync::Arc;
use std::time::{Duration, Instant};

use lis_analytical::reexport::{RowDecision, SourceRow};
use lis_analytical::{
    AddPatientPositionInput, AddQcPositionInput, AnalyticalError, AttemptKind, CreateRunInput,
    InstrumentStatus, LotId, MaterialKind, PositionId, QcRole, QcRow, RegisterLotInput,
    RegisterSourceInput, RevisionIndicator, RunDetail, RunId, SCHEMA, SampleIdField,
    SetInstrumentStatusInput, SourceTestCodeInput,
};
use lis_catalog::{
    AnalyteId, ApproveMethodVersion, ApproveOrderableVersion, CodedName, ComponentInput,
    ComponentPurpose, CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    MethodAnalyte, MethodCertainty, MethodVersionContent, MethodVersionId, OrderableKind,
    OrderableVersionContent, OrderableVersionId, OutputScale, ThresholdRounding, ThresholdRule,
};
use lis_core::FixedClock;
use lis_core::{Capability, CorrelationId, DecimalText, ErrorCode, InstrumentId, Phi, Principal};
use lis_identity::{
    ClientId, CreateClient, DemographicsInput, PractitionerId, RegisterPractitioner,
    RegisterSubject, SubjectId,
};
use lis_interfaces::{ConnectionSettings, ListenerSupervisor, ReceiptId, RegisterInstrument};
use lis_orders::{
    AcceptOrderInput, AuthorityEvidenceInput, AuthorityEvidenceKind, BillingResponsibility,
    EvidenceInput, OrderId, OrderItemInput, ReflexAuthorization, RegisterOrderInput, SequenceRole,
    SourceKind, TemplateUseInput,
};
use lis_specimens::{
    AccessionDetail, CollectionInput, CollectionTimeInput, ContainerId, CreateAccessionInput,
    ReceiptDecision, RecordReceiptDecisionInput, SpecimenInput, VolumeKind,
};
use lis_store::testing::{local_port, principal, test_start};
use lis_store::{CommandContext, Database, StoreError, lint_schema};
use rusqlite::Connection;
use tempfile::TempDir;

pub(crate) const URINE: &str = "syn-ur";
pub(crate) const ORAL_FLUID: &str = "syn-of";
/// A lot usable throughout the tests (the fixed clock starts 2026-01-02).
pub(crate) const LATER: &str = "2027-12-31";

/// Everything an analyst, quality manager and configuration author may do here.
pub(crate) const STAFF: &[Capability] = &[
    Capability::PerformAnalysis,
    Capability::ManageQuality,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
];

/// One matrix's approved catalog: a two-analyte method and a test using it.
#[derive(Clone, Copy)]
pub(crate) struct Assay {
    pub(crate) method: MethodVersionId,
    pub(crate) analytes: [AnalyteId; 2],
    pub(crate) test: OrderableVersionId,
}

/// A database with every module up to analytical, a subject, a client, a
/// practitioner, urine and oral-fluid assays and one TCP analyzer.
pub(crate) struct Fixture {
    pub(crate) db: Arc<Database>,
    pub(crate) clock: Arc<FixedClock>,
    dir: TempDir,
    subject_id: SubjectId,
    client_id: ClientId,
    practitioner_id: PractitionerId,
    pub(crate) urine: Assay,
    pub(crate) oral_fluid: Assay,
    pub(crate) instrument: InstrumentId,
    port: u16,
}

pub(crate) const MODULES: [lis_core::ModuleSchema; 7] = [
    lis_auth::SCHEMA,
    lis_identity::SCHEMA,
    lis_catalog::SCHEMA,
    lis_interfaces::SCHEMA,
    lis_orders::SCHEMA,
    lis_specimens::SCHEMA,
    SCHEMA,
];

pub(crate) fn fixture() -> Fixture {
    fixture_on(&MODULES)
}

/// The fixture on `modules`, which hold every schema of [`MODULES`] at some version.
pub(crate) fn fixture_on(modules: &[lis_core::ModuleSchema]) -> Fixture {
    let dir = tempfile::tempdir().unwrap();
    let clock = Arc::new(FixedClock::new(test_start()));
    let db = Arc::new(Database::open(dir.path(), modules, clock.clone()).unwrap());
    if modules
        .iter()
        .any(|m| m.module == lis_catalog::SCHEMA.module && m.migrations.len() < 6)
    {
        // Temporary seed compatibility only; the legacy-upgrade test removes
        // these fields before snapshotting its exact historical schema.
        let conn = Connection::open(dir.path().join(lis_store::DATABASE_FILE)).unwrap();
        conn.execute_batch(include_str!(
            "../../../lis-catalog/migrations/0006_report_content.sql"
        ))
        .unwrap();
    }
    let subject_id = setup(&db, |ctx| {
        lis_identity::register_subject(ctx, synthetic_subject()).map(|done| done.subject_id)
    });
    let client_id = setup(&db, |ctx| {
        lis_identity::create_client(
            ctx,
            CreateClient {
                code: "syn-client".to_owned(),
                name: "Synthetic Clinic".to_owned(),
            },
        )
    });
    let practitioner_id = setup(&db, |ctx| {
        lis_identity::register_practitioner(
            ctx,
            RegisterPractitioner {
                npi: "1234567893".to_owned(),
                family_name: "Synthetic".to_owned(),
                given_names: Some("Prescriber".to_owned()),
            },
        )
    });
    let urine = approved_assay(&db, URINE);
    let oral_fluid = approved_assay(&db, ORAL_FLUID);
    let port = local_port();
    let instrument = register_instrument(&db, "syn-analyzer", port);
    Fixture {
        db,
        clock,
        dir,
        subject_id,
        client_id,
        practitioner_id,
        urine,
        oral_fluid,
        instrument,
        port,
    }
}

fn synthetic_subject() -> RegisterSubject {
    RegisterSubject {
        demographics: DemographicsInput {
            family_name: Phi::new("SYNTHETIC".to_owned()),
            given_names: Some(Phi::new("Patient Gamma".to_owned())),
            birth_date: Phi::new("1985-02-03".to_owned()),
            administrative_sex: None,
        },
        source: "requisition".to_owned(),
        identifiers: Vec::new(),
        acknowledged_candidates: Vec::new(),
    }
}

/// A loopback TCP analyzer, enabled.
pub(crate) fn register_instrument(db: &Database, code: &str, port: u16) -> InstrumentId {
    let input = RegisterInstrument {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        connection: ConnectionSettings::AstmTcp {
            bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port,
            allowed_peers: vec![IpAddr::V4(Ipv4Addr::LOCALHOST)],
        },
        enabled: true,
    };
    setup(db, move |ctx| {
        lis_interfaces::register_instrument(ctx, input)
    })
    .id
}

/// Runs another module's setup step as a fresh user holding every capability it may need.
fn setup<T, E, F>(db: &Database, command: F) -> T
where
    T: Send + 'static,
    E: From<StoreError> + std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
{
    let author = principal(&[
        Capability::EnterOrders,
        Capability::Accession,
        Capability::ManageQuality,
        Capability::ManageConfiguration,
        Capability::ApproveConfiguration,
    ]);
    db.write_blocking(author, CorrelationId::generate(), command)
        .unwrap()
}

fn coded(code: String) -> CodedName {
    CodedName {
        name: format!("Synthetic {code}"),
        code,
    }
}

/// An approved two-analyte method for `matrix` and an approved test using it:
/// each drafted by one user and approved by another.
fn approved_assay(db: &Database, matrix: &str) -> Assay {
    let specimen_type = setup(db, {
        let coded = coded(matrix.to_owned());
        move |ctx| lis_catalog::create_specimen_type(ctx, coded)
    });
    let analytes = ["a", "b"].map(|suffix| {
        let code = format!("{matrix}-{suffix}");
        setup(db, move |ctx| {
            lis_catalog::create_analyte(
                ctx,
                CreateAnalyte {
                    name: format!("Synthetic {code}"),
                    code,
                    loinc_code: None,
                    drug_class_code: None,
                },
            )
        })
    });
    let method = approved_method(db, specimen_type, &format!("{matrix}-m"), analytes);
    let orderable = setup(db, {
        let code = format!("{matrix}-t");
        move |ctx| {
            lis_catalog::create_orderable(
                ctx,
                CreateOrderable {
                    name: format!("Synthetic {code}"),
                    code,
                    kind: OrderableKind::Test,
                },
            )
        }
    });
    let test = setup(db, move |ctx| {
        lis_catalog::draft_orderable_version(
            ctx,
            DraftOrderableVersion {
                orderable_id: orderable,
                content: OrderableVersionContent {
                    components: vec![ComponentInput {
                        method_version_id: method,
                        purpose: ComponentPurpose::Analytical,
                        analyte_ids: analytes.to_vec(),
                    }],
                },
            },
        )
    })
    .id;
    setup(db, move |ctx| {
        lis_catalog::approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id: test,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    Assay {
        method,
        analytes,
        test,
    }
}

/// An approved quantitative method version of a new method with `analytes`.
pub(crate) fn approved_method(
    db: &Database,
    specimen_type: lis_catalog::SpecimenTypeId,
    code: &str,
    analytes: [AnalyteId; 2],
) -> MethodVersionId {
    let method = setup(db, {
        let coded = coded(code.to_owned());
        move |ctx| lis_catalog::create_method(ctx, coded)
    });
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: specimen_type,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: analytes
            .iter()
            .map(|analyte_id| MethodAnalyte {
                reference_interval: None,
                range_policy: Default::default(),
                analyte_id: *analyte_id,
                unit: Some("ng/mL".to_owned()),
                cutoff: Some(DecimalText::parse("100").unwrap()),
                limit_of_detection: None,
                limit_of_quantitation: None,
                reportable_low: None,
                reportable_high: None,
                display_scale: Some(0),
            })
            .collect(),
    };
    let version = setup(db, move |ctx| {
        lis_catalog::draft_method_version(
            ctx,
            DraftMethodVersion {
                method_id: method,
                content,
            },
        )
    })
    .id;
    setup(db, move |ctx| {
        lis_catalog::approve_method_version(
            ctx,
            ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id: version,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    version
}

/// An accessioned order: its order, accession and first container.
pub(crate) struct Material {
    pub(crate) order_id: OrderId,
    pub(crate) order_revision: i64,
    pub(crate) container_id: ContainerId,
    pub(crate) label: String,
}

impl Fixture {
    /// An accepted order for `test`, accessioned as one `matrix` specimen
    /// with one container. The specimen is accepted when `accept` is set.
    pub(crate) fn material(
        &self,
        test: OrderableVersionId,
        matrix: &str,
        accept: bool,
    ) -> Material {
        let order = self.order(test);
        let accepted = setup(&self.db, move |ctx| {
            lis_orders::accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: order,
                    expected_revision: 1,
                },
            )
        });
        let items = accepted.items.iter().map(|item| item.item_id).collect();
        let input = accession_input(accepted.order_id, items, matrix);
        let mut accession: AccessionDetail = setup(&self.db, move |ctx| {
            lis_specimens::create_accession(ctx, input)
        });
        if accept {
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
            accession = setup(&self.db, move |ctx| {
                lis_specimens::record_receipt_decision(ctx, decision)
            });
        }
        let container = &accession.specimens[0].containers[0];
        Material {
            order_id: accepted.order_id,
            order_revision: accepted.revision,
            container_id: container.container_id,
            label: container.label_code.clone(),
        }
    }

    /// Accepted, accessioned and received urine material for the urine test.
    pub(crate) fn urine_material(&self) -> Material {
        self.material(self.urine.test, URINE, true)
    }

    pub(crate) fn order(&self, test: OrderableVersionId) -> OrderId {
        let input = RegisterOrderInput {
            subject_id: self.subject_id,
            client_id: self.client_id,
            client_location_id: None,
            practitioner_id: self.practitioner_id,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("SYN-REQ-9".to_owned()),
            billing_responsibility: BillingResponsibility::NotSupplied,
            facility_setting: None,
            standing_order_id: None,
            template_use: TemplateUseInput::None,
            items: vec![OrderItemInput {
                orderable_version_id: test,
                sequence_role: SequenceRole::FirstLine,
                reflex_authorization: ReflexAuthorization::NotStated,
                reflex_scope_orderable_ids: Vec::new(),
            }],
            evidence: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::WetSignature,
                    document_ref: Some("SYN-REQ-9".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        };
        setup(&self.db, move |ctx| lis_orders::register_order(ctx, input)).order_id
    }

    /// Runs another module's command with every capability it may need.
    pub(crate) fn setup<T, E, F>(&self, command: F) -> T
    where
        T: Send + 'static,
        E: From<StoreError> + std::fmt::Debug + Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
    {
        setup(&self.db, command)
    }

    /// Marks the fixture's analyzer in service.
    pub(crate) fn ready(&self) {
        self.set_status(self.instrument, InstrumentStatus::InService);
    }

    pub(crate) fn set_status(&self, instrument_id: InstrumentId, status: InstrumentStatus) {
        run(self, STAFF, move |ctx| {
            lis_analytical::set_instrument_status(
                ctx,
                SetInstrumentStatusInput {
                    instrument_id,
                    status,
                    reason: "Synthetic maintenance log".to_owned(),
                },
            )
        })
        .unwrap();
    }

    /// An active lot; controls and calibrators get `label` as sample label.
    pub(crate) fn lot(&self, kind: MaterialKind, number: &str, expires_on: &str) -> LotId {
        let sample_label = matches!(kind, MaterialKind::Control | MaterialKind::Calibrator)
            .then(|| format!("QC-{number}"));
        let input = RegisterLotInput {
            material_kind: kind,
            product_name: "Synthetic material".to_owned(),
            lot_number: number.to_owned(),
            level_text: None,
            sample_label,
            expires_on: expires_on.to_owned(),
        };
        run(self, STAFF, move |ctx| {
            lis_analytical::register_lot(ctx, input)
        })
        .unwrap()
        .lot_id
    }

    /// An open urine run on the ready analyzer with one reagent lot.
    pub(crate) fn open_run(&self) -> RunDetail {
        self.ready();
        let reagent = self.lot(
            MaterialKind::Reagent,
            &format!("R-{}", RunId::generate()),
            LATER,
        );
        let input = CreateRunInput {
            instrument_id: self.instrument,
            method_version_id: self.urine.method,
            lot_ids: vec![reagent],
        };
        run(self, STAFF, move |ctx| {
            lis_analytical::create_run(ctx, input)
        })
        .unwrap()
    }

    /// Places `label` on `run_id` as an initial patient position.
    pub(crate) fn place(
        &self,
        run_id: RunId,
        position: &str,
        label: &str,
    ) -> Result<RunDetail, AnalyticalError> {
        let input = patient_input(run_id, position, label);
        run(self, STAFF, move |ctx| {
            lis_analytical::add_patient_position(ctx, input)
        })
    }

    /// Places a new control lot on `run_id`; returns the run and the lot.
    pub(crate) fn control(&self, run_id: RunId, position: &str) -> (RunDetail, LotId) {
        let lot = self.lot(
            MaterialKind::Control,
            &format!("C-{position}-{run_id}"),
            LATER,
        );
        let input = AddQcPositionInput {
            run_id,
            position_label: position.to_owned(),
            role: QcRole::Control,
            lot_id: Some(lot),
            sample_label: None,
        };
        let run = run(self, STAFF, move |ctx| {
            lis_analytical::add_qc_position(ctx, input)
        })
        .unwrap();
        (run, lot)
    }

    /// Records and approves (as another user) a registration mapping the
    /// urine analytes to `A1` and `B1`.
    pub(crate) fn register_source(&self, indicator: RevisionIndicator, final_concentration: bool) {
        let input = RegisterSourceInput {
            au_profile: None,
            instrument_id: self.instrument,
            sender_name: "SYNTHETIC".to_owned(),
            sender_version: "1.0".to_owned(),
            sample_id_field: SampleIdField::SpecimenId,
            revision_indicator: indicator,
            reports_final_concentration: final_concentration,
            test_codes: vec![
                SourceTestCodeInput {
                    source_test_code: "A1".to_owned(),
                    analyte_id: self.urine.analytes[0],
                },
                SourceTestCodeInput {
                    source_test_code: "B1".to_owned(),
                    analyte_id: self.urine.analytes[1],
                },
            ],
        };
        let draft = run(self, STAFF, move |ctx| {
            lis_analytical::register_source(ctx, input)
        })
        .unwrap();
        run(self, STAFF, move |ctx| {
            lis_analytical::approve_source(
                ctx,
                lis_analytical::ApproveSourceInput {
                    registration_id: draft.registration_id,
                    expected_revision: draft.revision,
                    self_approval: false,
                },
            )
        })
        .unwrap();
    }

    /// A complete durable receipt from the fixture's analyzer, received over
    /// its real TCP listener. `tag` makes the message unique.
    pub(crate) fn receipt(&self, tag: &str) -> ReceiptId {
        let supervisor = ListenerSupervisor::new(Arc::clone(&self.db));
        supervisor.start_all().unwrap();
        let header = format!("H|\\^&|||SYNTHETIC^1.0|||||||P|1|{tag}");
        send_message(self.port, &[header.as_bytes(), b"L|1|N"]);
        let receipt = wait_for_receipt(&self.db);
        supervisor.stop_all().unwrap();
        receipt
    }

    /// A writable connection outside the store, for tamper tests only.
    pub(crate) fn raw(&self) -> Connection {
        Connection::open(self.dir.path().join(lis_store::DATABASE_FILE)).unwrap()
    }
}

/// Sends one ASTM transfer as a simulator would: ENQ, frames, EOT.
fn send_message(port: u16, records: &[&[u8]]) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .unwrap();
    let mut control = [0_u8; 1];
    stream.write_all(&[lis_astm::ENQ]).unwrap();
    stream.read_exact(&mut control).unwrap();
    assert_eq!(control[0], lis_astm::ACK);
    let records: Vec<Vec<u8>> = records.iter().map(|record| record.to_vec()).collect();
    for frame in lis_astm::encode_records(&records).unwrap() {
        stream.write_all(&frame).unwrap();
        stream.read_exact(&mut control).unwrap();
        assert_eq!(control[0], lis_astm::ACK);
    }
    stream.write_all(&[lis_astm::EOT]).unwrap();
}

/// Waits for the listener's completed-receipt outbox item and takes it.
fn wait_for_receipt(db: &Database) -> ReceiptId {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let worker = Principal::system("test-importer", [Capability::InstrumentIntake]);
        let leased = db
            .write_blocking(worker, CorrelationId::generate(), |ctx| {
                let items = lis_store::outbox::lease(
                    ctx,
                    lis_interfaces::RECEIPT_COMPLETED_TOPIC,
                    1,
                    Duration::from_secs(60),
                )?;
                for item in &items {
                    lis_store::outbox::complete(ctx, item)?;
                }
                Ok::<_, StoreError>(items)
            })
            .unwrap();
        if let Some(item) = leased.first() {
            return item.dedup_key.parse().unwrap();
        }
        assert!(Instant::now() < deadline, "no receipt completed");
        std::thread::sleep(Duration::from_millis(10));
    }
}

/// Records one QC row through the importer's intake function.
pub(crate) fn observe(
    f: &Fixture,
    position_id: PositionId,
    receipt_id: ReceiptId,
    code: &str,
    value: &str,
    status: &str,
) -> Result<RowDecision, AnalyticalError> {
    let row = QcRow {
        position_id,
        receipt_id,
        record_path: "P1/O1/R1".to_owned(),
        source_test_code: code.to_owned(),
        row: SourceRow {
            value_text: Some(value.to_owned()),
            units_text: Some("ng/mL".to_owned()),
            flags_text: None,
            status_text: Some(status.to_owned()),
        },
    };
    let intake = Principal::system("result-import", [Capability::InstrumentIntake]);
    run_as(f, intake, move |ctx| {
        lis_analytical::record_qc_observations(ctx, &[row]).map(|mut decisions| decisions.remove(0))
    })
}

/// An initial patient position input.
pub(crate) fn patient_input(run_id: RunId, position: &str, label: &str) -> AddPatientPositionInput {
    AddPatientPositionInput {
        run_id,
        position_label: position.to_owned(),
        container_label: label.to_owned(),
        attempt_kind: AttemptKind::Initial,
        related_execution_id: None,
        dilution_factor: None,
        prep_batch_id: None,
        reason: None,
    }
}

/// An accession request for one specimen of `matrix` with one container.
pub(crate) fn accession_input(
    order_id: OrderId,
    items: Vec<lis_orders::OrderItemId>,
    matrix: &str,
) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids: items,
        collection: CollectionInput {
            time: CollectionTimeInput::Datetime {
                local: "2026-01-01T09:30".to_owned(),
                utc_offset_minutes: -300,
            },
            collector: None,
            observations: Vec::new(),
        },
        specimens: vec![SpecimenInput {
            specimen_type_code: matrix.to_owned(),
            container_count: 1,
            volume: None,
            volume_unit: None,
            volume_kind: VolumeKind::Unknown,
        }],
    }
}

/// Runs an analytical command as a user holding `capabilities`.
pub(crate) fn run<T, F>(
    f: &Fixture,
    capabilities: &[Capability],
    command: F,
) -> Result<T, AnalyticalError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, AnalyticalError> + Send + 'static,
{
    run_as(f, principal(capabilities), command)
}

pub(crate) fn run_as<T, F>(
    f: &Fixture,
    principal: Principal,
    command: F,
) -> Result<T, AnalyticalError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, AnalyticalError> + Send + 'static,
{
    f.db.write_blocking(principal, CorrelationId::generate(), command)
}

pub(crate) fn read<T, F>(f: &Fixture, query: F) -> T
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, AnalyticalError> + Send + 'static,
{
    f.db.read_blocking(query).unwrap()
}

/// Row counts of every analytical table, the audit trail and the outbox.
pub(crate) fn counts(f: &Fixture) -> Vec<i64> {
    read(f, |conn| {
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_schema WHERE type = 'table' \
                 AND (name LIKE 'ana_%' OR name IN ('audit_events', 'store_outbox')) ORDER BY name",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        tables
            .iter()
            .map(|table| {
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map_err(AnalyticalError::from)
            })
            .collect()
    })
}

/// `(action, record_id, reason)` of the newest clinical audit event.
pub(crate) fn last_event(f: &Fixture) -> (String, Option<String>, Option<String>) {
    read(f, |conn| {
        Ok(conn.query_row(
            "SELECT action, record_id, reason FROM audit_events WHERE stream = 'clinical' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    })
}

/// The error code of a failed command.
pub(crate) fn code_of<T: std::fmt::Debug>(result: Result<T, AnalyticalError>) -> ErrorCode {
    result.unwrap_err().code()
}

/// The gate codes of a refusal.
pub(crate) fn gate_codes<T: std::fmt::Debug>(result: Result<T, AnalyticalError>) -> Vec<String> {
    let error = result.unwrap_err();
    assert_eq!(error.code(), ErrorCode::GateBlocked, "{error:?}");
    error
        .gate_reasons()
        .iter()
        .map(|gate| gate.code.clone())
        .collect()
}

#[test]
fn schema_passes_the_lint() {
    assert_eq!(lint_schema(&MODULES).unwrap(), Vec::new());
}
