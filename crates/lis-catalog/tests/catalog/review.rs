//! Catalog Preview and All Data Reviewed (REQ-OPS-015, proposed;
//! REQ-ANA-009): value sources, digest stability, the recorded draft list and
//! every refusal of the review, including the two-person rule.

use lis_catalog::{
    ApproveReviewedCatalog, CatalogBlankCutoff, CatalogDraft, CatalogError, CatalogPreviewId,
    CatalogPreviewSnapshot, CatalogReviewApproved, CatalogValueSourceKind, ComponentInput,
    DemoValues, DraftMethodVersion, DraftOrderableVersion, MethodVersionContent, MethodVersionId,
    OrderableVersionContent, RecordCatalogPreview, UpdateMethodDraft, VersionStatus,
    approve_reviewed_catalog, catalog_preview_snapshot, draft_method_version,
    draft_orderable_version, get_method_version, get_orderable_version, list_active_orderables,
    list_method_versions, list_methods, list_orderable_versions, list_orderables,
    record_catalog_preview, standard_catalog_status, update_method_draft,
};
use lis_core::{Capability, DecimalText, ErrorCode, UserId, UtcTimestamp};

use crate::standard::{fixture_file, load, selection};
use crate::{Catalog, dec};

/// A catalog with the whole synthetic file loaded by the author, plus a
/// validity policy draft.
pub(crate) fn loaded() -> Catalog {
    let c = Catalog::open();
    load(&c, selection(&["SY-T-CONF", "SY-P-SCREEN"], &[])).unwrap();
    c.draft_validity(c.specimen_type).unwrap();
    c
}

pub(crate) fn snapshot(c: &Catalog, demo_values: DemoValues) -> CatalogPreviewSnapshot {
    let file = fixture_file();
    c.read(move |conn| catalog_preview_snapshot(conn, &file, &demo_values))
}

pub(crate) fn record(
    c: &Catalog,
    user: UserId,
    capabilities: &[Capability],
    snapshot: &CatalogPreviewSnapshot,
) -> Result<CatalogPreviewId, CatalogError> {
    let input = RecordCatalogPreview {
        preview_id: CatalogPreviewId::generate(),
        content_digest: snapshot.content_digest.clone(),
        artifact_digest: "a".repeat(64),
        drafts: snapshot.drafts.clone(),
    };
    c.as_user(user, capabilities, move |ctx| {
        record_catalog_preview(ctx, input)
    })
}

pub(crate) fn blank_cutoffs(snapshot: &CatalogPreviewSnapshot) -> Vec<CatalogBlankCutoff> {
    let versions = snapshot.methods.iter().flat_map(|m| &m.versions);
    let versions = versions.filter(|v| v.lifecycle.status == VersionStatus::Draft);
    versions
        .flat_map(|v| v.analytes.iter().map(move |a| (v.id, a)))
        .filter(|(_, a)| a.cutoff_source.kind == CatalogValueSourceKind::Blank)
        .map(|(method_version_id, a)| CatalogBlankCutoff {
            method_version_id,
            analyte_id: a.analyte.limits.analyte_id,
        })
        .collect()
}

fn review(
    c: &Catalog,
    user: UserId,
    preview_id: CatalogPreviewId,
    confirmed_blank_cutoffs: Vec<CatalogBlankCutoff>,
) -> Result<CatalogReviewApproved, CatalogError> {
    let input = ApproveReviewedCatalog {
        external_approval_reference: None,
        preview_id,
        confirmed_blank_cutoffs,
        self_approval: false,
    };
    c.as_user(user, &[Capability::ApproveConfiguration], move |ctx| {
        approve_reviewed_catalog(ctx, input)
    })
}

/// The author edits the open draft of the method with `code`.
fn edit_method(c: &Catalog, code: &str, change: impl FnOnce(&mut MethodVersionContent)) {
    let method = c
        .read(list_methods)
        .into_iter()
        .find(|m| m.code == code)
        .unwrap();
    let history = c.read(move |conn| list_method_versions(conn, method.id));
    let draft = history
        .iter()
        .find(|v| v.lifecycle.status == VersionStatus::Draft)
        .unwrap();
    let (id, expected_revision) = (draft.id, draft.lifecycle.revision);
    let view = c.read(move |conn| get_method_version(conn, id));
    let mut content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: view.specimen_type_id,
        certainty: view.certainty,
        output_scale: view.output_scale,
        threshold_rule: view.threshold_rule,
        threshold_rounding: view.threshold_rounding,
        analytes: view.analytes.into_iter().map(|a| a.limits).collect(),
    };
    change(&mut content);
    c.as_author(move |ctx| {
        update_method_draft(
            ctx,
            UpdateMethodDraft {
                method_version_id: id,
                expected_revision,
                content,
            },
        )
    })
    .unwrap();
}

fn cutoff_kind(s: &CatalogPreviewSnapshot, method: &str, analyte: &str) -> CatalogValueSourceKind {
    let method = s.methods.iter().find(|m| m.method.code == method).unwrap();
    let version = method.versions.last().unwrap();
    let entry = version
        .analytes
        .iter()
        .find(|a| a.analyte.analyte_code == analyte);
    entry.unwrap().cutoff_source.kind
}

/// The loaded catalog with a laboratory method, edited values and one
/// matching demo value.
fn labeled() -> CatalogPreviewSnapshot {
    let c = loaded();
    c.approved_method("SYN-M1");
    edit_method(&c, "SY-SCREEN", |content| {
        content.analytes[0].cutoff = dec("250");
        content.threshold_rounding = lis_catalog::ThresholdRounding::AfterRounding;
    });
    edit_method(&c, "SY-CONF", |content| {
        content.analytes[1].cutoff = dec("5")
    });
    let mut demo = DemoValues::new();
    demo.insert(
        ("SY-CONF".to_owned(), "SY-A2".to_owned()),
        DecimalText::parse("5").unwrap(),
    );
    snapshot(&c, demo)
}

#[test]
fn every_cutoff_and_threshold_is_labeled_with_its_source() {
    let s = labeled();
    let conf = s
        .methods
        .iter()
        .find(|m| m.method.code == "SY-CONF")
        .unwrap();
    let a1 = &conf.versions[0].analytes[0].cutoff_source;
    assert_eq!(a1.kind, CatalogValueSourceKind::Published);
    assert_eq!(
        a1.source_title.as_deref(),
        Some("Synthetic published source")
    );
    assert_eq!(a1.label.as_deref(), Some("Synthetic published cutoff"));
    assert_eq!(
        cutoff_kind(&s, "SY-CONF", "SY-A2"),
        CatalogValueSourceKind::DemoValue
    );
    assert_eq!(
        cutoff_kind(&s, "SY-SCREEN", "SY-SCR"),
        CatalogValueSourceKind::ChangedByLaboratory
    );
    assert_eq!(
        cutoff_kind(&s, "SY-VALID", "SY-VAL"),
        CatalogValueSourceKind::Blank
    );
    assert_eq!(
        cutoff_kind(&s, "SYN-M1", "SYN-A"),
        CatalogValueSourceKind::LaboratoryEntered
    );
    let screen = s
        .methods
        .iter()
        .find(|m| m.method.code == "SY-SCREEN")
        .unwrap();
    let version = &screen.versions[0];
    assert_eq!(
        version.threshold_rule_source.kind,
        CatalogValueSourceKind::StarterValue
    );
    assert_eq!(
        version.threshold_rounding_source.kind,
        CatalogValueSourceKind::ChangedByLaboratory
    );
}

#[test]
fn gaps_list_blank_cutoffs_and_starter_values() {
    let s = labeled();
    let gaps: Vec<_> = s
        .gaps
        .iter()
        .map(|g| {
            (
                g.method_code.as_str(),
                g.field.as_str(),
                g.analyte_code.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        gaps,
        [
            ("SY-CONF", "threshold_rule", None),
            ("SY-CONF", "threshold_rounding", None),
            ("SY-SCREEN", "threshold_rule", None),
            ("SY-VALID", "threshold_rule", None),
            ("SY-VALID", "threshold_rounding", None),
            ("SY-VALID", "cutoff", Some("SY-VAL")),
        ]
    );
    let analyte = s
        .analytes
        .iter()
        .find(|a| a.analyte.code == "SY-A1")
        .unwrap();
    assert_eq!(
        analyte.loinc_display_name.as_deref(),
        Some("Synthetic display SY-A1")
    );
    let lab = s
        .analytes
        .iter()
        .find(|a| a.analyte.code == "SYN-A")
        .unwrap();
    assert_eq!(lab.loinc_display_name, None);
    assert_eq!(
        s.drafts.len(),
        6,
        "three methods, two orderables, one policy"
    );
}

#[test]
fn the_digest_covers_content_but_not_time_or_user() {
    let c = loaded();
    let first = snapshot(&c, DemoValues::new());
    assert_eq!(first.content_digest.len(), 64);
    c.t.clock
        .set(UtcTimestamp::parse("2026-03-04T05:06:07.000000000Z").unwrap());
    record(&c, c.author, &[Capability::ManageConfiguration], &first).unwrap();
    assert_eq!(snapshot(&c, DemoValues::new()), first);
    edit_method(&c, "SY-CONF", |content| content.analytes[1].unit = None);
    assert_ne!(
        snapshot(&c, DemoValues::new()).content_digest,
        first.content_digest
    );
}

#[test]
fn a_preview_must_list_exactly_the_open_drafts() {
    let c = loaded();
    let s = snapshot(&c, DemoValues::new());
    let manage = [Capability::ManageConfiguration];
    let before = c.counts();
    let mut missing = s.clone();
    missing.drafts.pop();
    let mut stale = s.clone();
    if let CatalogDraft::MethodVersion { revision, .. } = &mut stale.drafts[0] {
        *revision += 1;
    }
    let mut repeated = s.clone();
    repeated.drafts.push(s.drafts[0]);
    let mut bad_digest = s.clone();
    bad_digest.content_digest = "A".repeat(64);
    for case in [missing, stale] {
        let error = record(&c, c.author, &manage, &case).unwrap_err();
        assert!(matches!(&error, CatalogError::CatalogChanged { records } if records.len() == 1));
    }
    for case in [repeated, bad_digest] {
        let error = record(&c, c.author, &manage, &case).unwrap_err();
        assert_eq!(error.code(), ErrorCode::Validation, "{error:?}");
    }
    let error = record(&c, c.author, &[Capability::Accession], &s).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert_eq!(c.counts(), before);
    record(&c, c.approver, &[Capability::ApproveConfiguration], &s).unwrap();
    record(&c, c.author, &manage, &s).unwrap();
    assert!(c.audit_actions().ends_with(&[
        "catalog.record_catalog_preview".to_owned(),
        "catalog.record_catalog_preview".to_owned()
    ]));
}

#[test]
fn all_data_reviewed_approves_the_listed_drafts_in_order() {
    let c = loaded();
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    let before = c.audit_actions().len();
    let approved = review(&c, c.approver, preview, blank_cutoffs(&s)).unwrap();
    assert_eq!(
        (
            approved.validity_policies,
            approved.methods,
            approved.orderables,
            approved.confirmed_blank_cutoffs
        ),
        (1, 3, 2, 2)
    );
    let actions = c.audit_actions().split_off(before);
    assert_eq!(
        actions,
        [
            "catalog.approve_validity_policy_version",
            "catalog.approve_method_version",
            "catalog.approve_method_version",
            "catalog.approve_method_version",
            "catalog.approve_orderable_version",
            "catalog.approve_orderable_version",
            "catalog.approve_reviewed_catalog",
        ]
    );
    let after = snapshot(&c, DemoValues::new());
    assert!(after.drafts.is_empty());
    assert!(
        after
            .methods
            .iter()
            .all(|m| m.versions.iter().all(|v| v.lifecycle.is_current()))
    );
    assert_eq!(c.read(standard_catalog_status).drafts_remaining, 0);
    let rows: (i64, i64) = c.read(|conn| {
        Ok(conn.query_row(
            "SELECT (SELECT count(*) FROM cat_catalog_reviews), \
             (SELECT count(*) FROM cat_catalog_review_confirmations)",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    assert_eq!(rows, (1, 2));
}

#[test]
fn all_data_reviewed_is_refused_naming_the_records() {
    let c = loaded();
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    let blank = blank_cutoffs(&s);
    let before = c.counts();

    let error = review(&c, c.approver, CatalogPreviewId::generate(), blank.clone()).unwrap_err();
    assert!(matches!(error, CatalogError::NotFound("catalog_preview")));
    let error = review(&c, c.approver, preview, blank[..1].to_vec()).unwrap_err();
    assert!(
        matches!(&error, CatalogError::BlankCutoffsUnconfirmed { records }
            if records == &["method SY-VALID v1: SY-VAL not confirmed"]),
        "{error:?}"
    );
    let mut extra = blank.clone();
    extra.push(CatalogBlankCutoff {
        method_version_id: blank[0].method_version_id,
        analyte_id: c.analyte_a,
    });
    let error = review(&c, c.approver, preview, extra).unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let both = [
        Capability::ManageConfiguration,
        Capability::ApproveConfiguration,
    ];
    let author = c.author;
    let input = ApproveReviewedCatalog {
        external_approval_reference: None,
        preview_id: preview,
        confirmed_blank_cutoffs: blank.clone(),
        self_approval: false,
    };
    let error = c
        .as_user(author, &both, move |ctx| {
            approve_reviewed_catalog(ctx, input)
        })
        .unwrap_err();
    assert!(
        matches!(&error, CatalogError::ReviewerAuthoredDrafts { records } if records.len() == 6)
    );
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    let input = ApproveReviewedCatalog {
        external_approval_reference: None,
        preview_id: preview,
        confirmed_blank_cutoffs: blank.clone(),
        self_approval: false,
    };
    let error = c
        .as_user(c.approver, &[Capability::ManageConfiguration], move |ctx| {
            approve_reviewed_catalog(ctx, input)
        })
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert_eq!(c.counts(), before);

    review(&c, c.approver, preview, blank.clone()).unwrap();
    let error = review(&c, c.approver, preview, blank).unwrap_err();
    assert!(matches!(error, CatalogError::PreviewAlreadyReviewed));
}

#[test]
fn a_catalog_changed_after_the_report_blocks_the_whole_review() {
    let c = loaded();
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    edit_method(&c, "SY-CONF", |content| content.analytes[0].unit = None);
    let new_draft = c.draft_method(c.method("SYN-NEW")).unwrap();
    let before = c.counts();
    let error = review(&c, c.approver, preview, blank_cutoffs(&s)).unwrap_err();
    match &error {
        CatalogError::CatalogChanged { records } => {
            assert_eq!(records, &["method SY-CONF v1", "method SYN-NEW v1"]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(error.code(), ErrorCode::Conflict);
    assert_eq!(c.counts(), before);
    let status = c
        .read(move |conn| get_method_version(conn, new_draft))
        .lifecycle
        .status;
    assert_eq!(status, VersionStatus::Draft);
}

/// The loaded catalog after a first All Data Reviewed approved all of it.
fn reviewed() -> Catalog {
    let c = loaded();
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    review(&c, c.approver, preview, blank_cutoffs(&s)).unwrap();
    c
}

/// The author opens version 2 of the method with `code`, a copy of version 1.
fn redraft_method(c: &Catalog, code: &str) -> MethodVersionId {
    let method = c.read(list_methods).into_iter().find(|m| m.code == code);
    let method_id = method.unwrap().id;
    let history = c.read(move |conn| list_method_versions(conn, method_id));
    let current = history
        .iter()
        .find(|v| v.lifecycle.is_current())
        .unwrap()
        .id;
    let view = c.read(move |conn| get_method_version(conn, current));
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: view.specimen_type_id,
        certainty: view.certainty,
        output_scale: view.output_scale,
        threshold_rule: view.threshold_rule,
        threshold_rounding: view.threshold_rounding,
        analytes: view.analytes.into_iter().map(|a| a.limits).collect(),
    };
    let input = DraftMethodVersion { method_id, content };
    c.as_author(move |ctx| draft_method_version(ctx, input))
        .unwrap()
        .id
}

/// The author opens version 2 of the test with `code`, moved to `method`.
fn redraft_test(c: &Catalog, code: &str, method: MethodVersionId) {
    let orderable = c.read(list_orderables).into_iter().find(|o| o.code == code);
    let orderable_id = orderable.unwrap().id;
    let history = c.read(move |conn| list_orderable_versions(conn, orderable_id));
    let current = history
        .iter()
        .find(|v| v.lifecycle.is_current())
        .unwrap()
        .id;
    let view = c.read(move |conn| get_orderable_version(conn, current));
    let components = view
        .components
        .into_iter()
        .map(|component| ComponentInput {
            method_version_id: method,
            purpose: component.purpose,
            analyte_ids: component.analytes.iter().map(|a| a.analyte_id).collect(),
        })
        .collect();
    let input = DraftOrderableVersion {
        orderable_id,
        content: OrderableVersionContent { components },
    };
    c.as_author(move |ctx| draft_orderable_version(ctx, input))
        .unwrap();
}

#[test]
fn a_second_review_moves_a_current_test_to_its_new_method_version() {
    let c = reviewed();
    let method_v2 = redraft_method(&c, "SY-CONF");
    redraft_test(&c, "SY-T-CONF", method_v2);
    let s = snapshot(&c, DemoValues::new());
    assert_eq!(s.drafts.len(), 2);
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    let approved = review(&c, c.approver, preview, blank_cutoffs(&s)).unwrap();
    assert_eq!((approved.methods, approved.orderables), (1, 1));
    let active = c.read(list_active_orderables);
    let test = active.iter().find(|o| o.code == "SY-T-CONF").unwrap();
    assert_eq!(test.version, 2);
    assert_eq!(test.components[0].method_version_id, method_v2);
    assert!(
        c.read(move |conn| get_method_version(conn, method_v2))
            .lifecycle
            .is_current()
    );
    let actions = c.audit_actions();
    for action in [
        "catalog.supersede_method_version",
        "catalog.supersede_orderable_version",
    ] {
        assert!(actions.iter().any(|a| a == action), "{action}");
    }
}

#[test]
fn a_current_test_left_on_the_replaced_method_blocks_the_whole_review() {
    let c = reviewed();
    let method_v2 = redraft_method(&c, "SY-CONF");
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    let before = c.counts();
    let error = review(&c, c.approver, preview, blank_cutoffs(&s)).unwrap_err();
    match &error {
        CatalogError::OrderablesOnReplacedMethods { records } => {
            assert_eq!(records, &["SY-T-CONF v1"]);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert_eq!(c.counts(), before, "the whole review rolled back");
    let status = c
        .read(move |conn| get_method_version(conn, method_v2))
        .lifecycle
        .status;
    assert_eq!(status, VersionStatus::Draft);
}
