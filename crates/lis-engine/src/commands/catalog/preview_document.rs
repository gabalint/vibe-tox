//! Maps the Catalog Preview snapshot (`lis-catalog`) to the data contract of
//! the `catalog-preview` v1 template (`lis-report`), REQ-OPS-015 (proposed).
//! Pure: every value is copied as stored, decimals as their lexical text.

use lis_catalog::{
    CatalogDraft, CatalogGap, CatalogGapField, CatalogPreviewMethod, CatalogPreviewMethodVersion,
    CatalogPreviewOrderable, CatalogPreviewSnapshot, CatalogValueSource, CatalogValueSourceKind,
    ComponentPurpose, MethodCertainty, OrderableKind, OrderableVersionView, OutputScale,
    ThresholdRounding, ThresholdRule, ValidityAction, ValidityPolicyView, VersionStatus,
};
use lis_report::catalog_preview_v1 as doc;

/// What the report prints besides the snapshot.
pub(super) struct Header {
    pub(super) report: doc::CatalogPreviewHeader,
    pub(super) laboratory_name: String,
    /// The Standard Catalog disclaimer while loaded drafts remain.
    pub(super) disclaimer: Option<String>,
}

/// The snapshot contradicts itself, which is a defect; the detail is logged.
#[derive(Debug)]
pub(super) struct Inconsistent(pub(super) &'static str);

/// The printed document of `snapshot` under `header`.
pub(super) fn document(
    snapshot: &CatalogPreviewSnapshot,
    header: Header,
) -> Result<doc::CatalogPreviewDocument, Inconsistent> {
    let s = snapshot;
    let any_loinc = s.analytes.iter().any(|a| a.analyte.loinc_code.is_some());
    Ok(doc::CatalogPreviewDocument {
        report: header.report,
        laboratory_name: header.laboratory_name,
        disclaimer: header.disclaimer,
        gaps: s.gaps.iter().map(gap).collect(),
        drafts: s
            .drafts
            .iter()
            .map(|draft| listed_draft(s, *draft))
            .collect::<Result<_, _>>()?,
        specimen_types: s
            .specimen_types
            .iter()
            .map(|t| doc::PreviewSpecimenType {
                code: t.code.clone(),
                name: t.name.clone(),
            })
            .collect(),
        analytes: s
            .analytes
            .iter()
            .map(|a| doc::PreviewAnalyte {
                code: a.analyte.code.clone(),
                name: a.analyte.name.clone(),
                loinc_code: a.analyte.loinc_code.clone(),
                loinc_display_name: a.loinc_display_name.clone(),
                drug_class: a.analyte.drug_class_code.clone(),
            })
            .collect(),
        methods: s.methods.iter().map(method).collect::<Result<_, _>>()?,
        orderables: s
            .orderables
            .iter()
            .map(orderable)
            .collect::<Result<_, _>>()?,
        validity_policies: validity_policies(&s.validity_policies)?,
        loinc_notice: any_loinc.then(|| s.standard.loinc_notice.clone()),
    })
}

fn gap(g: &CatalogGap) -> doc::CatalogGap {
    doc::CatalogGap {
        record: format!("{} version {}", g.method_code, g.version),
        analyte: g.analyte_code.clone(),
        gap: match g.field {
            CatalogGapField::Cutoff => doc::GapKind::BlankCutoff,
            CatalogGapField::ThresholdRule => doc::GapKind::StarterThresholdRule,
            CatalogGapField::ThresholdRounding => doc::GapKind::StarterThresholdRounding,
        },
    }
}

/// A listed draft with its code, name and version, looked up in the snapshot.
fn listed_draft(
    s: &CatalogPreviewSnapshot,
    draft: CatalogDraft,
) -> Result<doc::CatalogDraft, Inconsistent> {
    let (record_type, code, name, version, revision) = match draft {
        CatalogDraft::MethodVersion { id, revision } => {
            let (method, version) = s
                .methods
                .iter()
                .find_map(|m| m.versions.iter().find(|v| v.id == id).map(|v| (m, v)))
                .ok_or(Inconsistent("listed method draft"))?;
            let m = &method.method;
            let (code, name) = (m.code.clone(), m.name.clone());
            (
                doc::DraftRecordType::Method,
                code,
                name,
                version.version,
                revision,
            )
        }
        CatalogDraft::OrderableVersion { id, revision } => {
            let version = s
                .orderables
                .iter()
                .find_map(|o| o.versions.iter().find(|v| v.id == id))
                .ok_or(Inconsistent("listed orderable draft"))?;
            let record_type = match version.kind {
                OrderableKind::Test => doc::DraftRecordType::Test,
                OrderableKind::Panel => doc::DraftRecordType::Panel,
            };
            let (code, name) = (version.code.clone(), version.name.clone());
            (record_type, code, name, version.version, revision)
        }
        CatalogDraft::ValidityPolicyVersion { id, revision } => {
            let policy = s
                .validity_policies
                .iter()
                .find(|p| p.id == id)
                .ok_or(Inconsistent("listed validity policy draft"))?;
            let name = s
                .specimen_types
                .iter()
                .find(|t| t.id == policy.specimen_type_id)
                .map(|t| t.name.clone())
                .ok_or(Inconsistent("validity policy specimen type"))?;
            let code = policy.specimen_type_code.clone();
            let record_type = doc::DraftRecordType::ValidityPolicy;
            (record_type, code, name, policy.version, revision)
        }
    };
    Ok(doc::CatalogDraft {
        record_type,
        code,
        name,
        version: version.to_string(),
        revision: revision.to_string(),
    })
}

/// Draft or in effect; the snapshot holds no other versions.
fn status(status: VersionStatus) -> Result<doc::PreviewVersionStatus, Inconsistent> {
    match status {
        VersionStatus::Draft => Ok(doc::PreviewVersionStatus::Draft),
        VersionStatus::Approved => Ok(doc::PreviewVersionStatus::InEffect),
        VersionStatus::Retired => Err(Inconsistent("retired version in preview")),
    }
}

fn source(source: &CatalogValueSource) -> Result<doc::SourceLabel, Inconsistent> {
    Ok(match source.kind {
        CatalogValueSourceKind::Published => doc::SourceLabel::Published {
            title: source
                .source_title
                .clone()
                .ok_or(Inconsistent("published value without source"))?,
            label: source
                .label
                .clone()
                .ok_or(Inconsistent("published value without label"))?,
        },
        CatalogValueSourceKind::DemoValue => doc::SourceLabel::DemoValue,
        CatalogValueSourceKind::Blank => doc::SourceLabel::Blank,
        CatalogValueSourceKind::ChangedByLaboratory => doc::SourceLabel::ChangedByLaboratory,
        CatalogValueSourceKind::LaboratoryEntered => doc::SourceLabel::EnteredByLaboratory,
        CatalogValueSourceKind::StarterValue => doc::SourceLabel::StarterValue,
    })
}

fn method(m: &CatalogPreviewMethod) -> Result<doc::PreviewMethod, Inconsistent> {
    Ok(doc::PreviewMethod {
        code: m.method.code.clone(),
        name: m.method.name.clone(),
        versions: m
            .versions
            .iter()
            .map(method_version)
            .collect::<Result<_, _>>()?,
    })
}

fn method_version(
    v: &CatalogPreviewMethodVersion,
) -> Result<doc::PreviewMethodVersion, Inconsistent> {
    let text = |value: &Option<lis_core::DecimalText>| value.as_ref().map(ToString::to_string);
    let mut analytes = Vec::new();
    for a in &v.analytes {
        let limits = &a.analyte.limits;
        analytes.push(doc::PreviewMethodAnalyte {
            code: a.analyte.analyte_code.clone(),
            name: a.analyte.analyte_name.clone(),
            unit: limits.unit.clone(),
            cutoff: text(&limits.cutoff),
            cutoff_source: source(&a.cutoff_source)?,
            lod: text(&limits.limit_of_detection),
            loq: text(&limits.limit_of_quantitation),
            reportable_low: text(&limits.reportable_low),
            reportable_high: text(&limits.reportable_high),
            display_scale: limits.display_scale.map(|scale| scale.to_string()),
        });
    }
    Ok(doc::PreviewMethodVersion {
        version: v.version.to_string(),
        status: status(v.lifecycle.status)?,
        specimen_type: v.specimen_type_code.clone(),
        certainty: match v.certainty {
            MethodCertainty::Presumptive => doc::PreviewCertainty::Presumptive,
            MethodCertainty::Definitive => doc::PreviewCertainty::Definitive,
        },
        output_scale: match v.output_scale {
            OutputScale::Qualitative => doc::PreviewOutputScale::Qualitative,
            OutputScale::Quantitative => doc::PreviewOutputScale::Quantitative,
        },
        threshold_rule: doc::SourcedThresholdRule {
            value: match v.threshold_rule {
                ThresholdRule::AtOrAbove => doc::PreviewThresholdRule::AtOrAbove,
                ThresholdRule::Above => doc::PreviewThresholdRule::Above,
            },
            source: source(&v.threshold_rule_source)?,
        },
        threshold_rounding: doc::SourcedThresholdRounding {
            value: match v.threshold_rounding {
                ThresholdRounding::BeforeRounding => doc::PreviewThresholdRounding::BeforeRounding,
                ThresholdRounding::AfterRounding => doc::PreviewThresholdRounding::AfterRounding,
            },
            source: source(&v.threshold_rounding_source)?,
        },
        analytes,
    })
}

fn orderable(o: &CatalogPreviewOrderable) -> Result<doc::PreviewOrderable, Inconsistent> {
    Ok(doc::PreviewOrderable {
        code: o.orderable.code.clone(),
        name: o.orderable.name.clone(),
        kind: match o.orderable.kind {
            OrderableKind::Test => doc::PreviewOrderableKind::Test,
            OrderableKind::Panel => doc::PreviewOrderableKind::Panel,
        },
        versions: o
            .versions
            .iter()
            .map(orderable_version)
            .collect::<Result<_, _>>()?,
    })
}

fn orderable_version(
    v: &OrderableVersionView,
) -> Result<doc::PreviewOrderableVersion, Inconsistent> {
    Ok(doc::PreviewOrderableVersion {
        version: v.version.to_string(),
        status: status(v.lifecycle.status)?,
        components: v
            .components
            .iter()
            .map(|c| doc::PreviewComponent {
                position: c.position.to_string(),
                method_code: c.method_code.clone(),
                method_version: c.method_version.to_string(),
                purpose: match c.purpose {
                    ComponentPurpose::Analytical => doc::PreviewComponentPurpose::Analytical,
                    ComponentPurpose::Validity => doc::PreviewComponentPurpose::Validity,
                },
                analyte_codes: c.analytes.iter().map(|a| a.code.clone()).collect(),
            })
            .collect(),
    })
}

/// Policy versions grouped by specimen type; the snapshot lists them by
/// specimen type code.
fn validity_policies(
    views: &[ValidityPolicyView],
) -> Result<Vec<doc::PreviewValidityPolicy>, Inconsistent> {
    let mut policies: Vec<doc::PreviewValidityPolicy> = Vec::new();
    for view in views {
        let version = doc::PreviewValidityVersion {
            version: view.version.to_string(),
            status: status(view.lifecycle.status)?,
            categories: view
                .categories
                .iter()
                .map(|c| doc::PreviewValidityCategory {
                    code: c.code.clone(),
                    label: c.label.clone(),
                    action: validity_action(c.action),
                    qualifier_text: c.qualifier_text.clone(),
                })
                .collect(),
        };
        match policies.last_mut() {
            Some(policy) if policy.specimen_type == view.specimen_type_code => {
                policy.versions.push(version);
            }
            _ => policies.push(doc::PreviewValidityPolicy {
                specimen_type: view.specimen_type_code.clone(),
                versions: vec![version],
            }),
        }
    }
    Ok(policies)
}

fn validity_action(action: ValidityAction) -> doc::PreviewValidityAction {
    match action {
        ValidityAction::None => doc::PreviewValidityAction::None,
        ValidityAction::Qualifier => doc::PreviewValidityAction::Qualifier,
        ValidityAction::Hold => doc::PreviewValidityAction::Hold,
        ValidityAction::Recollect => doc::PreviewValidityAction::Recollect,
        ValidityAction::Comment => doc::PreviewValidityAction::Comment,
    }
}
