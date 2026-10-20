//! Orderable versions: manifests, activation completeness and frozen
//! versions for orders (REQ-ANA-009, REQ-ORD-002).

use lis_catalog::{
    ApproveOrderableVersion, CatalogError, ComponentInput, ComponentPurpose, OrderableKind,
    OrderableVersionContent, OrderableVersionId, RetireMethodVersion, RetireOrderableVersion,
    UpdateMethodDraft, UpdateOrderableDraft, VersionStatus, approve_orderable_version,
    get_orderable_version, list_active_orderables, list_orderable_versions, retire_method_version,
    retire_orderable_version, update_method_draft, update_orderable_draft,
};
use lis_core::ErrorCode;

use crate::Catalog;

fn approve(
    c: &Catalog,
    id: OrderableVersionId,
    expected_revision: i64,
) -> Result<i64, CatalogError> {
    c.as_approver(move |ctx| {
        approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id: id,
                expected_revision,
                self_approval: false,
            },
        )
    })
    .map(|approved| approved.revision)
}

fn field_code(error: CatalogError) -> (String, String) {
    let CatalogError::Invalid(field) = error else {
        panic!("{error:?}")
    };
    (field.field, field.code)
}

#[test]
fn an_approved_orderable_is_offered_with_its_manifest() {
    let c = Catalog::open();
    let screen = c.approved_method("SYN-M1");
    let confirm = c.approved_method("SYN-M2");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let id = c
        .draft_orderable(panel, c.manifest(&[screen, confirm]))
        .unwrap();
    assert!(
        c.read(list_active_orderables).is_empty(),
        "a draft is not offered"
    );
    approve(&c, id, 1).unwrap();

    let active = c.read(list_active_orderables);
    assert_eq!(active.len(), 1);
    let offered = &active[0];
    assert_eq!((offered.id, offered.code.as_str()), (id, "SYN-P1"));
    assert_eq!(offered.components.len(), 2);
    assert_eq!(offered.components[0].position, 1);
    assert_eq!(offered.components[0].method_version_id, screen);
    assert_eq!(offered.components[1].method_version_id, confirm);
    assert_eq!(offered.components[0].specimen_type_code, "SYN-UR");
    assert_eq!(offered.components[0].analytes.len(), 1);
    assert_eq!(offered.components[0].analytes[0].analyte_id, c.analyte_a);
    assert_eq!(
        offered.components[0].analytes[0].drug_class_code.as_deref(),
        Some("SYN-CLASS")
    );
}

#[test]
fn a_frozen_version_keeps_its_manifest_after_the_catalog_changes() {
    let c = Catalog::open();
    let m1 = c.approved_method("SYN-M1");
    let m2 = c.approved_method("SYN-M2");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let v1 = c.draft_orderable(panel, c.manifest(&[m1])).unwrap();
    approve(&c, v1, 1).unwrap();
    let frozen = c.read(move |conn| get_orderable_version(conn, v1));

    let v2 = c.draft_orderable(panel, c.manifest(&[m1, m2])).unwrap();
    approve(&c, v2, 1).unwrap();
    let active = c.read(list_active_orderables);
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, v2);

    let later = c.read(move |conn| get_orderable_version(conn, v1));
    assert_eq!(later.components, frozen.components);
    assert_eq!(later.lifecycle.replaced_by, Some(v2));
    let history = c.read(move |conn| list_orderable_versions(conn, panel));
    assert_eq!(history.len(), 2);
}

#[test]
fn approval_needs_every_component_on_a_current_method_version() {
    let c = Catalog::open();
    let method = c.approved_method("SYN-M1");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let draft = c.draft_orderable(panel, c.manifest(&[method])).unwrap();
    // The method version is retired after the draft referenced it.
    c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: method,
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            },
        )
    })
    .unwrap();
    let before = c.counts();
    let error = approve(&c, draft, 1).unwrap_err();
    assert!(matches!(error, CatalogError::IncompleteConfiguration));
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert_eq!(c.counts(), before);
}

#[test]
fn a_superseded_method_version_also_blocks_approval() {
    let c = Catalog::open();
    let method_id = c.method("SYN-M1");
    let v1 = c.draft_method(method_id).unwrap();
    c.approve_method(v1, 1).unwrap();
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let draft = c.draft_orderable(panel, c.manifest(&[v1])).unwrap();
    let v2 = c.draft_method(method_id).unwrap();
    c.approve_method(v2, 1).unwrap();
    assert!(matches!(
        approve(&c, draft, 1),
        Err(CatalogError::IncompleteConfiguration)
    ));
}

#[test]
fn a_current_orderable_blocks_method_supersession() {
    let c = Catalog::open();
    let method_id = c.method("SYN-M1");
    let m1 = c.draft_method(method_id).unwrap();
    c.approve_method(m1, 1).unwrap();
    let orderable = c.orderable("SYN-O1", OrderableKind::Test);
    let orderable_version = c.draft_orderable(orderable, c.manifest(&[m1])).unwrap();
    approve(&c, orderable_version, 1).unwrap();
    let m2 = c.draft_method(method_id).unwrap();
    let before_counts = c.counts();
    let before_audit = c.audit_actions();

    let error = c.approve_method(m2, 1).unwrap_err();
    match &error {
        CatalogError::MethodInUse {
            blocking_orderables,
        } => assert_eq!(blocking_orderables, &[orderable]),
        other => panic!("{other:?}"),
    }
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert!(
        c.read(move |conn| lis_catalog::get_method_version(conn, m1))
            .lifecycle
            .is_current()
    );
    assert_eq!(
        c.read(move |conn| lis_catalog::get_method_version(conn, m2))
            .lifecycle
            .status,
        VersionStatus::Draft
    );
    assert_eq!(c.counts(), before_counts);
    assert_eq!(c.audit_actions(), before_audit);
}

#[test]
fn drafts_refuse_retired_methods_and_unmeasured_analytes() {
    let c = Catalog::open();
    let retired = c.approved_method("SYN-M1");
    c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: retired,
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            },
        )
    })
    .unwrap();
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let error = c
        .draft_orderable(panel, c.manifest(&[retired]))
        .unwrap_err();
    assert_eq!(
        field_code(error),
        (
            "components[0].methodVersionId".to_owned(),
            "not_current".to_owned()
        )
    );

    let approved = c.approved_method("SYN-M2");
    let other = c.analyte("SYN-Z").unwrap();
    let content = OrderableVersionContent {
        components: vec![ComponentInput {
            method_version_id: approved,
            purpose: ComponentPurpose::Analytical,
            analyte_ids: vec![other],
        }],
    };
    let error = c.draft_orderable(panel, content).unwrap_err();
    assert_eq!(
        field_code(error),
        (
            "components[0].analyteIds[0]".to_owned(),
            "not_measured".to_owned()
        )
    );
}

#[test]
fn a_test_has_exactly_one_component() {
    let c = Catalog::open();
    let m1 = c.approved_method("SYN-M1");
    let m2 = c.approved_method("SYN-M2");
    let test = c.orderable("SYN-T1", OrderableKind::Test);
    let error = c.draft_orderable(test, c.manifest(&[m1, m2])).unwrap_err();
    assert_eq!(field_code(error).1, "component_count");
    assert!(c.draft_orderable(test, c.manifest(&[m1])).is_ok());
}

#[test]
fn a_method_version_in_current_use_cannot_be_retired() {
    let c = Catalog::open();
    let method = c.approved_method("SYN-M1");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let id = c.draft_orderable(panel, c.manifest(&[method])).unwrap();
    approve(&c, id, 1).unwrap();
    let result = c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: method,
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            },
        )
    });
    match result.unwrap_err() {
        CatalogError::MethodInUse {
            blocking_orderables,
        } => assert_eq!(blocking_orderables, [panel]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_retired_orderable_is_withdrawn_from_new_orders_but_readable() {
    let c = Catalog::open();
    let method = c.approved_method("SYN-M1");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let id = c.draft_orderable(panel, c.manifest(&[method])).unwrap();
    approve(&c, id, 1).unwrap();
    let approved = c.read(move |conn| get_orderable_version(conn, id));
    c.as_approver(move |ctx| {
        retire_orderable_version(
            ctx,
            RetireOrderableVersion {
                orderable_version_id: id,
                expected_revision: 2,
                reason: "Panel withdrawn".to_owned(),
            },
        )
    })
    .unwrap();
    assert!(c.read(list_active_orderables).is_empty());
    let retired = c.read(move |conn| get_orderable_version(conn, id));
    assert_eq!(retired.lifecycle.status, VersionStatus::Retired);
    assert_eq!(retired.components, approved.components);
    assert_eq!(
        c.audit_actions().last().unwrap(),
        "catalog.retire_orderable_version"
    );
}

#[test]
fn an_approved_manifest_cannot_be_edited() {
    let c = Catalog::open();
    let method = c.approved_method("SYN-M1");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let id = c.draft_orderable(panel, c.manifest(&[method])).unwrap();
    approve(&c, id, 1).unwrap();
    let content = c.manifest(&[method]);
    let result = c.as_author(move |ctx| {
        update_orderable_draft(
            ctx,
            UpdateOrderableDraft {
                orderable_version_id: id,
                expected_revision: 2,
                content,
            },
        )
    });
    assert!(matches!(result, Err(CatalogError::NotDraft)));
    let conn = c.raw();
    for sql in [
        "DELETE FROM cat_orderable_component_analytes WHERE orderable_version_id = ?1",
        "DELETE FROM cat_orderable_components WHERE orderable_version_id = ?1",
        "UPDATE cat_orderable_components SET position = 9 WHERE orderable_version_id = ?1",
        "UPDATE cat_orderable_versions SET approved_by = created_by WHERE id = ?1",
        "DELETE FROM cat_orderable_versions WHERE id = ?1",
    ] {
        assert!(conn.execute(sql, [id]).is_err(), "{sql}");
    }
}

#[test]
fn a_draft_manifest_is_replaced_on_update() {
    let c = Catalog::open();
    let m1 = c.approved_method("SYN-M1");
    let m2 = c.approved_method("SYN-M2");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let id = c.draft_orderable(panel, c.manifest(&[m1])).unwrap();
    let content = c.manifest(&[m2, m1]);
    let updated = c
        .as_author(move |ctx| {
            update_orderable_draft(
                ctx,
                UpdateOrderableDraft {
                    orderable_version_id: id,
                    expected_revision: 1,
                    content,
                },
            )
        })
        .unwrap();
    assert_eq!(updated.revision, 2);
    let view = c.read(move |conn| get_orderable_version(conn, id));
    let order: Vec<_> = view
        .components
        .iter()
        .map(|x| x.method_version_id)
        .collect();
    assert_eq!(order, [m2, m1]);
}

#[test]
fn a_draft_may_use_a_method_draft_but_is_approved_only_after_it() {
    let c = Catalog::open();
    let method_draft = c.draft_method(c.method("SYN-M1")).unwrap();
    let test = c.orderable("SYN-T1", OrderableKind::Test);
    let draft = c
        .draft_orderable(test, c.manifest(&[method_draft]))
        .unwrap();
    let before = c.counts();
    // REQ-ANA-009: approval still needs every component current approved.
    let error = approve(&c, draft, 1).unwrap_err();
    assert!(matches!(error, CatalogError::IncompleteConfiguration));
    assert_eq!(c.counts(), before);
    c.approve_method(method_draft, 1).unwrap();
    approve(&c, draft, 1).unwrap();
    assert_eq!(c.read(list_active_orderables)[0].id, draft);
}

#[test]
fn a_method_draft_keeps_the_analytes_an_orderable_draft_expects() {
    let c = Catalog::open();
    let method_draft = c.draft_method(c.method("SYN-M1")).unwrap();
    let test = c.orderable("SYN-T1", OrderableKind::Test);
    c.draft_orderable(test, c.manifest(&[method_draft]))
        .unwrap();
    let update = |content: lis_catalog::MethodVersionContent, expected_revision| {
        c.as_author(move |ctx| {
            update_method_draft(
                ctx,
                UpdateMethodDraft {
                    method_version_id: method_draft,
                    expected_revision,
                    content,
                },
            )
        })
    };
    let before = c.counts();
    let mut without_a = c.content();
    without_a.analytes.remove(0);
    let error = update(without_a, 1).unwrap_err();
    let CatalogError::Invalid(field) = &error else {
        panic!("{error:?}")
    };
    assert_eq!(
        (field.field.as_str(), field.code.as_str()),
        ("analytes", "in_use_by_orderable")
    );
    assert!(field.message.contains("SYN-T1 v1"));
    assert_eq!(c.counts(), before);

    // Changing the kept analyte's limits and dropping an unused one is allowed.
    let mut kept = c.content();
    kept.analytes.truncate(1);
    kept.analytes[0].cutoff = crate::dec("222.2");
    assert_eq!(update(kept, 1).unwrap().revision, 2);
    let view = c.read(move |conn| lis_catalog::get_method_version(conn, method_draft));
    assert_eq!(view.analytes.len(), 1);
    assert_eq!(
        view.analytes[0].limits.cutoff.as_ref().unwrap().as_str(),
        "222.2"
    );
}
