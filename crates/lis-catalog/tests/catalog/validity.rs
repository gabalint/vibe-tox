//! Validity policy versions (REQ-SPC-008) and component purpose (REQ-SPC-004).

use lis_catalog::{
    ApproveOrderableVersion, CatalogError, ComponentInput, ComponentPurpose, OrderableKind,
    OrderableVersionContent, RetireValidityPolicyVersion, UpdateValidityPolicyDraft,
    ValidityAction, VersionStatus, approve_orderable_version, approved_validity_policy,
    get_orderable_version, list_active_orderables, list_validity_policy_versions,
    retire_validity_policy_version, update_validity_policy_draft, validity_policy,
};
use lis_core::{Capability, ErrorCode};

use crate::Catalog;

#[test]
fn an_approved_policy_is_current_for_its_specimen_type_with_its_categories() {
    let c = Catalog::open();
    let id = c.draft_validity(c.specimen_type).unwrap();
    assert!(
        c.read(|conn| approved_validity_policy(conn, "SYN-UR"))
            .is_none(),
        "a draft is not current"
    );
    assert_eq!(c.approve_validity(id, 1).unwrap(), 2);

    let current = c
        .read(|conn| approved_validity_policy(conn, "SYN-UR"))
        .unwrap();
    assert_eq!((current.id, current.version), (id, 1));
    assert_eq!(current.specimen_type_code, "SYN-UR");
    assert_eq!(current.lifecycle.status, VersionStatus::Approved);
    assert_eq!(current.lifecycle.approved_by, Some(c.approver));
    // Categories come back by code, with the qualifier only on the qualifier action.
    let categories: Vec<_> = current
        .categories
        .iter()
        .map(|category| {
            (
                category.code.as_str(),
                category.action,
                category.qualifier_text.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        categories,
        [
            (
                "SYN-DILUTE",
                ValidityAction::Qualifier,
                Some("Synthetic qualifier text")
            ),
            ("SYN-NORMAL", ValidityAction::None, None),
        ]
    );
    assert!(
        c.read(|conn| approved_validity_policy(conn, "SYN-OTHER"))
            .is_none()
    );
    assert!(c.audit_actions().ends_with(&[
        "catalog.draft_validity_policy_version".to_owned(),
        "catalog.approve_validity_policy_version".to_owned(),
    ]));
}

#[test]
fn a_new_approval_supersedes_and_retirement_leaves_no_policy() {
    let c = Catalog::open();
    let first = c.draft_validity(c.specimen_type).unwrap();
    c.approve_validity(first, 1).unwrap();
    let second = c.draft_validity(c.specimen_type).unwrap();
    c.approve_validity(second, 1).unwrap();

    let superseded = c.read(move |conn| validity_policy(conn, first));
    assert_eq!(superseded.lifecycle.replaced_by, Some(second));
    assert!(superseded.lifecycle.effective_to.is_some());
    assert_eq!(superseded.categories.len(), 2, "history keeps its content");
    let current = c.read(|conn| approved_validity_policy(conn, "SYN-UR"));
    assert_eq!(current.map(|policy| policy.id), Some(second));

    c.as_approver(move |ctx| {
        retire_validity_policy_version(
            ctx,
            RetireValidityPolicyVersion {
                policy_version_id: second,
                expected_revision: 2,
                reason: "Synthetic withdrawal".to_owned(),
            },
        )
    })
    .unwrap();
    assert!(
        c.read(|conn| approved_validity_policy(conn, "SYN-UR"))
            .is_none()
    );
    let specimen_type = c.specimen_type;
    let history = c.read(move |conn| list_validity_policy_versions(conn, specimen_type));
    let states: Vec<_> = history
        .iter()
        .map(|v| (v.version, v.lifecycle.status))
        .collect();
    assert_eq!(
        states,
        [(1, VersionStatus::Approved), (2, VersionStatus::Retired)]
    );
}

#[test]
fn the_author_cannot_approve_their_own_policy() {
    let c = Catalog::open();
    let id = c.draft_validity(c.specimen_type).unwrap();
    let before = c.counts();
    let both = [
        Capability::ManageConfiguration,
        Capability::ApproveConfiguration,
    ];
    let result = c.as_user(c.author, &both, move |ctx| {
        lis_catalog::approve_validity_policy_version(
            ctx,
            lis_catalog::ApproveValidityPolicyVersion {
                policy_version_id: id,
                expected_revision: 1,
                self_approval: false,
            },
        )
    });
    let error = result.unwrap_err();
    assert!(matches!(error, CatalogError::SelfApproval));
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert_eq!(c.counts(), before);
}

#[test]
fn a_draft_is_edited_by_its_author_and_a_stale_revision_conflicts() {
    let c = Catalog::open();
    let id = c.draft_validity(c.specimen_type).unwrap();
    let mut content = c.validity_content();
    content.categories.truncate(1);
    let edit = move |expected_revision| {
        let content = content.clone();
        move |ctx: &mut lis_store::CommandContext<'_>| {
            update_validity_policy_draft(
                ctx,
                UpdateValidityPolicyDraft {
                    policy_version_id: id,
                    expected_revision,
                    content,
                },
            )
        }
    };
    assert_eq!(c.as_author(edit(1)).unwrap().revision, 2);
    assert_eq!(
        c.read(move |conn| validity_policy(conn, id))
            .categories
            .len(),
        1
    );
    assert!(matches!(
        c.as_author(edit(1)),
        Err(CatalogError::RevisionConflict)
    ));
    assert!(matches!(
        c.as_user(
            lis_core::UserId::generate(),
            &[Capability::ManageConfiguration],
            edit(2)
        ),
        Err(CatalogError::NotDraftAuthor)
    ));
}

#[test]
fn only_one_draft_per_specimen_type() {
    let c = Catalog::open();
    c.draft_validity(c.specimen_type).unwrap();
    assert!(matches!(
        c.draft_validity(c.specimen_type),
        Err(CatalogError::DraftExists)
    ));
    assert!(matches!(
        c.draft_validity(lis_catalog::SpecimenTypeId::generate()),
        Err(CatalogError::NotFound("specimen_type"))
    ));
}

#[test]
fn approved_policies_cannot_be_changed_or_fabricated_with_sql() {
    let c = Catalog::open();
    let approved = c.draft_validity(c.specimen_type).unwrap();
    c.approve_validity(approved, 1).unwrap();
    let conn = c.raw();
    for sql in [
        "UPDATE cat_validity_categories SET action = 'none', qualifier_text = NULL \
         WHERE policy_version_id = ?1",
        "DELETE FROM cat_validity_categories WHERE policy_version_id = ?1",
        "INSERT INTO cat_validity_categories (policy_version_id, code, label, action) \
         VALUES (?1, 'SYN-NEW', 'New', 'hold')",
        "UPDATE cat_validity_policy_versions SET status = 'draft', approved_by = NULL, \
         effective_from = NULL WHERE id = ?1",
        "DELETE FROM cat_validity_policy_versions WHERE id = ?1",
    ] {
        assert!(conn.execute(sql, [approved]).is_err(), "{sql}");
    }

    let other = c
        .as_author(|ctx| {
            lis_catalog::create_specimen_type(ctx, crate::coded("SYN-OF", "Synthetic oral fluid"))
        })
        .unwrap();
    let draft = c.draft_validity(other).unwrap();
    let fabricated = conn.execute(
        "UPDATE cat_validity_policy_versions SET status = 'approved', revision = 2, \
         approved_by = ?2, effective_from = '2026-01-01T00:00:00Z' WHERE id = ?1",
        rusqlite::params![draft, c.approver],
    );
    let message = fabricated.unwrap_err().to_string();
    assert!(message.contains("authorized audit event"), "{message}");
    let qualifier_without_text = conn.execute(
        "INSERT INTO cat_validity_categories (policy_version_id, code, label, action) \
         VALUES (?1, 'SYN-Q', 'Q', 'qualifier')",
        [draft],
    );
    assert!(qualifier_without_text.is_err());
}

#[test]
fn component_purpose_is_stored_and_frozen_with_the_manifest() {
    let c = Catalog::open();
    let drugs = c.approved_method("SYN-M1");
    let validity = c.approved_method("SYN-M2");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    let content = OrderableVersionContent {
        components: vec![
            ComponentInput {
                method_version_id: drugs,
                purpose: ComponentPurpose::Analytical,
                analyte_ids: vec![c.analyte_a],
            },
            ComponentInput {
                method_version_id: validity,
                purpose: ComponentPurpose::Validity,
                analyte_ids: vec![c.analyte_b],
            },
        ],
    };
    let id = c.draft_orderable(panel, content).unwrap();
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
    let offered = c.read(list_active_orderables);
    let purposes: Vec<_> = offered[0]
        .components
        .iter()
        .map(|component| (component.position, component.purpose))
        .collect();
    assert_eq!(
        purposes,
        [
            (1, ComponentPurpose::Analytical),
            (2, ComponentPurpose::Validity)
        ]
    );
    let refused = c.raw().execute(
        "UPDATE cat_orderable_components SET purpose = 'analytical' \
         WHERE orderable_version_id = ?1",
        [id],
    );
    assert!(refused.is_err(), "an approved purpose is immutable");
    let frozen = c.read(move |conn| get_orderable_version(conn, id));
    assert_eq!(frozen.components[1].purpose, ComponentPurpose::Validity);
}

#[test]
fn component_purpose_is_required_input() {
    let json = serde_json::json!({
        "methodVersionId": lis_catalog::MethodVersionId::generate(),
        "analyteIds": [],
    });
    assert!(serde_json::from_value::<ComponentInput>(json).is_err());
}
