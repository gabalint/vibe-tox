//! Integration tests on real SQLite files with the production pragmas.
//! Every code, name and number is synthetic; none is a clinical value.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod approval_references;
mod authorization;
mod directories;
mod lifecycle;
mod orderables;
mod review;
mod separation;
mod standard;
mod upgrade;
mod validity;
mod values;

use lis_catalog::{
    AnalyteId, ApproveMethodVersion, ApproveValidityPolicyVersion, CatalogError, CodedName,
    ComponentInput, ComponentPurpose, CreateAnalyte, CreateOrderable, DraftMethodVersion,
    DraftOrderableVersion, DraftValidityPolicyVersion, MethodAnalyte, MethodCertainty, MethodId,
    MethodVersionContent, MethodVersionId, OrderableId, OrderableKind, OrderableVersionContent,
    OrderableVersionId, OutputScale, SCHEMA, SpecimenTypeId, ThresholdRounding, ThresholdRule,
    ValidityAction, ValidityCategory, ValidityPolicyContent, ValidityPolicyVersionId,
    approve_method_version, approve_validity_policy_version, create_analyte, create_method,
    create_orderable, create_specimen_type, draft_method_version, draft_orderable_version,
    draft_validity_policy_version,
};
use lis_core::{
    Capability, CorrelationId, DecimalText, Principal, SessionId, UserId, WorkstationId,
};
use lis_store::testing::TestDatabase;
use lis_store::{CommandContext, lint_schema};
use rusqlite::Connection;

/// Every table this module and the audit trail write.
const TABLES: [&str; 19] = [
    "cat_approval_references",
    "cat_specimen_types",
    "cat_analytes",
    "cat_methods",
    "cat_orderables",
    "cat_method_versions",
    "cat_method_version_analytes",
    "cat_orderable_versions",
    "cat_orderable_components",
    "cat_orderable_component_analytes",
    "cat_validity_policy_versions",
    "cat_validity_categories",
    "audit_events",
    "cat_standard_loads",
    "cat_standard_load_items",
    "cat_catalog_previews",
    "cat_catalog_preview_drafts",
    "cat_catalog_reviews",
    "cat_catalog_review_confirmations",
];

/// Two people: the author drafts, the approver approves and retires.
pub(crate) struct Catalog {
    pub(crate) t: TestDatabase,
    pub(crate) author: UserId,
    pub(crate) approver: UserId,
    pub(crate) specimen_type: SpecimenTypeId,
    pub(crate) analyte_a: AnalyteId,
    pub(crate) analyte_b: AnalyteId,
}

impl Catalog {
    pub(crate) fn open() -> Self {
        Self::on(TestDatabase::open(&[lis_auth::SCHEMA, SCHEMA]).unwrap())
    }

    /// The fixture's reference data in `t`, which holds the catalog schema.
    pub(crate) fn on(t: TestDatabase) -> Self {
        let mut catalog = Self {
            t,
            author: UserId::generate(),
            approver: UserId::generate(),
            specimen_type: SpecimenTypeId::generate(),
            analyte_a: AnalyteId::generate(),
            analyte_b: AnalyteId::generate(),
        };
        catalog.specimen_type = catalog
            .as_author(|ctx| create_specimen_type(ctx, coded("SYN-UR", "Synthetic urine")))
            .unwrap();
        catalog.analyte_a = catalog.analyte("SYN-A").unwrap();
        catalog.analyte_b = catalog.analyte("SYN-B").unwrap();
        catalog
    }

    pub(crate) fn as_user<T, F>(
        &self,
        user: UserId,
        capabilities: &[Capability],
        command: F,
    ) -> Result<T, CatalogError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, CatalogError> + Send + 'static,
    {
        self.run(user_principal(user, capabilities), command)
    }

    pub(crate) fn run<T, F>(&self, principal: Principal, command: F) -> Result<T, CatalogError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, CatalogError> + Send + 'static,
    {
        self.t
            .db
            .write_blocking(principal, CorrelationId::generate(), command)
    }

    pub(crate) fn as_author<T, F>(&self, command: F) -> Result<T, CatalogError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, CatalogError> + Send + 'static,
    {
        self.as_user(self.author, &[Capability::ManageConfiguration], command)
    }

    pub(crate) fn as_approver<T, F>(&self, command: F) -> Result<T, CatalogError>
    where
        T: Send + 'static,
        F: FnOnce(&mut CommandContext<'_>) -> Result<T, CatalogError> + Send + 'static,
    {
        self.as_user(self.approver, &[Capability::ApproveConfiguration], command)
    }

    pub(crate) fn read<T, F>(&self, query: F) -> T
    where
        T: Send + 'static,
        F: FnOnce(&Connection) -> Result<T, CatalogError> + Send + 'static,
    {
        self.t.db.read_blocking(query).unwrap()
    }

    pub(crate) fn analyte(&self, code: &str) -> Result<AnalyteId, CatalogError> {
        let code = code.to_owned();
        self.as_author(move |ctx| {
            create_analyte(
                ctx,
                CreateAnalyte {
                    name: format!("Synthetic analyte {code}"),
                    code,
                    loinc_code: Some("00000-0".to_owned()),
                    drug_class_code: Some("SYN-CLASS".to_owned()),
                },
            )
        })
    }

    pub(crate) fn method(&self, code: &str) -> MethodId {
        let input = coded(code, "Synthetic method");
        self.as_author(move |ctx| create_method(ctx, input))
            .unwrap()
    }

    /// A content with both analytes and synthetic limits.
    pub(crate) fn content(&self) -> MethodVersionContent {
        MethodVersionContent {
            report_notes: None,
            specimen_type_id: self.specimen_type,
            certainty: MethodCertainty::Definitive,
            output_scale: OutputScale::Quantitative,
            threshold_rule: ThresholdRule::AtOrAbove,
            threshold_rounding: ThresholdRounding::BeforeRounding,
            analytes: vec![limits(self.analyte_a), limits(self.analyte_b)],
        }
    }

    pub(crate) fn draft_method(
        &self,
        method_id: MethodId,
    ) -> Result<MethodVersionId, CatalogError> {
        let content = self.content();
        self.as_author(move |ctx| {
            draft_method_version(ctx, DraftMethodVersion { method_id, content })
        })
        .map(|draft| draft.id)
    }

    pub(crate) fn approve_method(
        &self,
        id: MethodVersionId,
        expected_revision: i64,
    ) -> Result<i64, CatalogError> {
        self.as_approver(move |ctx| {
            approve_method_version(
                ctx,
                ApproveMethodVersion {
                    external_approval_reference: None,
                    method_version_id: id,
                    expected_revision,
                    self_approval: false,
                },
            )
        })
        .map(|approved| approved.revision)
    }

    /// A new method with one approved version.
    pub(crate) fn approved_method(&self, code: &str) -> MethodVersionId {
        let id = self.draft_method(self.method(code)).unwrap();
        self.approve_method(id, 1).unwrap();
        id
    }

    pub(crate) fn orderable(&self, code: &str, kind: OrderableKind) -> OrderableId {
        let code = code.to_owned();
        self.as_author(move |ctx| {
            create_orderable(
                ctx,
                CreateOrderable {
                    code,
                    name: "Synthetic panel".to_owned(),
                    kind,
                },
            )
        })
        .unwrap()
    }

    /// A manifest with one component per method version, expecting analyte A.
    pub(crate) fn manifest(&self, method_versions: &[MethodVersionId]) -> OrderableVersionContent {
        OrderableVersionContent {
            components: method_versions
                .iter()
                .map(|&method_version_id| ComponentInput {
                    method_version_id,
                    purpose: ComponentPurpose::Analytical,
                    analyte_ids: vec![self.analyte_a],
                })
                .collect(),
        }
    }

    pub(crate) fn draft_orderable(
        &self,
        orderable_id: OrderableId,
        content: OrderableVersionContent,
    ) -> Result<OrderableVersionId, CatalogError> {
        self.as_author(move |ctx| {
            draft_orderable_version(
                ctx,
                DraftOrderableVersion {
                    orderable_id,
                    content,
                },
            )
        })
        .map(|draft| draft.id)
    }

    /// Synthetic categories: one without consequence, one with a qualifier.
    pub(crate) fn validity_content(&self) -> ValidityPolicyContent {
        ValidityPolicyContent {
            categories: vec![
                ValidityCategory {
                    code: "SYN-NORMAL".to_owned(),
                    label: "Synthetic normal".to_owned(),
                    action: ValidityAction::None,
                    qualifier_text: None,
                },
                ValidityCategory {
                    code: "SYN-DILUTE".to_owned(),
                    label: "Synthetic dilute".to_owned(),
                    action: ValidityAction::Qualifier,
                    qualifier_text: Some("Synthetic qualifier text".to_owned()),
                },
            ],
        }
    }

    pub(crate) fn draft_validity(
        &self,
        specimen_type_id: SpecimenTypeId,
    ) -> Result<ValidityPolicyVersionId, CatalogError> {
        let content = self.validity_content();
        self.as_author(move |ctx| {
            draft_validity_policy_version(
                ctx,
                DraftValidityPolicyVersion {
                    specimen_type_id,
                    content,
                },
            )
        })
        .map(|draft| draft.id)
    }

    pub(crate) fn approve_validity(
        &self,
        id: ValidityPolicyVersionId,
        expected_revision: i64,
    ) -> Result<i64, CatalogError> {
        self.as_approver(move |ctx| {
            approve_validity_policy_version(
                ctx,
                ApproveValidityPolicyVersion {
                    policy_version_id: id,
                    expected_revision,
                    self_approval: false,
                },
            )
        })
        .map(|approved| approved.revision)
    }

    /// Row counts of every module table plus the audit trail.
    pub(crate) fn counts(&self) -> Vec<i64> {
        self.read(|conn| TABLES.iter().map(|table| count(conn, table)).collect())
    }

    /// Audit actions in order.
    pub(crate) fn audit_actions(&self) -> Vec<String> {
        self.read(|conn| {
            let mut statement = conn.prepare("SELECT action FROM audit_events ORDER BY seq")?;
            let actions = statement.query_map([], |row| row.get(0))?;
            Ok(actions.collect::<rusqlite::Result<_>>()?)
        })
    }

    /// A direct connection, as an administrator editing the file would use.
    pub(crate) fn raw(&self) -> Connection {
        Connection::open(self.t.data_dir().join(lis_store::DATABASE_FILE)).unwrap()
    }
}

fn count(conn: &Connection, table: &str) -> Result<i64, CatalogError> {
    let sql = format!("SELECT count(*) FROM {table}");
    Ok(conn.query_row(&sql, [], |row| row.get(0))?)
}

pub(crate) fn user_principal(user: UserId, capabilities: &[Capability]) -> Principal {
    Principal::user(
        user,
        SessionId::generate(),
        WorkstationId::generate(),
        capabilities.iter().copied(),
    )
}

pub(crate) fn coded(code: &str, name: &str) -> CodedName {
    CodedName {
        code: code.to_owned(),
        name: name.to_owned(),
    }
}

pub(crate) fn dec(text: &str) -> Option<DecimalText> {
    Some(DecimalText::parse(text).unwrap())
}

/// Obviously synthetic limits, with trailing zeros that must survive storage.
pub(crate) fn limits(analyte_id: AnalyteId) -> MethodAnalyte {
    MethodAnalyte {
        reference_interval: None,
        range_policy: Default::default(),
        analyte_id,
        unit: Some("ng/mL".to_owned()),
        cutoff: dec("111.10"),
        limit_of_detection: dec("0.000100"),
        limit_of_quantitation: dec("2.500"),
        reportable_low: dec("5.0"),
        reportable_high: dec("1000.00"),
        display_scale: Some(2),
    }
}

#[test]
fn schema_passes_the_lint() {
    assert_eq!(
        lint_schema(&[lis_auth::SCHEMA, SCHEMA]).unwrap(),
        Vec::new()
    );
}

#[test]
fn command_inputs_refuse_unknown_fields() {
    let valid = serde_json::json!({ "code": "SYN-X", "name": "Synthetic", "kind": "panel" });
    assert!(serde_json::from_value::<CreateOrderable>(valid.clone()).is_ok());
    let mut extra = valid;
    extra["approved"] = serde_json::json!(true);
    assert!(serde_json::from_value::<CreateOrderable>(extra).is_err());
}
