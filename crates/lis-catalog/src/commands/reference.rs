//! Reference data: specimen types, analytes, methods and orderables. These
//! rows are immutable once created; their meaning becomes usable only
//! through an approved method or orderable version.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::model::{
    AnalyteId, MethodId, NAME_MAX, OrderableId, OrderableKind, SpecimenTypeId, code, optional_code,
    text,
};
use crate::repo::reference::{self as repo, CodeTable, NewAnalyte};

/// Input of [`create_specimen_type`] and [`create_method`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export, rename = "CatalogCodedName")]
pub struct CodedName {
    /// Lab-defined code, unique within its kind.
    pub code: String,
    /// Display name.
    pub name: String,
}

/// Checks capability, code, name and uniqueness; returns the code and name.
fn begin_create(
    ctx: &mut CommandContext<'_>,
    table: CodeTable,
    record: &'static str,
    code_input: &str,
    name_input: &str,
) -> Result<(String, String), CatalogError> {
    ctx.require(Capability::ManageConfiguration)?;
    let code = code("code", code_input)?;
    let name = text("name", name_input, NAME_MAX)?;
    if repo::code_exists(ctx.tx(), table, &code)? {
        return Err(CatalogError::Duplicate(record));
    }
    Ok((code, name))
}

fn audit_created(
    ctx: &CommandContext<'_>,
    action: &'static str,
    record: &'static str,
    id: impl std::fmt::Display,
) -> Result<(), CatalogError> {
    ctx.audit(AuditEvent::new(Stream::Clinical, action, Outcome::Succeeded).record(record, id))?;
    Ok(())
}

/// Creates a lab-defined specimen type.
pub fn create_specimen_type(
    ctx: &mut CommandContext<'_>,
    input: CodedName,
) -> Result<SpecimenTypeId, CatalogError> {
    let (code, name) = begin_create(
        ctx,
        CodeTable::SpecimenTypes,
        "specimen type",
        &input.code,
        &input.name,
    )?;
    let id = SpecimenTypeId::generate();
    repo::insert_specimen_type(ctx.tx(), id, &code, &name, ctx.now())?;
    audit_created(ctx, "catalog.create_specimen_type", "specimen_type", id)?;
    Ok(id)
}

/// Creates a method; its content is added as versions.
pub fn create_method(
    ctx: &mut CommandContext<'_>,
    input: CodedName,
) -> Result<MethodId, CatalogError> {
    let (code, name) = begin_create(ctx, CodeTable::Methods, "method", &input.code, &input.name)?;
    let id = MethodId::generate();
    repo::insert_method(ctx.tx(), id, &code, &name, ctx.now())?;
    audit_created(ctx, "catalog.create_method", "method", id)?;
    Ok(id)
}

/// Input of [`create_analyte`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateAnalyte {
    /// Local code, unique.
    pub code: String,
    /// Display name.
    pub name: String,
    /// LOINC code as provided by the laboratory; not validated against LOINC.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub loinc_code: Option<String>,
    /// Drug class code as provided by the laboratory.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub drug_class_code: Option<String>,
}

/// Creates a lab-defined analyte.
pub fn create_analyte(
    ctx: &mut CommandContext<'_>,
    input: CreateAnalyte,
) -> Result<AnalyteId, CatalogError> {
    let (code, name) = begin_create(
        ctx,
        CodeTable::Analytes,
        "analyte",
        &input.code,
        &input.name,
    )?;
    let loinc_code = optional_code("loincCode", input.loinc_code.as_deref())?;
    let drug_class_code = optional_code("drugClassCode", input.drug_class_code.as_deref())?;
    let id = AnalyteId::generate();
    repo::insert_analyte(
        ctx.tx(),
        &NewAnalyte {
            id,
            code: &code,
            name: &name,
            loinc_code: loinc_code.as_deref(),
            drug_class_code: drug_class_code.as_deref(),
            created_at: ctx.now(),
        },
    )?;
    audit_created(ctx, "catalog.create_analyte", "analyte", id)?;
    Ok(id)
}

/// Input of [`create_orderable`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct CreateOrderable {
    /// Code, unique.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Test or panel.
    pub kind: OrderableKind,
}

/// Creates a test or panel; its manifest is added as versions.
pub fn create_orderable(
    ctx: &mut CommandContext<'_>,
    input: CreateOrderable,
) -> Result<OrderableId, CatalogError> {
    let (code, name) = begin_create(
        ctx,
        CodeTable::Orderables,
        "orderable",
        &input.code,
        &input.name,
    )?;
    let id = OrderableId::generate();
    repo::insert_orderable(ctx.tx(), id, &code, &name, input.kind, ctx.now())?;
    audit_created(ctx, "catalog.create_orderable", "orderable", id)?;
    Ok(id)
}
