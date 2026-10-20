//! Result-file imports on real SQLite files with the production pragmas
//! (result files brief §5, F7–F12, F17; REQ-ANA-003, REQ-RES-001). The
//! fixture builds the catalog, a result-file instrument with an approved
//! template, an open run with a control position, and accepted, received
//! urine samples on it, through the owners' public commands. Each file is
//! received as a real durable receipt and imported as the outbox worker
//! does. Expected outcomes come from the brief, not from the importer. All
//! names, labels, values and limits are synthetic.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod au;
mod awaiting;
mod dilution;
mod failures;
mod kept;
mod range_policy;
mod review_queues;
mod review_scan;
mod rows;

use std::cell::Cell;

use lis_analytical::{
    AddPatientPositionInput, AddQcPositionInput, ApproveSourceInput, AttemptKind, CloseRunInput,
    CreateRunInput, EndOfData, ExecutionId, FileColumns, FileDelimiter, FileEncoding, FileLayout,
    FileShape, InstrumentStatus, MaterialKind, PositionRole, QcRole, RegisterFileSourceInput,
    RegisterLotInput, RunDetail, RunId, SampleTypeWord, SetInstrumentStatusInput,
    SourceRegistrationView, SourceTestCodeInput, ValueWord, ValueWordMeaning,
};
use lis_catalog::{
    AnalyteId, ApproveMethodVersion, ApproveOrderableVersion, CodedName, ComponentInput,
    ComponentPurpose, CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    MethodAnalyte, MethodCertainty, MethodVersionContent, MethodVersionId, OrderableKind,
    OrderableVersionContent, OrderableVersionId, OutputScale, ThresholdRounding, ThresholdRule,
};
use lis_core::{Capability, CorrelationId, DecimalText, InstrumentId, Phi, Principal};
use lis_identity::{CreateClient, DemographicsInput, RegisterPractitioner, RegisterSubject};
use lis_interfaces::{
    ConnectionSettings, FileFacts, FileSource, ReceiptId, ReceiveResultFileInput,
    RegisterInstrument,
};
use lis_orders::{
    AcceptOrderInput, AuthorityEvidenceInput, AuthorityEvidenceKind, BillingResponsibility,
    EvidenceInput, OrderId, OrderItemId, OrderItemInput, ReflexAuthorization, RegisterOrderInput,
    SequenceRole, SourceKind, TemplateUseInput,
};
use lis_results::delimited::extract_file;
use lis_results::{
    ImportAttemptDetail, ImportAttemptSummary, ImportReceiptInput, ImportSource, ResultError,
    SpecimenResults,
};
use lis_specimens::{
    AccessionId, CollectionInput, CollectionTimeInput, CreateAccessionInput, ReceiptDecision,
    RecordReceiptDecisionInput, SpecimenId, SpecimenInput, VolumeKind,
};
use lis_store::testing::{TestDatabase, principal, test_start};
use lis_store::{CommandContext, StoreError};
use rusqlite::Connection;

pub(crate) const URINE: &str = "syn-ur";
pub(crate) const UNIT: &str = "ng/mL";
/// The control lot's sample label, placed on the run.
pub(crate) const CONTROL: &str = "QC-LOW-L1234";
/// A calibrator lot's sample label, registered but not on the run.
pub(crate) const CALIBRATOR_LOT: &str = "CAL-LOT-1";
const HEADER: [&str; 6] = [
    "Sample Name",
    "Sample Type",
    "Component",
    "Final Conc",
    "Units",
    "Dilution",
];

/// Everything the set-up steps may need; each call is a fresh user, so a
/// second call approves what the first drafted.
const STAFF: &[Capability] = &[
    Capability::EnterOrders,
    Capability::Accession,
    Capability::ManageQuality,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
];

/// The method's analytes. Morphine has an LOD of 1 and an LOQ of 5;
/// oxycodone has no limits; fentanyl, which the panel does not order, has
/// an LOQ of 0.5 and no LOD. Every cutoff is 50.
#[derive(Clone, Copy)]
pub(crate) struct Analytes {
    pub(crate) mor: AnalyteId,
    pub(crate) oxy: AnalyteId,
    pub(crate) fen: AnalyteId,
}

/// The world described in the module documentation.
pub(crate) struct Fixture {
    pub(crate) t: TestDatabase,
    parties: (
        lis_identity::SubjectId,
        lis_identity::ClientId,
        lis_identity::PractitionerId,
    ),
    pub(crate) analytes: Analytes,
    pub(crate) method: MethodVersionId,
    /// Morphine and oxycodone.
    pub(crate) panel: OrderableVersionId,
    /// Fentanyl alone: the add-on of F17.
    pub(crate) fentanyl: OrderableVersionId,
    pub(crate) instrument: InstrumentId,
    pub(crate) run: RunId,
    files: Cell<u32>,
}

/// One accepted order with one received, accepted urine container on the run.
pub(crate) struct Sample {
    pub(crate) order_id: OrderId,
    pub(crate) order_revision: i64,
    pub(crate) accession_id: AccessionId,
    pub(crate) specimen_id: SpecimenId,
    pub(crate) execution_id: ExecutionId,
    pub(crate) label: String,
}

/// The fixture's template (brief §4, synthetic example): long, comma, with
/// sample type, units and dilution columns.
pub(crate) fn layout() -> FileLayout {
    let typed = |word: &str, role| SampleTypeWord {
        word: word.to_owned(),
        role,
    };
    let word = |word: &str, meaning| ValueWord {
        word: word.to_owned(),
        meaning,
    };
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![HEADER.map(str::to_owned).to_vec()],
        shape: FileShape::Long {
            analyte_column: 2,
            value_column: 3,
        },
        columns: FileColumns {
            sample_label: 0,
            units: Some(4),
            sample_type: Some(1),
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: Some(5),
        },
        end_of_data: EndOfData::EndOfFile,
        sample_types: vec![
            typed("Unknown", PositionRole::Patient),
            typed("QC", PositionRole::Control),
            typed("Standard", PositionRole::Calibrator),
            typed("Blank", PositionRole::Blank),
        ],
        value_words: vec![
            word("N/F", ValueWordMeaning::BelowLoq),
            word("ND", ValueWordMeaning::BelowLod),
            word("NP", ValueWordMeaning::NotPerformed),
            word("<0", ValueWordMeaning::BelowLoq),
        ],
        not_results: vec!["Morphine-D6".to_owned()],
        declared_unit: None,
    }
}

/// The header line and `rows`, LF-terminated.
pub(crate) fn csv(rows: &[String]) -> Vec<u8> {
    let mut text = HEADER.join(",");
    for row in rows {
        text.push('\n');
        text.push_str(row);
    }
    text.push('\n');
    text.into_bytes()
}

pub(crate) fn fixture() -> Fixture {
    fixture_with_au_profile(None)
}

pub(crate) fn fixture_with_au_profile(protocol: Option<lis_au::AuConfig>) -> Fixture {
    let t = TestDatabase::open(&[
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
        lis_results::SCHEMA,
    ])
    .unwrap();
    let parties = (
        setup(&t, |ctx| {
            lis_identity::register_subject(ctx, subject()).map(|done| done.subject_id)
        }),
        setup(&t, |ctx| {
            lis_identity::create_client(
                ctx,
                CreateClient {
                    code: "syn-client".to_owned(),
                    name: "Synthetic Clinic".to_owned(),
                },
            )
        }),
        setup(&t, |ctx| {
            lis_identity::register_practitioner(
                ctx,
                RegisterPractitioner {
                    npi: "1234567893".to_owned(),
                    family_name: "Synthetic".to_owned(),
                    given_names: None,
                },
            )
        }),
    );
    let specimen_type = setup(&t, |ctx| {
        lis_catalog::create_specimen_type(ctx, coded(URINE))
    });
    let analytes = Analytes {
        mor: analyte(&t, "SYN-MOR"),
        oxy: analyte(&t, "SYN-OXY"),
        fen: analyte(&t, "SYN-FEN"),
    };
    let method = approved_method(&t, specimen_type, analytes);
    let panel = orderable(&t, "syn-panel", method, vec![analytes.mor, analytes.oxy]);
    let fentanyl = orderable(&t, "syn-fen", method, vec![analytes.fen]);
    let connection = protocol
        .as_ref()
        .map_or(ConnectionSettings::ResultFile {}, |protocol| {
            au::connection(protocol.clone())
        });
    let instrument = configured_instrument(&t, connection);
    let f = Fixture {
        t,
        parties,
        analytes,
        method,
        panel,
        fentanyl,
        instrument,
        run: RunId::generate(),
        files: Cell::new(0),
    };
    if let Some(protocol) = protocol {
        au::register(&f, protocol, UNIT);
    } else {
        f.replace_layout(layout());
    }
    let status = SetInstrumentStatusInput {
        instrument_id: instrument,
        status: InstrumentStatus::InService,
        reason: "Synthetic maintenance log".to_owned(),
    };
    f.setup(|ctx| lis_analytical::set_instrument_status(ctx, status));
    f.lot(MaterialKind::Calibrator, CALIBRATOR_LOT);
    let run = f.open_run(true);
    Fixture { run, ..f }
}

fn configured_instrument(t: &TestDatabase, connection: ConnectionSettings) -> InstrumentId {
    setup(t, move |ctx| {
        lis_interfaces::register_instrument(
            ctx,
            RegisterInstrument {
                code: "syn-lcms-files".to_owned(),
                name: "Synthetic LC-MS/MS files".to_owned(),
                connection,
                enabled: true,
            },
        )
    })
    .id
}

impl Fixture {
    /// Runs a set-up command as a fresh user with every set-up capability.
    pub(crate) fn setup<T, E, F>(&self, command: F) -> T
    where
        T: Send + 'static,
        E: From<StoreError> + std::fmt::Debug + Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
    {
        setup(&self.t, command)
    }

    /// Runs `command` as `principal`.
    pub(crate) fn run_as<T, E, F>(&self, principal: Principal, command: F) -> Result<T, E>
    where
        T: Send + 'static,
        E: From<StoreError> + Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
    {
        self.t
            .db
            .write_blocking(principal, CorrelationId::generate(), command)
    }

    pub(crate) fn read<T, F>(&self, query: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, ResultError> + Send + 'static,
    {
        self.t.db.read_blocking(query).unwrap()
    }

    /// Drafts `layout` as the instrument's file template and approves it as
    /// another user; the previous registration retires.
    pub(crate) fn replace_layout(&self, layout: FileLayout) -> SourceRegistrationView {
        let codes = [
            ("Morphine", self.analytes.mor),
            ("Oxycodone", self.analytes.oxy),
            ("Fentanyl", self.analytes.fen),
        ]
        .map(|(code, analyte_id)| (code.to_owned(), analyte_id))
        .to_vec();
        self.replace_source(layout, codes)
    }

    /// As [`Fixture::replace_layout`], with the analyte names `codes`.
    pub(crate) fn replace_source(
        &self,
        layout: FileLayout,
        codes: Vec<(String, AnalyteId)>,
    ) -> SourceRegistrationView {
        let input = RegisterFileSourceInput {
            instrument_id: self.instrument,
            software_name: "SynthQuant".to_owned(),
            software_version: "3.4".to_owned(),
            reports_final_concentration: true,
            layout,
            test_codes: codes
                .into_iter()
                .map(|(source_test_code, analyte_id)| SourceTestCodeInput {
                    source_test_code,
                    analyte_id,
                })
                .collect(),
        };
        let draft = self.setup(|ctx| lis_analytical::register_file_source(ctx, input));
        self.setup(move |ctx| {
            lis_analytical::approve_source(
                ctx,
                ApproveSourceInput {
                    registration_id: draft.registration_id,
                    expected_revision: draft.revision,
                    self_approval: false,
                },
            )
        })
    }

    /// The instrument's active registration.
    pub(crate) fn registration(&self) -> Option<SourceRegistrationView> {
        let instrument = self.instrument;
        self.read(move |conn| Ok(lis_analytical::active_registration(conn, instrument)?))
    }

    fn lot(&self, kind: MaterialKind, sample_label: &str) -> lis_analytical::LotId {
        let input = RegisterLotInput {
            material_kind: kind,
            product_name: "Synthetic material".to_owned(),
            lot_number: format!("LOT-{sample_label}"),
            level_text: None,
            sample_label: Some(sample_label.to_owned()),
            expires_on: "2027-12-31".to_owned(),
        };
        self.setup(|ctx| lis_analytical::register_lot(ctx, input))
            .lot_id
    }

    /// An open run of the method; the fixture's first run also has the
    /// control lot on a control position.
    pub(crate) fn open_run(&self, control: bool) -> RunId {
        let lots: Vec<_> = control
            .then(|| self.lot(MaterialKind::Control, CONTROL))
            .into_iter()
            .collect();
        let input = CreateRunInput {
            instrument_id: self.instrument,
            method_version_id: self.method,
            lot_ids: lots.clone(),
        };
        let run = self
            .setup(|ctx| lis_analytical::create_run(ctx, input))
            .run_id;
        for lot in lots {
            let position = AddQcPositionInput {
                run_id: run,
                position_label: "Q1".to_owned(),
                role: QcRole::Control,
                lot_id: Some(lot),
                sample_label: None,
            };
            self.setup(|ctx| lis_analytical::add_qc_position(ctx, position));
        }
        run
    }

    /// Closes `run_id` without a reason.
    pub(crate) fn close_run(&self, run_id: RunId) {
        let revision = self
            .read(move |conn| Ok(lis_analytical::get_run(conn, run_id, test_start())?.revision));
        let input = CloseRunInput {
            run_id,
            expected_revision: revision,
            reason: None,
        };
        self.setup(|ctx| lis_analytical::close_run(ctx, input));
    }

    /// An accepted order for `test`, accessioned and received, not placed.
    pub(crate) fn unplaced_sample(&self, test: OrderableVersionId) -> Sample {
        let order = self.order(test);
        let accepted = self.setup(move |ctx| {
            lis_orders::accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: order,
                    expected_revision: 1,
                },
            )
        });
        let item_id = accepted.items[0].item_id;
        let input = accession_input(accepted.order_id, item_id);
        let accession = self.setup(|ctx| lis_specimens::create_accession(ctx, input));
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
        self.setup(|ctx| lis_specimens::record_receipt_decision(ctx, decision));
        Sample {
            order_id: accepted.order_id,
            order_revision: accepted.revision,
            accession_id: accession.accession_id,
            specimen_id: specimen.specimen_id,
            execution_id: ExecutionId::generate(),
            label: specimen.containers[0].label_code.clone(),
        }
    }

    /// A panel sample placed on the run as an initial attempt.
    pub(crate) fn sample(&self) -> Sample {
        let sample = self.unplaced_sample(self.panel);
        self.place(sample, self.run, AttemptKind::Initial, None)
    }

    /// Places `sample` on `run`; a follow-on attempt follows its current execution.
    pub(crate) fn place(
        &self,
        sample: Sample,
        run_id: RunId,
        attempt_kind: AttemptKind,
        dilution_factor: Option<&str>,
    ) -> Sample {
        let follow_on = attempt_kind != AttemptKind::Initial;
        let input = AddPatientPositionInput {
            run_id,
            position_label: format!("P-{}-{}", sample.label, attempt_kind.as_str()),
            container_label: sample.label.clone(),
            attempt_kind,
            related_execution_id: follow_on.then_some(sample.execution_id),
            dilution_factor: dilution_factor.map(|text| DecimalText::parse(text).unwrap()),
            prep_batch_id: None,
            reason: follow_on.then(|| "Synthetic follow-on".to_owned()),
        };
        let run: RunDetail = self.setup(|ctx| lis_analytical::add_patient_position(ctx, input));
        let execution_id = run
            .positions
            .iter()
            .rev()
            .find(|position| position.sample_label == sample.label)
            .and_then(|position| position.execution.as_ref())
            .unwrap()
            .execution_id;
        Sample {
            execution_id,
            ..sample
        }
    }

    fn order(&self, test: OrderableVersionId) -> OrderId {
        let (subject_id, client_id, practitioner_id) = self.parties;
        let input = RegisterOrderInput {
            subject_id,
            client_id,
            client_location_id: None,
            practitioner_id,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("SYN-REQ-F".to_owned()),
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
                    document_ref: Some("SYN-REQ-F".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        };
        self.setup(|ctx| lis_orders::register_order(ctx, input))
            .order_id
    }

    /// A durable upload receipt of `bytes` from the instrument (F1, F16).
    pub(crate) fn receive(&self, bytes: &[u8]) -> ReceiptId {
        let digest = self.t.db.artifacts().put(bytes).unwrap().digest.to_string();
        self.files.set(self.files.get() + 1);
        let input = ReceiveResultFileInput {
            file: FileFacts {
                instrument_id: self.instrument,
                source: FileSource::Upload,
                file_name: Phi::new(format!("batch-{}.csv", self.files.get())),
                byte_len: u64::try_from(bytes.len()).unwrap(),
                modified_at: test_start(),
            },
            sha256: digest.clone(),
            artifact_digest: digest,
        };
        self.setup(|ctx| lis_interfaces::receive_result_file(ctx, input))
            .id
    }

    /// Imports `source` as attempt `attempt_no` of `receipt_id` with the
    /// active registration, as the import worker does.
    pub(crate) fn import_source(
        &self,
        receipt_id: ReceiptId,
        attempt_no: i64,
        source: ImportSource,
    ) -> ImportAttemptSummary {
        let input = ImportReceiptInput {
            receipt_id,
            attempt_no,
            instrument_id: self.instrument,
            registration: self.registration(),
            source,
        };
        let intake = Principal::system("result-import", [Capability::InstrumentIntake]);
        self.run_as(intake, |ctx| lis_results::import_receipt(ctx, input))
            .unwrap()
    }

    /// Receives `bytes` and imports them under the active template.
    pub(crate) fn import_bytes(&self, bytes: &[u8]) -> ImportAttemptSummary {
        let receipt = self.receive(bytes);
        let layout = self.registration().unwrap().file_layout.unwrap();
        self.import_source(receipt, 1, ImportSource::File(extract_file(bytes, &layout)))
    }

    /// Receives and imports a file of `rows` under the fixture's header.
    pub(crate) fn import(&self, rows: &[String]) -> ImportAttemptSummary {
        self.import_bytes(&csv(rows))
    }

    /// The audited detail of an attempt.
    pub(crate) fn detail(&self, attempt: &ImportAttemptSummary) -> ImportAttemptDetail {
        let id = attempt.import_attempt_id;
        self.read(move |conn| lis_results::get_import_attempt(conn, id))
    }

    pub(crate) fn results(&self, sample: &Sample) -> SpecimenResults {
        let id = sample.specimen_id;
        self.read(move |conn| lis_results::get_specimen_results(conn, id))
    }

    /// `(form, value_text, units_text)` of every measurement at the
    /// sample's executions for `analyte`, oldest first.
    pub(crate) fn measurements(
        &self,
        sample: &Sample,
        analyte: AnalyteId,
    ) -> Vec<(String, Option<String>, Option<String>)> {
        let specimen = sample.specimen_id;
        self.read(move |conn| {
            Ok(conn
                .prepare(
                    "SELECT m.form, m.value_text, m.units_text FROM res_measurements m \
                     JOIN ana_v_positions p ON p.position_id = m.position_id \
                     JOIN spc_v_containers c ON c.container_id = p.container_id \
                     WHERE c.specimen_id = ?1 AND m.analyte_id = ?2 ORDER BY m.rowid",
                )?
                .query_map(rusqlite::params![specimen, analyte], |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?))
                })?
                .collect::<Result<_, _>>()?)
        })
    }
}

fn setup<T, E, F>(t: &TestDatabase, command: F) -> T
where
    T: Send + 'static,
    E: From<StoreError> + std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
{
    t.db.write_blocking(principal(STAFF), CorrelationId::generate(), command)
        .unwrap()
}

fn subject() -> RegisterSubject {
    RegisterSubject {
        demographics: DemographicsInput {
            family_name: Phi::new("SYNTHETIC".to_owned()),
            given_names: Some(Phi::new("Patient Files".to_owned())),
            birth_date: Phi::new("1984-04-05".to_owned()),
            administrative_sex: None,
        },
        source: "requisition".to_owned(),
        identifiers: Vec::new(),
        acknowledged_candidates: Vec::new(),
    }
}

fn coded(code: &str) -> CodedName {
    CodedName {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
    }
}

fn analyte(t: &TestDatabase, code: &str) -> AnalyteId {
    let input = CreateAnalyte {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        loinc_code: None,
        drug_class_code: None,
    };
    setup(t, |ctx| lis_catalog::create_analyte(ctx, input))
}

fn limits(analyte_id: AnalyteId, lod: Option<&str>, loq: Option<&str>) -> MethodAnalyte {
    let decimal = |text: Option<&str>| text.map(|text| DecimalText::parse(text).unwrap());
    MethodAnalyte {
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
        analyte_id,
        unit: Some(UNIT.to_owned()),
        cutoff: decimal(Some("50")),
        limit_of_detection: decimal(lod),
        limit_of_quantitation: decimal(loq),
        reportable_low: None,
        reportable_high: None,
        display_scale: Some(1),
    }
}

fn approved_method(
    t: &TestDatabase,
    specimen_type_id: lis_catalog::SpecimenTypeId,
    analytes: Analytes,
) -> MethodVersionId {
    let method_id = setup(t, |ctx| {
        lis_catalog::create_method(ctx, coded("syn-lcms-m"))
    });
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![
            limits(analytes.mor, Some("1"), Some("5")),
            limits(analytes.oxy, None, None),
            limits(analytes.fen, None, Some("0.5")),
        ],
    };
    let version = setup(t, move |ctx| {
        lis_catalog::draft_method_version(ctx, DraftMethodVersion { method_id, content })
    })
    .id;
    setup(t, move |ctx| {
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

fn orderable(
    t: &TestDatabase,
    code: &str,
    method_version_id: MethodVersionId,
    analyte_ids: Vec<AnalyteId>,
) -> OrderableVersionId {
    let input = CreateOrderable {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
        kind: OrderableKind::Test,
    };
    let orderable_id = setup(t, |ctx| lis_catalog::create_orderable(ctx, input));
    let content = OrderableVersionContent {
        components: vec![ComponentInput {
            method_version_id,
            purpose: ComponentPurpose::Analytical,
            analyte_ids,
        }],
    };
    let version = setup(t, move |ctx| {
        lis_catalog::draft_orderable_version(
            ctx,
            DraftOrderableVersion {
                orderable_id,
                content,
            },
        )
    })
    .id;
    setup(t, move |ctx| {
        lis_catalog::approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id: version,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    version
}

fn accession_input(order_id: OrderId, item_id: OrderItemId) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids: vec![item_id],
        collection: CollectionInput {
            time: CollectionTimeInput::Datetime {
                local: "2026-01-01T09:30".to_owned(),
                utc_offset_minutes: -300,
            },
            collector: None,
            observations: Vec::new(),
        },
        specimens: vec![SpecimenInput {
            specimen_type_code: URINE.to_owned(),
            container_count: 1,
            volume: None,
            volume_unit: None,
            volume_kind: VolumeKind::Unknown,
        }],
    }
}
