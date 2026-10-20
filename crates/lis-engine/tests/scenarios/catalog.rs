//! Catalog IPC through the engine (REQ-ANA-002, REQ-ANA-009, REQ-SPC-008,
//! REQ-OPS-001): one administrator drafts, another administrator approves, and order
//! entry sees the approved orderable. Synthetic codes and limits only.

use lis_auth::RoleCode;
use lis_catalog::{
    ComponentInput, ComponentPurpose, MethodAnalyte, MethodCertainty, MethodVersionContent,
    OrderableKind, OrderableVersionContent, OutputScale, ThresholdRounding, ThresholdRule,
    ValidityAction, ValidityCategory, ValidityPolicyContent,
};
use lis_core::{CommandError, DecimalText, ErrorCode, UserId};
use lis_engine::{
    ApproveMethodVersion, ApproveOrderableVersion, ApproveValidityPolicyVersion, CodedName,
    CreateAnalyte, CreateOrderable, DraftMethodVersion, DraftOrderableVersion,
    DraftValidityPolicyVersion, Engine, GetMethodVersionInput, GetOrderableVersionInput,
    GetValidityPolicyInput, ListMethodVersionsInput, ListOrderableVersionsInput,
    ListValidityPolicyVersionsInput, RetireMethodVersion, RetireOrderableVersion,
    RetireValidityPolicyVersion, UpdateMethodDraft, UpdateOrderableDraft,
    UpdateValidityPolicyDraft,
};

use crate::fixture::{ADMIN, NEW_PASSWORD, PASSWORD, TestEngine, code_of};

fn coded(code: &str) -> CodedName {
    CodedName {
        code: code.to_owned(),
        name: format!("Synthetic {code}"),
    }
}

fn method_content(
    specimen_type_id: lis_catalog::SpecimenTypeId,
    analyte_id: lis_catalog::AnalyteId,
) -> MethodVersionContent {
    MethodVersionContent {
        report_notes: None,
        specimen_type_id,
        certainty: MethodCertainty::Definitive,
        output_scale: OutputScale::Quantitative,
        threshold_rule: ThresholdRule::AtOrAbove,
        threshold_rounding: ThresholdRounding::BeforeRounding,
        analytes: vec![MethodAnalyte {
            reference_interval: None,
            range_policy: lis_catalog::RangePolicy::ReportOnly,
            analyte_id,
            unit: Some("ng/mL".to_owned()),
            cutoff: Some(DecimalText::parse("111.10").unwrap()),
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: Some(2),
        }],
    }
}

fn validity_content() -> ValidityPolicyContent {
    ValidityPolicyContent {
        categories: vec![ValidityCategory {
            code: "SYN-DILUTE".to_owned(),
            label: "Synthetic dilute".to_owned(),
            action: ValidityAction::Qualifier,
            qualifier_text: Some("Synthetic qualifier".to_owned()),
        }],
    }
}

/// Calls the catalog command `name` with plausible input; `None` for other areas.
pub(crate) async fn catalog_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    if let Some(result) = reference_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = validity_call(engine, name).await {
        return Some(result);
    }
    if let Some(result) = method_call(engine, name).await {
        return Some(result);
    }
    orderable_call(engine, name).await
}

async fn reference_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    Some(match name {
        "list_specimen_types" => engine.list_specimen_types().await.map(drop),
        "create_specimen_type" => engine.create_specimen_type(coded("SYN-UR")).await.map(drop),
        "list_analytes" => engine.list_analytes().await.map(drop),
        "create_analyte" => engine
            .create_analyte(CreateAnalyte {
                code: "SYN-A".to_owned(),
                name: "Synthetic analyte".to_owned(),
                loinc_code: None,
                drug_class_code: None,
            })
            .await
            .map(drop),
        "list_methods" => engine.list_methods().await.map(drop),
        "create_method" => engine.create_method(coded("SYN-M")).await.map(drop),
        "list_orderables" => engine.list_orderables().await.map(drop),
        "create_orderable" => engine
            .create_orderable(CreateOrderable {
                code: "SYN-P".to_owned(),
                name: "Synthetic panel".to_owned(),
                kind: OrderableKind::Panel,
            })
            .await
            .map(drop),
        "list_active_orderables" => engine.list_active_orderables().await.map(drop),
        _ => return None,
    })
}

async fn validity_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let specimen_type_id = lis_catalog::SpecimenTypeId::generate();
    Some(match name {
        "list_validity_policy_versions" => engine
            .list_validity_policy_versions(ListValidityPolicyVersionsInput { specimen_type_id })
            .await
            .map(drop),
        "get_validity_policy" => engine
            .get_validity_policy(GetValidityPolicyInput {
                policy_version_id: lis_catalog::ValidityPolicyVersionId::generate(),
            })
            .await
            .map(drop),
        "draft_validity_policy_version" => engine
            .draft_validity_policy_version(DraftValidityPolicyVersion {
                specimen_type_id,
                content: validity_content(),
            })
            .await
            .map(drop),
        "update_validity_policy_draft" => engine
            .update_validity_policy_draft(UpdateValidityPolicyDraft {
                policy_version_id: lis_catalog::ValidityPolicyVersionId::generate(),
                expected_revision: 1,
                content: validity_content(),
            })
            .await
            .map(drop),
        "approve_validity_policy_version" => engine
            .approve_validity_policy_version(ApproveValidityPolicyVersion {
                policy_version_id: lis_catalog::ValidityPolicyVersionId::generate(),
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "retire_validity_policy_version" => engine
            .retire_validity_policy_version(RetireValidityPolicyVersion {
                policy_version_id: lis_catalog::ValidityPolicyVersionId::generate(),
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn method_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let method_version_id = lis_catalog::MethodVersionId::generate();
    let content = method_content(
        lis_catalog::SpecimenTypeId::generate(),
        lis_catalog::AnalyteId::generate(),
    );
    Some(match name {
        "list_method_versions" => engine
            .list_method_versions(ListMethodVersionsInput {
                method_id: lis_catalog::MethodId::generate(),
            })
            .await
            .map(drop),
        "get_method_version" => engine
            .get_method_version(GetMethodVersionInput { method_version_id })
            .await
            .map(drop),
        "draft_method_version" => engine
            .draft_method_version(DraftMethodVersion {
                method_id: lis_catalog::MethodId::generate(),
                content,
            })
            .await
            .map(drop),
        "update_method_draft" => engine
            .update_method_draft(UpdateMethodDraft {
                method_version_id,
                expected_revision: 1,
                content,
            })
            .await
            .map(drop),
        "approve_method_version" => engine
            .approve_method_version(ApproveMethodVersion {
                external_approval_reference: None,
                method_version_id,
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "retire_method_version" => engine
            .retire_method_version(RetireMethodVersion {
                method_version_id,
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

async fn orderable_call(engine: &Engine, name: &str) -> Option<Result<(), CommandError>> {
    let orderable_version_id = lis_catalog::OrderableVersionId::generate();
    let manifest = OrderableVersionContent { components: vec![] };
    Some(match name {
        "list_orderable_versions" => engine
            .list_orderable_versions(ListOrderableVersionsInput {
                orderable_id: lis_catalog::OrderableId::generate(),
            })
            .await
            .map(drop),
        "get_orderable_version" => engine
            .get_orderable_version(GetOrderableVersionInput {
                orderable_version_id,
            })
            .await
            .map(drop),
        "draft_orderable_version" => engine
            .draft_orderable_version(DraftOrderableVersion {
                orderable_id: lis_catalog::OrderableId::generate(),
                content: manifest,
            })
            .await
            .map(drop),
        "update_orderable_draft" => engine
            .update_orderable_draft(UpdateOrderableDraft {
                orderable_version_id,
                expected_revision: 1,
                content: manifest,
            })
            .await
            .map(drop),
        "approve_orderable_version" => engine
            .approve_orderable_version(ApproveOrderableVersion {
                orderable_version_id,
                expected_revision: 1,
                self_approval: false,
            })
            .await
            .map(drop),
        "retire_orderable_version" => engine
            .retire_orderable_version(RetireOrderableVersion {
                orderable_version_id,
                expected_revision: 2,
                reason: "Withdrawn".to_owned(),
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// The author administrator is signed in; returns two administrators' TOTP secrets.
async fn configurers(t: &TestEngine) -> (String, String) {
    let admin = t.admin_session().await;
    t.create_user("cfg2", &[RoleCode::Administrator]).await;
    let (_, approver) = t.first_sign_in("cfg2").await;
    t.sign_in_with(ADMIN, PASSWORD, &admin).await;
    (admin, approver)
}

/// Drafts written by the administrator.
struct Drafts {
    urine: lis_catalog::SpecimenTypeId,
    analyte: lis_catalog::AnalyteId,
    method_version: lis_catalog::MethodVersionId,
    policy_version: lis_catalog::ValidityPolicyVersionId,
}

/// Reference data, a method version draft and a validity policy draft.
async fn draft_reference_data(t: &TestEngine) -> Drafts {
    let urine = t
        .engine
        .create_specimen_type(coded("SYN-UR"))
        .await
        .unwrap();
    let analyte = t
        .engine
        .create_analyte(CreateAnalyte {
            code: "SYN-A".to_owned(),
            name: "Synthetic analyte".to_owned(),
            loinc_code: None,
            drug_class_code: Some("SYN-CLASS".to_owned()),
        })
        .await
        .unwrap();
    let method = t.engine.create_method(coded("SYN-M")).await.unwrap();
    let method_version = t
        .engine
        .draft_method_version(DraftMethodVersion {
            method_id: method,
            content: method_content(urine, analyte),
        })
        .await
        .unwrap()
        .id;
    let policy_version = t
        .engine
        .draft_validity_policy_version(DraftValidityPolicyVersion {
            specimen_type_id: urine,
            content: validity_content(),
        })
        .await
        .unwrap()
        .id;
    Drafts {
        urine,
        analyte,
        method_version,
        policy_version,
    }
}

/// A panel draft with one validity component on the approved method version.
async fn draft_panel(t: &TestEngine, drafts: &Drafts) -> lis_catalog::OrderableVersionId {
    let panel = t
        .engine
        .create_orderable(CreateOrderable {
            code: "SYN-P".to_owned(),
            name: "Synthetic panel".to_owned(),
            kind: OrderableKind::Panel,
        })
        .await
        .unwrap();
    t.engine
        .draft_orderable_version(DraftOrderableVersion {
            orderable_id: panel,
            content: OrderableVersionContent {
                components: vec![ComponentInput {
                    method_version_id: drafts.method_version,
                    purpose: ComponentPurpose::Validity,
                    analyte_ids: vec![drafts.analyte],
                }],
            },
        })
        .await
        .unwrap()
        .id
}

#[tokio::test]
async fn configuration_round_trip_from_draft_to_order_entry() {
    let t = TestEngine::start().await;
    let (admin, approver) = configurers(&t).await;
    let drafts = draft_reference_data(&t).await;
    let approve_method = ApproveMethodVersion {
        external_approval_reference: None,
        method_version_id: drafts.method_version,
        expected_revision: 1,
        self_approval: false,
    };
    // The author has approval capability but default separation of duties blocks self-approval.
    let own = t
        .engine
        .approve_method_version(approve_method.clone())
        .await;
    assert_eq!(code_of(own), ErrorCode::GateBlocked);

    t.sign_in_with("cfg2", NEW_PASSWORD, &approver).await;
    t.engine
        .approve_method_version(approve_method)
        .await
        .unwrap();
    t.engine
        .approve_validity_policy_version(ApproveValidityPolicyVersion {
            policy_version_id: drafts.policy_version,
            expected_revision: 1,
            self_approval: false,
        })
        .await
        .unwrap();
    t.sign_in_with(ADMIN, PASSWORD, &admin).await;
    let panel_version = draft_panel(&t, &drafts).await;
    assert!(t.engine.list_active_orderables().await.unwrap().is_empty());
    t.sign_in_with("cfg2", NEW_PASSWORD, &approver).await;
    t.engine
        .approve_orderable_version(ApproveOrderableVersion {
            orderable_version_id: panel_version,
            expected_revision: 1,
            self_approval: false,
        })
        .await
        .unwrap();

    let active = t.engine.list_active_orderables().await.unwrap();
    assert_eq!(active.len(), 1);
    assert_eq!(active[0].id, panel_version);
    assert_eq!(active[0].components[0].purpose, ComponentPurpose::Validity);
    assert_eq!(
        (
            active[0].lifecycle.created_by_display.as_deref(),
            active[0].lifecycle.approved_by_display.as_deref(),
        ),
        (Some("Test Administrator"), Some("Test cfg2"))
    );
    assert_eq!(
        active[0].components[0].analytes[0].analyte_id,
        drafts.analyte
    );
    assert_approved_configuration(&t, &drafts, active[0].orderable_id).await;
}

/// The approved versions read back exactly, and every step is audited.
async fn assert_approved_configuration(
    t: &TestEngine,
    drafts: &Drafts,
    orderable_id: lis_catalog::OrderableId,
) {
    let versions = t
        .engine
        .list_orderable_versions(ListOrderableVersionsInput { orderable_id })
        .await
        .unwrap();
    assert!(versions[0].lifecycle.approved_by.is_some());
    assert_eq!(
        (
            versions[0].lifecycle.created_by_display.as_deref(),
            versions[0].lifecycle.approved_by_display.as_deref(),
        ),
        (Some("Test Administrator"), Some("Test cfg2"))
    );
    let method = t
        .engine
        .get_method_version(GetMethodVersionInput {
            method_version_id: drafts.method_version,
        })
        .await
        .unwrap();
    let cutoff = method.analytes[0].limits.cutoff.as_ref().unwrap();
    assert_eq!(cutoff.as_str(), "111.10");
    assert_eq!(
        (
            method.lifecycle.created_by_display.as_deref(),
            method.lifecycle.approved_by_display.as_deref(),
        ),
        (Some("Test Administrator"), Some("Test cfg2"))
    );
    let policies = t
        .engine
        .list_validity_policy_versions(ListValidityPolicyVersionsInput {
            specimen_type_id: drafts.urine,
        })
        .await
        .unwrap();
    assert_eq!(policies.len(), 1);
    let policy = t
        .engine
        .get_validity_policy(GetValidityPolicyInput {
            policy_version_id: drafts.policy_version,
        })
        .await
        .unwrap();
    assert_eq!(policy.categories[0].action, ValidityAction::Qualifier);
    assert_eq!(
        (
            policy.lifecycle.created_by_display.as_deref(),
            policy.lifecycle.approved_by_display.as_deref(),
        ),
        (Some("Test Administrator"), Some("Test cfg2"))
    );
    let clinical: Vec<String> = t.events("clinical").into_iter().map(|e| e.0).collect();
    for action in [
        "catalog.draft_method_version",
        "catalog.approve_method_version",
        "catalog.draft_validity_policy_version",
        "catalog.approve_validity_policy_version",
        "catalog.approve_orderable_version",
    ] {
        assert!(clinical.iter().any(|a| a == action), "{action}");
    }
}

#[tokio::test]
async fn invalid_catalog_input_returns_field_errors() {
    let t = TestEngine::start().await;
    configurers(&t).await;
    let bad = t.engine.create_specimen_type(coded("bad code")).await;
    let error = bad.unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "code");
    let urine = t
        .engine
        .create_specimen_type(coded("SYN-UR"))
        .await
        .unwrap();
    let mut content = validity_content();
    content.categories[0].qualifier_text = None;
    let error = t
        .engine
        .draft_validity_policy_version(DraftValidityPolicyVersion {
            specimen_type_id: urine,
            content,
        })
        .await
        .unwrap_err();
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(
        json["fieldErrors"][0]["field"],
        "categories[0].qualifierText"
    );
    let missing = t
        .engine
        .get_method_version(GetMethodVersionInput {
            method_version_id: lis_catalog::MethodVersionId::generate(),
        })
        .await;
    assert_eq!(code_of(missing), ErrorCode::NotFound);
}
