//! Optional external approval evidence is attributed, atomic, immutable and nonclinical.
use crate::*;
use lis_catalog::{
    ApproveReviewedCatalog, DemoValues, VersionStatus, approve_reviewed_catalog,
    get_method_version, list_catalog_previews,
};

#[test]
fn method_external_reference_is_optional_attributed_and_immutable() {
    let c = Catalog::open();
    let legacy = c.approved_method("SYN-UNREFERENCED");
    assert_eq!(
        c.read(move |db| get_method_version(db, legacy))
            .external_approval_reference,
        None
    );
    let id = c.draft_method(c.method("SYN-REFERENCED")).unwrap();
    c.as_approver(move |ctx| {
        approve_method_version(
            ctx,
            ApproveMethodVersion {
                method_version_id: id,
                expected_revision: 1,
                self_approval: false,
                external_approval_reference: Some("  LAB-APPROVAL-2026/17  ".into()),
            },
        )
    })
    .unwrap();
    let view = c.read(move |db| get_method_version(db, id));
    assert_eq!(
        view.external_approval_reference.as_deref(),
        Some("LAB-APPROVAL-2026/17")
    );
    assert_eq!(view.report_notes, None);
    let db = c.raw();
    let attribution: (UserId, lis_core::UtcTimestamp) = db
        .query_row(
            "SELECT recorded_by, recorded_at FROM cat_approval_references WHERE record_id = ?1",
            [id],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )
        .unwrap();
    assert_eq!(attribution.0, c.approver);
    assert_eq!(Some(attribution.1), view.lifecycle.effective_from);
    assert!(
        c.audit_actions()
            .contains(&"catalog.external_approval_reference".into())
    );
    let audit: (UserId, String, String) = db.query_row(
        "SELECT principal_id, authority, reason FROM audit_events WHERE action = 'catalog.external_approval_reference' AND record_id = ?1", [id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?))).unwrap();
    assert_eq!(audit.0, c.approver);
    assert_eq!(audit.1, "approve_configuration");
    assert_eq!(audit.2, "LAB-APPROVAL-2026/17");

    assert!(
        db.execute(
            "UPDATE cat_approval_references SET external_reference = 'other'",
            []
        )
        .is_err()
    );
    assert!(
        db.execute("DELETE FROM cat_approval_references", [])
            .is_err()
    );
    assert!(db.execute("INSERT INTO cat_approval_references SELECT record_kind, record_id, external_reference, recorded_by, recorded_at FROM cat_approval_references", []).is_err());
}

#[test]
fn invalid_denied_stale_and_interrupted_reference_approval_leave_draft_unchanged() {
    let c = Catalog::open();
    let id = c.draft_method(c.method("SYN-REF-FAIL")).unwrap();
    let input = |reference: &str| ApproveMethodVersion {
        method_version_id: id,
        expected_revision: 1,
        self_approval: false,
        external_approval_reference: Some(reference.into()),
    };
    let before = c.counts();
    for reference in [" ".to_owned(), "x".repeat(1001), "a\nb".to_owned()] {
        let request = input(&reference);
        assert!(
            c.as_approver(move |ctx| approve_method_version(ctx, request))
                .is_err()
        );
        assert_eq!(c.counts(), before);
    }
    let request = input("SOURCE-17");
    assert!(
        c.as_user(c.author, &[Capability::ManageConfiguration], move |ctx| {
            approve_method_version(ctx, request)
        })
        .is_err()
    );
    let mut request = input("SOURCE-17");
    request.expected_revision = 99;
    assert!(matches!(
        c.as_approver(move |ctx| approve_method_version(ctx, request)),
        Err(CatalogError::RevisionConflict)
    ));
    let request = input("SOURCE-17");
    let interrupted: Result<(), CatalogError> = c.as_approver(move |ctx| {
        approve_method_version(ctx, request)?;
        Err(CatalogError::RevisionConflict)
    });
    assert!(interrupted.is_err());
    assert_eq!(c.counts(), before);
    assert_eq!(
        c.read(move |db| get_method_version(db, id))
            .lifecycle
            .status,
        VersionStatus::Draft
    );
}

#[test]
fn catalog_review_reference_is_retained_and_shared_by_exact_approved_methods() {
    let c = crate::review::loaded();
    let snapshot = crate::review::snapshot(&c, DemoValues::default());
    let preview_id =
        crate::review::record(&c, c.author, &[Capability::ManageConfiguration], &snapshot).unwrap();
    let confirmed_blank_cutoffs = crate::review::blank_cutoffs(&snapshot);
    c.as_approver(move |ctx| {
        approve_reviewed_catalog(
            ctx,
            ApproveReviewedCatalog {
                preview_id,
                confirmed_blank_cutoffs,
                self_approval: false,
                external_approval_reference: Some("BATCH-APPROVAL-23".into()),
            },
        )
    })
    .unwrap();
    let previews = c.read(list_catalog_previews);
    assert_eq!(
        previews[0].external_approval_reference.as_deref(),
        Some("BATCH-APPROVAL-23")
    );
    for draft in snapshot.drafts {
        if let lis_catalog::CatalogDraft::MethodVersion { id, .. } = draft {
            assert_eq!(
                c.read(move |db| get_method_version(db, id))
                    .external_approval_reference
                    .as_deref(),
                Some("BATCH-APPROVAL-23")
            );
        }
    }
}

#[test]
fn catalog_review_without_methods_still_records_optional_approval_reference() {
    let c = Catalog::open();
    c.draft_validity(c.specimen_type).unwrap();
    let snapshot = crate::review::snapshot(&c, DemoValues::default());
    let preview_id =
        crate::review::record(&c, c.author, &[Capability::ManageConfiguration], &snapshot).unwrap();
    let result = c
        .as_approver(move |ctx| {
            approve_reviewed_catalog(
                ctx,
                ApproveReviewedCatalog {
                    preview_id,
                    confirmed_blank_cutoffs: vec![],
                    self_approval: false,
                    external_approval_reference: Some("POLICY-APPROVAL-4".into()),
                },
            )
        })
        .unwrap();
    assert_eq!(result.methods, 0);
    let wrong_actor = c.raw().execute(
        "INSERT INTO cat_approval_references SELECT 'catalog_review', id, 'wrong actor', ?1, reviewed_at FROM cat_catalog_reviews WHERE id = ?2",
        rusqlite::params![c.author, result.review_id],
    ).unwrap_err();
    assert!(
        wrong_actor
            .to_string()
            .contains("reference must match catalog review")
    );
    assert_eq!(
        c.read(list_catalog_previews)[0]
            .external_approval_reference
            .as_deref(),
        Some("POLICY-APPROVAL-4")
    );
}

#[test]
fn external_reference_migration_preserves_legacy_approvals_without_backfill() {
    let c = Catalog::open();
    let id = c.approved_method("SYN-LEGACY-REF");
    let before = c.read(move |db| get_method_version(db, id));
    let raw = c.raw();
    raw.execute_batch("DROP TABLE cat_approval_references;")
        .unwrap();
    raw.execute_batch(lis_catalog::MIGRATIONS[6].sql).unwrap();
    assert_eq!(c.read(move |db| get_method_version(db, id)), before);
    assert_eq!(
        raw.query_row("SELECT count(*) FROM cat_approval_references", [], |row| {
            row.get::<_, i64>(0)
        })
        .unwrap(),
        0
    );
    // The storage guard refuses inventing an approval actor for existing versions.
    assert!(raw.execute("INSERT INTO cat_approval_references VALUES ('method_version', ?1, 'fabricated', ?2, ?3)",
        rusqlite::params![id, c.author, before.lifecycle.effective_from]).is_err());
}
