//! Standard Catalog and catalog review through the engine (REQ-OPS-014,
//! REQ-OPS-015, both proposed; REQ-ANA-009): load the shipped file as
//! drafts, generate, list, show and export a Catalog Preview, and approve exactly
//! its drafts by a second person after fresh authentication. Expected counts
//! come from the shipped file, so the tests hold for any content version.

use lis_auth::RoleCode;
use lis_catalog::{
    CatalogBlankCutoff, CatalogPreviewSummary, MethodVersionContent, StandardFile,
    StandardItemState, VersionStatus,
};
use lis_core::{CommandError, ErrorCode, UserId};
use lis_engine::{
    ApproveReviewedCatalogInput, CatalogPreviewId, CatalogPreviewPdfInput, Engine,
    GetMethodVersionInput, ListMethodVersionsInput, LoadStandardCatalog, StandardCatalogTree,
    UpdateMethodDraft,
};

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of, secret};

/// Calls the starter-catalog command `name` with plausible input; `None` for other areas.
pub(crate) async fn starter_catalog_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let preview_id = CatalogPreviewId::generate();
    Some(match name {
        "standard_catalog_tree" => engine.standard_catalog_tree().await.map(drop),
        "load_standard_catalog" => engine
            .load_standard_catalog(selection("1.0.0", &[], &[]))
            .await
            .map(drop),
        "standard_catalog_status" => engine.standard_catalog_status().await.map(drop),
        "generate_catalog_preview" => engine.generate_catalog_preview().await.map(drop),
        "list_catalog_previews" => engine.list_catalog_previews().await.map(drop),
        "view_catalog_preview_pdf" => engine
            .view_catalog_preview_pdf(CatalogPreviewPdfInput { preview_id })
            .await
            .map(drop),
        "export_catalog_preview_pdf" => engine
            .export_catalog_preview_pdf(
                CatalogPreviewPdfInput { preview_id },
                Some(std::env::temp_dir()),
            )
            .await
            .map(drop),
        "approve_reviewed_catalog" => engine
            .approve_reviewed_catalog(approval(preview_id, Vec::new(), PASSWORD))
            .await
            .map(drop),
        _ => return None,
    })
}

pub(crate) fn selection(
    version: &str,
    orderables: &[String],
    methods: &[String],
) -> LoadStandardCatalog {
    LoadStandardCatalog {
        catalog_version: version.to_owned(),
        orderable_codes: orderables.to_vec(),
        method_codes: methods.to_vec(),
        omitted_analytes: Vec::new(),
        disclaimer_acknowledged: true,
    }
}

pub(crate) fn approval(
    preview_id: CatalogPreviewId,
    confirmed_blank_cutoffs: Vec<CatalogBlankCutoff>,
    password: &str,
) -> ApproveReviewedCatalogInput {
    ApproveReviewedCatalogInput {
        external_approval_reference: None,
        preview_id,
        confirmed_blank_cutoffs,
        password: secret(password).into(),
        self_approval: false,
    }
}

/// Every blank cutoff a listed preview asks the reviewer to confirm.
pub(crate) fn confirmations(preview: &CatalogPreviewSummary) -> Vec<CatalogBlankCutoff> {
    preview
        .blank_cutoffs
        .iter()
        .map(|b| CatalogBlankCutoff {
            method_version_id: b.method_version_id,
            analyte_id: b.analyte_id,
        })
        .collect()
}

pub(crate) async fn preview(engine: &Engine, id: CatalogPreviewId) -> CatalogPreviewSummary {
    let previews = engine.list_catalog_previews().await.unwrap();
    previews.into_iter().find(|p| p.id == id).unwrap()
}

/// The administrator creates `username` with `roles`, who then signs in for
/// the first time; returns their user ID and TOTP secret.
async fn user(
    t: &TestEngine,
    admin_secret: &str,
    username: &str,
    roles: &[RoleCode],
) -> (UserId, String) {
    t.sign_in_with(ADMIN, PASSWORD, admin_secret).await;
    let user = t.create_user(username, roles).await;
    let (_, secret) = t.first_sign_in(username).await;
    (user.user_id, secret)
}

// REQ-OPS-015: the preview commands are registered as `Session` because the
// registry has no either-of access, so the table-driven test skips them; this
// is their row: `manage_configuration` or `approve_configuration` is allowed,
// anyone else is refused with only the denial event written. The banner
// status is open to every unlocked session.
#[tokio::test]
async fn preview_commands_need_manage_or_approve_configuration() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    let (_, approver) = user(&t, &admin, "cfg2", &[RoleCode::Administrator]).await;
    let (_, quality_only) = user(&t, &admin, "qm-only", &[RoleCode::QualityManager]).await;
    user(&t, &admin, "noroles", &[]).await;
    let either_of = [
        "generate_catalog_preview",
        "list_catalog_previews",
        "view_catalog_preview_pdf",
        "export_catalog_preview_pdf",
    ];
    for name in either_of {
        let before = t.fingerprint();
        let refused = starter_catalog_call(&t.engine, name, UserId::generate()).await;
        assert_eq!(code_of(refused.unwrap()), ErrorCode::Forbidden, "{name}");
        let after = t.fingerprint();
        let added: Vec<&String> = after.iter().filter(|row| !before.contains(row)).collect();
        assert_eq!(added.len(), 1, "{name}: {added:?}");
        let (action, outcome) = t.events("security").pop().unwrap();
        assert_eq!(outcome, "denied", "{name}");
        assert_eq!(action, format!("catalog.{name}"));
    }
    t.engine.standard_catalog_status().await.unwrap();
    t.sign_in_with("qm-only", NEW_PASSWORD, &quality_only).await;
    for name in either_of {
        let refused = starter_catalog_call(&t.engine, name, UserId::generate()).await;
        assert_eq!(code_of(refused.unwrap()), ErrorCode::Forbidden, "{name}");
    }
    for (username, password, secret) in [
        (ADMIN, PASSWORD, admin.as_str()),
        ("cfg2", NEW_PASSWORD, approver.as_str()),
    ] {
        t.sign_in_with(username, password, secret).await;
        for name in either_of {
            let result = starter_catalog_call(&t.engine, name, UserId::generate()).await;
            // Allowed; the export's synthetic preview ID does not exist.
            let code = result.unwrap().err().map(|error| error.code());
            assert!(matches!(code, None | Some(ErrorCode::NotFound)), "{name}");
        }
    }
}

/// Loads every item of the shipped file; returns the tree it was chosen from.
async fn load_everything(engine: &Engine) -> StandardCatalogTree {
    let tree = engine.standard_catalog_tree().await.unwrap();
    assert_eq!(tree.info, StandardFile::embedded().unwrap().info());
    let orderables: Vec<String> = tree.orderables.iter().map(|o| o.code.clone()).collect();
    let methods: Vec<String> = tree.methods.iter().map(|m| m.code.clone()).collect();
    let input = selection(&tree.info.catalog_version, &orderables, &methods);
    let loaded = engine.load_standard_catalog(input).await.unwrap();
    assert!(loaded.load_id.is_some());
    assert_eq!(
        (
            loaded.specimen_types,
            loaded.analytes,
            loaded.methods,
            loaded.orderables,
            loaded.skipped
        ),
        (
            tree.specimen_types.len(),
            tree.analytes.len(),
            tree.methods.len(),
            tree.orderables.len(),
            0
        )
    );
    // REQ-OPS-014: loading the same selection again creates nothing.
    let input = selection(&tree.info.catalog_version, &orderables, &methods);
    let again = engine.load_standard_catalog(input).await.unwrap();
    assert_eq!(
        (again.load_id, again.methods, again.orderables),
        (None, 0, 0)
    );
    tree
}

#[tokio::test]
async fn standard_catalog_is_reviewed_and_approved_by_a_second_person() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    // The loader also holds `approve_configuration`, to show the two-person rule.
    let (lead, lead_secret) = user(&t, &admin, "lead1", &[RoleCode::Administrator]).await;
    let (approver, approver_secret) = user(&t, &admin, "cfg2", &[RoleCode::Administrator]).await;
    t.sign_in_with("lead1", NEW_PASSWORD, &lead_secret).await;
    let tree = load_everything(&t.engine).await;
    let drafts = i64::try_from(tree.methods.len() + tree.orderables.len()).unwrap();
    let blank = tree
        .methods
        .iter()
        .flat_map(|m| &m.analytes)
        .filter(|a| a.cutoff.is_none())
        .count();
    let tree = t.engine.standard_catalog_tree().await.unwrap();
    assert!(
        tree.methods
            .iter()
            .all(|m| m.status.state == StandardItemState::AlreadyPresent)
    );
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!((status.loads.len(), status.drafts_remaining), (1, drafts));

    let id = t.engine.generate_catalog_preview().await.unwrap();
    let listed = preview(&t.engine, id).await;
    assert_eq!((listed.drafts, listed.generated_by), (drafts, lead));
    assert_eq!(listed.generated_by_display.as_deref(), Some("Test lead1"));
    assert_eq!(
        (listed.reviewed_at, listed.blank_cutoffs.len()),
        (None, blank)
    );
    assert_exported(&t, id).await;

    // The loader authored every draft, so cannot review them.
    let own = approval(id, confirmations(&listed), NEW_PASSWORD);
    let refused = t.engine.approve_reviewed_catalog(own).await;
    assert_eq!(code_of(refused), ErrorCode::GateBlocked);

    t.sign_in_with("cfg2", NEW_PASSWORD, &approver_secret).await;
    let wrong = approval(id, confirmations(&listed), "not the passphrase");
    let refused = t.engine.approve_reviewed_catalog(wrong).await.unwrap_err();
    assert_eq!(refused.code(), ErrorCode::Validation);
    let fields = serde_json::to_value(&refused).unwrap()["fieldErrors"].to_string();
    assert!(fields.contains("password"), "{fields}");
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!(status.drafts_remaining, drafts);

    let approved = t
        .engine
        .approve_reviewed_catalog(approval(id, confirmations(&listed), NEW_PASSWORD))
        .await
        .unwrap();
    assert_eq!(
        (
            approved.methods,
            approved.orderables,
            approved.validity_policies
        ),
        (tree.methods.len(), tree.orderables.len(), 0)
    );
    assert_eq!(approved.confirmed_blank_cutoffs, blank);
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!(status.drafts_remaining, 0);
    let reviewed = preview(&t.engine, id).await;
    assert_eq!(reviewed.reviewed_by, Some(approver));
    assert!(reviewed.reviewed_at.is_some() && reviewed.blank_cutoffs.is_empty());

    t.advance(1);
    let second = t.engine.generate_catalog_preview().await.unwrap();
    assert_eq!(preview(&t.engine, second).await.drafts, 0);
    assert_review_audited(&t);
}

/// The stored PDF is written to the folder as a new file, byte for byte,
/// and shown as the same bytes, the showing audited as a read.
async fn assert_exported(t: &TestEngine, id: CatalogPreviewId) {
    let folder = tempfile::tempdir().unwrap();
    let input = CatalogPreviewPdfInput { preview_id: id };
    let exported = t
        .engine
        .export_catalog_preview_pdf(input, Some(folder.path().to_owned()))
        .await
        .unwrap();
    assert_eq!(exported.file_name, format!("catalog-preview-{id}.pdf"));
    let bytes = std::fs::read(folder.path().join(&exported.file_name)).unwrap();
    assert!(bytes.starts_with(b"%PDF"));
    let again = t
        .engine
        .export_catalog_preview_pdf(input, Some(folder.path().to_owned()))
        .await
        .unwrap();
    assert_eq!(again.file_name, format!("catalog-preview-{id}-1.pdf"));
    assert_eq!(
        std::fs::read(folder.path().join(&again.file_name)).unwrap(),
        bytes
    );
    let shown = t.engine.view_catalog_preview_pdf(input).await.unwrap();
    assert_eq!(shown, bytes);
    assert_eq!(
        t.events("access").pop(),
        Some((
            "catalog.view_catalog_preview_pdf".to_owned(),
            "succeeded".to_owned()
        ))
    );
}

fn assert_review_audited(t: &TestEngine) {
    let clinical = t.events("clinical");
    for action in [
        "catalog.load_standard_catalog",
        "catalog.record_catalog_preview",
        "catalog.approve_reviewed_catalog",
    ] {
        assert!(
            clinical.contains(&(action.to_owned(), "succeeded".to_owned())),
            "{action}"
        );
    }
    let security = t.events("security");
    for (action, outcome) in [
        ("auth.reauthenticate", "failed"),
        ("auth.reauthenticate", "succeeded"),
        ("catalog.export_catalog_preview_pdf", "succeeded"),
    ] {
        assert!(
            security.contains(&(action.to_owned(), outcome.to_owned())),
            "{action} {outcome}"
        );
    }
}

// REQ-OPS-015: a draft changed after the report blocks the whole approval.
#[tokio::test]
async fn a_draft_changed_after_the_preview_blocks_all_data_reviewed() {
    let t = TestEngine::start().await;
    let admin = t.admin_session().await;
    let (_, approver) = user(&t, &admin, "cfg2", &[RoleCode::Administrator]).await;
    t.sign_in_with(ADMIN, PASSWORD, &admin).await;
    let tree = t.engine.standard_catalog_tree().await.unwrap();
    let method = tree.methods[0].code.clone();
    let input = selection(
        &tree.info.catalog_version,
        &[],
        std::slice::from_ref(&method),
    );
    t.engine.load_standard_catalog(input).await.unwrap();
    let id = t.engine.generate_catalog_preview().await.unwrap();
    let listed = preview(&t.engine, id).await;
    assert_eq!(listed.drafts, 1);

    // The author saves the draft again: any edit is a new revision.
    let methods = t.engine.list_methods().await.unwrap();
    let method_id = methods.iter().find(|m| m.code == method).unwrap().id;
    let versions = t
        .engine
        .list_method_versions(ListMethodVersionsInput { method_id })
        .await
        .unwrap();
    let draft = versions
        .iter()
        .find(|v| v.lifecycle.status == VersionStatus::Draft)
        .unwrap();
    let view = t
        .engine
        .get_method_version(GetMethodVersionInput {
            method_version_id: draft.id,
        })
        .await
        .unwrap();
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: view.specimen_type_id,
        certainty: view.certainty,
        output_scale: view.output_scale,
        threshold_rule: view.threshold_rule,
        threshold_rounding: view.threshold_rounding,
        analytes: view.analytes.iter().map(|a| a.limits.clone()).collect(),
    };
    let update = UpdateMethodDraft {
        method_version_id: draft.id,
        expected_revision: draft.lifecycle.revision,
        content,
    };
    t.engine.update_method_draft(update).await.unwrap();

    t.sign_in_with("cfg2", NEW_PASSWORD, &approver).await;
    let input = approval(id, confirmations(&listed), NEW_PASSWORD);
    let refused = t.engine.approve_reviewed_catalog(input).await;
    assert_eq!(code_of(refused), ErrorCode::Conflict);
    let status = t.engine.standard_catalog_status().await.unwrap();
    assert_eq!(status.drafts_remaining, 1);
}
