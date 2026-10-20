//! Draft, approval, supersession and retirement of method versions (REQ-ANA-002).

use lis_catalog::{
    ApproveMethodVersion, CatalogError, OrderableKind, RetireMethodVersion, UpdateMethodDraft,
    VersionStatus, approve_method_version, get_method_version, list_method_versions,
    retire_method_version, update_method_draft,
};
use lis_core::{Capability, ErrorCode, UserId};

use crate::Catalog;

fn update(
    c: &Catalog,
    user: UserId,
    id: lis_catalog::MethodVersionId,
    expected_revision: i64,
) -> Result<i64, CatalogError> {
    let mut content = c.content();
    content.analytes.truncate(1);
    c.as_user(user, &[Capability::ManageConfiguration], move |ctx| {
        update_method_draft(
            ctx,
            UpdateMethodDraft {
                method_version_id: id,
                expected_revision,
                content,
            },
        )
    })
    .map(|changed| changed.revision)
}

#[test]
fn the_author_edits_a_draft_and_a_stale_revision_conflicts() {
    let c = Catalog::open();
    let id = c.draft_method(c.method("SYN-M1")).unwrap();
    assert_eq!(update(&c, c.author, id, 1).unwrap(), 2);
    assert_eq!(
        c.read(move |conn| get_method_version(conn, id))
            .analytes
            .len(),
        1
    );
    let before = c.counts();
    assert!(matches!(
        update(&c, c.author, id, 1),
        Err(CatalogError::RevisionConflict)
    ));
    assert!(matches!(
        c.approve_method(id, 1),
        Err(CatalogError::RevisionConflict)
    ));
    assert_eq!(c.counts(), before);
}

#[test]
fn only_the_author_may_edit_a_draft() {
    let c = Catalog::open();
    let id = c.draft_method(c.method("SYN-M1")).unwrap();
    let result = update(&c, UserId::generate(), id, 1);
    assert!(matches!(result, Err(CatalogError::NotDraftAuthor)));
}

#[test]
fn the_author_cannot_approve_their_own_draft() {
    let c = Catalog::open();
    let id = c.draft_method(c.method("SYN-M1")).unwrap();
    let before = c.counts();
    let both = [
        Capability::ManageConfiguration,
        Capability::ApproveConfiguration,
    ];
    let result = c.as_user(c.author, &both, move |ctx| {
        approve_method_version(
            ctx,
            ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    let error = result.unwrap_err();
    assert!(matches!(error, CatalogError::SelfApproval));
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert_eq!(c.counts(), before);
    // The database refuses it too.
    let direct = c.raw().execute(
        "UPDATE cat_method_versions SET status = 'approved', approved_by = created_by, \
         effective_from = created_at WHERE id = ?1",
        [id],
    );
    assert!(direct.is_err());
}

#[test]
fn approved_content_cannot_be_edited_in_code() {
    let c = Catalog::open();
    let id = c.approved_method("SYN-M1");
    let result = update(&c, c.author, id, 2);
    assert!(matches!(result, Err(CatalogError::NotDraft)));
    assert!(matches!(
        c.approve_method(id, 2),
        Err(CatalogError::NotDraft)
    ));
}

#[test]
fn approved_content_cannot_be_edited_with_sql() {
    let c = Catalog::open();
    let approved = c.approved_method("SYN-M1");
    let draft = c.draft_method(c.method("SYN-M2")).unwrap();
    let conn = c.raw();
    for sql in [
        "UPDATE cat_method_versions SET certainty = 'presumptive' WHERE id = ?1",
        "UPDATE cat_method_versions SET threshold_rule = 'above' WHERE id = ?1",
        "UPDATE cat_method_versions SET status = 'draft', approved_by = NULL, \
         effective_from = NULL WHERE id = ?1",
        "UPDATE cat_method_version_analytes SET cutoff = '1' WHERE method_version_id = ?1",
        "DELETE FROM cat_method_version_analytes WHERE method_version_id = ?1",
        "DELETE FROM cat_method_versions WHERE id = ?1",
    ] {
        assert!(conn.execute(sql, [approved]).is_err(), "{sql}");
    }
    let insert = conn.execute(
        "INSERT INTO cat_method_version_analytes (method_version_id, analyte_id) \
         SELECT ?1, id FROM cat_analytes WHERE code = 'SYN-A'",
        [approved],
    );
    assert!(insert.is_err());
    // A draft stays editable, so the triggers are selective.
    let edited = conn.execute(
        "UPDATE cat_method_version_analytes SET cutoff = '1' WHERE method_version_id = ?1",
        [draft],
    );
    assert_eq!(edited.unwrap(), 2);
    for sql in [
        "UPDATE cat_analytes SET drug_class_code = 'OTHER'",
        "UPDATE cat_specimen_types SET code = 'OTHER'",
        "DELETE FROM cat_methods",
    ] {
        assert!(conn.execute(sql, []).is_err(), "{sql}");
    }
}

#[test]
fn approving_a_new_version_ends_the_previous_one() {
    let c = Catalog::open();
    let method = c.method("SYN-M1");
    let v1 = c.draft_method(method).unwrap();
    c.approve_method(v1, 1).unwrap();
    let before = c.read(move |conn| get_method_version(conn, v1));
    assert!(before.lifecycle.is_current());

    let v2 = c.draft_method(method).unwrap();
    c.t.clock
        .set(lis_core::UtcTimestamp::parse("2026-02-03T04:05:06.000000000Z").unwrap());
    assert_eq!(c.approve_method(v2, 1).unwrap(), 2);

    let old = c.read(move |conn| get_method_version(conn, v1));
    assert_eq!(old.lifecycle.status, VersionStatus::Approved);
    assert!(!old.lifecycle.is_current());
    assert_eq!(old.lifecycle.replaced_by, Some(v2));
    assert_eq!(
        old.lifecycle.effective_to.unwrap().to_string(),
        "2026-02-03T04:05:06.000000000Z"
    );
    assert_eq!(old.lifecycle.revision, 3);
    // Content is exactly what was approved.
    assert_eq!(old.analytes, before.analytes);
    let new = c.read(move |conn| get_method_version(conn, v2));
    assert!(new.lifecycle.is_current());
    assert_eq!(new.lifecycle.approved_by, Some(c.approver));
    assert_eq!(new.lifecycle.created_by, c.author);

    let history = c.read(move |conn| list_method_versions(conn, method));
    let versions: Vec<i64> = history.iter().map(|v| v.version).collect();
    assert_eq!(versions, [1, 2]);
    let actions = c.audit_actions();
    let tail: Vec<&str> = actions
        .iter()
        .rev()
        .take(2)
        .rev()
        .map(String::as_str)
        .collect();
    assert_eq!(
        tail,
        [
            "catalog.supersede_method_version",
            "catalog.approve_method_version"
        ]
    );
}

#[test]
fn only_one_draft_per_method_at_a_time() {
    let c = Catalog::open();
    let method = c.method("SYN-M1");
    c.draft_method(method).unwrap();
    assert!(matches!(
        c.draft_method(method),
        Err(CatalogError::DraftExists)
    ));
}

#[test]
fn a_retired_version_stays_readable_and_cannot_return() {
    let c = Catalog::open();
    let id = c.approved_method("SYN-M1");
    let approved = c.read(move |conn| get_method_version(conn, id));
    let retire = |reason: &str, expected_revision: i64| {
        let reason = reason.to_owned();
        c.as_approver(move |ctx| {
            retire_method_version(
                ctx,
                RetireMethodVersion {
                    method_version_id: id,
                    expected_revision,
                    reason,
                },
            )
        })
    };
    assert!(matches!(retire(" ", 2), Err(CatalogError::Invalid(_))));
    assert_eq!(retire("Reagent discontinued", 2).unwrap().revision, 3);
    let retired = c.read(move |conn| get_method_version(conn, id));
    assert_eq!(retired.lifecycle.status, VersionStatus::Retired);
    assert_eq!(
        retired.lifecycle.retire_reason.as_deref(),
        Some("Reagent discontinued")
    );
    assert!(retired.lifecycle.effective_to.is_some());
    assert_eq!(retired.analytes, approved.analytes);
    assert_eq!(retired.certainty, approved.certainty);
    assert!(matches!(
        retire("Again", 3),
        Err(CatalogError::AlreadyRetired)
    ));
    let revived = c.raw().execute(
        "UPDATE cat_method_versions SET status = 'approved', retired_at = NULL, \
         retired_by = NULL, retire_reason = NULL WHERE id = ?1",
        [id],
    );
    assert!(revived.is_err());
}

#[test]
fn a_draft_cannot_be_retired() {
    let c = Catalog::open();
    let id = c.draft_method(c.method("SYN-M1")).unwrap();
    let result = c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: id,
                expected_revision: 1,
                reason: "Not needed".to_owned(),
            },
        )
    });
    assert!(matches!(result, Err(CatalogError::NotApproved)));
}

#[test]
fn version_rows_must_be_inserted_as_unapproved_drafts() {
    let c = Catalog::open();
    let method = c.method("SYN-RAW-M");
    let orderable = c.orderable("SYN-RAW-O", OrderableKind::Test);
    let conn = c.raw();
    let method_insert = conn.execute(
        "INSERT INTO cat_method_versions (id, method_id, version, status, revision, \
         specimen_type_id, certainty, output_scale, threshold_rule, threshold_rounding, \
         created_by, created_at, approved_by, effective_from) \
         VALUES ('raw-approved-method', ?1, 1, 'approved', 2, ?2, 'definitive', \
         'quantitative', 'at_or_above', 'before_rounding', ?3, '2026-01-01T00:00:00Z', \
         ?4, '2026-01-01T00:00:00Z')",
        rusqlite::params![method, c.specimen_type, c.author, c.approver],
    );
    assert!(method_insert.is_err());
    let orderable_insert = conn.execute(
        "INSERT INTO cat_orderable_versions (id, orderable_id, version, status, revision, \
         created_by, created_at, approved_by, effective_from) \
         VALUES ('raw-approved-orderable', ?1, 1, 'approved', 2, ?2, \
         '2026-01-01T00:00:00Z', ?3, '2026-01-01T00:00:00Z')",
        rusqlite::params![orderable, c.author, c.approver],
    );
    assert!(orderable_insert.is_err());
}

#[test]
fn direct_sql_cannot_fabricate_lifecycle_transitions() {
    let c = Catalog::open();
    let method = c.draft_method(c.method("SYN-RAW-M")).unwrap();
    let orderable = c.orderable("SYN-RAW-O", OrderableKind::Test);
    let conn = c.raw();
    conn.execute(
        "INSERT INTO cat_orderable_versions (id, orderable_id, version, status, revision, \
         created_by, created_at) VALUES ('raw-orderable-draft', ?1, 1, 'draft', 1, ?2, \
         '2026-01-01T00:00:00Z')",
        rusqlite::params![orderable, c.author],
    )
    .unwrap();

    let fabricated_approval = conn.execute(
        "UPDATE cat_method_versions SET status = 'approved', revision = 2, approved_by = ?2, \
         effective_from = '2026-01-01T00:00:00Z' WHERE id = ?1",
        rusqlite::params![method, c.approver],
    );
    let message = fabricated_approval.unwrap_err().to_string();
    assert!(message.contains("authorized audit event"), "{message}");

    // ADR 0015: the default policy needs a second person.
    let equal_author = conn.execute(
        "UPDATE cat_orderable_versions SET status = 'approved', revision = 2, \
         approved_by = created_by, effective_from = '2026-01-01T00:00:00Z' \
         WHERE id = 'raw-orderable-draft'",
        [],
    );
    let message = equal_author.unwrap_err().to_string();
    assert!(message.contains("separation of duties"), "{message}");

    let skipped_approval = conn.execute(
        "UPDATE cat_method_versions SET status = 'retired', revision = 2, approved_by = ?2, \
         effective_from = '2026-01-01T00:00:00Z', effective_to = '2026-01-02T00:00:00Z', \
         retired_by = ?2, retired_at = '2026-01-02T00:00:00Z', retire_reason = 'Withdrawn' \
         WHERE id = ?1",
        rusqlite::params![method, c.approver],
    );
    assert!(skipped_approval.is_err());
}
