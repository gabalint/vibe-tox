//! Analyte corrections and retirement (REQ-ANA-002): each change is a new
//! revision with a reason, an immutable history and an audit event; the code
//! is permanent once anything refers to the analyte; retirement is refused
//! while a current or draft method version includes it.

use lis_catalog::{
    AnalyteId, AnalyteInput, AnalyteView, CatalogError, MIGRATIONS, SCHEMA, UpdateAnalyte,
    analyte_history, analyte_usage, list_analytes, update_analyte,
};
use lis_core::{Capability, ErrorCode, ModuleSchema, UserId};
use lis_store::testing::TestDatabase;
use rusqlite::Connection;

use crate::{Catalog, limits};

fn current(c: &Catalog, id: AnalyteId) -> AnalyteView {
    c.read(list_analytes)
        .into_iter()
        .find(|row| row.id == id)
        .unwrap()
}

/// The analyte's current facts with `change` applied, at its current revision.
fn correction(
    c: &Catalog,
    id: AnalyteId,
    change: impl FnOnce(&mut UpdateAnalyte),
) -> UpdateAnalyte {
    let now = current(c, id);
    let mut input = UpdateAnalyte {
        analyte_id: id,
        expected_revision: now.revision,
        code: now.code,
        name: now.name,
        loinc_code: now.loinc_code,
        drug_class_code: now.drug_class_code,
        active: now.active,
        reason: "Synthetic correction from the supplier sheet".to_owned(),
    };
    change(&mut input);
    input
}

fn correct(c: &Catalog, input: UpdateAnalyte) -> Result<AnalyteView, CatalogError> {
    c.as_author(move |ctx| update_analyte(ctx, input))
}

fn field_error(result: Result<AnalyteView, CatalogError>) -> (String, String) {
    match result {
        Err(CatalogError::Invalid(error)) => (error.field, error.code),
        other => panic!("expected a validation error, got {other:?}"),
    }
}

#[test]
fn a_correction_adds_a_revision_with_history_and_audit() {
    let c = Catalog::open();
    let input = correction(&c, c.analyte_a, |input| {
        input.code = "SYN-A2".to_owned();
        input.name = "Synthetic analyte corrected".to_owned();
        input.loinc_code = None;
        input.drug_class_code = Some("SYN-CLASS2".to_owned());
    });
    let view = correct(&c, input).unwrap();
    assert_eq!(view.revision, 2);
    assert_eq!(view.code, "SYN-A2");
    assert_eq!(view.loinc_code, None);
    assert!(view.active);
    assert_eq!(current(&c, c.analyte_a), view);

    let id = c.analyte_a;
    let history = c.read(move |conn| analyte_history(conn, AnalyteInput { analyte_id: id }));
    let summary: Vec<_> = history
        .iter()
        .map(|row| (row.revision, row.code.as_str(), row.origin.as_str()))
        .collect();
    assert_eq!(
        summary,
        [(1, "SYN-A", "registered"), (2, "SYN-A2", "corrected")]
    );
    assert_eq!(history[0].recorded_by, Some(c.author.to_string()));
    assert_eq!(history[0].reason, None);
    assert_eq!(history[1].name, "Synthetic analyte corrected");
    assert_eq!(
        history[1].reason.as_deref(),
        Some("Synthetic correction from the supplier sheet")
    );
    assert_eq!(c.audit_actions().last().unwrap(), "catalog.update_analyte");
}

#[test]
fn the_code_is_permanent_once_a_method_uses_the_analyte() {
    let c = Catalog::open();
    c.draft_method(c.method("SYN-M1")).unwrap();
    let id = c.analyte_a;
    let usage = c.read(move |conn| analyte_usage(conn, AnalyteInput { analyte_id: id }));
    assert!(usage.code_in_use);
    assert_eq!(usage.methods.len(), 1);
    assert_eq!(usage.methods[0].method_code, "SYN-M1");

    let renamed = correction(&c, id, |input| input.code = "SYN-A2".to_owned());
    assert_eq!(
        field_error(correct(&c, renamed)),
        ("code".to_owned(), "in_use".to_owned())
    );
    let raw = c.raw().execute(
        "UPDATE cat_analytes SET code = 'SYN-A3', revision = revision + 1 WHERE id = ?1",
        [id.to_string()],
    );
    assert!(
        raw.unwrap_err()
            .to_string()
            .contains("matching immutable history")
    );

    let name = correction(&c, id, |input| {
        input.name = "Synthetic analyte renamed".to_owned()
    });
    assert_eq!(correct(&c, name).unwrap().name, "Synthetic analyte renamed");
}

#[test]
fn retirement_waits_for_the_methods_and_blocks_new_use() {
    let c = Catalog::open();
    let used = c.analyte_a;
    c.approved_method("SYN-M1");
    let retire = correction(&c, used, |input| input.active = false);
    let (field, code) = field_error(correct(&c, retire));
    assert_eq!((field.as_str(), code.as_str()), ("active", "in_use"));

    let spare = c.analyte("SYN-C").unwrap();
    let retire = correction(&c, spare, |input| input.active = false);
    let retired = correct(&c, retire).unwrap();
    assert!(!retired.active);
    assert!(c.read(list_analytes).iter().any(|row| row.id == spare));

    let mut content = c.content();
    content.analytes.push(limits(spare));
    let method = c.method("SYN-M2");
    let refused = c.as_author(move |ctx| {
        lis_catalog::draft_method_version(
            ctx,
            lis_catalog::DraftMethodVersion {
                method_id: method,
                content,
            },
        )
    });
    match refused {
        Err(CatalogError::Invalid(error)) => assert_eq!(error.code, "retired"),
        other => panic!("expected a retired analyte refusal, got {other:?}"),
    }
}

#[test]
fn corrections_are_refused_without_capability_reason_change_or_current_revision() {
    let c = Catalog::open();
    let id = c.analyte_a;
    let before = c.counts();
    let input = correction(&c, id, |input| input.name = "Synthetic other".to_owned());
    let refused = c.as_user(
        UserId::generate(),
        &[Capability::ApproveConfiguration],
        move |ctx| update_analyte(ctx, input),
    );
    assert_eq!(refused.unwrap_err().code(), ErrorCode::Forbidden);

    let unchanged = correction(&c, id, |_| {});
    assert_eq!(field_error(correct(&c, unchanged)).1, "unchanged");
    let no_reason = correction(&c, id, |input| {
        input.name = "Synthetic other".to_owned();
        input.reason = "  ".to_owned();
    });
    assert_eq!(
        field_error(correct(&c, no_reason)),
        ("reason".to_owned(), "required".to_owned())
    );
    let stale = correction(&c, id, |input| {
        input.name = "Synthetic other".to_owned();
        input.expected_revision = 7;
    });
    assert!(matches!(
        correct(&c, stale),
        Err(CatalogError::RevisionConflict)
    ));
    let taken = correction(&c, id, |input| input.code = "SYN-B".to_owned());
    assert!(matches!(
        correct(&c, taken),
        Err(CatalogError::Duplicate("analyte"))
    ));
    assert_eq!(c.counts(), before);
}

#[test]
fn the_migration_records_a_legacy_baseline_for_existing_analytes() {
    let legacy = ModuleSchema {
        migrations: &MIGRATIONS[..7],
        ..SCHEMA
    };
    let t = TestDatabase::open(&[lis_auth::SCHEMA, legacy]).unwrap();
    let conn = Connection::open(t.data_dir().join(lis_store::DATABASE_FILE)).unwrap();
    conn.execute(
        "INSERT INTO cat_analytes (id, code, name, loinc_code, drug_class_code, created_at) \
         VALUES ('legacy-analyte', 'SYN-OLD', 'Synthetic legacy', NULL, 'SYN-CLASS', \
         '2026-01-01T00:00:00Z')",
        [],
    )
    .unwrap();
    conn.execute_batch(MIGRATIONS[7].sql).unwrap();
    let row: (i64, String, Option<String>, String) = conn
        .query_row(
            "SELECT revision, code, recorded_by, origin FROM cat_analyte_history \
             WHERE analyte_id = 'legacy-analyte'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
        )
        .unwrap();
    assert_eq!(
        row,
        (1, "SYN-OLD".to_owned(), None, "legacy_baseline".to_owned())
    );
    let active: (bool, i64) = conn
        .query_row(
            "SELECT active, revision FROM cat_analytes WHERE id = 'legacy-analyte'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(active, (true, 1));
}
