//! Load Standard Catalog (REQ-OPS-014, proposed). Creates the chosen items
//! and the items they need as drafts, through the ordinary create and draft
//! commands, so their checks and per-record audit events apply. Nothing is
//! approved here; All Data Reviewed does that (REQ-OPS-015).

mod plan;

use std::collections::HashMap;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::{CommandContext, StoreError};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::commands::lifecycle::acting_user;
use crate::commands::methods::{DraftMethodVersion, draft_method_version};
use crate::commands::orderables::{DraftOrderableVersion, draft_orderable_version};
use crate::commands::reference::{
    CodedName, CreateAnalyte, CreateOrderable, create_analyte, create_method, create_orderable,
    create_specimen_type,
};
use crate::content::{
    ComponentInput, MethodAnalyte, MethodVersionContent, OrderableVersionContent,
};
use crate::error::{CatalogError, invalid};
use crate::lifecycle::Versioned;
use crate::model::{
    AnalyteId, MethodVersion, MethodVersionId, OrderableVersion, SpecimenTypeId,
    StandardCatalogLoadId,
};
use crate::repo::standard::{self as repo, NewLoad};
use crate::standard::StandardFile;
use crate::standard::state::Existing;

use plan::{Plan, PlannedMethod, PlannedOrderable};

/// Input of [`load_standard_catalog`]. The UI sends the user's explicit
/// choices; the command computes what they imply.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct LoadStandardCatalog {
    /// The file version the user saw; a different embedded version is refused.
    pub catalog_version: String,
    /// Tests and panels chosen.
    pub orderable_codes: Vec<String>,
    /// Methods chosen on their own.
    pub method_codes: Vec<String>,
    /// Analytes removed from chosen methods, and from every component using them.
    pub omitted_analytes: Vec<StandardOmittedAnalyte>,
    /// The user acknowledged the not-for-production disclaimer.
    pub disclaimer_acknowledged: bool,
}

/// An analyte removed from a method before loading.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct StandardOmittedAnalyte {
    /// The method's code.
    pub method_code: String,
    /// The analyte's code.
    pub analyte_code: String,
}

/// What a load created. Items already in the catalog were skipped.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StandardCatalogLoaded {
    /// The load record; none when there was nothing new to create.
    pub load_id: Option<StandardCatalogLoadId>,
    /// Specimen types created.
    pub specimen_types: usize,
    /// Analytes created.
    pub analytes: usize,
    /// Methods created, each with a draft version.
    pub methods: usize,
    /// Tests and panels created, each with a draft version.
    pub orderables: usize,
    /// Items of the selection already in the catalog.
    pub skipped: usize,
}

/// Loads the chosen Standard Catalog items and everything they need, as
/// drafts, in one transaction (REQ-OPS-014). Refused without
/// `manage_configuration`, without the acknowledged disclaimer, for another
/// file version, and when any needed code is in use or blocked. Loading the
/// same selection again creates nothing.
pub fn load_standard_catalog(
    ctx: &mut CommandContext<'_>,
    file: &StandardFile,
    input: LoadStandardCatalog,
) -> Result<StandardCatalogLoaded, CatalogError> {
    ctx.require(Capability::ManageConfiguration)?;
    let loaded_by = acting_user(ctx.principal())?;
    if !input.disclaimer_acknowledged {
        return Err(invalid(
            "disclaimerAcknowledged",
            "required",
            "Acknowledge the disclaimer before loading.",
        ));
    }
    let c = file.catalog();
    if input.catalog_version != c.catalog_version {
        return Err(CatalogError::StandardVersionMismatch);
    }
    let existing = Existing::load(ctx.tx())?;
    let plan = plan::plan(c, &existing, &input)?;
    if plan.is_empty() {
        return Ok(loaded(None, &plan));
    }
    let items = write(ctx, &plan, &existing)?;
    let id = StandardCatalogLoadId::generate();
    let load = NewLoad {
        id,
        catalog_version: &c.catalog_version,
        file_digest: file.digest(),
        loaded_by,
        loaded_at: ctx.now(),
    };
    repo::insert_load(ctx.tx(), &load, &items)?;
    // Catalog codes stay out of audit details; the load items name the records.
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "catalog.load_standard_catalog",
            Outcome::Succeeded,
        )
        .record("standard_catalog_load", id)
        .detail(
            "catalog_version",
            AuditDetail::machine_token(&c.catalog_version).map_err(StoreError::from)?,
        )
        .detail(
            "file_digest",
            AuditDetail::sha256(file.digest()).map_err(StoreError::from)?,
        )
        .detail(
            "specimen_types",
            AuditDetail::count(plan.specimen_types.len()),
        )
        .detail("analytes", AuditDetail::count(plan.analytes.len()))
        .detail("methods", AuditDetail::count(plan.methods.len()))
        .detail("orderables", AuditDetail::count(plan.orderables.len()))
        .detail("skipped", AuditDetail::count(plan.skipped)),
    )?;
    Ok(loaded(Some(id), &plan))
}

fn loaded(load_id: Option<StandardCatalogLoadId>, plan: &Plan<'_>) -> StandardCatalogLoaded {
    StandardCatalogLoaded {
        load_id,
        specimen_types: plan.specimen_types.len(),
        analytes: plan.analytes.len(),
        methods: plan.methods.len(),
        orderables: plan.orderables.len(),
        skipped: plan.skipped,
    }
}

/// IDs by code: the laboratory's existing rows plus the rows this load creates.
struct Ids<'a> {
    specimen_types: HashMap<&'a str, SpecimenTypeId>,
    analytes: HashMap<&'a str, AnalyteId>,
    method_versions: HashMap<&'a str, MethodVersionId>,
}

fn lookup<T: Copy>(
    ids: &HashMap<&str, T>,
    code: &str,
    record: &'static str,
) -> Result<T, CatalogError> {
    ids.get(code).copied().ok_or(CatalogError::NotFound(record))
}

/// Creates the planned items through the ordinary commands; returns the
/// created records as (record type, ID).
fn write<'a>(
    ctx: &mut CommandContext<'_>,
    plan: &'a Plan<'a>,
    existing: &'a Existing,
) -> Result<Vec<(&'static str, String)>, CatalogError> {
    let mut items = Vec::new();
    let mut ids = Ids {
        specimen_types: existing
            .specimen_types
            .iter()
            .map(|(code, row)| (code.as_str(), row.id))
            .collect(),
        analytes: existing
            .analytes
            .iter()
            .map(|(code, row)| (code.as_str(), row.id))
            .collect(),
        method_versions: HashMap::new(),
    };
    for s in &plan.specimen_types {
        let input = CodedName {
            code: s.code.clone(),
            name: s.name.clone(),
        };
        let id = create_specimen_type(ctx, input)?;
        ids.specimen_types.insert(&s.code, id);
        items.push(("specimen_type", id.to_string()));
    }
    for a in &plan.analytes {
        let input = CreateAnalyte {
            code: a.code.clone(),
            name: a.name.clone(),
            loinc_code: a.loinc.as_ref().map(|l| l.code.clone()),
            drug_class_code: a.drug_class.clone(),
        };
        let id = create_analyte(ctx, input)?;
        ids.analytes.insert(&a.code, id);
        items.push(("analyte", id.to_string()));
    }
    for m in &plan.methods {
        write_method(ctx, m, &mut ids, &mut items)?;
    }
    for o in &plan.orderables {
        write_orderable(ctx, o, &ids, &mut items)?;
    }
    Ok(items)
}

/// A method with a draft version holding the file's content. Limits the
/// file does not carry stay blank for the laboratory to fill.
fn write_method<'a>(
    ctx: &mut CommandContext<'_>,
    planned: &'a PlannedMethod<'a>,
    ids: &mut Ids<'a>,
    items: &mut Vec<(&'static str, String)>,
) -> Result<(), CatalogError> {
    let m = planned.method;
    let mut analytes = Vec::new();
    for a in &planned.analytes {
        analytes.push(MethodAnalyte {
            reference_interval: None,
            range_policy: Default::default(),
            analyte_id: lookup(&ids.analytes, &a.analyte, "analyte")?,
            unit: a.unit.clone(),
            cutoff: a.cutoff.as_ref().map(|cutoff| cutoff.value.clone()),
            limit_of_detection: None,
            limit_of_quantitation: None,
            reportable_low: None,
            reportable_high: None,
            display_scale: None,
        });
    }
    let content = MethodVersionContent {
        report_notes: None,
        specimen_type_id: lookup(&ids.specimen_types, &m.specimen_type, "specimen_type")?,
        certainty: m.certainty,
        output_scale: m.output_scale,
        threshold_rule: m.threshold_rule,
        threshold_rounding: m.threshold_rounding,
        analytes,
    };
    let method_id = create_method(
        ctx,
        CodedName {
            code: m.code.clone(),
            name: m.name.clone(),
        },
    )?;
    let draft = draft_method_version(ctx, DraftMethodVersion { method_id, content })?;
    ids.method_versions.insert(&m.code, draft.id);
    items.push(("method", method_id.to_string()));
    items.push((MethodVersion::RECORD_TYPE, draft.id.to_string()));
    Ok(())
}

/// A test or panel with a draft version whose components use the method
/// drafts this load created.
fn write_orderable(
    ctx: &mut CommandContext<'_>,
    planned: &PlannedOrderable<'_>,
    ids: &Ids<'_>,
    items: &mut Vec<(&'static str, String)>,
) -> Result<(), CatalogError> {
    let mut components = Vec::new();
    for (component, analytes) in &planned.components {
        let mut analyte_ids = Vec::new();
        for code in analytes {
            analyte_ids.push(lookup(&ids.analytes, code, "analyte")?);
        }
        components.push(ComponentInput {
            method_version_id: lookup(&ids.method_versions, &component.method, "method_version")?,
            purpose: component.purpose,
            analyte_ids,
        });
    }
    let o = planned.orderable;
    let orderable_id = create_orderable(
        ctx,
        CreateOrderable {
            code: o.code.clone(),
            name: o.name.clone(),
            kind: o.kind,
        },
    )?;
    let content = OrderableVersionContent { components };
    let draft = draft_orderable_version(
        ctx,
        DraftOrderableVersion {
            orderable_id,
            content,
        },
    )?;
    items.push(("orderable", orderable_id.to_string()));
    items.push((OrderableVersion::RECORD_TYPE, draft.id.to_string()));
    Ok(())
}
