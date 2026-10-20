//! Orders integration tests on production SQLite migrations. Synthetic data only.

#![cfg(test)]

mod billing_context;
mod billing_reviews;
mod historical_labels;
mod paging;

use std::sync::Arc;

use lis_catalog::{
    AnalyteId, ApproveMethodVersion, ApproveOrderableVersion, CodedName, ComponentInput,
    ComponentPurpose, CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    MethodAnalyte, MethodCertainty, MethodVersionContent, OrderableKind, OrderableVersionContent,
    OrderableVersionId, OutputScale, SpecimenTypeId, ThresholdRounding, ThresholdRule,
    approve_method_version, approve_orderable_version, create_analyte, create_method,
    create_orderable, create_specimen_type, draft_method_version, draft_orderable_version,
};
use lis_core::{
    Capability, CorrelationId, FixedClock, ModuleSchema, Phi, Principal, SessionId, UserId,
    WorkstationId,
};
use lis_identity::{
    CreateClient, DemographicsInput, RegisterPractitioner, RegisterSubject, SubjectId,
    create_client, register_practitioner, register_subject,
};
use lis_orders::{
    AcceptOrderInput, AmendOrderEvidenceInput, AuthorityEvidenceInput, AuthorityEvidenceKind,
    BillingResponsibility, BillingReviewElement, ClarificationDeficiency,
    CreateBillingReviewItemInput, DecideCancellationInput, DecideFollowOnInput, Decision,
    EvidenceInput, ItemKind, LaboratoryResultFollowOnInput, LaboratoryResultTriggerNotMetInput,
    ListOrdersInput, MedicationInput, MedicationSource, OrderFilter, OrderItemInput,
    PlaceClarificationHoldInput, PointOfCareResultInput, RecordTriggerNotMetInput,
    ReflexAuthorization, RegisterOrderInput, RegisterStandingOrderInput, RejectOrderInput,
    RequestCancellationInput, RequestFollowOnInput, ResolveClarificationHoldInput,
    ResultRevisionId, SequenceRole, SourceKind, TemplateUseInput, TriggerSource, accept_order,
    amend_order_evidence, create_billing_review_item, decide_cancellation, decide_follow_on,
    items_ready_for_accession, place_clarification_hold, record_trigger_not_met,
    record_trigger_not_met_from_result, register_order, register_standing_order, reject_order,
    request_cancellation, request_follow_on, request_follow_on_from_result,
    resolve_clarification_hold, testing_gate,
};
use lis_store::testing::{TestDatabase, test_start};
use lis_store::{CommandContext, DATABASE_FILE, Database, lint_schema};

struct Fixture {
    t: TestDatabase,
    orderer: Principal,
    subject_id: SubjectId,
    client_id: lis_identity::ClientId,
    practitioner_id: lis_identity::PractitionerId,
    orderable_id: lis_catalog::OrderableId,
    orderable_version_id: OrderableVersionId,
}

impl Fixture {
    fn open() -> Self {
        let t = TestDatabase::open(&[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            lis_orders::SCHEMA,
        ])
        .unwrap();
        let orderer = user(UserId::generate(), &[Capability::EnterOrders]);
        let config = user(UserId::generate(), &[Capability::ManageConfiguration]);
        let approver = user(UserId::generate(), &[Capability::ApproveConfiguration]);
        let (subject_id, client_id, practitioner_id) = identity_fixture(&t, &orderer, &config);
        let (orderable_id, orderable_version_id) = catalog_fixture(&t.db, config, approver);
        Self {
            t,
            orderer,
            subject_id,
            client_id,
            practitioner_id,
            orderable_id,
            orderable_version_id,
        }
    }

    fn register(
        &self,
        standing_order_id: Option<lis_orders::StandingOrderId>,
    ) -> lis_orders::OrderDetail {
        self.register_with_authority(standing_order_id, ReflexAuthorization::Authorized)
    }

    fn register_with_authority(
        &self,
        standing_order_id: Option<lis_orders::StandingOrderId>,
        reflex_authorization: ReflexAuthorization,
    ) -> lis_orders::OrderDetail {
        let input = self.registration_input(standing_order_id, reflex_authorization);
        self.write(move |ctx| register_order(ctx, input)).unwrap()
    }

    fn registration_input(
        &self,
        standing_order_id: Option<lis_orders::StandingOrderId>,
        reflex_authorization: ReflexAuthorization,
    ) -> RegisterOrderInput {
        RegisterOrderInput {
            subject_id: self.subject_id,
            client_id: self.client_id,
            client_location_id: None,
            practitioner_id: self.practitioner_id,
            placer_order_id: None,
            placer_authority: None,
            source_kind: SourceKind::PaperRequisition,
            source_document_ref: Some("synthetic-source".to_owned()),
            billing_responsibility: BillingResponsibility::NotSupplied,
            facility_setting: None,
            standing_order_id,
            template_use: TemplateUseInput::None,
            items: vec![OrderItemInput {
                orderable_version_id: self.orderable_version_id,
                sequence_role: SequenceRole::FirstLine,
                reflex_authorization,
                reflex_scope_orderable_ids: vec![self.orderable_id],
            }],
            evidence: EvidenceInput {
                authority: AuthorityEvidenceInput {
                    kind: AuthorityEvidenceKind::IntentAttestation,
                    document_ref: Some("synthetic-source".to_owned()),
                },
                diagnoses: Vec::new(),
                medications: Vec::new(),
                point_of_care_results: Vec::new(),
            },
        }
    }

    fn write<T, F>(&self, command: F) -> Result<T, lis_orders::OrderError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, lis_orders::OrderError> + Send + 'static,
    {
        self.t
            .db
            .write_blocking(self.orderer.clone(), CorrelationId::generate(), command)
    }
}

#[test]
fn schema_passes_lint() {
    assert_eq!(
        lint_schema(&[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            lis_orders::SCHEMA
        ])
        .unwrap(),
        Vec::new()
    );
}

#[test]
fn v2_requested_items_remain_readable_and_gate_acceptance_after_v3() {
    let dir = tempfile::tempdir().unwrap();
    let legacy = seed_v2_legacy_orders(dir.path());
    let LegacyOrderIds {
        received_order_id,
        accepted_item_id,
        legacy_add_on_id,
        ..
    } = legacy;
    let db = Database::open(
        dir.path(),
        &[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            lis_orders::SCHEMA,
        ],
        Arc::new(FixedClock::new(test_start())),
    )
    .unwrap();
    let detail = db
        .read_blocking(move |conn| lis_orders::get_order(conn, received_order_id))
        .unwrap();
    assert_eq!(detail.items[0].sequence_role, None);
    assert_eq!(detail.allowed_actions, ["reject"]);
    let orderer = user(UserId::generate(), &[Capability::EnterOrders]);
    let refused = db.write_blocking(orderer.clone(), CorrelationId::generate(), move |ctx| {
        accept_order(
            ctx,
            AcceptOrderInput {
                order_id: received_order_id,
                expected_revision: 1,
            },
        )
    });
    assert!(matches!(
        refused,
        Err(lis_orders::OrderError::LegacySequenceRoleMissing)
    ));
    let rejected = db
        .write_blocking(orderer, CorrelationId::generate(), move |ctx| {
            reject_order(
                ctx,
                RejectOrderInput {
                    order_id: received_order_id,
                    expected_revision: 1,
                    reason: "Synthetic legacy order re-entry".to_owned(),
                },
            )
        })
        .unwrap();
    assert_eq!(rejected.status, lis_orders::OrderStatus::Rejected);
    assert_eq!(rejected.items[0].sequence_role, None);
    let cancellation = db
        .write_blocking(
            user(UserId::generate(), &[Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| {
                request_cancellation(
                    ctx,
                    RequestCancellationInput {
                        item_id: accepted_item_id,
                        requested_by: "Synthetic clinic".to_owned(),
                        reason: "Synthetic cancellation with legacy sibling".to_owned(),
                    },
                )
            },
        )
        .unwrap();
    assert_eq!(cancellation.cancellations.len(), 1);
    assert!(
        cancellation
            .items
            .iter()
            .any(|item| item.item_id == legacy_add_on_id && item.sequence_role.is_none())
    );
}

#[derive(Clone, Copy)]
struct LegacyOrderIds {
    received_order_id: lis_orders::OrderId,
    received_item_id: lis_orders::OrderItemId,
    accepted_order_id: lis_orders::OrderId,
    accepted_item_id: lis_orders::OrderItemId,
    legacy_add_on_id: lis_orders::OrderItemId,
    subject_id: SubjectId,
    client_id: lis_identity::ClientId,
    practitioner_id: lis_identity::PractitionerId,
    orderable_version_id: OrderableVersionId,
}

fn seed_v2_legacy_orders(path: &std::path::Path) -> LegacyOrderIds {
    let legacy = ModuleSchema {
        module: "orders",
        table_prefix: "ord_",
        migrations: &lis_orders::MIGRATIONS[..2],
    };
    let db = Database::open(
        path,
        &[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            legacy,
        ],
        Arc::new(FixedClock::new(test_start())),
    )
    .unwrap();
    let (_, orderable_version_id) = catalog_fixture(
        &db,
        user(UserId::generate(), &[Capability::ManageConfiguration]),
        user(UserId::generate(), &[Capability::ApproveConfiguration]),
    );
    drop(db);
    let ids = LegacyOrderIds {
        received_order_id: lis_orders::OrderId::generate(),
        received_item_id: lis_orders::OrderItemId::generate(),
        accepted_order_id: lis_orders::OrderId::generate(),
        accepted_item_id: lis_orders::OrderItemId::generate(),
        legacy_add_on_id: lis_orders::OrderItemId::generate(),
        subject_id: SubjectId::generate(),
        client_id: lis_identity::ClientId::generate(),
        practitioner_id: lis_identity::PractitionerId::generate(),
        orderable_version_id,
    };
    let conn = rusqlite::Connection::open(path.join(DATABASE_FILE)).unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    insert_received_legacy_order(&conn, &ids);
    insert_accepted_legacy_order(&conn, &ids);
    ids
}

fn insert_received_legacy_order(conn: &rusqlite::Connection, ids: &LegacyOrderIds) {
    conn.execute(
        "INSERT INTO ord_orders (id, subject_id, client_id, practitioner_id, source_kind, \
         billing_responsibility, template_use, status, version, received_at, received_by_kind, \
         received_by_id, revision) VALUES (?1, ?2, ?3, ?4, 'paper_requisition', 'not_supplied', \
         'none', 'received', 1, ?5, 'user', ?6, 1)",
        rusqlite::params![
            ids.received_order_id,
            ids.subject_id,
            ids.client_id,
            ids.practitioner_id,
            test_start(),
            UserId::generate().to_string()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ord_order_items (id, order_id, orderable_version_id, kind, \
         reflex_authorization, state, requested_at, revision) \
         VALUES (?1, ?2, ?3, 'initial', 'not_stated', 'requested', ?4, 1)",
        rusqlite::params![
            ids.received_item_id,
            ids.received_order_id,
            ids.orderable_version_id,
            test_start()
        ],
    )
    .unwrap();
}

fn insert_accepted_legacy_order(conn: &rusqlite::Connection, ids: &LegacyOrderIds) {
    conn.execute(
        "INSERT INTO ord_orders (id, subject_id, client_id, practitioner_id, source_kind, \
         billing_responsibility, template_use, status, version, received_at, received_by_kind, \
         received_by_id, decided_at, decided_by, revision) \
         VALUES (?1, ?2, ?3, ?4, 'paper_requisition', 'not_supplied', 'none', 'accepted', 1, \
                 ?5, 'user', ?6, ?5, ?6, 1)",
        rusqlite::params![
            ids.accepted_order_id,
            ids.subject_id,
            ids.client_id,
            ids.practitioner_id,
            test_start(),
            UserId::generate().to_string()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ord_order_items (id, order_id, orderable_version_id, kind, \
         reflex_authorization, state, requested_at, decided_at, decided_by, revision) \
         VALUES (?1, ?2, ?3, 'initial', 'not_stated', 'accepted', ?4, ?4, 'synthetic-user', 1)",
        rusqlite::params![
            ids.accepted_item_id,
            ids.accepted_order_id,
            ids.orderable_version_id,
            test_start()
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ord_order_components (id, item_id, catalog_orderable_version_id, \
         catalog_component_position, specimen_type_code, purpose, sequence_role, state, revision) \
         VALUES (?1, ?2, ?3, 1, 'SYN', 'analytical', 'first_line', 'active', 1)",
        rusqlite::params![
            lis_orders::OrderComponentId::generate(),
            ids.accepted_item_id,
            ids.orderable_version_id
        ],
    )
    .unwrap();
    conn.execute(
        "INSERT INTO ord_order_items (id, order_id, orderable_version_id, kind, parent_item_id, \
         reflex_authorization, state, requested_at, reason, revision) \
         VALUES (?1, ?2, ?3, 'reflex', ?4, 'authorized', 'requested', ?5, \
                 'Synthetic legacy follow-on', 1)",
        rusqlite::params![
            ids.legacy_add_on_id,
            ids.accepted_order_id,
            ids.orderable_version_id,
            ids.accepted_item_id,
            test_start()
        ],
    )
    .unwrap();
}

#[test]
fn accepted_items_are_ready_only_without_an_open_hold() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let before = fixture
        .t
        .db
        .read_blocking(move |conn| items_ready_for_accession(conn, received.order_id))
        .unwrap();
    assert!(before.is_empty());
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let order_id = accepted.order_id;
    assert_item_role(&accepted.items[0], SequenceRole::FirstLine);
    assert_eq!(
        fixture
            .t
            .db
            .read_blocking(move |conn| items_ready_for_accession(conn, order_id))
            .unwrap()
            .len(),
        1
    );
    let held = fixture
        .write(move |ctx| {
            place_clarification_hold(
                ctx,
                PlaceClarificationHoldInput {
                    order_id: accepted.order_id,
                    expected_revision: accepted.revision,
                    deficiency: ClarificationDeficiency::OtherAuthority,
                    note: None,
                },
            )
        })
        .unwrap();
    let component_id = held.items[0].components[0].component_id;
    assert_testing_gate(&fixture, component_id, "clarification_hold_open");
    let order_id = held.order_id;
    assert!(
        fixture
            .t
            .db
            .read_blocking(move |conn| items_ready_for_accession(conn, order_id))
            .unwrap()
            .is_empty()
    );
    let hold = held.holds.iter().find(|hold| hold.state == "open").unwrap();
    let hold_id = hold.hold_id;
    let hold_revision = hold.revision;
    let resolved = fixture
        .write(move |ctx| {
            resolve_clarification_hold(
                ctx,
                ResolveClarificationHoldInput {
                    hold_id,
                    expected_revision: hold_revision,
                    reason: "Synthetic resolution".to_owned(),
                },
            )
        })
        .unwrap();
    let order_id = resolved.order_id;
    assert_eq!(
        fixture
            .t
            .db
            .read_blocking(move |conn| items_ready_for_accession(conn, order_id))
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn order_list_names_patient_client_placer_number_and_item_count() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let follow_on = add_on(&fixture, &accepted, accepted.revision);
    let first = fixture
        .write(move |ctx| request_follow_on(ctx, follow_on))
        .unwrap();
    let mut input = fixture.registration_input(None, ReflexAuthorization::Authorized);
    input.subject_id = register_second_subject(&fixture);
    input.placer_order_id = Some("PO-SYN-0002".to_owned());
    input.placer_authority = Some("SYN-CLINIC".to_owned());
    let second = fixture
        .write(move |ctx| register_order(ctx, input))
        .unwrap();

    let listed: Vec<_> = list_orders(&fixture, 10)
        .unwrap()
        .into_iter()
        .map(|order| {
            (
                order.order_id,
                order.patient_family_name.expose().clone(),
                order.patient_given_names.map(|name| name.expose().clone()),
                order.placer_order_id,
                order.client_name,
                order.item_count,
            )
        })
        .collect();
    let text = |value: &str| value.to_owned();
    assert_eq!(
        listed,
        [
            (
                second.order_id,
                text("Testpatient"),
                None,
                Some(text("PO-SYN-0002")),
                text("Synthetic clinic"),
                1
            ),
            (
                first.order_id,
                text("Synthetic"),
                Some(text("Order")),
                None,
                text("Synthetic clinic"),
                2
            ),
        ]
    );
}

#[test]
fn order_list_refuses_a_limit_outside_1_to_500() {
    let fixture = Fixture::open();
    for limit in [0, 501] {
        assert!(
            matches!(
                list_orders(&fixture, limit),
                Err(lis_orders::OrderError::Invalid(_))
            ),
            "limit {limit} must be refused"
        );
    }
}

#[test]
fn order_list_page_filters_use_paging_indexes() {
    let fixture = Fixture::open();
    let plans = fixture
        .t
        .db
        .read_blocking(|conn| {
            let queries = [
                "SELECT id FROM ord_orders ORDER BY received_at DESC, id DESC LIMIT 50",
                "SELECT id FROM ord_orders WHERE status = 'accepted' \
                 ORDER BY received_at DESC, id DESC LIMIT 50",
                "SELECT id FROM ord_orders WHERE subject_id = 'synthetic-subject' \
                 ORDER BY received_at DESC, id DESC LIMIT 50",
            ];
            queries
                .into_iter()
                .map(|sql| {
                    Ok(conn
                        .prepare(&format!("EXPLAIN QUERY PLAN {sql}"))?
                        .query_map([], |row| row.get::<_, String>(3))?
                        .collect::<Result<Vec<_>, _>>()?)
                })
                .collect::<Result<Vec<_>, lis_orders::OrderError>>()
        })
        .unwrap();
    for (plan, index) in plans.iter().zip([
        "ord_orders_received_page",
        "ord_orders_status_received_page",
        "ord_orders_subject_received_page",
    ]) {
        assert!(
            plan.iter().any(|step| step.contains(index)),
            "expected {index} in query plan: {plan:?}"
        );
    }
}

fn list_orders(
    fixture: &Fixture,
    limit: u32,
) -> Result<Vec<lis_orders::OrderSummary>, lis_orders::OrderError> {
    fixture.t.db.read_blocking(move |conn| {
        lis_orders::list_orders(
            conn,
            &ListOrdersInput {
                filter: OrderFilter::All,
                subject_id: None,
                limit,
                before_received_at: None,
                before_order_id: None,
            },
        )
    })
}

fn register_second_subject(fixture: &Fixture) -> SubjectId {
    fixture
        .write(|ctx| {
            register_subject(
                ctx,
                RegisterSubject {
                    demographics: DemographicsInput {
                        family_name: Phi::new("Testpatient".to_owned()),
                        given_names: None,
                        birth_date: Phi::new("1980-02-03".to_owned()),
                        administrative_sex: None,
                    },
                    source: "synthetic_requisition".to_owned(),
                    identifiers: Vec::new(),
                    acknowledged_candidates: Vec::new(),
                },
            )
            .map_err(lis_orders::OrderError::from)
        })
        .unwrap()
        .subject_id
}

#[test]
fn delayed_standing_order_intake_uses_collection_dates_instead_of_current_date() {
    let fixture = Fixture::open();
    let subject_id = fixture.subject_id;
    let practitioner_id = fixture.practitioner_id;
    let orderable_id = fixture.orderable_id;
    let standing = fixture
        .write(move |ctx| {
            register_standing_order(
                ctx,
                RegisterStandingOrderInput {
                    subject_id,
                    practitioner_id,
                    scope_orderable_ids: vec![orderable_id],
                    valid_from: "2025-01-01".to_owned(),
                    valid_until: "2025-12-31".to_owned(),
                    visit_limit: None,
                    document_ref: "synthetic-standing".to_owned(),
                },
            )
        })
        .unwrap();
    let received = fixture.register(Some(standing.standing_order_id));
    let decided = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    assert_eq!(decided.status, lis_orders::OrderStatus::Accepted);
    assert!(decided.holds.is_empty());
    let order_id = decided.order_id;
    for (date, expected) in [
        (Some("2025-01-01"), None),
        (Some("2025-12-31"), None),
        (
            Some("2024-12-31"),
            Some("standing_collection_outside_validity"),
        ),
        (
            Some("2026-01-01"),
            Some("standing_collection_outside_validity"),
        ),
        (None, Some("standing_collection_unknown")),
    ] {
        let gates = fixture
            .t
            .db
            .read_blocking(move |conn| lis_orders::standing_collection_gate(conn, order_id, date))
            .unwrap();
        assert_eq!(gates.first().map(|gate| gate.code.as_str()), expected);
    }
    let authority = fixture
        .t
        .db
        .read_blocking(move |conn| lis_orders::list_standing_orders(conn, subject_id))
        .unwrap();
    assert_eq!(authority[0].accepted_uses, 1);
}

#[test]
fn missing_active_authority_commits_one_hold_and_keeps_the_order_received() {
    let fixture = Fixture::open();
    let mut input = fixture.registration_input(None, ReflexAuthorization::Authorized);
    input.evidence.authority = AuthorityEvidenceInput {
        kind: AuthorityEvidenceKind::NotSupplied,
        document_ref: None,
    };
    let received = fixture
        .write(move |ctx| register_order(ctx, input))
        .unwrap();
    let decided = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    assert_eq!(decided.status, lis_orders::OrderStatus::Received);
    assert!(decided.items[0].components.is_empty());
    assert_eq!(decided.revision, received.revision + 1);
    assert_eq!(
        decided
            .holds
            .iter()
            .filter(|hold| {
                hold.state == "open"
                    && hold.deficiency == ClarificationDeficiency::SignatureOrIntentRequired
            })
            .count(),
        1
    );

    let retry = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: decided.order_id,
                    expected_revision: decided.revision,
                },
            )
        })
        .unwrap();
    assert_eq!(retry.status, lis_orders::OrderStatus::Received);
    assert_eq!(retry.revision, decided.revision);
    assert_eq!(
        retry
            .holds
            .iter()
            .filter(|hold| {
                hold.state == "open"
                    && hold.deficiency == ClarificationDeficiency::SignatureOrIntentRequired
            })
            .count(),
        1
    );
}

#[test]
fn add_on_without_authority_cannot_be_accepted() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let mut input = add_on(&fixture, &accepted, accepted.revision);
    input.authority_reference = None;
    let pending = fixture
        .write(move |ctx| request_follow_on(ctx, input))
        .unwrap();
    let item = pending.items.last().unwrap();
    let item_id = item.item_id;
    let item_revision = item.revision;
    let before = audit_count(&fixture);
    let error = fixture
        .write(move |ctx| {
            decide_follow_on(
                ctx,
                DecideFollowOnInput {
                    item_id,
                    expected_revision: item_revision,
                    decision: Decision::Accept,
                    reason: "Synthetic add-on decision".to_owned(),
                },
            )
        })
        .unwrap_err();
    assert_eq!(error.code(), lis_core::ErrorCode::GateBlocked);
    assert_eq!(audit_count(&fixture), before);
    let order_id = pending.order_id;
    let unchanged = fixture
        .t
        .db
        .read_blocking(move |conn| lis_orders::get_order(conn, order_id))
        .unwrap();
    let item = unchanged
        .items
        .iter()
        .find(|item| item.item_id == item_id)
        .unwrap();
    assert_eq!(item.state, lis_orders::ItemState::Requested);
    assert!(item.components.is_empty());
}

#[test]
fn public_commands_refuse_laboratory_result_trigger_references() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let parent_item_id = accepted.items[0].item_id;
    let before = audit_count(&fixture);
    let request = follow_on(&fixture, &accepted, parent_item_id);
    assert_eq!(
        fixture
            .write(move |ctx| request_follow_on(ctx, request))
            .unwrap_err()
            .code(),
        lis_core::ErrorCode::Validation
    );
    let assessment = not_met(&fixture, parent_item_id);
    assert_eq!(
        fixture
            .write(move |ctx| record_trigger_not_met(ctx, assessment))
            .unwrap_err()
            .code(),
        lis_core::ErrorCode::Validation
    );
    assert_eq!(audit_count(&fixture), before);
}

#[test]
fn reflex_scope_is_enforced_and_trigger_replay_is_idempotent() {
    let fixture = Fixture::open();
    let received = fixture.register_with_authority(None, ReflexAuthorization::NotAuthorized);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let parent_item_id = accepted.items[0].item_id;
    let follow_on = result_follow_on(&fixture, &accepted, parent_item_id);
    let refused = fixture
        .write(move |ctx| request_follow_on_from_result(ctx, follow_on))
        .unwrap_err();
    assert_eq!(refused.code(), lis_core::ErrorCode::GateBlocked);

    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let parent_item_id = accepted.items[0].item_id;
    // A result revision of another order cannot decide this parent's trigger.
    let unrelated = result_not_met(&fixture, lis_orders::OrderId::generate(), parent_item_id);
    let refused = fixture
        .write(move |ctx| record_trigger_not_met_from_result(ctx, unrelated))
        .unwrap_err();
    assert_eq!(refused.code(), lis_core::ErrorCode::GateBlocked);
    let first_input = result_not_met(&fixture, accepted.order_id, parent_item_id);
    let replay_input = first_input.clone();
    let first = fixture
        .write(move |ctx| record_trigger_not_met_from_result(ctx, first_input))
        .unwrap();
    let before = audit_count(&fixture);
    let replay = fixture
        .write(move |ctx| record_trigger_not_met_from_result(ctx, replay_input))
        .unwrap();
    assert_eq!(replay.trigger_assessments.len(), 1);
    assert_eq!(replay.revision, first.revision);
    assert_eq!(audit_count(&fixture), before);
}

#[test]
fn reflex_freezes_the_supplied_sequence_role_without_kind_inference() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let parent_item_id = accepted.items[0].item_id;
    let mut input = result_follow_on(&fixture, &accepted, parent_item_id);
    input.sequence_role = SequenceRole::FirstLine;
    let requested = fixture
        .write(move |ctx| request_follow_on_from_result(ctx, input))
        .unwrap();
    let item = requested.items.last().unwrap();
    assert_eq!(item.kind, ItemKind::Reflex);
    assert_eq!(item.sequence_role, Some(SequenceRole::FirstLine));
    let item_id = item.item_id;
    let item_revision = item.revision;
    let accepted = fixture
        .write(move |ctx| {
            decide_follow_on(
                ctx,
                DecideFollowOnInput {
                    item_id,
                    expected_revision: item_revision,
                    decision: Decision::Accept,
                    reason: "Synthetic reflex review".to_owned(),
                },
            )
        })
        .unwrap();
    let item = accepted
        .items
        .iter()
        .find(|candidate| candidate.item_id == item_id)
        .unwrap();
    assert!(
        item.components
            .iter()
            .all(|component| component.sequence_role == SequenceRole::FirstLine)
    );
}

#[test]
fn frozen_sequence_role_refuses_database_reclassification() {
    let fixture = Fixture::open();
    let accepted = prepare_accepted_order(&fixture);
    let item_id = accepted.items[0].item_id;
    let connection =
        rusqlite::Connection::open(fixture.t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    let error = connection
        .execute(
            "UPDATE ord_order_items SET sequence_role = 'confirmation' WHERE id = ?1",
            [item_id],
        )
        .unwrap_err();
    assert!(error.to_string().contains("sequence role is immutable"));
}

#[test]
fn accepted_cancellation_keeps_evidence_and_cancels_components() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let item_id = accepted.items[0].item_id;
    let evidence_before = accepted.evidence.authority.len();
    let requested = fixture
        .write(move |ctx| {
            request_cancellation(
                ctx,
                RequestCancellationInput {
                    item_id,
                    requested_by: "Synthetic clinic".to_owned(),
                    reason: "Synthetic cancellation".to_owned(),
                },
            )
        })
        .unwrap();
    let request = requested.cancellations.last().unwrap();
    let request_id = request.request_id;
    let request_revision = request.revision;
    let cancelled = fixture
        .write(move |ctx| {
            decide_cancellation(
                ctx,
                DecideCancellationInput {
                    request_id,
                    expected_revision: request_revision,
                    decision: Decision::Accept,
                    reason: "Synthetic acceptance".to_owned(),
                },
            )
        })
        .unwrap();
    assert_eq!(cancelled.evidence.authority.len(), evidence_before);
    assert_eq!(cancelled.items[0].state, lis_orders::ItemState::Cancelled);
    assert!(
        cancelled.items[0]
            .components
            .iter()
            .all(|component| component.state == lis_orders::ComponentState::Cancelled)
    );
    let component_id = cancelled.items[0].components[0].component_id;
    assert_testing_gate(&fixture, component_id, "component_cancelled");
    assert_testing_gate(&fixture, component_id, "item_not_accepted");
    let order_id = cancelled.order_id;
    assert!(
        fixture
            .t
            .db
            .read_blocking(move |conn| items_ready_for_accession(conn, order_id))
            .unwrap()
            .is_empty()
    );
}

#[test]
fn stale_revision_refuses_acceptance_without_an_audit_event() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let before = audit_count(&fixture);
    let error = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: 99,
                },
            )
        })
        .unwrap_err();
    assert_eq!(error.code(), lis_core::ErrorCode::Conflict);
    assert_eq!(audit_count(&fixture), before);
}

#[test]
fn supplied_context_is_preserved_and_poc_reflex_requires_its_active_evidence() {
    let fixture = Fixture::open();
    let mut input = fixture.registration_input(None, ReflexAuthorization::Authorized);
    input.evidence.medications.push(MedicationInput {
        received_text: "  Synthetic medication text  ".to_owned(),
        code: None,
        code_system: None,
        source: MedicationSource::Patient,
        reporter: Some("Synthetic reporter".to_owned()),
        provided_on: Some("as stated on synthetic form".to_owned()),
        as_of_date: None,
    });
    input
        .evidence
        .point_of_care_results
        .push(PointOfCareResultInput {
            device_or_test: "Synthetic POC device".to_owned(),
            analyte_text: "Synthetic analyte as supplied".to_owned(),
            result_text: "Synthetic supplied result".to_owned(),
            performed_at_text: None,
            source: "Synthetic clinic".to_owned(),
        });
    let received = fixture
        .write(move |ctx| register_order(ctx, input))
        .unwrap();
    assert_eq!(
        received.evidence.medications[0].received_text,
        "  Synthetic medication text  "
    );
    let poc_id = received.evidence.point_of_care_results[0].evidence_id;
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    assert_eq!(
        accepted.items[0].components[0].purpose,
        ComponentPurpose::Analytical
    );
    let parent_item_id = accepted.items[0].item_id;
    let mut fake = follow_on(&fixture, &accepted, parent_item_id);
    fake.trigger_source = Some(TriggerSource::ClientPointOfCare);
    fake.trigger_ref = None;
    fake.poc_result_id = Some(lis_orders::OrderEvidenceId::generate());
    assert_eq!(
        fixture
            .write(move |ctx| request_follow_on(ctx, fake))
            .unwrap_err()
            .code(),
        lis_core::ErrorCode::GateBlocked
    );
    let mut real = follow_on(&fixture, &accepted, parent_item_id);
    real.trigger_source = Some(TriggerSource::ClientPointOfCare);
    real.trigger_ref = None;
    real.poc_result_id = Some(poc_id);
    let activated = fixture
        .write(move |ctx| request_follow_on(ctx, real))
        .unwrap();
    assert_eq!(activated.trigger_assessments.len(), 1);
}

#[test]
fn standing_order_exhaustion_and_scope_failures_commit_system_holds() {
    let fixture = Fixture::open();
    let standing = register_standing(&fixture, vec![fixture.orderable_id], Some(1));
    let first = fixture.register(Some(standing.standing_order_id));
    fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: first.order_id,
                    expected_revision: first.revision,
                },
            )
        })
        .unwrap();
    let second = fixture.register(Some(standing.standing_order_id));
    let exhausted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: second.order_id,
                    expected_revision: second.revision,
                },
            )
        })
        .unwrap();
    assert!(exhausted.holds.iter().any(|hold| {
        hold.deficiency == ClarificationDeficiency::StandingOrderExhausted
            && hold.origin == "standing_order_check"
    }));

    let fixture = Fixture::open();
    let config = user(UserId::generate(), &[Capability::ManageConfiguration]);
    let other_orderable = fixture
        .t
        .db
        .write_blocking(config, CorrelationId::generate(), |ctx| {
            create_orderable(
                ctx,
                CreateOrderable {
                    code: "SYN-OTHER".to_owned(),
                    name: "Synthetic other orderable".to_owned(),
                    kind: OrderableKind::Test,
                },
            )
        })
        .unwrap();
    let standing = register_standing(&fixture, vec![other_orderable], None);
    let received = fixture.register(Some(standing.standing_order_id));
    let out_of_scope = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    assert!(out_of_scope.holds.iter().any(|hold| {
        hold.deficiency == ClarificationDeficiency::StandingOrderOutOfScope
            && hold.origin == "standing_order_check"
    }));
}

#[test]
fn every_revisioned_order_transition_refuses_stale_input_without_audit() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    assert_stale(&fixture, move |ctx| {
        amend_order_evidence(
            ctx,
            AmendOrderEvidenceInput {
                order_id: received.order_id,
                expected_revision: 99,
                add: empty_evidence(),
                remove_evidence_ids: Vec::new(),
                reason: "Synthetic stale amendment".to_owned(),
            },
        )
    });

    let fixture = Fixture::open();
    let received = fixture.register(None);
    assert_stale(&fixture, move |ctx| {
        reject_order(
            ctx,
            RejectOrderInput {
                order_id: received.order_id,
                expected_revision: 99,
                reason: "Synthetic stale rejection".to_owned(),
            },
        )
    });

    let fixture = Fixture::open();
    let received = fixture.register(None);
    assert_stale(&fixture, move |ctx| {
        place_clarification_hold(
            ctx,
            PlaceClarificationHoldInput {
                order_id: received.order_id,
                expected_revision: 99,
                deficiency: ClarificationDeficiency::OtherAuthority,
                note: None,
            },
        )
    });

    let fixture = Fixture::open();
    let received = fixture.register(None);
    let held = fixture
        .write(move |ctx| {
            place_clarification_hold(
                ctx,
                PlaceClarificationHoldInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                    deficiency: ClarificationDeficiency::OtherAuthority,
                    note: None,
                },
            )
        })
        .unwrap();
    let hold_id = held.holds[0].hold_id;
    assert_stale(&fixture, move |ctx| {
        resolve_clarification_hold(
            ctx,
            ResolveClarificationHoldInput {
                hold_id,
                expected_revision: 99,
                reason: "Synthetic stale resolution".to_owned(),
            },
        )
    });
}

#[test]
fn follow_on_and_cancellation_decisions_refuse_stale_input_without_audit() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let accepted = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                },
            )
        })
        .unwrap();
    let stale_follow_on = add_on(&fixture, &accepted, 99);
    assert_stale(&fixture, move |ctx| request_follow_on(ctx, stale_follow_on));

    let pending = fixture
        .write({
            let input = add_on(&fixture, &accepted, accepted.revision);
            move |ctx| request_follow_on(ctx, input)
        })
        .unwrap();
    let pending_item = pending.items.last().unwrap().item_id;
    assert_stale(&fixture, move |ctx| {
        decide_follow_on(
            ctx,
            DecideFollowOnInput {
                item_id: pending_item,
                expected_revision: 99,
                decision: Decision::Decline,
                reason: "Synthetic stale decision".to_owned(),
            },
        )
    });

    let original_item = accepted.items[0].item_id;
    let cancellation = fixture
        .write(move |ctx| {
            request_cancellation(
                ctx,
                RequestCancellationInput {
                    item_id: original_item,
                    requested_by: "Synthetic clinic".to_owned(),
                    reason: "Synthetic cancellation".to_owned(),
                },
            )
        })
        .unwrap();
    let request_id = cancellation.cancellations.last().unwrap().request_id;
    assert_stale(&fixture, move |ctx| {
        decide_cancellation(
            ctx,
            DecideCancellationInput {
                request_id,
                expected_revision: 99,
                decision: Decision::Decline,
                reason: "Synthetic stale cancellation decision".to_owned(),
            },
        )
    });
}

#[test]
fn open_hold_blocks_acceptance_without_audit() {
    let fixture = Fixture::open();
    let received = fixture.register(None);
    let held = fixture
        .write(move |ctx| {
            place_clarification_hold(
                ctx,
                PlaceClarificationHoldInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                    deficiency: ClarificationDeficiency::SignatureOrIntentRequired,
                    note: None,
                },
            )
        })
        .unwrap();
    let before = audit_count(&fixture);
    let error = fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: held.order_id,
                    expected_revision: held.revision,
                },
            )
        })
        .unwrap_err();
    assert_eq!(error.code(), lis_core::ErrorCode::GateBlocked);
    assert_eq!(audit_count(&fixture), before);
}

#[test]
fn every_successful_order_write_emits_its_named_audit_action() {
    let fixture = Fixture::open();
    register_standing(&fixture, vec![fixture.orderable_id], None);
    let accepted = prepare_accepted_order(&fixture);
    exercise_post_accept_commands(&fixture, &accepted);

    let received = fixture.register(None);
    fixture
        .write(move |ctx| {
            reject_order(
                ctx,
                RejectOrderInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                    reason: "Synthetic rejection".to_owned(),
                },
            )
        })
        .unwrap();

    let actions = audit_actions(&fixture);
    for expected in [
        "order.register_standing_order",
        "order.register",
        "order.amend_evidence",
        "order.hold",
        "order.resolve_hold",
        "order.billing_review",
        "order.accept",
        "order.request_follow_on",
        "order.decide_follow_on",
        "order.trigger_not_met",
        "order.request_cancel",
        "order.decide_cancel",
        "order.reject",
    ] {
        assert!(
            actions.contains(expected),
            "missing audit action {expected}"
        );
    }
}

#[test]
fn billing_reviews_and_standing_orders_are_append_only() {
    let fixture = Fixture::open();
    let standing = register_standing(&fixture, vec![fixture.orderable_id], None);
    let received = fixture.register(None);
    let reviewed = fixture
        .write(move |ctx| {
            create_billing_review_item(
                ctx,
                CreateBillingReviewItemInput {
                    order_id: received.order_id,
                    element: BillingReviewElement::Diagnosis,
                    note: Some("Synthetic review".to_owned()),
                },
            )
        })
        .unwrap();
    let review_id = reviewed.billing_review_items[0].item_id;
    let standing_id = standing.standing_order_id;
    let connection =
        rusqlite::Connection::open(fixture.t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    for (sql, id) in [
        (
            "UPDATE ord_billing_review_items SET note = 'tampered' WHERE id = ?1",
            review_id.to_string(),
        ),
        (
            "DELETE FROM ord_billing_review_items WHERE id = ?1",
            review_id.to_string(),
        ),
        (
            "UPDATE ord_standing_orders SET document_ref = 'tampered' WHERE id = ?1",
            standing_id.to_string(),
        ),
        (
            "DELETE FROM ord_standing_orders WHERE id = ?1",
            standing_id.to_string(),
        ),
    ] {
        let error = connection.execute(sql, [id]).unwrap_err();
        assert!(error.to_string().contains("append-only"), "{error}");
    }
}

fn identity_fixture(
    t: &TestDatabase,
    orderer: &Principal,
    config: &Principal,
) -> (
    SubjectId,
    lis_identity::ClientId,
    lis_identity::PractitionerId,
) {
    let subject_id =
        t.db.write_blocking(orderer.clone(), CorrelationId::generate(), |ctx| {
            register_subject(
                ctx,
                RegisterSubject {
                    demographics: DemographicsInput {
                        family_name: Phi::new("Synthetic".to_owned()),
                        given_names: Some(Phi::new("Order".to_owned())),
                        birth_date: Phi::new("1990-01-01".to_owned()),
                        administrative_sex: Some(Phi::new("U".to_owned())),
                    },
                    source: "synthetic_requisition".to_owned(),
                    identifiers: Vec::new(),
                    acknowledged_candidates: Vec::new(),
                },
            )
        })
        .unwrap()
        .subject_id;
    let client_id =
        t.db.write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            create_client(
                ctx,
                CreateClient {
                    code: "SYN-CLINIC".to_owned(),
                    name: "Synthetic clinic".to_owned(),
                },
            )
        })
        .unwrap();
    let practitioner_id =
        t.db.write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            register_practitioner(
                ctx,
                RegisterPractitioner {
                    npi: "1234567893".to_owned(),
                    family_name: "Synthetic".to_owned(),
                    given_names: Some("Practitioner".to_owned()),
                },
            )
        })
        .unwrap();
    (subject_id, client_id, practitioner_id)
}

fn catalog_fixture(
    db: &Database,
    config: Principal,
    approver: Principal,
) -> (lis_catalog::OrderableId, OrderableVersionId) {
    let specimen_type_id = db
        .write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            create_specimen_type(ctx, coded("SYN-UR", "Synthetic urine"))
        })
        .unwrap();
    let analyte_id = db
        .write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            create_analyte(
                ctx,
                CreateAnalyte {
                    code: "SYN-A".to_owned(),
                    name: "Synthetic analyte".to_owned(),
                    loinc_code: None,
                    drug_class_code: None,
                },
            )
        })
        .unwrap();
    let method_version_id = approved_method(
        db,
        config.clone(),
        approver.clone(),
        specimen_type_id,
        analyte_id,
    );
    let orderable_id = db
        .write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            create_orderable(
                ctx,
                CreateOrderable {
                    code: "SYN-O".to_owned(),
                    name: "Synthetic orderable".to_owned(),
                    kind: OrderableKind::Test,
                },
            )
        })
        .unwrap();
    let orderable_version_id = db
        .write_blocking(config, CorrelationId::generate(), move |ctx| {
            draft_orderable_version(
                ctx,
                DraftOrderableVersion {
                    orderable_id,
                    content: OrderableVersionContent {
                        components: vec![ComponentInput {
                            method_version_id,
                            purpose: ComponentPurpose::Analytical,
                            analyte_ids: vec![analyte_id],
                        }],
                    },
                },
            )
        })
        .unwrap()
        .id;
    db.write_blocking(approver, CorrelationId::generate(), move |ctx| {
        approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })
    .unwrap();
    (orderable_id, orderable_version_id)
}

fn approved_method(
    db: &Database,
    config: Principal,
    approver: Principal,
    specimen_type_id: SpecimenTypeId,
    analyte_id: AnalyteId,
) -> lis_catalog::MethodVersionId {
    let method_id = db
        .write_blocking(config.clone(), CorrelationId::generate(), |ctx| {
            create_method(ctx, coded("SYN-M", "Synthetic method"))
        })
        .unwrap();
    let method_version_id = db
        .write_blocking(config, CorrelationId::generate(), move |ctx| {
            draft_method_version(
                ctx,
                DraftMethodVersion {
                    method_id,
                    content: method_content(specimen_type_id, analyte_id),
                },
            )
        })
        .unwrap()
        .id;
    db.write_blocking(approver, CorrelationId::generate(), move |ctx| {
        approve_method_version(
            ctx,
            ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })
    .unwrap();
    method_version_id
}

fn user(user_id: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        user_id,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

fn coded(code: &str, name: &str) -> CodedName {
    CodedName {
        code: code.to_owned(),
        name: name.to_owned(),
    }
}

fn method_content(specimen_type_id: SpecimenTypeId, analyte_id: AnalyteId) -> MethodVersionContent {
    MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![MethodAnalyte {
            reference_interval: None,
            range_policy: lis_catalog::RangePolicy::ReportOnly,
            analyte_id,
            unit: None,
            cutoff: None,
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: None,
        }],
    }
}

fn prepare_accepted_order(fixture: &Fixture) -> lis_orders::OrderDetail {
    let received = fixture.register(None);
    let amended = fixture
        .write(move |ctx| {
            amend_order_evidence(
                ctx,
                AmendOrderEvidenceInput {
                    order_id: received.order_id,
                    expected_revision: received.revision,
                    add: empty_evidence(),
                    remove_evidence_ids: Vec::new(),
                    reason: "Synthetic evidence amendment".to_owned(),
                },
            )
        })
        .unwrap();
    let held = fixture
        .write(move |ctx| {
            place_clarification_hold(
                ctx,
                PlaceClarificationHoldInput {
                    order_id: amended.order_id,
                    expected_revision: amended.revision,
                    deficiency: ClarificationDeficiency::OtherAuthority,
                    note: None,
                },
            )
        })
        .unwrap();
    let hold = held.holds.last().unwrap();
    let hold_id = hold.hold_id;
    let hold_revision = hold.revision;
    let resolved = fixture
        .write(move |ctx| {
            resolve_clarification_hold(
                ctx,
                ResolveClarificationHoldInput {
                    hold_id,
                    expected_revision: hold_revision,
                    reason: "Synthetic hold resolution".to_owned(),
                },
            )
        })
        .unwrap();
    let reviewed = fixture
        .write(move |ctx| {
            create_billing_review_item(
                ctx,
                CreateBillingReviewItemInput {
                    order_id: resolved.order_id,
                    element: BillingReviewElement::Diagnosis,
                    note: None,
                },
            )
        })
        .unwrap();
    fixture
        .write(move |ctx| {
            accept_order(
                ctx,
                AcceptOrderInput {
                    order_id: reviewed.order_id,
                    expected_revision: reviewed.revision,
                },
            )
        })
        .unwrap()
}

fn exercise_post_accept_commands(fixture: &Fixture, accepted: &lis_orders::OrderDetail) {
    let requested = fixture
        .write({
            let input = add_on(fixture, accepted, accepted.revision);
            move |ctx| request_follow_on(ctx, input)
        })
        .unwrap();
    let item = requested.items.last().unwrap();
    let item_id = item.item_id;
    let item_revision = item.revision;
    fixture
        .write(move |ctx| {
            decide_follow_on(
                ctx,
                DecideFollowOnInput {
                    item_id,
                    expected_revision: item_revision,
                    decision: Decision::Decline,
                    reason: "Synthetic follow-on decision".to_owned(),
                },
            )
        })
        .unwrap();
    let parent_item_id = accepted.items[0].item_id;
    let assessment = result_not_met(fixture, accepted.order_id, parent_item_id);
    fixture
        .write(move |ctx| record_trigger_not_met_from_result(ctx, assessment))
        .unwrap();
    let cancellation = fixture
        .write(move |ctx| {
            request_cancellation(
                ctx,
                RequestCancellationInput {
                    item_id: parent_item_id,
                    requested_by: "Synthetic clinic".to_owned(),
                    reason: "Synthetic cancellation request".to_owned(),
                },
            )
        })
        .unwrap();
    let request = cancellation.cancellations.last().unwrap();
    let request_id = request.request_id;
    let expected_revision = request.revision;
    fixture
        .write(move |ctx| {
            decide_cancellation(
                ctx,
                DecideCancellationInput {
                    request_id,
                    expected_revision,
                    decision: Decision::Decline,
                    reason: "Synthetic cancellation decision".to_owned(),
                },
            )
        })
        .unwrap();
}

fn follow_on(
    fixture: &Fixture,
    order: &lis_orders::OrderDetail,
    parent_item_id: lis_orders::OrderItemId,
) -> RequestFollowOnInput {
    RequestFollowOnInput {
        order_id: order.order_id,
        expected_revision: order.revision,
        kind: ItemKind::Reflex,
        orderable_version_id: fixture.orderable_version_id,
        sequence_role: SequenceRole::Confirmation,
        parent_item_id: Some(parent_item_id),
        authority_reference: None,
        poc_result_id: None,
        trigger_source: Some(TriggerSource::LaboratoryResult),
        trigger_ref: Some("synthetic-trigger".to_owned()),
        reason: "Synthetic reflex decision".to_owned(),
    }
}

fn result_follow_on(
    fixture: &Fixture,
    order: &lis_orders::OrderDetail,
    parent_item_id: lis_orders::OrderItemId,
) -> LaboratoryResultFollowOnInput {
    LaboratoryResultFollowOnInput {
        order_id: order.order_id,
        expected_revision: order.revision,
        orderable_version_id: fixture.orderable_version_id,
        sequence_role: SequenceRole::Confirmation,
        parent_item_id,
        result_revision_id: ResultRevisionId::generate(),
        reason: "Synthetic verified result reflex".to_owned(),
    }
}

fn add_on(
    fixture: &Fixture,
    order: &lis_orders::OrderDetail,
    expected_revision: i64,
) -> RequestFollowOnInput {
    RequestFollowOnInput {
        order_id: order.order_id,
        expected_revision,
        kind: ItemKind::AddOn,
        orderable_version_id: fixture.orderable_version_id,
        sequence_role: SequenceRole::FirstLine,
        parent_item_id: None,
        authority_reference: Some("synthetic-add-on-authority".to_owned()),
        poc_result_id: None,
        trigger_source: None,
        trigger_ref: None,
        reason: "Synthetic add-on request".to_owned(),
    }
}

fn assert_item_role(item: &lis_orders::OrderItemView, expected: SequenceRole) {
    assert_eq!(item.sequence_role, Some(expected));
    assert!(
        item.components
            .iter()
            .all(|component| component.sequence_role == expected)
    );
}

fn empty_evidence() -> EvidenceInput {
    EvidenceInput {
        authority: AuthorityEvidenceInput {
            kind: AuthorityEvidenceKind::NotSupplied,
            document_ref: None,
        },
        diagnoses: Vec::new(),
        medications: Vec::new(),
        point_of_care_results: Vec::new(),
    }
}

fn not_met(fixture: &Fixture, parent_item_id: lis_orders::OrderItemId) -> RecordTriggerNotMetInput {
    RecordTriggerNotMetInput {
        parent_item_id,
        orderable_id: fixture.orderable_id,
        trigger_source: TriggerSource::LaboratoryResult,
        trigger_ref: "synthetic-not-met".to_owned(),
        reason: "Synthetic condition not met".to_owned(),
    }
}

fn result_not_met(
    fixture: &Fixture,
    order_id: lis_orders::OrderId,
    parent_item_id: lis_orders::OrderItemId,
) -> LaboratoryResultTriggerNotMetInput {
    LaboratoryResultTriggerNotMetInput {
        order_id,
        parent_item_id,
        orderable_id: fixture.orderable_id,
        result_revision_id: ResultRevisionId::generate(),
        reason: "Synthetic verified result condition not met".to_owned(),
    }
}

fn register_standing(
    fixture: &Fixture,
    scope_orderable_ids: Vec<lis_catalog::OrderableId>,
    visit_limit: Option<u32>,
) -> lis_orders::StandingOrderView {
    let subject_id = fixture.subject_id;
    let practitioner_id = fixture.practitioner_id;
    fixture
        .write(move |ctx| {
            register_standing_order(
                ctx,
                RegisterStandingOrderInput {
                    subject_id,
                    practitioner_id,
                    scope_orderable_ids,
                    valid_from: "2026-01-01".to_owned(),
                    valid_until: "2026-12-31".to_owned(),
                    visit_limit,
                    document_ref: "synthetic-standing".to_owned(),
                },
            )
        })
        .unwrap()
}

fn assert_stale<T, F>(fixture: &Fixture, command: F)
where
    T: std::fmt::Debug + Send + 'static,
    F: FnOnce(&mut CommandContext<'_>) -> Result<T, lis_orders::OrderError> + Send + 'static,
{
    let before = audit_count(fixture);
    let error = fixture.write(command).unwrap_err();
    assert_eq!(error.code(), lis_core::ErrorCode::Conflict);
    assert_eq!(audit_count(fixture), before);
}

fn assert_testing_gate(
    fixture: &Fixture,
    component_id: lis_orders::OrderComponentId,
    expected_code: &str,
) {
    let gates = fixture
        .t
        .db
        .read_blocking(move |conn| testing_gate(conn, component_id))
        .unwrap()
        .unwrap_err();
    assert!(gates.iter().any(|gate| gate.code == expected_code));
}

fn audit_count(fixture: &Fixture) -> i64 {
    fixture
        .t
        .db
        .read_blocking(|conn| {
            Ok::<_, lis_orders::OrderError>(conn.query_row(
                "SELECT count(*) FROM audit_events",
                [],
                |row| row.get(0),
            )?)
        })
        .unwrap()
}

fn audit_actions(fixture: &Fixture) -> std::collections::BTreeSet<String> {
    fixture
        .t
        .db
        .read_blocking(|conn| {
            Ok::<_, lis_orders::OrderError>(
                conn.prepare("SELECT action FROM audit_events WHERE action LIKE 'order.%'")?
                    .query_map([], |row| row.get(0))?
                    .collect::<Result<_, _>>()?,
            )
        })
        .unwrap()
}
