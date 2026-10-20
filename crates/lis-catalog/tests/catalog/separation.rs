//! Self-approval under the laboratory's separation-of-duties policy (ADR 0015;
//! REQ-ANA-009, REQ-OPS-015): refused as before while the policy needs a second
//! person, allowed only with the author's confirmation, audited as a
//! self-approval, and refused by the schema guards too.

use lis_catalog::{
    ApproveMethodVersion, ApproveOrderableVersion, ApproveReviewedCatalog,
    ApproveValidityPolicyVersion, CatalogBlankCutoff, CatalogError, CatalogPreviewId,
    CatalogReviewApproved, DemoValues, DraftMethodVersion, MethodVersionId, OrderableKind,
    OrderableVersionId, ValidityPolicyVersionId, approve_method_version, approve_orderable_version,
    approve_reviewed_catalog, approve_validity_policy_version, draft_method_version,
};
use lis_core::{Capability, ErrorCode, UserId};
use lis_store::testing::TEST_START;
use rusqlite::{Connection, OptionalExtension};

use crate::Catalog;
use crate::review::{blank_cutoffs, loaded, record, snapshot};

/// Both configuration capabilities, as a one-person laboratory's account holds them.
pub(crate) const BOTH: [Capability; 2] = [
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
];

/// Records a policy version that lets the author approve configuration, as a
/// quality manager would (`lis_auth::set_separation_policy`).
pub(crate) fn allow_self_approval(conn: &Connection) {
    conn.execute(
        "INSERT INTO auth_separation_policies (version, self_approve_configuration, \
         self_release_reports, self_verify_destinations, reason, set_by, set_at) \
         VALUES (1, 1, 0, 0, 'Synthetic one-person laboratory', 'synthetic-quality-manager', ?1)",
        [TEST_START],
    )
    .unwrap();
}

/// The `self_approval` detail of the `action` event of `record_id`: `Some(1)` when marked.
pub(crate) fn self_approval_mark(conn: &Connection, action: &str, record_id: &str) -> Option<i64> {
    conn.query_row(
        "SELECT json_extract(details, '$.self_approval') FROM audit_events \
         WHERE action = ?1 AND record_id = ?2",
        [action, record_id],
        |row| row.get(0),
    )
    .optional()
    .unwrap()
    .flatten()
}

/// A draft of one version kind, written by the fixture's author.
#[derive(Clone, Copy, Debug)]
enum Draft {
    Method(MethodVersionId),
    Orderable(OrderableVersionId),
    Validity(ValidityPolicyVersionId),
}

impl Draft {
    fn table(self) -> &'static str {
        match self {
            Self::Method(_) => "cat_method_versions",
            Self::Orderable(_) => "cat_orderable_versions",
            Self::Validity(_) => "cat_validity_policy_versions",
        }
    }

    fn action(self) -> &'static str {
        match self {
            Self::Method(_) => "catalog.approve_method_version",
            Self::Orderable(_) => "catalog.approve_orderable_version",
            Self::Validity(_) => "catalog.approve_validity_policy_version",
        }
    }

    fn id(self) -> String {
        match self {
            Self::Method(id) => id.to_string(),
            Self::Orderable(id) => id.to_string(),
            Self::Validity(id) => id.to_string(),
        }
    }

    /// Approves revision 1 as `user`, holding both configuration capabilities.
    fn approve(self, c: &Catalog, user: UserId, self_approval: bool) -> Result<i64, CatalogError> {
        c.as_user(user, &BOTH, move |ctx| match self {
            Self::Method(id) => approve_method_version(
                ctx,
                ApproveMethodVersion {
                    external_approval_reference: None,
                    method_version_id: id,
                    expected_revision: 1,
                    self_approval,
                },
            )
            .map(|approved| approved.revision),
            Self::Orderable(id) => approve_orderable_version(
                ctx,
                ApproveOrderableVersion {
                    orderable_version_id: id,
                    expected_revision: 1,
                    self_approval,
                },
            )
            .map(|approved| approved.revision),
            Self::Validity(id) => approve_validity_policy_version(
                ctx,
                ApproveValidityPolicyVersion {
                    policy_version_id: id,
                    expected_revision: 1,
                    self_approval,
                },
            )
            .map(|approved| approved.revision),
        })
    }

    /// Whether the stored approver is the author.
    fn approved_by_author(self, c: &Catalog) -> bool {
        let sql = format!(
            "SELECT approved_by = created_by FROM {} WHERE id = ?1",
            self.table()
        );
        c.raw()
            .query_row(&sql, [self.id()], |row| row.get(0))
            .unwrap()
    }
}

/// One draft of each kind by the author; the test uses a method the approver approved.
fn drafts(c: &Catalog) -> [Draft; 3] {
    let approved = c.approved_method("SYN-M0");
    let test = c.orderable("SYN-T1", OrderableKind::Test);
    [
        Draft::Method(c.draft_method(c.method("SYN-M1")).unwrap()),
        Draft::Orderable(c.draft_orderable(test, c.manifest(&[approved])).unwrap()),
        Draft::Validity(c.draft_validity(c.specimen_type).unwrap()),
    ]
}

fn assert_unconfirmed(error: &CatalogError) {
    assert_eq!(error.code(), ErrorCode::Validation, "{error:?}");
    let CatalogError::Invalid(field) = error else {
        panic!("{error:?}");
    };
    assert_eq!(
        (field.field.as_str(), field.code.as_str()),
        ("selfApproval", "self_approval_unconfirmed")
    );
}

#[test]
fn the_default_policy_refuses_an_author_approval_of_every_kind_even_when_confirmed() {
    let c = Catalog::open();
    for draft in drafts(&c) {
        let before = c.counts();
        let error = draft.approve(&c, c.author, true).unwrap_err();
        assert!(matches!(error, CatalogError::SelfApproval), "{error:?}");
        assert_eq!(error.code(), ErrorCode::GateBlocked);
        let message = error.to_string();
        assert!(message.contains("separation-of-duties policy"), "{message}");
        assert!(message.contains("quality manager"), "{message}");
        assert_eq!(c.counts(), before, "{draft:?}");
    }
}

#[test]
fn an_allowed_self_approval_still_needs_the_authors_confirmation() {
    let c = Catalog::open();
    let drafts = drafts(&c);
    allow_self_approval(&c.raw());
    for draft in drafts {
        let before = c.counts();
        let error = draft.approve(&c, c.author, false).unwrap_err();
        assert_unconfirmed(&error);
        assert_eq!(c.counts(), before, "{draft:?}");
    }
}

#[test]
fn a_confirmed_self_approval_is_approved_and_audited_as_one() {
    let c = Catalog::open();
    let drafts = drafts(&c);
    allow_self_approval(&c.raw());
    for draft in drafts {
        assert_eq!(draft.approve(&c, c.author, true).unwrap(), 2);
        assert!(draft.approved_by_author(&c), "{draft:?}");
        let mark = self_approval_mark(&c.raw(), draft.action(), &draft.id());
        assert_eq!(mark, Some(1), "{draft:?}");
    }
}

#[test]
fn another_approver_is_unaffected_by_the_policy_and_not_marked() {
    for allowed in [false, true] {
        let c = Catalog::open();
        let drafts = drafts(&c);
        if allowed {
            allow_self_approval(&c.raw());
        }
        for draft in drafts {
            assert_eq!(draft.approve(&c, c.approver, false).unwrap(), 2);
            assert!(!draft.approved_by_author(&c));
            let mark = self_approval_mark(&c.raw(), draft.action(), &draft.id());
            assert_eq!(mark, None, "{draft:?}");
        }
    }
}

#[test]
fn the_schema_refuses_an_author_approval_while_the_policy_needs_a_second_person() {
    let c = Catalog::open();
    let drafts = drafts(&c);
    let conn = c.raw();
    let set_author = |draft: Draft| {
        let sql = format!(
            "UPDATE {} SET approved_by = created_by WHERE id = ?1",
            draft.table()
        );
        conn.execute(&sql, [draft.id()]).unwrap_err().to_string()
    };
    for draft in drafts {
        let message = set_author(draft);
        assert!(message.contains("separation of duties"), "{message}");
    }
    // Under a policy that allows it the guard lets the change through, and the
    // lifecycle constraints refuse the half-made approval instead.
    allow_self_approval(&conn);
    for draft in drafts {
        let message = set_author(draft);
        assert!(!message.contains("separation of duties"), "{message}");
        assert!(message.contains("CHECK constraint failed"), "{message}");
    }
}

fn review(
    c: &Catalog,
    preview_id: CatalogPreviewId,
    confirmed_blank_cutoffs: Vec<CatalogBlankCutoff>,
    self_approval: bool,
) -> Result<CatalogReviewApproved, CatalogError> {
    let input = ApproveReviewedCatalog {
        external_approval_reference: None,
        preview_id,
        confirmed_blank_cutoffs,
        self_approval,
    };
    c.as_user(c.approver, &BOTH, move |ctx| {
        approve_reviewed_catalog(ctx, input)
    })
}

#[test]
fn all_data_reviewed_by_an_author_of_listed_drafts_follows_the_policy() {
    let c = loaded();
    let method_id = c.method("SYN-OWN");
    let content = c.content();
    let own = c
        .as_user(c.approver, &BOTH, move |ctx| {
            draft_method_version(ctx, DraftMethodVersion { method_id, content })
        })
        .unwrap()
        .id;
    let s = snapshot(&c, DemoValues::new());
    let preview = record(&c, c.author, &[Capability::ManageConfiguration], &s).unwrap();
    let blank = blank_cutoffs(&s);
    let before = c.counts();

    let error = review(&c, preview, blank.clone(), true).unwrap_err();
    assert!(
        matches!(&error, CatalogError::ReviewerAuthoredDrafts { records }
            if records == &["method SYN-OWN v1"]),
        "{error:?}"
    );
    assert_eq!(error.code(), ErrorCode::GateBlocked);
    assert!(error.to_string().contains("quality manager"));
    assert_eq!(c.counts(), before);

    allow_self_approval(&c.raw());
    assert_unconfirmed(&review(&c, preview, blank.clone(), false).unwrap_err());
    assert_eq!(c.counts(), before);

    let approved = review(&c, preview, blank, true).unwrap();
    let conn = c.raw();
    let review_id = approved.review_id.to_string();
    let outer = self_approval_mark(&conn, "catalog.approve_reviewed_catalog", &review_id);
    assert_eq!(outer, Some(1));
    let inner = self_approval_mark(&conn, "catalog.approve_method_version", &own.to_string());
    assert_eq!(inner, Some(1));
    // Only the reviewer's own draft is marked among the per-version approvals.
    let marked: i64 = conn
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action LIKE 'catalog.approve_%_version' \
             AND json_extract(details, '$.self_approval') IS NOT NULL",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(marked, 1);
    let approvals: i64 = conn
        .query_row(
            "SELECT count(*) FROM audit_events WHERE action LIKE 'catalog.approve_%_version'",
            [],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(approvals, i64::try_from(s.drafts.len()).unwrap());
}

#[test]
fn self_approval_is_a_required_input_field() {
    let input = serde_json::json!({ "methodVersionId": MethodVersionId::generate(), "expectedRevision": 1 });
    assert!(serde_json::from_value::<ApproveMethodVersion>(input.clone()).is_err());
    let mut confirmed = input;
    confirmed["selfApproval"] = serde_json::json!(true);
    let parsed = serde_json::from_value::<ApproveMethodVersion>(confirmed).unwrap();
    assert!(parsed.self_approval);
}
