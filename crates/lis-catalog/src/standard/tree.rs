//! Read side of the Standard Catalog: the preview tree the load screen shows
//! and the load status behind the "starter content, not reviewed" banner
//! (REQ-OPS-014). The engine checks `manage_configuration` before calling
//! [`standard_catalog_tree`].

use lis_core::{DecimalText, UserId, UtcTimestamp};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::model::{
    ComponentPurpose, MethodCertainty, OrderableKind, OutputScale, StandardCatalogLoadId,
    ThresholdRounding, ThresholdRule,
};
use crate::repo::standard as repo;
use crate::standard::format::{Analyte, Method, MethodAnalyte, Orderable, StandardCatalog};
use crate::standard::state::{Existing, StandardItemStatus};
use crate::standard::{StandardCatalogInfo, StandardFile};

/// Everything the file can load, each item with its state in this laboratory.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StandardCatalogTree {
    /// The file's version, digest, disclaimer and LOINC notice.
    pub info: StandardCatalogInfo,
    /// Specimen types, in file order.
    pub specimen_types: Vec<StandardSpecimenTypeItem>,
    /// Analytes, in file order.
    pub analytes: Vec<StandardAnalyteItem>,
    /// Methods, in file order. Choosing one implies its specimen type and analytes.
    pub methods: Vec<StandardMethodItem>,
    /// Tests and panels, in file order. Choosing one implies its methods.
    pub orderables: Vec<StandardOrderableItem>,
}

/// A specimen type of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardSpecimenTypeItem {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// Whether it can be loaded.
    #[serde(flatten)]
    pub status: StandardItemStatus,
}

/// An analyte of the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardAnalyteItem {
    /// Code, prefixed with its specimen type code.
    pub code: String,
    /// Name.
    pub name: String,
    /// The project's drug class code; none for validity analytes.
    pub drug_class_code: Option<String>,
    /// The drug class name.
    pub drug_class_name: Option<String>,
    /// LOINC code.
    pub loinc_code: Option<String>,
    /// LOINC display name from the pinned release.
    pub loinc_display_name: Option<String>,
    /// Whether it can be loaded.
    #[serde(flatten)]
    pub status: StandardItemStatus,
}

/// A method of the file. It depends on `specimenTypeCode` and on every
/// `analytes[].analyteCode`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardMethodItem {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// The specimen type it needs.
    pub specimen_type_code: String,
    /// Presumptive or definitive.
    pub certainty: MethodCertainty,
    /// Qualitative or quantitative.
    pub output_scale: OutputScale,
    /// Starter value the laboratory confirms.
    pub threshold_rule: ThresholdRule,
    /// Starter value the laboratory confirms.
    pub threshold_rounding: ThresholdRounding,
    /// The analytes it needs, with unit and published cutoff.
    pub analytes: Vec<StandardMethodAnalyteItem>,
    /// Whether it can be loaded.
    #[serde(flatten)]
    pub status: StandardItemStatus,
}

/// One analyte of a method.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardMethodAnalyteItem {
    /// The analyte's code.
    pub analyte_code: String,
    /// UCUM unit text.
    pub unit: Option<String>,
    /// Published cutoff; blank when no public source states one.
    pub cutoff: Option<DecimalText>,
    /// Title of the cutoff's source.
    pub cutoff_source_title: Option<String>,
    /// The cutoff's short label, such as "Federal workplace initial test cutoff".
    pub cutoff_label: Option<String>,
}

/// A test or panel of the file. It depends on every code in `methodCodes`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardOrderableItem {
    /// Code.
    pub code: String,
    /// Name.
    pub name: String,
    /// Test or panel.
    pub kind: OrderableKind,
    /// The distinct methods it needs, in component order.
    pub method_codes: Vec<String>,
    /// Components in order.
    pub components: Vec<StandardComponentItem>,
    /// Whether it can be loaded.
    #[serde(flatten)]
    pub status: StandardItemStatus,
}

/// One component of a test or panel.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardComponentItem {
    /// The method's code.
    pub method_code: String,
    /// Analytical or validity.
    pub purpose: ComponentPurpose,
    /// The analytes it expects, by code.
    pub analyte_codes: Vec<String>,
}

/// Every item of `file` with its state in the laboratory's catalog.
pub fn standard_catalog_tree(
    conn: &Connection,
    file: &StandardFile,
) -> Result<StandardCatalogTree, CatalogError> {
    let existing = Existing::load(conn)?;
    let c = file.catalog();
    Ok(StandardCatalogTree {
        info: file.info(),
        specimen_types: c
            .specimen_types
            .iter()
            .map(|s| StandardSpecimenTypeItem {
                code: s.code.clone(),
                name: s.name.clone(),
                status: existing.specimen_type(s),
            })
            .collect(),
        analytes: c
            .analytes
            .iter()
            .map(|a| analyte_item(c, a, existing.analyte(a)))
            .collect(),
        methods: c
            .methods
            .iter()
            .map(|m| method_item(c, m, existing.method(m)))
            .collect(),
        orderables: c
            .orderables
            .iter()
            .map(|o| orderable_item(o, existing.orderable(o)))
            .collect(),
    })
}

fn analyte_item(
    c: &StandardCatalog,
    a: &Analyte,
    status: StandardItemStatus,
) -> StandardAnalyteItem {
    let class = a.drug_class.as_deref().and_then(|code| c.drug_class(code));
    StandardAnalyteItem {
        code: a.code.clone(),
        name: a.name.clone(),
        drug_class_code: a.drug_class.clone(),
        drug_class_name: class.map(|class| class.name.clone()),
        loinc_code: a.loinc.as_ref().map(|l| l.code.clone()),
        loinc_display_name: a.loinc.as_ref().map(|l| l.display_name.clone()),
        status,
    }
}

fn method_item(c: &StandardCatalog, m: &Method, status: StandardItemStatus) -> StandardMethodItem {
    StandardMethodItem {
        code: m.code.clone(),
        name: m.name.clone(),
        specimen_type_code: m.specimen_type.clone(),
        certainty: m.certainty,
        output_scale: m.output_scale,
        threshold_rule: m.threshold_rule,
        threshold_rounding: m.threshold_rounding,
        analytes: m
            .analytes
            .iter()
            .map(|a| method_analyte_item(c, a))
            .collect(),
        status,
    }
}

fn method_analyte_item(c: &StandardCatalog, a: &MethodAnalyte) -> StandardMethodAnalyteItem {
    let cutoff = a.cutoff.as_ref();
    StandardMethodAnalyteItem {
        analyte_code: a.analyte.clone(),
        unit: a.unit.clone(),
        cutoff: cutoff.map(|cutoff| cutoff.value.clone()),
        cutoff_source_title: cutoff
            .and_then(|cutoff| c.source(&cutoff.source))
            .map(|source| source.title.clone()),
        cutoff_label: cutoff.map(|cutoff| cutoff.label.clone()),
    }
}

fn orderable_item(o: &Orderable, status: StandardItemStatus) -> StandardOrderableItem {
    StandardOrderableItem {
        code: o.code.clone(),
        name: o.name.clone(),
        kind: o.kind,
        method_codes: o.method_codes().into_iter().map(str::to_owned).collect(),
        components: o
            .components
            .iter()
            .map(|component| StandardComponentItem {
                method_code: component.method.clone(),
                purpose: component.purpose,
                analyte_codes: component.analytes.clone(),
            })
            .collect(),
        status,
    }
}

/// Which Standard Catalog versions were loaded, and how many loaded versions
/// are still drafts. While that count is above zero the app shows the
/// "starter content, not reviewed" banner.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StandardCatalogStatus {
    /// Every load, oldest first.
    pub loads: Vec<StandardCatalogLoadView>,
    /// Loaded method and orderable versions still in draft.
    #[ts(type = "number")]
    pub drafts_remaining: i64,
}

/// One recorded load.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct StandardCatalogLoadView {
    /// The load.
    pub id: StandardCatalogLoadId,
    /// The file's content version.
    pub catalog_version: String,
    /// SHA-256 of the file's bytes.
    pub file_digest: String,
    /// Who loaded it.
    pub loaded_by: UserId,
    /// When.
    pub loaded_at: UtcTimestamp,
    /// Records created, including versions.
    #[ts(type = "number")]
    pub records: i64,
}

/// The loads so far and the loaded versions still in draft.
pub fn standard_catalog_status(conn: &Connection) -> Result<StandardCatalogStatus, CatalogError> {
    Ok(StandardCatalogStatus {
        loads: repo::loads(conn)?,
        drafts_remaining: repo::loaded_drafts(conn)?,
    })
}
