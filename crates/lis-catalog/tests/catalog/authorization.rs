//! Every command refuses a principal lacking its capability and writes
//! nothing; with only its capability it succeeds and audits.

use lis_catalog::{
    ApproveMethodVersion, ApproveOrderableVersion, ApproveValidityPolicyVersion, CatalogError,
    CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    DraftValidityPolicyVersion, MethodId, MethodVersionContent, MethodVersionId, OrderableId,
    OrderableKind, OrderableVersionContent, OrderableVersionId, RetireMethodVersion,
    RetireOrderableVersion, RetireValidityPolicyVersion, SpecimenTypeId, UpdateMethodDraft,
    UpdateOrderableDraft, UpdateValidityPolicyDraft, ValidityPolicyContent,
    ValidityPolicyVersionId, approve_method_version, approve_orderable_version,
    approve_validity_policy_version, create_analyte, create_method, create_orderable,
    create_specimen_type, draft_method_version, draft_orderable_version,
    draft_validity_policy_version, retire_method_version, retire_orderable_version,
    retire_validity_policy_version, update_method_draft, update_orderable_draft,
    update_validity_policy_draft,
};
use lis_core::{Capability, ErrorCode, Principal, UserId};
use lis_store::CommandContext;

use crate::{Catalog, coded, user_principal};

const ALL: [Capability; 14] = [
    Capability::Accession,
    Capability::EnterOrders,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
    Capability::ReleaseReports,
    Capability::ManageQuality,
    Capability::Billing,
    Capability::ViewAudit,
    Capability::Export,
    Capability::BackupRestore,
    Capability::ManageUsers,
    Capability::ManageConfiguration,
    Capability::ApproveConfiguration,
    Capability::InstrumentIntake,
];

/// Records in every lifecycle state a command needs.
#[derive(Clone)]
struct Fixture {
    content: MethodVersionContent,
    empty_method: MethodId,
    method_draft: MethodVersionId,
    unused_method_version: MethodVersionId,
    empty_orderable: OrderableId,
    manifest: OrderableVersionContent,
    orderable_draft: OrderableVersionId,
    orderable_current: OrderableVersionId,
    validity: ValidityPolicyContent,
    empty_specimen_type: SpecimenTypeId,
    validity_draft: ValidityPolicyVersionId,
    validity_current: ValidityPolicyVersionId,
}

fn fixture(c: &Catalog) -> Fixture {
    let used = c.approved_method("SYN-M1");
    let method = c.method("SYN-M2");
    let unused_method_version = c.draft_method(method).unwrap();
    c.approve_method(unused_method_version, 1).unwrap();
    let method_draft = c.draft_method(method).unwrap();
    let orderable = c.orderable("SYN-P1", OrderableKind::Panel);
    let manifest = c.manifest(&[used]);
    let orderable_current = c.draft_orderable(orderable, manifest.clone()).unwrap();
    c.as_approver(move |ctx| {
        approve_orderable_version(
            ctx,
            ApproveOrderableVersion {
                orderable_version_id: orderable_current,
                expected_revision: 1,
                self_approval: false,
            },
        )
    })
    .unwrap();
    let orderable_draft = c.draft_orderable(orderable, manifest.clone()).unwrap();
    let validity_current = c.draft_validity(c.specimen_type).unwrap();
    c.approve_validity(validity_current, 1).unwrap();
    let validity_draft = c.draft_validity(c.specimen_type).unwrap();
    let empty_specimen_type = c
        .as_author(|ctx| create_specimen_type(ctx, coded("SYN-SER", "Synthetic serum")))
        .unwrap();
    Fixture {
        content: c.content(),
        empty_method: c.method("SYN-M3"),
        method_draft,
        unused_method_version,
        empty_orderable: c.orderable("SYN-P2", OrderableKind::Test),
        manifest,
        orderable_draft,
        orderable_current,
        validity: c.validity_content(),
        empty_specimen_type,
        validity_draft,
        validity_current,
    }
}

/// Who runs the case when it holds only its capability.
#[derive(Clone, Copy)]
enum Who {
    Author,
    Approver,
}

type Case = fn(Fixture, &mut CommandContext<'_>) -> Result<(), CatalogError>;

const MANAGE: Capability = Capability::ManageConfiguration;
const APPROVE: Capability = Capability::ApproveConfiguration;

const CASES: [(&str, Capability, Who, Case); 16] = [
    ("create_specimen_type", MANAGE, Who::Author, |_, ctx| {
        create_specimen_type(ctx, coded("SYN-OF", "Synthetic oral fluid")).map(drop)
    }),
    ("create_analyte", MANAGE, Who::Author, |_, ctx| {
        create_analyte(
            ctx,
            CreateAnalyte {
                code: "SYN-C".to_owned(),
                name: "Synthetic analyte C".to_owned(),
                loinc_code: None,
                drug_class_code: None,
            },
        )
        .map(drop)
    }),
    ("create_method", MANAGE, Who::Author, |_, ctx| {
        create_method(ctx, coded("SYN-M9", "Synthetic method")).map(drop)
    }),
    ("create_orderable", MANAGE, Who::Author, |_, ctx| {
        create_orderable(
            ctx,
            CreateOrderable {
                code: "SYN-P9".to_owned(),
                name: "Synthetic test".to_owned(),
                kind: OrderableKind::Test,
            },
        )
        .map(drop)
    }),
    ("draft_method_version", MANAGE, Who::Author, |f, ctx| {
        draft_method_version(
            ctx,
            DraftMethodVersion {
                method_id: f.empty_method,
                content: f.content,
            },
        )
        .map(drop)
    }),
    ("update_method_draft", MANAGE, Who::Author, |f, ctx| {
        update_method_draft(
            ctx,
            UpdateMethodDraft {
                method_version_id: f.method_draft,
                expected_revision: 1,
                content: f.content,
            },
        )
        .map(drop)
    }),
    (
        "approve_method_version",
        APPROVE,
        Who::Approver,
        |f, ctx| {
            approve_method_version(
                ctx,
                ApproveMethodVersion {
                    external_approval_reference: None,
                    method_version_id: f.method_draft,
                    expected_revision: 1,
                    self_approval: false,
                },
            )
            .map(drop)
        },
    ),
    ("retire_method_version", APPROVE, Who::Approver, |f, ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: f.unused_method_version,
                expected_revision: 2,
                reason: "Replaced by a revalidated method".to_owned(),
            },
        )
        .map(drop)
    }),
    ("draft_orderable_version", MANAGE, Who::Author, |f, ctx| {
        draft_orderable_version(
            ctx,
            DraftOrderableVersion {
                orderable_id: f.empty_orderable,
                content: f.manifest,
            },
        )
        .map(drop)
    }),
    ("update_orderable_draft", MANAGE, Who::Author, |f, ctx| {
        update_orderable_draft(
            ctx,
            UpdateOrderableDraft {
                orderable_version_id: f.orderable_draft,
                expected_revision: 1,
                content: f.manifest,
            },
        )
        .map(drop)
    }),
    (
        "approve_orderable_version",
        APPROVE,
        Who::Approver,
        |f, ctx| {
            approve_orderable_version(
                ctx,
                ApproveOrderableVersion {
                    orderable_version_id: f.orderable_draft,
                    expected_revision: 1,
                    self_approval: false,
                },
            )
            .map(drop)
        },
    ),
    (
        "retire_orderable_version",
        APPROVE,
        Who::Approver,
        |f, ctx| {
            retire_orderable_version(
                ctx,
                RetireOrderableVersion {
                    orderable_version_id: f.orderable_current,
                    expected_revision: 2,
                    reason: "Panel withdrawn".to_owned(),
                },
            )
            .map(drop)
        },
    ),
    (
        "draft_validity_policy_version",
        MANAGE,
        Who::Author,
        |f, ctx| {
            draft_validity_policy_version(
                ctx,
                DraftValidityPolicyVersion {
                    specimen_type_id: f.empty_specimen_type,
                    content: f.validity,
                },
            )
            .map(drop)
        },
    ),
    (
        "update_validity_policy_draft",
        MANAGE,
        Who::Author,
        |f, ctx| {
            update_validity_policy_draft(
                ctx,
                UpdateValidityPolicyDraft {
                    policy_version_id: f.validity_draft,
                    expected_revision: 1,
                    content: f.validity,
                },
            )
            .map(drop)
        },
    ),
    (
        "approve_validity_policy_version",
        APPROVE,
        Who::Approver,
        |f, ctx| {
            approve_validity_policy_version(
                ctx,
                ApproveValidityPolicyVersion {
                    policy_version_id: f.validity_draft,
                    expected_revision: 1,
                    self_approval: false,
                },
            )
            .map(drop)
        },
    ),
    (
        "retire_validity_policy_version",
        APPROVE,
        Who::Approver,
        |f, ctx| {
            retire_validity_policy_version(
                ctx,
                RetireValidityPolicyVersion {
                    policy_version_id: f.validity_current,
                    expected_revision: 2,
                    reason: "Policy withdrawn".to_owned(),
                },
            )
            .map(drop)
        },
    ),
];

fn principal_for(c: &Catalog, who: Who, capabilities: &[Capability]) -> Principal {
    let user: UserId = match who {
        Who::Author => c.author,
        Who::Approver => c.approver,
    };
    user_principal(user, capabilities)
}

#[test]
fn every_command_refuses_without_its_capability_and_writes_nothing() {
    let c = Catalog::open();
    let f = fixture(&c);
    for (name, required, who, case) in CASES {
        let others: Vec<Capability> = ALL.into_iter().filter(|cap| *cap != required).collect();
        let before = c.counts();
        let input = f.clone();
        let result = c.run(principal_for(&c, who, &others), move |ctx| case(input, ctx));
        let error = result.unwrap_err();
        assert_eq!(error.code(), ErrorCode::Forbidden, "{name}: {error:?}");
        assert_eq!(c.counts(), before, "{name} wrote despite the refusal");
    }
}

#[test]
fn every_command_succeeds_with_only_its_capability_and_audits() {
    for (name, required, who, case) in CASES {
        // A fresh catalog per case, so every fixture revision is as listed.
        let c = Catalog::open();
        let f = fixture(&c);
        let before = c.audit_actions().len();
        c.run(principal_for(&c, who, &[required]), move |ctx| case(f, ctx))
            .unwrap_or_else(|e| panic!("{name}: {e:?}"));
        assert!(
            c.audit_actions().len() > before,
            "{name} wrote no audit event"
        );
    }
}

#[test]
fn configuration_needs_a_signed_in_user() {
    let c = Catalog::open();
    let before = c.counts();
    let system = Principal::system("importer", [Capability::ManageConfiguration]);
    let method_id = c.method("SYN-M1");
    let content = c.content();
    let result = c.run(system, move |ctx| {
        draft_method_version(ctx, DraftMethodVersion { method_id, content })
    });
    assert!(matches!(result, Err(CatalogError::UserRequired)));
    assert_eq!(c.counts()[4], before[4], "no method version written");
}
