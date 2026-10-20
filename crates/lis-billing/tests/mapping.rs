//! Mapping lifecycle integration tests on production SQLite pragmas (REQ-BIL-005).

use std::collections::BTreeSet;

use lis_billing::{
    ApproveClassMappingVersion, BillingClassMappingEntry, BillingError, ConfirmClassMappingVersion,
    DraftClassMappingVersion, RetireClassMappingVersion, SCHEMA, UpdateClassMappingDraft,
    approve_class_mapping_version, confirm_class_mapping_version, current_class_mapping,
    draft_class_mapping_version, get_class_mapping_version, retire_class_mapping_version,
    update_class_mapping_draft,
};
use lis_catalog::{CreateAnalyte, create_analyte};
use lis_core::{
    Actor, Capability, CorrelationId, ErrorCode, ModuleSchema, Principal, SessionId, UserId,
    WorkstationId,
};
use lis_store::testing::{TEST_START, TestDatabase};
use lis_store::{CommandContext, DATABASE_FILE, Database};
use rusqlite::OptionalExtension;
use rusqlite::types::Value;

type TestResult<T = ()> = Result<T, Box<dyn std::error::Error>>;

struct Fixture {
    database: TestDatabase,
    author: UserId,
    approver: UserId,
    confirmer: UserId,
    analyte_id: lis_catalog::AnalyteId,
}

impl Fixture {
    fn open() -> TestResult<Self> {
        Self::open_on(SCHEMA)
    }

    /// The fixture with `billing`, the billing schema at some version.
    fn open_on(billing: ModuleSchema) -> TestResult<Self> {
        let database = TestDatabase::open(&[lis_auth::SCHEMA, lis_catalog::SCHEMA, billing])?;
        let author = UserId::generate();
        let approver = UserId::generate();
        let confirmer = UserId::generate();
        database.db.write_blocking(
            Principal::system("billing-test-setup", []),
            CorrelationId::generate(),
            move |ctx| -> Result<(), BillingError> {
                for (user, username) in [
                    (author, "billing-author"),
                    (approver, "billing-approver"),
                    (confirmer, "billing-confirmer"),
                ] {
                    ctx.tx().execute(
                        "INSERT INTO auth_users (id, username, display_name, status, \
                         password_hash, must_change_password, created_at, updated_at, revision) \
                         VALUES (?1, ?2, ?2, 'active', '$argon2id$synthetic', 1, ?3, ?3, 1)",
                        rusqlite::params![user, username, ctx.now()],
                    )?;
                }
                Ok(())
            },
        )?;
        let analyte_id = database.db.write_blocking(
            principal(UserId::generate(), &[Capability::ManageConfiguration]),
            CorrelationId::generate(),
            |ctx| {
                create_analyte(
                    ctx,
                    CreateAnalyte {
                        code: "SYN-A".to_owned(),
                        name: "Synthetic analyte".to_owned(),
                        loinc_code: None,
                        drug_class_code: None,
                    },
                )
            },
        )?;
        Ok(Self {
            database,
            author,
            approver,
            confirmer,
            analyte_id,
        })
    }

    fn entry(&self) -> BillingClassMappingEntry {
        BillingClassMappingEntry {
            analyte_id: self.analyte_id,
            class_code: "SYN-CLASS".to_owned(),
            class_label: "Synthetic class".to_owned(),
        }
    }

    fn run<T, F>(
        &self,
        user: UserId,
        capabilities: &[Capability],
        command: F,
    ) -> Result<T, BillingError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, BillingError> + Send + 'static,
    {
        self.database.db.write_blocking(
            principal(user, capabilities),
            CorrelationId::generate(),
            command,
        )
    }

    fn draft(&self) -> Result<lis_billing::BillingClassMappingVersionView, BillingError> {
        let entry = self.entry();
        self.run(self.author, &[Capability::Billing], move |ctx| {
            draft_class_mapping_version(
                ctx,
                DraftClassMappingVersion {
                    entries: vec![entry],
                },
            )
        })
    }
}

#[test]
fn handoff_numbers_cannot_skip_the_next_sequence_value() -> TestResult {
    let fixture = Fixture::open()?;
    let author = fixture.author;
    fixture.database.db.write_blocking(
        Principal::system("billing-test", []),
        CorrelationId::generate(),
        move |ctx| -> Result<(), BillingError> {
            let skipped = ctx.tx().execute(
                "INSERT INTO bil_handoffs \
                 (id, format, artifact_sha256, item_count, created_by, created_at, handoff_no) \
                 VALUES (?1, 'bil-handoff-v1', ?2, 0, ?3, ?4, 2)",
                rusqlite::params![
                    lis_billing::BillingHandoffId::generate(),
                    "00".repeat(32),
                    author,
                    ctx.now()
                ],
            );
            assert!(skipped.is_err());
            Ok(())
        },
    )?;
    Ok(())
}

#[test]
fn approval_by_non_author_then_lab_confirmation_makes_mapping_current() -> TestResult {
    let fixture = Fixture::open()?;
    let draft = fixture.draft()?;
    let id = draft.summary.version_id;
    let approved = fixture.run(fixture.approver, &[Capability::Billing], move |ctx| {
        approve_class_mapping_version(
            ctx,
            ApproveClassMappingVersion {
                version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })?;
    assert!(approved.summary.effective_from.is_none());
    let confirmed = fixture.run(
        fixture.confirmer,
        &[Capability::ApproveConfiguration],
        move |ctx| {
            confirm_class_mapping_version(
                ctx,
                ConfirmClassMappingVersion {
                    version_id: id,
                    expected_revision: 2,
                },
            )
        },
    )?;
    assert!(confirmed.summary.effective_from.is_some());
    assert_eq!(confirmed.summary.confirmed_by, Some(fixture.confirmer));
    assert_eq!(confirmed.entries, vec![fixture.entry()]);
    let current = fixture.database.db.read_blocking(current_class_mapping)?;
    assert_eq!(current, Some(confirmed));
    Ok(())
}

#[test]
fn self_approval_stale_revision_and_confirmation_before_approval_are_refused() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id;
    let self_approval = fixture.run(fixture.author, &[Capability::Billing], move |ctx| {
        approve_class_mapping_version(
            ctx,
            ApproveClassMappingVersion {
                version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    assert!(matches!(self_approval, Err(BillingError::SelfApproval)));
    let early_confirmation = fixture.run(
        fixture.confirmer,
        &[Capability::ApproveConfiguration],
        move |ctx| {
            confirm_class_mapping_version(
                ctx,
                ConfirmClassMappingVersion {
                    version_id: id,
                    expected_revision: 1,
                },
            )
        },
    );
    assert!(matches!(
        early_confirmation,
        Err(BillingError::ApprovalRequired)
    ));
    let entry = fixture.entry();
    let stale = fixture.run(fixture.author, &[Capability::Billing], move |ctx| {
        update_class_mapping_draft(
            ctx,
            UpdateClassMappingDraft {
                version_id: id,
                expected_revision: 0,
                entries: vec![entry],
            },
        )
    });
    assert!(matches!(stale, Err(BillingError::RevisionConflict)));
    Ok(())
}

#[test]
fn every_mapping_command_refuses_a_missing_capability_without_writes() -> TestResult {
    let fixture = Fixture::open()?;
    let entry = fixture.entry();
    let draft = fixture.run(fixture.author, &[], move |ctx| {
        draft_class_mapping_version(
            ctx,
            DraftClassMappingVersion {
                entries: vec![entry],
            },
        )
    });
    assert!(matches!(draft, Err(BillingError::Store(_))));
    assert_eq!(
        fixture
            .database
            .db
            .read_blocking(|conn| -> Result<i64, BillingError> {
                Ok(conn.query_row(
                    "SELECT count(*) FROM bil_class_mapping_versions",
                    [],
                    |row| row.get(0),
                )?)
            })?,
        0
    );
    Ok(())
}

#[test]
fn direct_sql_cannot_fabricate_or_mutate_an_approved_mapping() -> TestResult {
    let fixture = Fixture::open()?;
    let author = fixture.author;
    let approver = fixture.approver;
    let confirmer = fixture.confirmer;
    let fabricated = fixture.database.db.write_blocking(
        Principal::system("tamper-test", []),
        CorrelationId::generate(),
        move |ctx| -> Result<(), BillingError> {
            ctx.tx().execute(
                "INSERT INTO bil_class_mapping_versions
                 (id, version, status, revision, created_by, created_at, approved_by,
                  approved_at, confirmed_by, confirmed_at, effective_from)
                 VALUES (?1, 99, 'approved', 3, ?2, ?3, ?4, ?3, ?5, ?3, ?3)",
                rusqlite::params![
                    lis_billing::BillingClassMappingVersionId::generate(),
                    author,
                    ctx.now(),
                    approver,
                    confirmer
                ],
            )?;
            Ok(())
        },
    );
    assert!(fabricated.is_err());

    let id = fixture.draft()?.summary.version_id;
    let approver = fixture.approver;
    let direct_approval = fixture.database.db.write_blocking(
        Principal::system("tamper-test", []),
        CorrelationId::generate(),
        move |ctx| -> Result<(), BillingError> {
            ctx.tx().execute(
                "UPDATE bil_class_mapping_versions SET status = 'approved', revision = 2,
                 approved_by = ?2, approved_at = ?3 WHERE id = ?1",
                rusqlite::params![id, approver, ctx.now()],
            )?;
            Ok(())
        },
    );
    assert!(direct_approval.is_err());

    fixture.run(fixture.approver, &[Capability::Billing], move |ctx| {
        approve_class_mapping_version(
            ctx,
            ApproveClassMappingVersion {
                version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })?;
    let content_mutation = fixture.database.db.write_blocking(
        Principal::system("tamper-test", []),
        CorrelationId::generate(),
        move |ctx| -> Result<(), BillingError> {
            ctx.tx().execute(
                "UPDATE bil_class_mapping_entries SET class_label = 'Rewritten' \
                 WHERE version_id = ?1",
                [id],
            )?;
            Ok(())
        },
    );
    assert!(content_mutation.is_err());
    Ok(())
}

#[test]
fn every_billing_actor_column_references_an_auth_user() -> TestResult {
    let fixture = Fixture::open()?;
    let expected = [
        (
            "bil_class_mapping_versions",
            ["approved_by", "confirmed_by", "created_by", "retired_by"].as_slice(),
        ),
        ("bil_derivations", ["approved_by", "derived_by"].as_slice()),
        ("bil_handoffs", ["created_by"].as_slice()),
        ("bil_handoff_outcomes", ["recorded_by"].as_slice()),
    ];
    for (table, columns) in expected {
        let sql = format!("PRAGMA foreign_key_list({table})");
        let actual: BTreeSet<String> =
            fixture
                .database
                .db
                .read_blocking(move |conn| -> Result<_, BillingError> {
                    Ok(conn
                        .prepare(&sql)?
                        .query_map([], |row| {
                            Ok((
                                row.get::<_, String>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, String>(4)?,
                            ))
                        })?
                        .collect::<Result<Vec<_>, _>>()?
                        .into_iter()
                        .filter(|(target, _, to)| target == "auth_users" && to == "id")
                        .map(|(_, from, _)| from)
                        .collect())
                })?;
        assert_eq!(
            actual,
            columns.iter().map(ToString::to_string).collect(),
            "{table}"
        );
    }
    Ok(())
}

#[test]
fn unknown_mapping_actor_is_rejected_by_the_foreign_key() -> TestResult {
    let fixture = Fixture::open()?;
    let entry = fixture.entry();
    let result = fixture.run(UserId::generate(), &[Capability::Billing], move |ctx| {
        draft_class_mapping_version(
            ctx,
            DraftClassMappingVersion {
                entries: vec![entry],
            },
        )
    });
    assert!(matches!(result, Err(BillingError::Store(_))));
    assert_eq!(
        fixture
            .database
            .db
            .read_blocking(|conn| -> Result<i64, BillingError> {
                Ok(conn.query_row(
                    "SELECT count(*) FROM bil_class_mapping_versions",
                    [],
                    |row| row.get(0),
                )?)
            })?,
        0
    );
    Ok(())
}

#[test]
fn an_unconfirmed_approved_mapping_can_retire_without_an_effective_interval() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id;
    fixture.run(fixture.approver, &[Capability::Billing], move |ctx| {
        approve_class_mapping_version(
            ctx,
            ApproveClassMappingVersion {
                version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })?;
    let retired = fixture.run(fixture.approver, &[Capability::Billing], move |ctx| {
        retire_class_mapping_version(
            ctx,
            RetireClassMappingVersion {
                version_id: id,
                expected_revision: 2,
                reason: "Synthetic retirement".to_owned(),
            },
        )
    })?;
    assert!(retired.summary.effective_from.is_none());
    assert!(retired.summary.effective_to.is_none());
    let stored = fixture
        .database
        .db
        .read_blocking(move |conn| get_class_mapping_version(conn, id))?;
    assert_eq!(stored.summary.status.as_str(), "retired");
    Ok(())
}

fn principal(user: UserId, capabilities: &[Capability]) -> Principal {
    let principal = Principal::user(
        user,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    );
    assert!(matches!(principal.actor(), Actor::User { .. }));
    principal
}

// Separation of duties for mapping approval (ADR 0015; REQ-BIL-005): refused as
// before while the policy needs a second person, allowed only with the author's
// confirmation, audited as a self-approval, and refused by the schema guard too.

fn approve(
    fixture: &Fixture,
    user: UserId,
    version_id: lis_billing::BillingClassMappingVersionId,
    self_approval: bool,
) -> Result<lis_billing::BillingClassMappingVersionView, BillingError> {
    fixture.run(user, &[Capability::Billing], move |ctx| {
        approve_class_mapping_version(
            ctx,
            ApproveClassMappingVersion {
                version_id,
                expected_revision: 1,
                self_approval,
            },
        )
    })
}

/// A connection outside the store, as an administrator editing the file would use.
fn raw(fixture: &Fixture) -> rusqlite::Result<rusqlite::Connection> {
    rusqlite::Connection::open(fixture.database.data_dir().join(DATABASE_FILE))
}

/// Records a policy version that lets the author approve configuration, as a
/// quality manager would (`lis_auth::set_separation_policy`).
fn allow_self_approval(fixture: &Fixture) -> TestResult {
    raw(fixture)?.execute(
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (1, 1, 0, 0, 'Synthetic one-person laboratory', 'synthetic-quality-manager', ?1)",
        [TEST_START],
    )?;
    Ok(())
}

/// The `self_approval` detail of the approval event of `version_id`.
fn self_approval_mark(
    fixture: &Fixture,
    version_id: lis_billing::BillingClassMappingVersionId,
) -> TestResult<Option<i64>> {
    Ok(raw(fixture)?
        .query_row(
            "SELECT json_extract(details, '$.self_approval') FROM audit_events \
             WHERE action = 'billing.approve_class_mapping' AND record_id = ?1",
            [version_id.to_string()],
            |row| row.get(0),
        )
        .optional()?
        .flatten())
}

/// Row counts of the mapping tables and the audit trail.
fn written(fixture: &Fixture) -> TestResult<Vec<i64>> {
    let conn = raw(fixture)?;
    [
        "bil_class_mapping_versions",
        "bil_class_mapping_entries",
        "audit_events",
    ]
    .iter()
    .map(|table| {
        Ok(
            conn.query_row(&format!("SELECT count(*) FROM {table}"), [], |row| {
                row.get(0)
            })?,
        )
    })
    .collect()
}

#[test]
fn the_default_policy_refuses_the_author_even_when_confirmed() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id;
    let before = written(&fixture)?;
    let error = approve(&fixture, fixture.author, id, true).unwrap_err();
    assert!(matches!(error, BillingError::SelfApproval), "{error:?}");
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert!(error.to_string().contains("quality manager"), "{error}");
    assert_eq!(written(&fixture)?, before);
    Ok(())
}

#[test]
fn an_allowed_self_approval_still_needs_the_authors_confirmation() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id;
    allow_self_approval(&fixture)?;
    let before = written(&fixture)?;
    let error = approve(&fixture, fixture.author, id, false).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let BillingError::Invalid(field) = &error else {
        panic!("{error:?}");
    };
    assert_eq!(
        (field.field.as_str(), field.code.as_str()),
        ("selfApproval", "self_approval_unconfirmed")
    );
    assert_eq!(written(&fixture)?, before);
    Ok(())
}

#[test]
fn a_confirmed_self_approval_is_approved_and_audited_as_one() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id;
    allow_self_approval(&fixture)?;
    let approved = approve(&fixture, fixture.author, id, true)?;
    assert_eq!(approved.summary.approved_by, Some(fixture.author));
    assert_eq!(self_approval_mark(&fixture, id)?, Some(1));
    Ok(())
}

#[test]
fn another_approver_is_unaffected_by_the_policy_and_not_marked() -> TestResult {
    for allowed in [false, true] {
        let fixture = Fixture::open()?;
        let id = fixture.draft()?.summary.version_id;
        if allowed {
            allow_self_approval(&fixture)?;
        }
        let approved = approve(&fixture, fixture.approver, id, false)?;
        assert_eq!(approved.summary.approved_by, Some(fixture.approver));
        assert_eq!(self_approval_mark(&fixture, id)?, None);
    }
    Ok(())
}

#[test]
fn the_schema_refuses_an_author_approval_while_the_policy_needs_a_second_person() -> TestResult {
    let fixture = Fixture::open()?;
    let id = fixture.draft()?.summary.version_id.to_string();
    let set_author = |conn: &rusqlite::Connection| {
        conn.execute(
            "UPDATE bil_class_mapping_versions SET approved_by = created_by WHERE id = ?1",
            [id.as_str()],
        )
        .unwrap_err()
        .to_string()
    };
    let message = set_author(&raw(&fixture)?);
    assert!(message.contains("separation of duties"), "{message}");
    // Under a policy that allows it the guard lets the change through, and the
    // transition trigger refuses the unaudited approval instead.
    allow_self_approval(&fixture)?;
    let message = set_author(&raw(&fixture)?);
    assert!(!message.contains("separation of duties"), "{message}");
    assert!(message.contains("authorized audit event"), "{message}");
    Ok(())
}

/// Every row of `table`, ordered by its first two columns.
fn rows(conn: &rusqlite::Connection, table: &str) -> TestResult<Vec<Vec<Value>>> {
    let mut statement = conn.prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))?;
    let width = statement.column_count();
    Ok(statement
        .query_map([], |row| (0..width).map(|i| row.get(i)).collect())?
        .collect::<Result<_, _>>()?)
}

const MAPPING_TABLES: [&str; 2] = ["bil_class_mapping_versions", "bil_class_mapping_entries"];

/// A current (approved and confirmed), a retired and a draft mapping version.
fn seed_versions(fixture: &Fixture) -> TestResult<[lis_billing::BillingClassMappingVersionId; 3]> {
    let current = fixture.draft()?.summary.version_id;
    approve(fixture, fixture.approver, current, false)?;
    fixture.run(
        fixture.confirmer,
        &[Capability::ApproveConfiguration],
        move |ctx| {
            confirm_class_mapping_version(
                ctx,
                ConfirmClassMappingVersion {
                    version_id: current,
                    expected_revision: 2,
                },
            )
        },
    )?;
    let retired = fixture.draft()?.summary.version_id;
    approve(fixture, fixture.approver, retired, false)?;
    fixture.run(fixture.approver, &[Capability::Billing], move |ctx| {
        retire_class_mapping_version(
            ctx,
            RetireClassMappingVersion {
                version_id: retired,
                expected_revision: 2,
                reason: "Synthetic retirement".to_owned(),
            },
        )
    })?;
    let draft = fixture.draft()?.summary.version_id;
    Ok([current, retired, draft])
}

#[test]
fn an_installed_mapping_keeps_every_version_and_guard_through_the_rebuild() -> TestResult {
    let legacy = ModuleSchema {
        migrations: &lis_billing::MIGRATIONS[..2],
        ..SCHEMA
    };
    let fixture = Fixture::open_on(legacy)?;
    let [current, _, draft] = seed_versions(&fixture)?;
    let before = MAPPING_TABLES
        .iter()
        .map(|table| rows(&raw(&fixture)?, table))
        .collect::<TestResult<Vec<_>>>()?;

    let upgraded = fixture.database.data_dir().join("upgraded");
    std::fs::create_dir(&upgraded)?;
    fixture
        .database
        .db
        .snapshot(&upgraded.join(DATABASE_FILE))?;
    let modules = [lis_auth::SCHEMA, lis_catalog::SCHEMA, SCHEMA];
    let clock = std::sync::Arc::clone(&fixture.database.clock);
    let db = Database::open(&upgraded, &modules, clock)?;
    let conn = rusqlite::Connection::open(upgraded.join(DATABASE_FILE))?;
    let after = MAPPING_TABLES
        .iter()
        .map(|table| rows(&conn, table))
        .collect::<TestResult<Vec<_>>>()?;
    assert_eq!(after, before);
    let applied: String = conn.query_row(
        "SELECT name FROM store_migrations WHERE module = 'billing' AND version = 3",
        [],
        |row| row.get(0),
    )?;
    assert_eq!(applied, "self_approval");
    let violations: i64 =
        conn.query_row("SELECT count(*) FROM pragma_foreign_key_check", [], |row| {
            row.get(0)
        })?;
    assert_eq!(violations, 0);

    // The frozen-entry triggers still read the rebuilt table.
    let frozen = conn.execute(
        "UPDATE bil_class_mapping_entries SET class_label = 'Rewritten' WHERE version_id = ?1",
        [current.to_string()],
    );
    assert!(frozen.unwrap_err().to_string().contains("immutable"));
    // The default policy now governs approval, in schema and code.
    let message = conn
        .execute(
            "UPDATE bil_class_mapping_versions SET approved_by = created_by WHERE id = ?1",
            [draft.to_string()],
        )
        .unwrap_err()
        .to_string();
    assert!(message.contains("separation of duties"), "{message}");
    let as_user = |user| {
        let input = ApproveClassMappingVersion {
            version_id: draft,
            expected_revision: 1,
            self_approval: true,
        };
        let principal = principal(user, &[Capability::Billing]);
        db.write_blocking(principal, CorrelationId::generate(), move |ctx| {
            approve_class_mapping_version(ctx, input)
        })
    };
    assert!(matches!(
        as_user(fixture.author),
        Err(BillingError::SelfApproval)
    ));
    assert_eq!(as_user(fixture.approver)?.summary.revision, 2);
    Ok(())
}
