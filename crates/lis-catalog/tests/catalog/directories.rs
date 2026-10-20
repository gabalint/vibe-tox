//! Version state on the directory rows (REQ-ANA-002, REQ-SPC-008): the
//! number of the version in effect and of the draft awaiting approval. Each
//! test walks one record through its lifecycle next to a record that has no
//! versions; expectations follow the lifecycle rules.

use lis_catalog::{
    ApproveOrderableVersion, OrderableKind, RetireMethodVersion, RetireOrderableVersion,
    RetireValidityPolicyVersion, approve_orderable_version, create_specimen_type, list_methods,
    list_orderables, list_specimen_types, retire_method_version, retire_orderable_version,
    retire_validity_policy_version,
};

use crate::{Catalog, coded};

/// `(version in effect, draft version)` per row, in directory order.
type States = Vec<(String, Option<i64>, Option<i64>)>;

fn methods(c: &Catalog) -> States {
    c.read(list_methods)
        .into_iter()
        .map(|row| (row.code, row.version_in_effect, row.draft_version))
        .collect()
}

fn orderables(c: &Catalog) -> States {
    c.read(list_orderables)
        .into_iter()
        .map(|row| (row.code, row.version_in_effect, row.draft_version))
        .collect()
}

fn specimen_types(c: &Catalog) -> States {
    c.read(list_specimen_types)
        .into_iter()
        .map(|row| (row.code, row.version_in_effect, row.draft_version))
        .collect()
}

fn row(
    code: &str,
    in_effect: Option<i64>,
    draft: Option<i64>,
) -> (String, Option<i64>, Option<i64>) {
    (code.to_owned(), in_effect, draft)
}

#[test]
fn method_rows_show_the_version_in_effect_and_the_open_draft() {
    let c = Catalog::open();
    let method = c.method("SYN-M1");
    c.method("SYN-M2");
    let idle = row("SYN-M2", None, None);
    assert_eq!(methods(&c), [row("SYN-M1", None, None), idle.clone()]);

    let v1 = c.draft_method(method).unwrap();
    assert_eq!(methods(&c), [row("SYN-M1", None, Some(1)), idle.clone()]);

    c.approve_method(v1, 1).unwrap();
    assert_eq!(methods(&c), [row("SYN-M1", Some(1), None), idle.clone()]);

    let v2 = c.draft_method(method).unwrap();
    assert_eq!(methods(&c), [row("SYN-M1", Some(1), Some(2)), idle.clone()]);

    c.approve_method(v2, 1).unwrap();
    assert_eq!(methods(&c), [row("SYN-M1", Some(2), None), idle.clone()]);

    c.as_approver(move |ctx| {
        retire_method_version(
            ctx,
            RetireMethodVersion {
                method_version_id: v2,
                expected_revision: 2,
                reason: "Synthetic withdrawal".to_owned(),
            },
        )
    })
    .unwrap();
    // v1 was replaced and v2 retired: nothing is in effect.
    assert_eq!(methods(&c), [row("SYN-M1", None, None), idle]);
}

#[test]
fn orderable_rows_show_the_version_in_effect_and_the_open_draft() {
    let c = Catalog::open();
    let method_version = c.approved_method("SYN-M1");
    let panel = c.orderable("SYN-P1", OrderableKind::Panel);
    c.orderable("SYN-P2", OrderableKind::Test);
    let idle = row("SYN-P2", None, None);
    assert_eq!(orderables(&c), [row("SYN-P1", None, None), idle.clone()]);
    let approve = |id, expected_revision| {
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
        .unwrap()
    };

    let v1 = c
        .draft_orderable(panel, c.manifest(&[method_version]))
        .unwrap();
    assert_eq!(orderables(&c), [row("SYN-P1", None, Some(1)), idle.clone()]);

    approve(v1, 1);
    assert_eq!(orderables(&c), [row("SYN-P1", Some(1), None), idle.clone()]);

    let v2 = c
        .draft_orderable(panel, c.manifest(&[method_version]))
        .unwrap();
    assert_eq!(
        orderables(&c),
        [row("SYN-P1", Some(1), Some(2)), idle.clone()]
    );

    approve(v2, 1);
    assert_eq!(orderables(&c), [row("SYN-P1", Some(2), None), idle.clone()]);

    c.as_approver(move |ctx| {
        retire_orderable_version(
            ctx,
            RetireOrderableVersion {
                orderable_version_id: v2,
                expected_revision: 2,
                reason: "Synthetic withdrawal".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(orderables(&c), [row("SYN-P1", None, None), idle]);
}

#[test]
fn specimen_type_rows_show_the_validity_policy_in_effect_and_the_open_draft() {
    let c = Catalog::open();
    c.as_author(|ctx| create_specimen_type(ctx, coded("SYN-XX", "Synthetic other")))
        .unwrap();
    let urine = c.specimen_type;
    let idle = row("SYN-XX", None, None);
    assert_eq!(
        specimen_types(&c),
        [row("SYN-UR", None, None), idle.clone()]
    );

    let v1 = c.draft_validity(urine).unwrap();
    assert_eq!(
        specimen_types(&c),
        [row("SYN-UR", None, Some(1)), idle.clone()]
    );

    c.approve_validity(v1, 1).unwrap();
    assert_eq!(
        specimen_types(&c),
        [row("SYN-UR", Some(1), None), idle.clone()]
    );

    let v2 = c.draft_validity(urine).unwrap();
    assert_eq!(
        specimen_types(&c),
        [row("SYN-UR", Some(1), Some(2)), idle.clone()]
    );

    c.approve_validity(v2, 1).unwrap();
    assert_eq!(
        specimen_types(&c),
        [row("SYN-UR", Some(2), None), idle.clone()]
    );

    c.as_approver(move |ctx| {
        retire_validity_policy_version(
            ctx,
            RetireValidityPolicyVersion {
                policy_version_id: v2,
                expected_revision: 2,
                reason: "Synthetic withdrawal".to_owned(),
            },
        )
    })
    .unwrap();
    assert_eq!(specimen_types(&c), [row("SYN-UR", None, None), idle]);
}
