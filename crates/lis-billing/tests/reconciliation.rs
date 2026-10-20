//! Existing v1 evidence survives upgrade and rejected responses retain history (REQ-BIL-002).
#![cfg(test)]

use lis_billing::{BillingError, BillingHandoffId, BillingHandoffItemId, HandoffOutcome};
use lis_core::{
    Capability, CorrelationId, ModuleSchema, Phi, Principal, SessionId, UserId, WorkstationId,
};
use lis_store::{CommandContext, DATABASE_FILE, Database, testing::TestDatabase};
use rusqlite::params;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    database: TestDatabase,
    actor: UserId,
    handoff: BillingHandoffId,
    item: BillingHandoffItemId,
}

impl Fixture {
    fn open(schema: ModuleSchema) -> TestResult<Self> {
        let database = TestDatabase::open(&[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            schema,
        ])?;
        let actor = UserId::generate();
        let subject = database
            .db
            .write_blocking(
                principal(actor, &[Capability::EnterOrders]),
                CorrelationId::generate(),
                |ctx| {
                    lis_identity::register_subject(
                        ctx,
                        lis_identity::RegisterSubject {
                            demographics: lis_identity::DemographicsInput {
                                family_name: Phi::new("Synthetic".to_owned()),
                                given_names: None,
                                birth_date: Phi::new("1980-01-01".to_owned()),
                                administrative_sex: None,
                            },
                            source: "Synthetic fixture".to_owned(),
                            identifiers: vec![],
                            acknowledged_candidates: vec![],
                        },
                    )
                },
            )?
            .subject_id;
        let handoff = BillingHandoffId::generate();
        let item = BillingHandoffItemId::generate();
        database.db.write_blocking(
            Principal::system("billing-test-setup", []), CorrelationId::generate(),
            move |ctx| -> Result<(), BillingError> {
                ctx.tx().execute("INSERT INTO auth_users (id, username, display_name, status, password_hash, must_change_password, created_at, updated_at, revision) VALUES (?1, 'synthetic-biller', 'Synthetic biller', 'active', '$argon2id$synthetic', 1, ?2, ?2, 1)", params![actor, ctx.now()])?;
                let derivation = lis_billing::BillingDerivationId::generate();
                ctx.tx().execute("INSERT INTO bil_derivations (id, subject_id, date_of_service, dos_rule, derivation_no, inputs_sha256, completeness, state, derived_by, derived_at, facts_sealed, revision) VALUES (?1, ?2, '2026-01-01', 'collection_local_date', 1, ?3, 'complete', 'candidate', ?4, ?5, 0, 1)", params![derivation, subject, "01".repeat(32), actor, ctx.now()])?;
                ctx.tx().execute("INSERT INTO bil_derivation_inputs (derivation_id, kind, ref_id) VALUES (?1, 'order_version', 'synthetic-order:1')", [derivation])?;
                ctx.tx().execute("UPDATE bil_derivations SET facts_sealed = 1 WHERE id = ?1", [derivation])?;
                ctx.tx().execute("INSERT INTO bil_handoffs (id, format, artifact_sha256, item_count, created_by, created_at, handoff_no) VALUES (?1, 'bil-handoff-v1', ?2, 1, ?3, ?4, 1)", params![handoff, "02".repeat(32), actor, ctx.now()])?;
                ctx.tx().execute("INSERT INTO bil_handoff_items (id, handoff_id, derivation_id, kind) VALUES (?1, ?2, ?3, 'original')", params![item, handoff, derivation])?;
                Ok(())
            },
        )?;
        Ok(Self {
            database,
            actor,
            handoff,
            item,
        })
    }

    fn run<T: Send + 'static>(
        &self,
        capabilities: &[Capability],
        command: impl FnOnce(&mut CommandContext<'_>) -> Result<T, BillingError> + Send + 'static,
    ) -> Result<T, BillingError> {
        self.database.db.write_blocking(
            principal(self.actor, capabilities),
            CorrelationId::generate(),
            command,
        )
    }

    fn reject(&self) -> Result<BillingHandoffId, BillingError> {
        let item = self.item;
        self.run(&[Capability::Billing], move |ctx| {
            lis_billing::record_handoff_outcome(
                ctx,
                lis_billing::RecordHandoffOutcome {
                    item_id: item,
                    outcome: HandoffOutcome::Rejected,
                    external_reference: Some("SYN-RESPONSE-1".to_owned()),
                    reason: Some("Administrative correction requested".to_owned()),
                },
            )
        })
    }
}

fn principal(actor: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        actor,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

fn followup(item: BillingHandoffItemId, revision: i64) -> lis_billing::RecordHandoffFollowupInput {
    lis_billing::RecordHandoffFollowupInput {
        item_id: item,
        expected_revision: revision,
        outcome: HandoffOutcome::Acknowledged,
        external_reference: Some("SYN-RESPONSE-2".to_owned()),
        reason: "Billing owner accepted the resubmission".to_owned(),
    }
}

#[test]
fn followup_requires_authority_reason_current_rejection_and_retains_prior_response() -> TestResult {
    let fixture = Fixture::open(lis_billing::SCHEMA)?;
    let item = fixture.item;
    assert!(matches!(
        fixture.run(&[Capability::Billing], move |ctx| {
            lis_billing::record_handoff_followup(ctx, followup(item, 0))
        }),
        Err(BillingError::NotFound(_))
    ));
    fixture.reject()?;
    assert_eq!(
        fixture
            .run(&[], move |ctx| lis_billing::record_handoff_followup(
                ctx,
                followup(item, 1)
            ))
            .unwrap_err()
            .code(),
        lis_core::ErrorCode::Forbidden
    );
    assert!(matches!(
        fixture.run(&[Capability::Billing], move |ctx| {
            lis_billing::record_handoff_followup(ctx, followup(item, 0))
        }),
        Err(BillingError::RevisionConflict)
    ));
    assert!(matches!(
        fixture.run(&[Capability::Billing], move |ctx| {
            let mut input = followup(item, 1);
            input.reason = " ".to_owned();
            lis_billing::record_handoff_followup(ctx, input)
        }),
        Err(BillingError::Invalid(_))
    ));
    fixture.run(&[Capability::Billing], move |ctx| {
        lis_billing::record_handoff_followup(ctx, followup(item, 1))
    })?;
    assert!(matches!(
        fixture.run(&[Capability::Billing], move |ctx| {
            lis_billing::record_handoff_followup(ctx, followup(item, 1))
        }),
        Err(BillingError::RevisionConflict)
    ));
    assert!(matches!(
        fixture.run(&[Capability::Billing], move |ctx| {
            lis_billing::record_handoff_followup(ctx, followup(item, 2))
        }),
        Err(BillingError::OutcomeRecorded)
    ));
    assert!(matches!(
        fixture.reject(),
        Err(BillingError::OutcomeRecorded)
    ));
    assert_accepted_history(&fixture)?;
    Ok(())
}

#[test]
fn v1_handoff_and_original_outcome_survive_upgrade_and_can_be_reconciled() -> TestResult {
    let fixture = Fixture::open(ModuleSchema {
        migrations: &lis_billing::MIGRATIONS[..3],
        ..lis_billing::SCHEMA
    })?;
    fixture.reject()?;
    let upgraded = fixture.database.data_dir().join("upgraded");
    std::fs::create_dir(&upgraded)?;
    fixture
        .database
        .db
        .snapshot(&upgraded.join(DATABASE_FILE))?;
    let db = Database::open(
        &upgraded,
        &[
            lis_auth::SCHEMA,
            lis_identity::SCHEMA,
            lis_catalog::SCHEMA,
            lis_billing::SCHEMA,
        ],
        fixture.database.clock.clone(),
    )?;
    let handoff = fixture.handoff;
    let view = db.read_blocking(move |conn| lis_billing::get_billing_handoff(conn, handoff))?;
    assert_eq!(view.format, "bil-handoff-v1");
    assert_eq!(view.artifact_sha256, "02".repeat(32));
    assert_eq!(view.items[0].outcome_revision, Some(1));
    assert_eq!(
        view.items[0].outcome.as_ref().unwrap().outcome,
        HandoffOutcome::Rejected
    );
    let source = db.read_blocking(move |conn| lis_billing::handoff_export_source(conn, handoff))?;
    assert_eq!(source.artifact_sha256, view.artifact_sha256);
    let item = fixture.item;
    db.write_blocking(
        principal(fixture.actor, &[Capability::Billing]),
        CorrelationId::generate(),
        move |ctx| lis_billing::record_handoff_followup(ctx, followup(item, 1)),
    )?;
    let conn = rusqlite::Connection::open(upgraded.join(DATABASE_FILE))?;
    let violations: i64 =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(violations, 0);
    assert!(conn.execute("DELETE FROM bil_handoffs", []).is_err());
    assert!(
        conn.execute("UPDATE bil_derivation_inputs SET ref_id = 'rewritten'", [])
            .is_err()
    );
    assert!(conn.execute("INSERT INTO bil_derivation_inputs SELECT derivation_id, 'billing_context', 'new' FROM bil_derivation_inputs", []).is_err());
    Ok(())
}

fn assert_accepted_history(fixture: &Fixture) -> TestResult {
    let item = fixture.item;
    let handoff = fixture.handoff;
    let view = fixture
        .database
        .db
        .read_blocking(move |conn| lis_billing::get_billing_handoff(conn, handoff))?;
    assert_eq!(view.items[0].outcome_revision, Some(2));
    assert!(view.items[0].allowed_actions.is_empty());
    let history = view.items[0].outcome_history.as_ref().unwrap();
    assert_eq!(history.len(), 2);
    assert_eq!(history[0].outcome, HandoffOutcome::Rejected);
    assert_eq!(history[1].outcome, HandoffOutcome::Acknowledged);
    let conn = rusqlite::Connection::open(fixture.database.data_dir().join(DATABASE_FILE))?;
    let audit: (i64, i64, String) = conn.query_row("SELECT old_revision, new_revision, record_id FROM audit_events WHERE action = 'billing.record_handoff_followup'", [], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    assert_eq!(audit, (1, 2, item.to_string()));
    assert!(
        conn.execute("UPDATE bil_handoff_followups SET reason = 'rewritten'", [])
            .is_err()
    );
    assert!(
        conn.execute("DELETE FROM bil_handoff_followups", [])
            .is_err()
    );
    assert!(conn.execute("INSERT INTO bil_handoff_followups SELECT item_id, 3, 'rejected', NULL, 'reopen', recorded_by, recorded_at FROM bil_handoff_followups", []).is_err());
    Ok(())
}
