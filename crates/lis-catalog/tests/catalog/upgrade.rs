//! Upgrading an installed catalog to migration 5 (self-approval, ADR 0015): the
//! rebuilt version tables keep every row, foreign keys stay clean, the
//! frozen-content triggers of the tables that read them still fire, and the
//! separation guard applies from then on.

use std::sync::Arc;

use lis_catalog::{
    AnalyteId, ApproveOrderableVersion, ApproveValidityPolicyVersion, CatalogError, MIGRATIONS,
    MethodVersionId, OrderableKind, OrderableVersionId, RetireMethodVersion, SCHEMA,
    ValidityPolicyVersionId, approve_orderable_version, approve_validity_policy_version,
    retire_method_version,
};
use lis_core::{Capability, CorrelationId, ModuleSchema, Principal};
use lis_store::testing::TestDatabase;
use lis_store::{DATABASE_FILE, Database};
use rusqlite::Connection;
use rusqlite::types::Value;

use crate::separation::BOTH;
use crate::{Catalog, user_principal};

/// The rebuilt tables and every table whose triggers read them.
const TABLES: [&str; 7] = [
    "cat_method_versions",
    "cat_method_version_analytes",
    "cat_orderable_versions",
    "cat_orderable_components",
    "cat_orderable_component_analytes",
    "cat_validity_policy_versions",
    "cat_validity_categories",
];

/// Every row of `table`, ordered by its first two columns.
fn rows(conn: &Connection, table: &str) -> Vec<Vec<Value>> {
    let mut statement = conn
        .prepare(&format!("SELECT * FROM {table} ORDER BY 1, 2"))
        .unwrap();
    let width = statement.column_count();
    statement
        .query_map([], |row| (0..width).map(|i| row.get(i)).collect())
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap()
}

fn approve_test(c: &Catalog, id: OrderableVersionId) {
    c.as_approver(move |ctx| {
        approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })
    .unwrap();
}

/// What the installed catalog holds before the upgrade.
struct Seeded {
    spare_analyte: AnalyteId,
    current: MethodVersionId,
    method_draft: MethodVersionId,
    retired: MethodVersionId,
    test_current: OrderableVersionId,
    test_draft: OrderableVersionId,
    policy: ValidityPolicyVersionId,
    policy_draft: ValidityPolicyVersionId,
}

/// Superseded, current, retired and draft versions of every kind, with their
/// analytes, components and categories.
fn seed(c: &Catalog) -> Seeded {
    let spare_analyte = c.analyte("SYN-C").unwrap();
    let method = c.method("SYN-M1");
    let superseded = c.draft_method(method).unwrap();
    c.approve_method(superseded, 1).unwrap();
    let current = c.draft_method(method).unwrap();
    c.approve_method(current, 1).unwrap();
    let method_draft = c.draft_method(method).unwrap();
    let retired = c.approved_method("SYN-M2");
    c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: retired,
                expected_revision: 2,
                reason: "Synthetic withdrawal".to_owned(),
            },
        )
    })
    .unwrap();
    let test = c.orderable("SYN-T1", OrderableKind::Test);
    let test_current = c.draft_orderable(test, c.manifest(&[current])).unwrap();
    approve_test(c, test_current);
    let test_draft = c.draft_orderable(test, c.manifest(&[current])).unwrap();
    let policy = c.draft_validity(c.specimen_type).unwrap();
    c.approve_validity(policy, 1).unwrap();
    let policy_draft = c.draft_validity(c.specimen_type).unwrap();
    Seeded {
        spare_analyte,
        current,
        method_draft,
        retired,
        test_current,
        test_draft,
        policy,
        policy_draft,
    }
}

fn scalar(conn: &Connection, sql: &str) -> i64 {
    conn.query_row(sql, [], |row| row.get(0)).unwrap()
}

/// The frozen-content triggers still read the rebuilt tables: approved content
/// refuses changes, drafts accept them.
fn assert_frozen_content_guards(conn: &Connection, s: &Seeded) {
    let refused = [
        (
            "INSERT INTO cat_method_version_analytes (method_version_id, analyte_id) \
             VALUES (?1, ?2)",
            s.current.to_string(),
        ),
        (
            "UPDATE cat_method_version_analytes SET unit = 'mg/L' WHERE method_version_id = ?1",
            s.retired.to_string(),
        ),
        (
            "DELETE FROM cat_orderable_component_analytes WHERE orderable_version_id = ?1",
            s.test_current.to_string(),
        ),
        (
            "DELETE FROM cat_orderable_components WHERE orderable_version_id = ?1",
            s.test_current.to_string(),
        ),
        (
            "DELETE FROM cat_validity_categories WHERE policy_version_id = ?1",
            s.policy.to_string(),
        ),
    ];
    for (sql, id) in refused {
        let result = if sql.contains("?2") {
            conn.execute(sql, [id, s.spare_analyte.to_string()])
        } else {
            conn.execute(sql, [id])
        };
        let message = result.unwrap_err().to_string();
        assert!(message.contains("immutable"), "{sql}: {message}");
    }
    conn.execute(
        "INSERT INTO cat_method_version_analytes (method_version_id, analyte_id) VALUES (?1, ?2)",
        [s.method_draft.to_string(), s.spare_analyte.to_string()],
    )
    .unwrap();
}

fn approve_policy(
    db: &Database,
    principal: Principal,
    policy_version_id: ValidityPolicyVersionId,
    self_approval: bool,
) -> Result<i64, CatalogError> {
    db.write_blocking(principal, CorrelationId::generate(), move |ctx| {
        approve_validity_policy_version(
            ctx,
            ApproveValidityPolicyVersion {
                policy_version_id,
                expected_revision: 1,
                self_approval,
            },
        )
        .map(|approved| approved.revision)
    })
}

#[test]
fn an_installed_catalog_keeps_every_version_and_guard_through_the_rebuild() {
    let legacy = ModuleSchema {
        migrations: &MIGRATIONS[..4],
        ..SCHEMA
    };
    let t = TestDatabase::open(&[lis_auth::SCHEMA, legacy]).unwrap();
    // Today's analyte command records history: expose it while seeding too.
    let file = t.data_dir().join(DATABASE_FILE);
    Connection::open(&file)
        .unwrap()
        .execute_batch(MIGRATIONS[7].sql)
        .unwrap();
    let c = Catalog::on(t);
    // Today's seed commands write optional report fields. Temporarily expose
    // those columns while seeding, then restore the exact v4 schema before
    // testing the historical rebuild. No legacy row receives invented content.
    c.raw()
        .execute_batch(include_str!("../../migrations/0006_report_content.sql"))
        .unwrap();
    let seeded = seed(&c);
    c.raw()
        .execute_batch(
            "DROP TRIGGER cat_method_report_notes_frozen;
         ALTER TABLE cat_method_versions DROP COLUMN report_notes;
         ALTER TABLE cat_method_version_analytes DROP COLUMN reference_interval;
         ALTER TABLE cat_method_version_analytes DROP COLUMN range_policy;
         DROP TRIGGER cat_analytes_versioned;
         DROP TABLE cat_analyte_history;
         ALTER TABLE cat_analytes DROP COLUMN active;
         ALTER TABLE cat_analytes DROP COLUMN revision;
         CREATE TRIGGER cat_analytes_no_update BEFORE UPDATE ON cat_analytes
         BEGIN SELECT RAISE(ABORT, 'catalog reference data is immutable'); END;",
        )
        .unwrap();
    let before: Vec<_> = TABLES.iter().map(|table| rows(&c.raw(), table)).collect();

    let upgraded = c.t.data_dir().join("upgraded");
    std::fs::create_dir(&upgraded).unwrap();
    c.t.db.snapshot(&upgraded.join(DATABASE_FILE)).unwrap();
    let modules = [
        lis_auth::SCHEMA,
        ModuleSchema {
            migrations: &MIGRATIONS[..5],
            ..SCHEMA
        },
    ];
    let db = Database::open(&upgraded, &modules, Arc::clone(&c.t.clock) as _).unwrap();
    let conn = Connection::open(upgraded.join(DATABASE_FILE)).unwrap();

    let after: Vec<_> = TABLES.iter().map(|table| rows(&conn, table)).collect();
    assert_eq!(after, before);
    let applied = "SELECT max(version) FROM store_migrations WHERE module = 'catalog'";
    assert_eq!(scalar(&conn, applied), 5);
    let violations = "SELECT count(*) FROM pragma_foreign_key_check";
    assert_eq!(scalar(&conn, violations), 0);
    assert_frozen_content_guards(&conn, &seeded);

    // The default policy now governs approval: the author is refused, in schema and code.
    let message = conn
        .execute(
            "UPDATE cat_orderable_versions SET approved_by = created_by WHERE id = ?1",
            [seeded.test_draft.to_string()],
        )
        .unwrap_err()
        .to_string();
    assert!(message.contains("separation of duties"), "{message}");
    let author = user_principal(c.author, &BOTH);
    let refused = approve_policy(&db, author, seeded.policy_draft, true);
    assert!(matches!(refused, Err(CatalogError::SelfApproval)));
    let approver = user_principal(c.approver, &[Capability::ApproveConfiguration]);
    assert_eq!(
        approve_policy(&db, approver, seeded.policy_draft, false).unwrap(),
        2
    );
}

#[test]
fn report_content_migration_preserves_legacy_columns_and_leaves_new_content_absent() {
    let c = Catalog::open();
    let seeded = seed(&c);
    let raw = c.raw();
    // Reconstruct the immediately preceding schema with genuine existing rows.
    raw.execute_batch(
        "DROP TRIGGER cat_method_report_notes_frozen;
         ALTER TABLE cat_method_versions DROP COLUMN report_notes;
         ALTER TABLE cat_method_version_analytes DROP COLUMN reference_interval;
         ALTER TABLE cat_method_version_analytes DROP COLUMN range_policy;",
    )
    .unwrap();
    let before: Vec<_> = TABLES.iter().map(|table| rows(&raw, table)).collect();
    raw.execute_batch(MIGRATIONS[5].sql).unwrap();
    let after: Vec<_> = TABLES.iter().map(|table| rows(&raw, table)).collect();
    for (old_table, new_table) in before.iter().zip(&after) {
        assert_eq!(old_table.len(), new_table.len());
        for (old_row, new_row) in old_table.iter().zip(new_table) {
            assert_eq!(old_row.as_slice(), &new_row[..old_row.len()]);
        }
    }
    assert_eq!(
        scalar(
            &raw,
            "SELECT count(*) FROM cat_method_versions WHERE report_notes IS NOT NULL"
        ),
        0
    );
    assert_eq!(
        scalar(
            &raw,
            "SELECT count(*) FROM cat_method_version_analytes WHERE reference_interval IS NOT NULL OR range_policy <> 'report_only'"
        ),
        0
    );
    assert_eq!(
        scalar(&raw, "SELECT count(*) FROM pragma_foreign_key_check"),
        0
    );
    assert_frozen_content_guards(&raw, &seeded);
}
