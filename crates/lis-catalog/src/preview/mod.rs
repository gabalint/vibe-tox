//! The Catalog Preview snapshot (REQ-OPS-015, proposed): every catalog value
//! the report prints, each with its source, the gaps, the open drafts and a
//! content digest. Pure data: the engine maps it to the report renderer's
//! own data struct, renders and stores the report, then calls
//! `record_catalog_preview` with the digest and the drafts.

mod sources;

use lis_core::Id;
use lis_store::{ArtifactDigest, StoreError};
use rusqlite::Connection;
use serde::Serialize;

use crate::error::CatalogError;
use crate::lifecycle::{VersionLifecycle, VersionSummary, Versioned};
use crate::model::{
    CatalogDraft, MethodCertainty, MethodVersion, MethodVersionId, OutputScale, ThresholdRounding,
    ThresholdRule, VersionStatus,
};
use crate::queries::{get_method_version, get_orderable_version, validity_policy};
use crate::repo::{lifecycle, reference};
use crate::standard::format::StandardCatalog;
use crate::standard::{StandardCatalogInfo, StandardFile};
use crate::views::{
    AnalyteView, MethodAnalyteView, MethodVersionView, MethodView, OrderableVersionView,
    OrderableView, SpecimenTypeView, ValidityPolicyView,
};

pub use sources::{
    CatalogGap, CatalogGapField, CatalogValueSource, CatalogValueSourceKind, DemoValues,
};

/// Versioned label of the canonical serialization the digest covers.
const DIGEST_FORMAT: &str = "catalog-preview-v1";

/// Everything the Catalog Preview report prints.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewSnapshot {
    /// The Standard Catalog file's version, disclaimer and LOINC notice.
    pub standard: StandardCatalogInfo,
    /// Specimen types by code.
    pub specimen_types: Vec<SpecimenTypeView>,
    /// Analytes by code.
    pub analytes: Vec<CatalogPreviewAnalyte>,
    /// Methods by code, each with its current and draft versions.
    pub methods: Vec<CatalogPreviewMethod>,
    /// Tests and panels by code, each with its current and draft versions.
    pub orderables: Vec<CatalogPreviewOrderable>,
    /// Current and draft validity policy versions, by specimen type code.
    pub validity_policies: Vec<ValidityPolicyView>,
    /// Blank cutoffs and starter values, for the first page.
    pub gaps: Vec<CatalogGap>,
    /// Every open draft at its current revision; All Data Reviewed approves these.
    pub drafts: Vec<CatalogDraft>,
    /// Lowercase hex SHA-256 over a canonical serialization of the fields
    /// above; it excludes generation time and user.
    pub content_digest: String,
}

/// An analyte, with the LOINC display name from the file when the analyte
/// came from it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewAnalyte {
    /// The analyte as stored.
    #[serde(flatten)]
    pub analyte: AnalyteView,
    /// The pinned release's display name, when code and LOINC code match the file.
    pub loinc_display_name: Option<String>,
}

/// A method with its current approved and draft versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewMethod {
    /// The method.
    #[serde(flatten)]
    pub method: MethodView,
    /// Current approved, then draft.
    pub versions: Vec<CatalogPreviewMethodVersion>,
}

/// A method version with each value's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewMethodVersion {
    /// The version.
    pub id: MethodVersionId,
    /// Version number.
    pub version: i64,
    /// Status, revision, author and approval.
    #[serde(flatten)]
    pub lifecycle: VersionLifecycle<MethodVersion>,
    /// The specimen type's code.
    pub specimen_type_code: String,
    /// Presumptive or definitive.
    pub certainty: MethodCertainty,
    /// Qualitative or quantitative.
    pub output_scale: OutputScale,
    /// Positive at ≥ or at > the cutoff.
    pub threshold_rule: ThresholdRule,
    /// Its source.
    pub threshold_rule_source: CatalogValueSource,
    /// Comparison before or after rounding.
    pub threshold_rounding: ThresholdRounding,
    /// Its source.
    pub threshold_rounding_source: CatalogValueSource,
    /// Analytes by code, with every limit and the cutoff's source.
    pub analytes: Vec<CatalogPreviewMethodAnalyte>,
}

/// One analyte of a method version with its limits and the cutoff's source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewMethodAnalyte {
    /// Code, name, unit, cutoff, limits, reportable range and display scale.
    #[serde(flatten)]
    pub analyte: MethodAnalyteView,
    /// Where the cutoff came from.
    pub cutoff_source: CatalogValueSource,
}

/// A test or panel with its current approved and draft versions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CatalogPreviewOrderable {
    /// The test or panel.
    #[serde(flatten)]
    pub orderable: OrderableView,
    /// Current approved, then draft, each with its components.
    pub versions: Vec<OrderableVersionView>,
}

/// The whole catalog for the Catalog Preview report. `demo_values` labels
/// the demo laboratory's values; it is empty in a real installation. The
/// engine checks `manage_configuration` or `approve_configuration` first.
pub fn catalog_preview_snapshot(
    conn: &Connection,
    file: &StandardFile,
    demo_values: &DemoValues,
) -> Result<CatalogPreviewSnapshot, CatalogError> {
    let c = file.catalog();
    let mut drafts = Vec::new();
    let mut methods = Vec::new();
    for method in reference::methods(conn)? {
        let mut versions = Vec::new();
        for summary in open_versions::<MethodVersion>(conn, method.id)? {
            let view = get_method_version(conn, summary.id)?;
            if view.lifecycle.status == VersionStatus::Draft {
                drafts.push(CatalogDraft::MethodVersion {
                    id: view.id,
                    revision: view.lifecycle.revision,
                });
            }
            versions.push(method_version(c, demo_values, view));
        }
        methods.push(CatalogPreviewMethod { method, versions });
    }
    let mut orderables = Vec::new();
    for orderable in reference::orderables(conn)? {
        let mut versions = Vec::new();
        for summary in open_versions(conn, orderable.id)? {
            if summary.lifecycle.status == VersionStatus::Draft {
                drafts.push(CatalogDraft::OrderableVersion {
                    id: summary.id,
                    revision: summary.lifecycle.revision,
                });
            }
            versions.push(get_orderable_version(conn, summary.id)?);
        }
        orderables.push(CatalogPreviewOrderable {
            orderable,
            versions,
        });
    }
    let specimen_types = reference::specimen_types(conn)?;
    let mut validity_policies = Vec::new();
    for specimen_type in &specimen_types {
        for summary in open_versions(conn, specimen_type.id)? {
            if summary.lifecycle.status == VersionStatus::Draft {
                drafts.push(CatalogDraft::ValidityPolicyVersion {
                    id: summary.id,
                    revision: summary.lifecycle.revision,
                });
            }
            validity_policies.push(validity_policy(conn, summary.id)?);
        }
    }
    let mut snapshot = CatalogPreviewSnapshot {
        standard: file.info(),
        specimen_types,
        analytes: analytes(conn, c)?,
        gaps: gaps(&methods),
        methods,
        orderables,
        validity_policies,
        drafts,
        content_digest: String::new(),
    };
    snapshot.content_digest = content_digest(&snapshot)?;
    Ok(snapshot)
}

/// The current approved version and the open draft of `parent`, oldest first.
fn open_versions<K: Versioned>(
    conn: &Connection,
    parent: Id<K::Parent>,
) -> Result<Vec<VersionSummary<K>>, CatalogError> {
    Ok(lifecycle::history::<K>(conn, parent)?
        .into_iter()
        .filter(|v| v.lifecycle.is_current() || v.lifecycle.status == VersionStatus::Draft)
        .collect())
}

fn analytes(
    conn: &Connection,
    c: &StandardCatalog,
) -> Result<Vec<CatalogPreviewAnalyte>, CatalogError> {
    Ok(reference::analytes(conn)?
        .into_iter()
        .map(|analyte| {
            let loinc = c
                .analyte(&analyte.code)
                .and_then(|a| a.loinc.as_ref())
                .filter(|l| analyte.loinc_code.as_deref() == Some(l.code.as_str()));
            CatalogPreviewAnalyte {
                loinc_display_name: loinc.map(|l| l.display_name.clone()),
                analyte,
            }
        })
        .collect())
}

fn method_version(
    c: &StandardCatalog,
    demo_values: &DemoValues,
    view: MethodVersionView,
) -> CatalogPreviewMethodVersion {
    let (threshold_rule_source, threshold_rounding_source) = sources::threshold_sources(
        c.method(&view.method_code),
        view.threshold_rule,
        view.threshold_rounding,
    );
    let analytes = view
        .analytes
        .into_iter()
        .map(|analyte| CatalogPreviewMethodAnalyte {
            cutoff_source: sources::cutoff_source(
                c,
                demo_values,
                &view.method_code,
                &analyte.analyte_code,
                analyte.limits.cutoff.as_ref(),
            ),
            analyte,
        })
        .collect();
    CatalogPreviewMethodVersion {
        id: view.id,
        version: view.version,
        lifecycle: view.lifecycle,
        specimen_type_code: view.specimen_type_code,
        certainty: view.certainty,
        output_scale: view.output_scale,
        threshold_rule: view.threshold_rule,
        threshold_rule_source,
        threshold_rounding: view.threshold_rounding,
        threshold_rounding_source,
        analytes,
    }
}

/// Blank cutoffs and starter values of every listed method version.
fn gaps(methods: &[CatalogPreviewMethod]) -> Vec<CatalogGap> {
    let mut gaps = Vec::new();
    for method in methods {
        for version in &method.versions {
            version_gaps(&method.method.code, version, &mut gaps);
        }
    }
    gaps
}

fn version_gaps(
    method_code: &str,
    version: &CatalogPreviewMethodVersion,
    gaps: &mut Vec<CatalogGap>,
) {
    let gap = |kind, field, analyte: Option<&MethodAnalyteView>| CatalogGap {
        kind,
        field,
        method_version_id: version.id,
        method_code: method_code.to_owned(),
        version: version.version,
        status: version.lifecycle.status,
        analyte_id: analyte.map(|a| a.limits.analyte_id),
        analyte_code: analyte.map(|a| a.analyte_code.clone()),
    };
    let starter = CatalogValueSourceKind::StarterValue;
    if version.threshold_rule_source.kind == starter {
        gaps.push(gap(starter, CatalogGapField::ThresholdRule, None));
    }
    if version.threshold_rounding_source.kind == starter {
        gaps.push(gap(starter, CatalogGapField::ThresholdRounding, None));
    }
    let blank = CatalogValueSourceKind::Blank;
    for analyte in &version.analytes {
        if analyte.cutoff_source.kind == blank {
            gaps.push(gap(blank, CatalogGapField::Cutoff, Some(&analyte.analyte)));
        }
    }
}

/// SHA-256 over the versioned canonical JSON of every field but the digest.
/// Field order is fixed by the types and row order by the queries.
fn content_digest(s: &CatalogPreviewSnapshot) -> Result<String, CatalogError> {
    let canonical = (
        DIGEST_FORMAT,
        &s.standard,
        &s.specimen_types,
        &s.analytes,
        &s.methods,
        &s.orderables,
        &s.validity_policies,
        &s.gaps,
        &s.drafts,
    );
    let bytes = serde_json::to_vec(&canonical).map_err(|error| StoreError::Io(error.into()))?;
    let (digest, _) = ArtifactDigest::of_reader(bytes.as_slice()).map_err(StoreError::from)?;
    Ok(digest.to_string())
}
