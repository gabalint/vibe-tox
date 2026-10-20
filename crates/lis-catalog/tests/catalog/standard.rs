//! Load Standard Catalog (REQ-OPS-014, proposed) on a small synthetic file:
//! item states, the dependency closure, refusals, idempotent reload, the load
//! record and its audit trail. No value here is a real cutoff.

use lis_catalog::{
    CatalogError, LoadStandardCatalog, StandardCatalogLoaded, StandardFile, StandardItemState,
    StandardOmittedAnalyte, create_analyte, create_method, create_orderable, create_specimen_type,
    get_method_version, get_orderable_version, list_method_versions, list_methods,
    list_orderable_versions, list_orderables, load_standard_catalog, standard_catalog_status,
    standard_catalog_tree,
};
use lis_catalog::{CreateAnalyte, CreateOrderable, OrderableKind, VersionStatus};
use lis_core::{Capability, ErrorCode};

use crate::{Catalog, coded};

/// A synthetic file: one specimen type, a screen, a confirmation and a
/// validity method, one test and one panel.
pub(crate) fn fixture_file() -> StandardFile {
    let method = |code: &str, certainty: &str, scale: &str, analytes: serde_json::Value| {
        serde_json::json!({
            "code": code, "name": format!("Synthetic method {code}"), "specimenType": "SY",
            "certainty": certainty, "outputScale": scale, "thresholdRule": "at_or_above",
            "thresholdRounding": "before_rounding", "analytes": analytes, "sources": ["SRC-MENU"]
        })
    };
    let analyte = |code: &str, class: Option<&str>| {
        serde_json::json!({
            "code": code, "name": format!("Synthetic analyte {code}"), "drugClass": class,
            "loinc": { "code": "00000-0", "displayName": format!("Synthetic display {code}") },
            "sources": ["SRC-MENU"]
        })
    };
    let published = |value: &str| serde_json::json!({ "value": value, "source": "SRC-PUB", "label": "Synthetic published cutoff" });
    let source = |id: &str, title: &str| {
        serde_json::json!({
            "id": id, "title": title, "publisher": "Synthetic publisher",
            "url": "https://example.com/synthetic", "published": null,
            "checked": "2026-10-13", "note": ""
        })
    };
    let file = serde_json::json!({
        "format": "vibe-tox/standard-catalog", "formatVersion": 1,
        "catalogVersion": "0.1.0-synthetic", "reviewStatus": "unreviewed",
        "disclaimer": "Synthetic starter content.",
        "loinc": { "version": "0", "notice": "Synthetic notice" },
        "sources": [source("SRC-PUB", "Synthetic published source"), source("SRC-MENU", "Synthetic menu source")],
        "drugClasses": [{ "code": "SYN-CLASS", "name": "Synthetic class" }],
        "specimenTypes": [{ "code": "SY", "name": "Synthetic matrix", "sources": ["SRC-MENU"] }],
        "analytes": [
            analyte("SY-SCR", Some("SYN-CLASS")), analyte("SY-A1", Some("SYN-CLASS")),
            analyte("SY-A2", Some("SYN-CLASS")), analyte("SY-VAL", None)
        ],
        "methods": [
            method("SY-SCREEN", "presumptive", "qualitative", serde_json::json!([
                { "analyte": "SY-SCR", "unit": "ng/mL", "cutoff": published("300") }
            ])),
            method("SY-CONF", "definitive", "quantitative", serde_json::json!([
                { "analyte": "SY-A1", "unit": "ng/mL", "cutoff": published("100.0") },
                { "analyte": "SY-A2", "unit": "ng/mL", "cutoff": null }
            ])),
            method("SY-VALID", "definitive", "quantitative", serde_json::json!([
                { "analyte": "SY-VAL", "unit": "mg/dL", "cutoff": null }
            ]))
        ],
        "orderables": [
            { "code": "SY-T-CONF", "name": "Synthetic confirmation test", "kind": "test",
              "components": [{ "method": "SY-CONF", "purpose": "analytical", "analytes": ["SY-A1", "SY-A2"] }],
              "sources": ["SRC-MENU"] },
            { "code": "SY-P-SCREEN", "name": "Synthetic screen panel", "kind": "panel",
              "components": [
                  { "method": "SY-SCREEN", "purpose": "analytical", "analytes": ["SY-SCR"] },
                  { "method": "SY-VALID", "purpose": "validity", "analytes": ["SY-VAL"] }
              ],
              "sources": ["SRC-MENU"] }
        ]
    });
    StandardFile::parse(&file.to_string()).unwrap()
}

pub(crate) fn selection(orderables: &[&str], methods: &[&str]) -> LoadStandardCatalog {
    LoadStandardCatalog {
        catalog_version: "0.1.0-synthetic".to_owned(),
        orderable_codes: orderables.iter().map(|c| (*c).to_owned()).collect(),
        method_codes: methods.iter().map(|c| (*c).to_owned()).collect(),
        omitted_analytes: Vec::new(),
        disclaimer_acknowledged: true,
    }
}

pub(crate) fn load(
    c: &Catalog,
    input: LoadStandardCatalog,
) -> Result<StandardCatalogLoaded, CatalogError> {
    let file = fixture_file();
    c.as_author(move |ctx| load_standard_catalog(ctx, &file, input))
}

fn states(c: &Catalog) -> Vec<(String, StandardItemState)> {
    let file = fixture_file();
    let tree = c.read(move |conn| standard_catalog_tree(conn, &file));
    let mut states = Vec::new();
    states.extend(
        tree.specimen_types
            .into_iter()
            .map(|i| (i.code, i.status.state)),
    );
    states.extend(tree.analytes.into_iter().map(|i| (i.code, i.status.state)));
    states.extend(tree.methods.into_iter().map(|i| (i.code, i.status.state)));
    states.extend(
        tree.orderables
            .into_iter()
            .map(|i| (i.code, i.status.state)),
    );
    states
}

fn state_of(states: &[(String, StandardItemState)], code: &str) -> StandardItemState {
    states.iter().find(|(c, _)| c == code).unwrap().1
}

#[test]
fn the_tree_lists_every_item_with_its_dependencies_and_cutoff_sources() {
    let c = Catalog::open();
    let file = fixture_file();
    let tree = c.read(move |conn| standard_catalog_tree(conn, &file));
    assert_eq!(tree.info.catalog_version, "0.1.0-synthetic");
    assert!(states(&c).iter().all(|(_, s)| *s == StandardItemState::New));
    let panel = tree
        .orderables
        .iter()
        .find(|o| o.code == "SY-P-SCREEN")
        .unwrap();
    assert_eq!(panel.method_codes, ["SY-SCREEN", "SY-VALID"]);
    let conf = tree.methods.iter().find(|m| m.code == "SY-CONF").unwrap();
    assert_eq!(conf.specimen_type_code, "SY");
    let a1 = &conf.analytes[0];
    assert_eq!(a1.cutoff.as_ref().unwrap().as_str(), "100.0");
    assert_eq!(
        a1.cutoff_source_title.as_deref(),
        Some("Synthetic published source")
    );
    assert!(conf.analytes[1].cutoff.is_none());
    let validity = tree.analytes.iter().find(|a| a.code == "SY-VAL").unwrap();
    assert_eq!(validity.drug_class_code, None);
    assert_eq!(
        validity.loinc_display_name.as_deref(),
        Some("Synthetic display SY-VAL")
    );
}

#[test]
fn item_states_follow_the_laboratory_catalog() {
    let c = Catalog::open();
    c.as_author(|ctx| create_specimen_type(ctx, coded("SY", "Synthetic matrix")))
        .unwrap();
    c.as_author(|ctx| {
        create_analyte(
            ctx,
            CreateAnalyte {
                code: "SY-A1".to_owned(),
                name: "Synthetic analyte SY-A1".to_owned(),
                loinc_code: Some("11111-1".to_owned()),
                drug_class_code: Some("SYN-CLASS".to_owned()),
            },
        )
    })
    .unwrap();
    c.as_author(|ctx| create_method(ctx, coded("SY-SCREEN", "Synthetic method SY-SCREEN")))
        .unwrap();
    c.as_author(|ctx| {
        create_orderable(
            ctx,
            CreateOrderable {
                code: "SY-T-CONF".to_owned(),
                name: "Another synthetic test".to_owned(),
                kind: OrderableKind::Test,
            },
        )
    })
    .unwrap();
    let states = states(&c);
    assert_eq!(state_of(&states, "SY"), StandardItemState::AlreadyPresent);
    assert_eq!(state_of(&states, "SY-A1"), StandardItemState::CodeInUse);
    assert_eq!(state_of(&states, "SY-A2"), StandardItemState::New);
    assert_eq!(
        state_of(&states, "SY-SCREEN"),
        StandardItemState::AlreadyPresent
    );
    assert_eq!(state_of(&states, "SY-T-CONF"), StandardItemState::CodeInUse);
    assert_eq!(state_of(&states, "SY-P-SCREEN"), StandardItemState::Blocked);

    let file = fixture_file();
    let tree = c.read(move |conn| standard_catalog_tree(conn, &file));
    let analyte = tree.analytes.iter().find(|a| a.code == "SY-A1").unwrap();
    assert!(
        analyte
            .status
            .reason
            .as_deref()
            .unwrap()
            .contains("LOINC code")
    );
    let panel = tree
        .orderables
        .iter()
        .find(|o| o.code == "SY-P-SCREEN")
        .unwrap();
    assert!(
        panel
            .status
            .reason
            .as_deref()
            .unwrap()
            .contains("add this test by hand")
    );
}

#[test]
fn a_test_loads_with_its_closure_as_drafts_matching_the_file() {
    let c = Catalog::open();
    let loaded = load(&c, selection(&["SY-T-CONF"], &[])).unwrap();
    assert!(loaded.load_id.is_some());
    assert_eq!(
        (
            loaded.specimen_types,
            loaded.analytes,
            loaded.methods,
            loaded.orderables
        ),
        (1, 2, 1, 1),
        "only SY, SY-A1, SY-A2, SY-CONF and the test"
    );
    let methods = c.read(list_methods);
    let conf = methods.iter().find(|m| m.code == "SY-CONF").unwrap().id;
    assert!(!methods.iter().any(|m| m.code == "SY-SCREEN"));
    let history = c.read(move |conn| list_method_versions(conn, conf));
    assert_eq!(history.len(), 1);
    let draft = c.read(move |conn| get_method_version(conn, history[0].id));
    assert_eq!(draft.lifecycle.status, VersionStatus::Draft);
    assert_eq!(draft.analytes.len(), 2);
    let a1 = &draft.analytes[0].limits;
    assert_eq!(a1.cutoff.as_ref().unwrap().as_str(), "100.0");
    assert_eq!(a1.unit.as_deref(), Some("ng/mL"));
    assert!(a1.limit_of_detection.is_none() && a1.reportable_high.is_none());
    assert!(a1.display_scale.is_none() && draft.analytes[1].limits.cutoff.is_none());

    let test = c.read(list_orderables)[0].id;
    let versions = c.read(move |conn| list_orderable_versions(conn, test));
    let manifest = c.read(move |conn| get_orderable_version(conn, versions[0].id));
    assert_eq!(manifest.lifecycle.status, VersionStatus::Draft);
    assert_eq!(manifest.components[0].method_version_id, draft.id);
    assert_eq!(manifest.components[0].analytes.len(), 2);
}

#[test]
fn the_load_is_recorded_and_audited_without_catalog_codes() {
    let c = Catalog::open();
    let before = c.audit_actions().len();
    let loaded = load(&c, selection(&["SY-T-CONF"], &[])).unwrap();
    let actions = c.audit_actions().split_off(before);
    assert_eq!(
        actions,
        [
            "catalog.create_specimen_type",
            "catalog.create_analyte",
            "catalog.create_analyte",
            "catalog.create_method",
            "catalog.draft_method_version",
            "catalog.create_orderable",
            "catalog.draft_orderable_version",
            "catalog.load_standard_catalog",
        ]
    );
    let digest = fixture_file().info().file_digest;
    let (record_id, details): (String, String) = c.read(|conn| {
        Ok(conn.query_row(
            "SELECT record_id, details FROM audit_events \
             WHERE action = 'catalog.load_standard_catalog'",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?)
    });
    assert_eq!(record_id, loaded.load_id.unwrap().to_string());
    assert!(details.contains("0.1.0-synthetic") && details.contains(&digest));
    assert!(
        !details.contains("SY-"),
        "catalog codes stay out of audit details"
    );
    let status = c.read(standard_catalog_status);
    assert_eq!(status.loads.len(), 1);
    assert_eq!(status.loads[0].file_digest, digest);
    assert_eq!(status.loads[0].records, 7);
    assert_eq!(status.drafts_remaining, 2);
}

#[test]
fn an_omitted_analyte_leaves_its_method_and_components() {
    let c = Catalog::open();
    let mut input = selection(&["SY-T-CONF"], &[]);
    input.omitted_analytes.push(StandardOmittedAnalyte {
        method_code: "SY-CONF".to_owned(),
        analyte_code: "SY-A2".to_owned(),
    });
    let loaded = load(&c, input).unwrap();
    assert_eq!(loaded.analytes, 1);
    assert!(
        !c.read(lis_catalog::list_analytes)
            .iter()
            .any(|a| a.code == "SY-A2")
    );
    let test = c.read(list_orderables)[0].id;
    let versions = c.read(move |conn| list_orderable_versions(conn, test));
    let manifest = c.read(move |conn| get_orderable_version(conn, versions[0].id));
    let codes: Vec<_> = manifest.components[0]
        .analytes
        .iter()
        .map(|a| a.code.clone())
        .collect();
    assert_eq!(codes, ["SY-A1"]);
}

#[test]
fn reloading_the_same_selection_creates_nothing() {
    let c = Catalog::open();
    load(&c, selection(&["SY-T-CONF", "SY-P-SCREEN"], &[])).unwrap();
    let before = c.counts();
    let again = load(&c, selection(&["SY-T-CONF", "SY-P-SCREEN"], &["SY-CONF"])).unwrap();
    assert_eq!(again.load_id, None);
    assert_eq!(
        (
            again.specimen_types,
            again.analytes,
            again.methods,
            again.orderables
        ),
        (0, 0, 0, 0)
    );
    assert_eq!(again.skipped, 3, "two orderables and the chosen method");
    assert_eq!(c.counts(), before);
}

#[test]
fn a_load_is_refused_whole_and_writes_nothing() {
    let c = Catalog::open();
    c.as_author(|ctx| create_method(ctx, coded("SY-VALID", "Another synthetic method")))
        .unwrap();
    let before = c.counts();
    let omit_all = {
        let mut input = selection(&[], &["SY-CONF"]);
        for analyte in ["SY-A1", "SY-A2"] {
            input.omitted_analytes.push(StandardOmittedAnalyte {
                method_code: "SY-CONF".to_owned(),
                analyte_code: analyte.to_owned(),
            });
        }
        input
    };
    let unacknowledged = LoadStandardCatalog {
        disclaimer_acknowledged: false,
        ..selection(&["SY-T-CONF"], &[])
    };
    let other_version = LoadStandardCatalog {
        catalog_version: "0.0.1".to_owned(),
        ..selection(&["SY-T-CONF"], &[])
    };
    let cases: Vec<(LoadStandardCatalog, ErrorCode)> = vec![
        (unacknowledged, ErrorCode::Validation),
        (other_version, ErrorCode::Conflict),
        (selection(&["SY-NONE"], &[]), ErrorCode::Validation),
        (omit_all, ErrorCode::Validation),
        (
            selection(&["SY-T-CONF", "SY-P-SCREEN"], &[]),
            ErrorCode::Conflict,
        ),
        (
            selection(&[], &["SY-CONF", "SY-VALID"]),
            ErrorCode::Conflict,
        ),
    ];
    for (input, code) in cases {
        let error = load(&c, input).unwrap_err();
        assert_eq!(error.code(), code, "{error:?}");
        assert_eq!(c.counts(), before);
    }
    match load(&c, selection(&["SY-T-CONF", "SY-P-SCREEN"], &[])).unwrap_err() {
        CatalogError::StandardItemsUnavailable { codes } => assert_eq!(codes, ["SY-P-SCREEN"]),
        other => panic!("{other:?}"),
    }
    match load(&c, selection(&[], &["SY-VALID"])).unwrap_err() {
        CatalogError::StandardItemsUnavailable { codes } => assert_eq!(codes, ["SY-VALID"]),
        other => panic!("{other:?}"),
    }
}

#[test]
fn loading_needs_manage_configuration() {
    let c = Catalog::open();
    let before = c.counts();
    let file = fixture_file();
    let input = selection(&["SY-T-CONF"], &[]);
    let error = c
        .as_user(
            c.approver,
            &[Capability::ApproveConfiguration],
            move |ctx| load_standard_catalog(ctx, &file, input),
        )
        .unwrap_err();
    assert_eq!(error.code(), ErrorCode::Forbidden);
    assert_eq!(c.counts(), before);
}
