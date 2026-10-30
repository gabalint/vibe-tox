//! Analyte corrections and retirement (REQ-ANA-002). A correction keeps the
//! analyte's ID and adds a revision with a reason to its immutable history, so
//! methods and tests show the current name while issued reports keep the text
//! they froze. The code is permanent once anything refers to the analyte;
//! results, reports, exports and billing mappings rely on it.

use lis_audit::{AuditEvent, Outcome, Stream};
use lis_core::{Capability, UtcTimestamp};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::commands::lifecycle::acting_user;
use crate::error::{CatalogError, invalid};
use crate::model::{AnalyteId, NAME_MAX, REASON_MAX, VersionStatus, code, optional_code, text};
use crate::repo::analytes::{self as repo, AnalyteFacts, HistoryRow};
use crate::repo::reference::{self as reference, CodeTable};
use crate::views::AnalyteView;
use lis_store::CommandContext;

/// Records the first revision of a new analyte, in its creating transaction.
pub(crate) fn record_registration(
    ctx: &mut CommandContext<'_>,
    id: AnalyteId,
    facts: AnalyteFacts<'_>,
) -> Result<(), CatalogError> {
    let recorded_by = acting_user(ctx.principal())?;
    repo::insert_history(
        ctx.tx(),
        &HistoryRow {
            id,
            revision: 1,
            facts,
            recorded_at: ctx.now(),
            recorded_by,
            reason: None,
            origin: "registered",
        },
    )?;
    Ok(())
}

/// Input of [`update_analyte`]: the complete corrected facts.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct UpdateAnalyte {
    /// The analyte; its ID never changes.
    pub analyte_id: AnalyteId,
    /// Revision displayed before editing.
    #[ts(type = "number")]
    pub expected_revision: i64,
    /// Local code, unique; changeable only while nothing refers to the analyte.
    pub code: String,
    /// Display name.
    pub name: String,
    /// Corrected LOINC code as provided, or null to clear it; not validated against LOINC.
    pub loinc_code: Option<String>,
    /// Corrected drug class code as provided, or null to clear it.
    pub drug_class_code: Option<String>,
    /// Offered for new use; false retires the analyte.
    pub active: bool,
    /// Required explanation of the correction or retirement.
    pub reason: String,
}

/// Corrects or retires an analyte as a new revision with a reason. Refuses a
/// code change once anything refers to the analyte, and retirement while a
/// method version in effect or in draft includes it.
pub fn update_analyte(
    ctx: &mut CommandContext<'_>,
    input: UpdateAnalyte,
) -> Result<AnalyteView, CatalogError> {
    ctx.require(Capability::ManageConfiguration)?;
    let recorded_by = acting_user(ctx.principal())?;
    let old = repo::get(ctx.tx(), input.analyte_id)?.ok_or(CatalogError::NotFound("analyte"))?;
    if old.revision != input.expected_revision {
        return Err(CatalogError::RevisionConflict);
    }
    let code = code("code", &input.code)?;
    let name = text("name", &input.name, NAME_MAX)?;
    let loinc_code = optional_code("loincCode", input.loinc_code.as_deref())?;
    let drug_class_code = optional_code("drugClassCode", input.drug_class_code.as_deref())?;
    let facts = AnalyteFacts {
        code: &code,
        name: &name,
        loinc_code: loinc_code.as_deref(),
        drug_class_code: drug_class_code.as_deref(),
        active: input.active,
    };
    if unchanged(&old, &facts) {
        return Err(invalid(
            "analyte",
            "unchanged",
            "Change at least one value before saving.",
        ));
    }
    let reason = text("reason", &input.reason, REASON_MAX)?;
    if code != old.code {
        if repo::referenced(ctx.tx(), old.id)? {
            return Err(invalid(
                "code",
                "in_use",
                "The code is permanent because records use this analyte. Retire it and add a new analyte instead.",
            ));
        }
        if reference::code_exists(ctx.tx(), CodeTable::Analytes, &code)? {
            return Err(CatalogError::Duplicate("analyte"));
        }
    }
    if old.active && !input.active {
        check_retirable(ctx.tx(), old.id)?;
    }
    let revision = old.revision + 1;
    repo::insert_history(
        ctx.tx(),
        &HistoryRow {
            id: old.id,
            revision,
            facts,
            recorded_at: ctx.now(),
            recorded_by,
            reason: Some(&reason),
            origin: "corrected",
        },
    )?;
    repo::update(ctx.tx(), old.id, revision, &facts)?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "catalog.update_analyte",
            Outcome::Succeeded,
        )
        .record("analyte", old.id)
        .old_revision(old.revision)
        .new_revision(revision)
        .reason(&reason),
    )?;
    repo::get(ctx.tx(), old.id)?.ok_or(CatalogError::NotFound("analyte"))
}

fn unchanged(old: &AnalyteView, new: &AnalyteFacts<'_>) -> bool {
    old.code == new.code
        && old.name == new.name
        && old.loinc_code.as_deref() == new.loinc_code
        && old.drug_class_code.as_deref() == new.drug_class_code
        && old.active == new.active
}

/// A method version in effect or in draft still includes the analyte.
fn check_retirable(conn: &Connection, id: AnalyteId) -> Result<(), CatalogError> {
    let methods = repo::current_methods(conn, id)?;
    if methods.is_empty() {
        return Ok(());
    }
    let names: Vec<String> = methods
        .iter()
        .map(|m| format!("{} version {}", m.method_code, m.version))
        .collect();
    Err(invalid(
        "active",
        "in_use",
        &format!(
            "Remove the analyte from these methods in a new method version first: {}.",
            names.join(", ")
        ),
    ))
}

/// Input of [`analyte_history`] and [`analyte_usage`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AnalyteInput {
    /// The analyte.
    pub analyte_id: AnalyteId,
}

/// One immutable revision of an analyte.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalyteHistoryEntry {
    /// Revision number, from 1.
    #[ts(type = "number")]
    pub revision: i64,
    /// Code at this revision.
    pub code: String,
    /// Name at this revision.
    pub name: String,
    /// LOINC code at this revision.
    pub loinc_code: Option<String>,
    /// Drug class code at this revision.
    pub drug_class_code: Option<String>,
    /// Whether the analyte was offered for new use.
    pub active: bool,
    /// Creation time for legacy baselines, command time otherwise.
    pub recorded_at: UtcTimestamp,
    /// Acting user; absent only for analytes created before history was kept.
    pub recorded_by: Option<String>,
    /// Current display name of the acting user, resolved by the engine.
    #[ts(optional)]
    pub recorded_by_display: Option<String>,
    /// The correction or retirement reason.
    pub reason: Option<String>,
    /// `legacy_baseline`, `registered` or `corrected`.
    pub origin: String,
}

/// Every revision of the analyte, oldest first.
pub fn analyte_history(
    conn: &Connection,
    input: AnalyteInput,
) -> Result<Vec<AnalyteHistoryEntry>, CatalogError> {
    let rows = repo::history(conn, input.analyte_id)?;
    if rows.is_empty() && repo::get(conn, input.analyte_id)?.is_none() {
        return Err(CatalogError::NotFound("analyte"));
    }
    Ok(rows)
}

/// A method version in effect or in draft that includes an analyte.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalyteMethodUse {
    /// The method's code.
    pub method_code: String,
    /// The method's name.
    pub method_name: String,
    /// The version number.
    #[ts(type = "number")]
    pub version: i64,
    /// Approved (in effect) or draft.
    pub status: VersionStatus,
}

/// What a correction of the analyte would affect.
#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnalyteUsage {
    /// Records refer to the analyte, so its code is permanent.
    pub code_in_use: bool,
    /// Method versions in effect or in draft that include it; retirement is
    /// refused while any is listed.
    pub methods: Vec<AnalyteMethodUse>,
}

/// Whether the analyte's code is permanent and which current methods use it.
pub fn analyte_usage(conn: &Connection, input: AnalyteInput) -> Result<AnalyteUsage, CatalogError> {
    if repo::get(conn, input.analyte_id)?.is_none() {
        return Err(CatalogError::NotFound("analyte"));
    }
    Ok(AnalyteUsage {
        code_in_use: repo::referenced(conn, input.analyte_id)?,
        methods: repo::current_methods(conn, input.analyte_id)?,
    })
}
