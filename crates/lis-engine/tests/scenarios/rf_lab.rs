//! The result-file laboratory of the wave 10 import scenarios (result files
//! brief §5, §10, F17): the M2 journey laboratory ([`Lab`]) plus a method
//! with limits of quantitation, tests for one or both of its analytes, a
//! result-file instrument with an approved template, and an open run on it
//! with both controls. [`Lab`]'s instrument, run and samples are switched to
//! the file instrument, so its QC, review and result steps apply unchanged.
//! Files are written to a temporary folder and loaded through the engine's
//! `load_result_file`, as the shell's dialog would hand them over.
//! Synthetic data only.

use std::time::Duration;

use lis_analytical::{
    ApproveSourceInput, EndOfData, FileColumns, FileDelimiter, FileEncoding, FileShape,
    PositionRole, QcRole, RegistrationState, RevisionIndicator, RunId, SampleTypeWord,
    SourceTestCodeInput, ValueWord, ValueWordMeaning,
};
use lis_auth::RoleCode;
use lis_catalog::{
    ComponentInput, ComponentPurpose, MethodAnalyte, MethodVersionId, OrderableVersionId,
};
use lis_core::{CommandError, DecimalText, InstrumentId, UserId};
use lis_engine::{
    AddQcPositionInput, ApproveMethodVersion, CreateRunInput, Engine, FileLayout,
    GetImportAttemptInput, ImportAttemptDetail, ImportAttemptSummary, ListImportAttemptsInput,
    ListLotsInput, ListSourceRegistrationsInput, LoadResultFileInput, LoadedResultFile,
    PrepareReportCandidateInput, RegisterFileSourceInput, RegisterInstrument, ReleaseReportInput,
    RevisionSummary, SetLaboratoryProfile, SpecimenTypeId, UseMeasuredResultInput,
};
use lis_interfaces::{ConnectionSettings, ReceiptId};
use lis_orders::OrderComponentId;
use lis_reporting::ReportCompleteness;
use tempfile::TempDir;

use crate::fixture::{NEW_PASSWORD, secret};
use crate::m2_config::{URINE, draft_method, orderable};
use crate::m2_lab::{Lab, Role, Sample};

const UNIT: &str = "ng/mL";
pub(crate) const HEADER: &str = "Sample Name,Sample Type,Component,Final Conc,Units,Dilution";

/// Calls `use_measured_result` with plausible input; `None` for other commands.
pub(crate) async fn result_import_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    Some(match name {
        "use_measured_result" => engine
            .use_measured_result(UseMeasuredResultInput {
                order_component_id: OrderComponentId::generate(),
                execution_id: lis_analytical::ExecutionId::generate(),
                reason: "Synthetic add-on".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// The tests of the file method.
pub(crate) struct FileTests {
    /// Morphine and oxycodone.
    pub(crate) both: OrderableVersionId,
    /// Morphine alone.
    pub(crate) morphine: OrderableVersionId,
    /// Oxycodone alone: the add-on of F17.
    pub(crate) oxycodone: OrderableVersionId,
}

/// See the module documentation.
pub(crate) struct FileLab {
    pub(crate) lab: Lab,
    pub(crate) tests: FileTests,
    /// The method version of the file tests.
    pub(crate) method: MethodVersionId,
    /// The journey's ASTM analyzer and its open run.
    pub(crate) astm: (InstrumentId, RunId),
    folder: TempDir,
}

/// The template (brief §4, synthetic example): long, comma, with sample
/// type, units and dilution columns; `N/F` is below the LOQ.
pub(crate) fn layout(header: &str) -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![header.split(',').map(str::to_owned).collect()],
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
        sample_types: [
            ("Unknown", PositionRole::Patient),
            ("QC", PositionRole::Control),
            ("Standard", PositionRole::Calibrator),
        ]
        .map(|(word, role)| SampleTypeWord {
            word: word.to_owned(),
            role,
        })
        .to_vec(),
        value_words: vec![ValueWord {
            word: "N/F".to_owned(),
            meaning: ValueWordMeaning::BelowLoq,
        }],
        not_results: vec!["Morphine-D6".to_owned()],
        declared_unit: None,
    }
}

/// One data row: a patient, control or calibrator label, its type word,
/// the analyte name and the value, in `ng/mL` without dilution.
pub(crate) fn row(label: &str, kind: &str, component: &str, value: &str) -> String {
    format!("{label},{kind},{component},{value},{UNIT},1")
}

/// Both controls' rows for both analytes.
pub(crate) fn control_rows() -> Vec<String> {
    let mut rows = Vec::new();
    for (label, value) in [("QC-LOW-SYN", "40.0"), ("QC-HIGH-SYN", "200.0")] {
        for component in ["Morphine", "Oxycodone"] {
            rows.push(row(label, "QC", component, value));
        }
    }
    rows
}

impl FileLab {
    pub(crate) async fn start() -> Self {
        let mut lab = Lab::start_as_with_profile(
            RevisionIndicator::None,
            lis_engine::InstallationKind::Laboratory,
            Some(laboratory_profile()),
        )
        .await;
        let astm = (lab.instrument, lab.run);
        let (method, tests) = file_tests(&lab).await;
        lab.sign_in(Role::Admin).await;
        let instrument = lab
            .engine()
            .register_instrument(RegisterInstrument {
                code: "SYN-LCMS-FILES".to_owned(),
                name: "Synthetic LC-MS/MS result files".to_owned(),
                connection: ConnectionSettings::ResultFile {},
                enabled: true,
            })
            .await
            .unwrap()
            .instrument
            .id;
        lab.instrument = instrument;
        let mut file_lab = Self {
            lab,
            tests,
            method,
            astm,
            folder: tempfile::tempdir().unwrap(),
        };
        file_lab.register_template(HEADER).await;
        crate::m2_config::approve_analyzer(&file_lab.lab.desk, instrument).await;
        file_lab.lab.run = file_lab.open_run().await;
        file_lab.lab.samples.clear();
        file_lab
    }

    pub(crate) fn engine(&self) -> &Engine {
        self.lab.engine()
    }

    /// The administrator drafts a template over `header` (the previous
    /// registration stays active until this one is approved).
    pub(crate) async fn register_template(
        &self,
        header: &str,
    ) -> lis_analytical::SourceRegistrationId {
        self.lab.sign_in(Role::Admin).await;
        let a = &self.lab.analytes;
        self.engine()
            .register_file_source(RegisterFileSourceInput {
                instrument_id: self.lab.instrument,
                software_name: "SynthQuant".to_owned(),
                software_version: "3.4".to_owned(),
                reports_final_concentration: false,
                layout: layout(header),
                test_codes: [("Morphine", a.mor), ("Oxycodone", a.oxy)]
                    .map(|(code, analyte_id)| SourceTestCodeInput {
                        source_test_code: code.to_owned(),
                        analyte_id,
                    })
                    .to_vec(),
            })
            .await
            .unwrap()
            .registration_id
    }

    /// The quality manager approves the instrument's draft registration.
    pub(crate) async fn approve_template(&self) {
        self.lab.sign_in(Role::Quality).await;
        let draft = self
            .engine()
            .list_source_registrations(ListSourceRegistrationsInput {
                instrument_id: self.lab.instrument,
            })
            .await
            .unwrap()
            .into_iter()
            .find(|registration| registration.state == RegistrationState::Draft)
            .unwrap();
        self.engine()
            .approve_source(ApproveSourceInput {
                registration_id: draft.registration_id,
                expected_revision: draft.revision,
                self_approval: false,
            })
            .await
            .unwrap();
    }

    /// The analyst's run of the file method with both control lots.
    async fn open_run(&self) -> RunId {
        self.lab.sign_in(Role::Analyst).await;
        let e = self.engine();
        let lots: Vec<_> = e
            .list_lots(ListLotsInput { active_only: true })
            .await
            .unwrap()
            .into_iter()
            .filter(|lot| lot.sample_label.is_some())
            .map(|lot| lot.lot_id)
            .collect();
        let run = e
            .create_run(CreateRunInput {
                instrument_id: self.lab.instrument,
                method_version_id: self.method,
                lot_ids: lots.clone(),
            })
            .await
            .unwrap()
            .run_id;
        for (n, lot_id) in lots.into_iter().enumerate() {
            e.add_qc_position(AddQcPositionInput {
                run_id: run,
                position_label: format!("FQ{}", n + 1),
                role: QcRole::Control,
                lot_id: Some(lot_id),
                sample_label: None,
            })
            .await
            .unwrap();
        }
        run
    }

    /// An accepted, received sample for `test`, placed on the file run.
    pub(crate) async fn add_sample(&mut self, test: OrderableVersionId) -> Sample {
        let sample = self.lab.add_sample(test).await;
        self.lab.place(&sample).await;
        self.lab.samples.push(sample.clone());
        sample
    }

    /// `rows` under `header`, written to a new file in the folder.
    pub(crate) fn file(&self, name: &str, header: &str, rows: &[String]) -> std::path::PathBuf {
        let mut text = header.to_owned();
        for row in rows {
            text.push('\n');
            text.push_str(row);
        }
        text.push('\n');
        let path = self.folder.path().join(name);
        std::fs::write(&path, text).unwrap();
        path
    }

    /// The analyst loads `path` for the file instrument.
    pub(crate) async fn load(&self, path: std::path::PathBuf) -> LoadedResultFile {
        self.lab.sign_in(Role::Analyst).await;
        self.engine()
            .load_result_file(
                LoadResultFileInput {
                    instrument_id: self.lab.instrument,
                },
                Some(path),
            )
            .await
            .unwrap()
            .unwrap()
    }

    /// Loads a new file of `rows` under the fixture's header and waits for
    /// its first import attempt.
    pub(crate) async fn import(&self, name: &str, rows: &[String]) -> ImportAttemptSummary {
        let loaded = self.load(self.file(name, HEADER, rows)).await;
        self.attempt(loaded.receipt_id, 1).await
    }

    /// Waits for import attempt `attempt_no` of `receipt_id` (any instrument).
    pub(crate) async fn attempt(
        &self,
        receipt_id: ReceiptId,
        attempt_no: i64,
    ) -> ImportAttemptSummary {
        self.lab.sign_in(Role::Reviewer).await;
        for _ in 0..400 {
            let found = self
                .engine()
                .list_import_attempts(ListImportAttemptsInput {
                    after_import_attempt_id: None,
                    instrument_id: None,
                    limit: 100,
                    before: None,
                })
                .await
                .unwrap()
                .into_iter()
                .find(|a| a.receipt_id == receipt_id && a.attempt_no == attempt_no);
            if let Some(attempt) = found {
                return attempt;
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
        panic!("the result-import worker recorded no attempt");
    }

    /// The reviewer's audited attempt detail.
    pub(crate) async fn detail(&self, attempt: &ImportAttemptSummary) -> ImportAttemptDetail {
        self.lab.sign_in(Role::Reviewer).await;
        self.engine()
            .get_import_attempt(GetImportAttemptInput {
                import_attempt_id: attempt.import_attempt_id,
            })
            .await
            .unwrap()
    }

    /// The username of `user_id`.
    pub(crate) fn username(&self, user_id: UserId) -> String {
        self.lab
            .t()
            .conn()
            .query_row(
                "SELECT username FROM auth_users WHERE id = ?1",
                [user_id.to_string()],
                |row| row.get(0),
            )
            .unwrap()
    }

    /// Creates a releaser and releases the report of `sample` with fresh
    /// authentication. The performing site was configured before run creation.
    pub(crate) async fn release(&self, sample: &Sample) -> RevisionSummary {
        self.lab.sign_in(Role::Admin).await;
        let e = self.engine();
        self.lab
            .t()
            .create_user("rel9", &[RoleCode::Releaser])
            .await;
        self.lab.t().first_sign_in("rel9").await;
        let candidate = e
            .prepare_report_candidate(PrepareReportCandidateInput {
                condition_review: None,
                specimen_id: sample.specimen_id,
                amendment_reason: None,
            })
            .await
            .unwrap();
        let released = e
            .release_report(ReleaseReportInput {
                candidate_id: candidate.candidate_id,
                expected_revision: candidate.revision,
                content_sha256: candidate.content_sha256.clone(),
                acknowledge_partial: candidate.completeness == ReportCompleteness::Partial,
                password: secret(NEW_PASSWORD).into(),
                self_approval: false,
            })
            .await
            .unwrap();
        // The desk believed another role was signed in; make the next sign-in real.
        self.lab.sign_in(Role::Admin).await;
        self.lab.sign_in(Role::Quality).await;
        released
    }
}

/// The file method (morphine: LOD 1, LOQ 5; oxycodone: LOQ 2; cutoffs 50)
/// and its three tests, drafted by the administrator and approved by the
/// quality manager.
async fn file_tests(lab: &Lab) -> (MethodVersionId, FileTests) {
    lab.sign_in(Role::Admin).await;
    let e = lab.engine();
    let urine: SpecimenTypeId = e
        .list_specimen_types()
        .await
        .unwrap()
        .into_iter()
        .find(|item| item.code == URINE)
        .unwrap()
        .id;
    let limits = |analyte_id, lod: Option<&str>, loq: &str| MethodAnalyte {
        reference_interval: None,
        range_policy: lis_catalog::RangePolicy::ReportOnly,
        analyte_id,
        unit: Some(UNIT.to_owned()),
        cutoff: Some(DecimalText::parse("50").unwrap()),
        limit_of_detection: lod.map(|text| DecimalText::parse(text).unwrap()),
        limit_of_quantitation: Some(DecimalText::parse(loq).unwrap()),
        reportable_low: None,
        reportable_high: None,
        display_scale: Some(1),
    };
    let a = lab.analytes;
    let analytes = vec![limits(a.mor, Some("1"), "5"), limits(a.oxy, None, "2")];
    let method = draft_method(e, "SYN-FILE-M", urine, analytes).await;
    lab.sign_in(Role::Quality).await;
    e.approve_method_version(ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id: method,
        expected_revision: 1,
        self_approval: false,
    })
    .await
    .unwrap();
    let test = |analyte_ids| {
        vec![ComponentInput {
            method_version_id: method,
            purpose: ComponentPurpose::Analytical,
            analyte_ids,
        }]
    };
    let tests = FileTests {
        both: orderable(&lab.desk, "SYN-FILE-BOTH", test(vec![a.mor, a.oxy]))
            .await
            .1,
        morphine: orderable(&lab.desk, "SYN-FILE-MOR", test(vec![a.mor]))
            .await
            .1,
        oxycodone: orderable(&lab.desk, "SYN-FILE-OXY", test(vec![a.oxy]))
            .await
            .1,
    };
    (method, tests)
}

fn laboratory_profile() -> SetLaboratoryProfile {
    SetLaboratoryProfile {
        expected_version: None,
        name: "Synthetic Files Laboratory".to_owned(),
        address_lines: vec!["200 Synthetic Way".to_owned()],
        clia_id: "00D0000001".to_owned(),
        director: "Director Synthetic".to_owned(),
        phone: "555-0101".to_owned(),
        time_zone: "America/Chicago".to_owned(),
        billing_npi: None,
        tax_id: None,
        reason: None,
    }
}
