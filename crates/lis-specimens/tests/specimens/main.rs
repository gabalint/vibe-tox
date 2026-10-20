//! Integration tests on real SQLite files with the production pragmas. The
//! fixture builds an accepted order through the identity, catalog and orders
//! commands. All names, dates, codes and identifiers are synthetic.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod accessioning;
mod authorization;
mod collection_corrections;
mod configuration;
mod containers;
mod floors;
mod holds;
mod join;
mod labels;
mod order_accession_context;
mod reporting_reads;
mod standing_authority;

use lis_catalog::{
    AnalyteId, ApproveMethodVersion, ApproveOrderableVersion, CodedName, ComponentInput,
    ComponentPurpose, CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    MethodAnalyte, MethodCertainty, MethodVersionContent, MethodVersionId, OrderableKind,
    OrderableVersionContent, OrderableVersionId, OutputScale, ThresholdRounding, ThresholdRule,
};
use lis_core::{Capability, CorrelationId, DecimalText, ErrorCode, Phi, Principal};
use lis_identity::{
    ClientId, CreateClient, DemographicsInput, PractitionerId, RegisterPractitioner,
    RegisterSubject, SubjectId,
};
use lis_orders::{
    AcceptOrderInput, AuthorityEvidenceInput, AuthorityEvidenceKind, BillingResponsibility,
    EvidenceInput, OrderDetail, OrderId, OrderItemId, OrderItemInput, ReflexAuthorization,
    RegisterOrderInput, SequenceRole, SourceKind, TemplateUseInput,
};
use lis_specimens::{
    AccessionDetail, CollectionInput, CollectionTimeInput, CreateAccessionInput, SCHEMA,
    SpecimenError, SpecimenInput, VolumeKind,
};
use lis_store::testing::{TestDatabase, principal};
use lis_store::{CommandContext, lint_schema};
use rusqlite::Connection;

pub(crate) const URINE: &str = "syn-ur";
pub(crate) const ORAL_FLUID: &str = "syn-of";

/// Everything a capability-holding accessioner may do here.
pub(crate) const STAFF: &[Capability] = &[
    Capability::Accession,
    Capability::EnterOrders,
    Capability::ManageQuality,
    Capability::ManageConfiguration,
];

/// A database with the catalog, a subject, a client and a practitioner.
pub(crate) struct Fixture {
    pub(crate) t: TestDatabase,
    pub(crate) subject_id: SubjectId,
    client_id: ClientId,
    practitioner_id: PractitionerId,
    pub(crate) urine_test: OrderableVersionId,
    pub(crate) oral_fluid_test: OrderableVersionId,
}

pub(crate) fn fixture() -> Fixture {
    let t = TestDatabase::open(&[
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_orders::SCHEMA,
        SCHEMA,
    ])
    .unwrap();
    let subject_id = setup(&t, |ctx| {
        lis_identity::register_subject(ctx, synthetic_subject()).map(|done| done.subject_id)
    });
    let client_id = setup(&t, |ctx| {
        lis_identity::create_client(
            ctx,
            CreateClient {
                code: "syn-client".to_owned(),
                name: "Synthetic Clinic".to_owned(),
            },
        )
    });
    let practitioner_id = setup(&t, |ctx| {
        lis_identity::register_practitioner(
            ctx,
            RegisterPractitioner {
                npi: "1234567893".to_owned(),
                family_name: "Synthetic".to_owned(),
                given_names: Some("Prescriber".to_owned()),
            },
        )
    });
    let urine_test = approved_test(&t, URINE);
    let oral_fluid_test = approved_test(&t, ORAL_FLUID);
    Fixture {
        t,
        subject_id,
        client_id,
        practitioner_id,
        urine_test,
        oral_fluid_test,
    }
}

fn synthetic_subject() -> RegisterSubject {
    RegisterSubject {
        demographics: DemographicsInput {
            family_name: Phi::new("SYNTHETIC".to_owned()),
            given_names: Some(Phi::new("Patient Alpha".to_owned())),
            birth_date: Phi::new("1980-01-01".to_owned()),
            administrative_sex: None,
        },
        source: "requisition".to_owned(),
        identifiers: Vec::new(),
        acknowledged_candidates: Vec::new(),
    }
}

/// Runs a setup step as a fresh user holding every capability it may need.
fn setup<T, E, F>(t: &TestDatabase, command: F) -> T
where
    T: Send + 'static,
    E: From<lis_store::StoreError> + std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, E> + Send + 'static,
{
    let author = principal(&[
        Capability::EnterOrders,
        Capability::ManageConfiguration,
        Capability::ApproveConfiguration,
    ]);
    t.db.write_blocking(author, CorrelationId::generate(), command)
        .unwrap()
}

/// An approved single-component test for `matrix`: drafted by one user and
/// approved by another, as the catalog requires.
fn approved_test(t: &TestDatabase, matrix: &str) -> OrderableVersionId {
    let (method_version, analyte) = approved_method(t, matrix);
    let orderable = setup(t, {
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
    let version = setup(t, move |ctx| {
        lis_catalog::draft_orderable_version(
            ctx,
            DraftOrderableVersion {
                orderable_id: orderable,
                content: OrderableVersionContent {
                    components: vec![ComponentInput {
                        method_version_id: method_version,
                        purpose: ComponentPurpose::Analytical,
                        analyte_ids: vec![analyte],
                    }],
                },
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

/// An approved quantitative method for `matrix` with one analyte.
fn approved_method(t: &TestDatabase, matrix: &str) -> (MethodVersionId, AnalyteId) {
    let coded = |code: String| CodedName {
        name: format!("Synthetic {code}"),
        code,
    };
    let specimen_type = setup(t, {
        let coded = coded(matrix.to_owned());
        move |ctx| lis_catalog::create_specimen_type(ctx, coded)
    });
    let analyte = setup(t, {
        let code = format!("{matrix}-a");
        move |ctx| {
            lis_catalog::create_analyte(
                ctx,
                CreateAnalyte {
                    name: format!("Synthetic {code}"),
                    code,
                    loinc_code: None,
                    drug_class_code: None,
                },
            )
        }
    });
    let method = setup(t, {
        let coded = coded(format!("{matrix}-m"));
        move |ctx| lis_catalog::create_method(ctx, coded)
    });
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: specimen_type,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![MethodAnalyte {
            reference_interval: None,
            range_policy: Default::default(),
            analyte_id: analyte,
            unit: Some("ng/mL".to_owned()),
            cutoff: Some(DecimalText::parse("100").unwrap()),
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: Some(0),
        }],
    };
    let method_version = setup(t, move |ctx| {
        lis_catalog::draft_method_version(
            ctx,
            DraftMethodVersion {
                method_id: method,
                content,
            },
        )
    })
    .id;
    setup(t, move |ctx| {
        lis_catalog::approve_method_version(
            ctx,
            ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id: method_version,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    (method_version, analyte)
}

impl Fixture {
    /// Registers an order for `tests`, as supplied on a synthetic requisition.
    pub(crate) fn received_order(&self, tests: &[OrderableVersionId]) -> OrderDetail {
        self.received_standing_order(tests, None)
    }

    fn received_standing_order(
        &self,
        tests: &[OrderableVersionId],
        standing_order_id: Option<lis_orders::StandingOrderId>,
    ) -> OrderDetail {
        let input = RegisterOrderInput {
            subject_id: self.subject_id,
            client_id: self.client_id,
            client_location_id: None,
            practitioner_id: self.practitioner_id,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("SYN-REQ-1".to_owned()),
            billing_responsibility: BillingResponsibility::NotSupplied,
            facility_setting: None,
            standing_order_id,
            template_use: TemplateUseInput::None,
            items: tests
                .iter()
                .map(|test| OrderItemInput {
                    orderable_version_id: *test,
                    sequence_role: SequenceRole::FirstLine,
                    reflex_authorization: ReflexAuthorization::NotStated,
                    reflex_scope_orderable_ids: Vec::new(),
                })
                .collect(),
            evidence: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::WetSignature,
                    document_ref: Some("SYN-REQ-1".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        };
        setup(&self.t, move |ctx| lis_orders::register_order(ctx, input))
    }

    /// An accepted order for `tests`; returns it with its item IDs.
    pub(crate) fn accepted_order(
        &self,
        tests: &[OrderableVersionId],
    ) -> (OrderId, Vec<OrderItemId>) {
        let order = self.received_order(tests);
        let accept = AcceptOrderInput {
            order_id: order.order_id,
            expected_revision: order.revision,
        };
        let accepted = setup(&self.t, move |ctx| lis_orders::accept_order(ctx, accept));
        let items = accepted.items.iter().map(|item| item.item_id).collect();
        (accepted.order_id, items)
    }

    /// An accession of a new accepted urine order: one specimen with
    /// `containers` primary containers of 10.0 mL each.
    pub(crate) fn accession(&self, containers: u8) -> AccessionDetail {
        let (order_id, items) = self.accepted_order(&[self.urine_test]);
        let mut input = accession_input(order_id, items);
        input.specimens[0].container_count = containers;
        run(&self.t, STAFF, move |ctx| {
            lis_specimens::create_accession(ctx, input)
        })
        .unwrap()
    }
}

/// An accession request for one urine specimen with one measured container.
pub(crate) fn accession_input(order_id: OrderId, items: Vec<OrderItemId>) -> CreateAccessionInput {
    CreateAccessionInput {
        order_id,
        order_item_ids: items,
        collection: CollectionInput {
            time: CollectionTimeInput::Datetime {
                local: "2026-01-01T09:30".to_owned(),
                utc_offset_minutes: -300,
            },
            collector: Some("Synthetic Collector".to_owned()),
            observations: Vec::new(),
        },
        specimens: vec![SpecimenInput {
            specimen_type_code: URINE.to_owned(),
            container_count: 1,
            volume: Some(DecimalText::parse("10.0").unwrap()),
            volume_unit: Some("mL".to_owned()),
            volume_kind: VolumeKind::Measured,
        }],
    }
}

/// Runs `command` as a user holding `capabilities`.
pub(crate) fn run<T, F>(
    t: &TestDatabase,
    capabilities: &[Capability],
    command: F,
) -> Result<T, SpecimenError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, SpecimenError> + Send + 'static,
{
    run_as(t, principal(capabilities), command)
}

pub(crate) fn run_as<T, F>(
    t: &TestDatabase,
    principal: Principal,
    command: F,
) -> Result<T, SpecimenError>
where
    T: Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, SpecimenError> + Send + 'static,
{
    t.db.write_blocking(principal, CorrelationId::generate(), command)
}

pub(crate) fn read<T, F>(t: &TestDatabase, query: F) -> T
where
    T: Send + 'static,
    F: FnOnce(&Connection) -> Result<T, SpecimenError> + Send + 'static,
{
    t.db.read_blocking(query).unwrap()
}

/// Row counts of every specimens table, the audit trail and the outbox.
pub(crate) fn counts(t: &TestDatabase) -> Vec<i64> {
    read(t, |conn| {
        let tables: Vec<String> = conn
            .prepare(
                "SELECT name FROM sqlite_schema WHERE type = 'table' \
                 AND (name LIKE 'spc_%' OR name IN ('audit_events', 'store_outbox')) ORDER BY name",
            )?
            .query_map([], |row| row.get(0))?
            .collect::<Result<_, _>>()?;
        tables
            .iter()
            .map(|table| {
                conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                    row.get(0)
                })
                .map_err(SpecimenError::from)
            })
            .collect()
    })
}

/// `(action, record_id, reason)` of the newest clinical audit event.
pub(crate) fn last_event(t: &TestDatabase) -> (String, Option<String>, Option<String>) {
    read(t, |conn| {
        Ok(conn.query_row(
            "SELECT action, record_id, reason FROM audit_events WHERE stream = 'clinical' \
             ORDER BY seq DESC LIMIT 1",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )?)
    })
}

/// A writable connection outside the store, for tamper tests only.
pub(crate) fn raw(f: &Fixture) -> Connection {
    Connection::open(f.t.data_dir().join(lis_store::DATABASE_FILE)).unwrap()
}

/// The error code of a failed command.
pub(crate) fn code_of<T: std::fmt::Debug>(result: Result<T, SpecimenError>) -> ErrorCode {
    result.unwrap_err().code()
}

#[test]
fn schema_passes_the_lint() {
    let modules = [
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_orders::SCHEMA,
        SCHEMA,
    ];
    assert_eq!(lint_schema(&modules).unwrap(), Vec::new());
}

mod material_paging;
