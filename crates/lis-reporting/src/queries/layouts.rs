//! The report layout choice as the Report templates page shows it (REQ-RPT-007).

use lis_core::{Capability, Principal};
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::layouts::ReportLayout;
use crate::model::LayoutVersionState;
use crate::queries::candidate::user_view;
use crate::repo::layouts::{self, LayoutVersionRow};
use crate::views::{LayoutInForce, LayoutVersionView, ReportLayoutOption, ReportLayoutsView};

/// The ready-made layouts, the layout in force, the open draft and the
/// version history, with what `caller` may do now.
pub fn report_layouts(
    conn: &Connection,
    caller: &Principal,
) -> Result<ReportLayoutsView, ReportingError> {
    let versions = layouts::all(conn)?
        .iter()
        .map(|row| version_view(conn, row))
        .collect::<Result<Vec<_>, _>>()?;
    let active = versions
        .iter()
        .find(|v| v.state == LayoutVersionState::Active)
        .cloned();
    let draft = versions
        .iter()
        .find(|v| v.state == LayoutVersionState::Draft)
        .cloned();
    let layout = active.as_ref().map_or(ReportLayout::DEFAULT, |v| v.layout);
    let mut allowed_actions = Vec::new();
    if draft.is_none() && caller.require(Capability::ManageConfiguration).is_ok() {
        allowed_actions.push("draft_report_layout");
    }
    if draft.is_some() && caller.require(Capability::ManageQuality).is_ok() {
        allowed_actions.extend(["approve_report_layout", "reject_report_layout"]);
    }
    Ok(ReportLayoutsView {
        layouts: ReportLayout::ALL.into_iter().map(option).collect(),
        in_force: LayoutInForce {
            layout,
            name: layout.name().to_owned(),
            version: active,
        },
        draft,
        versions,
        allowed_actions: allowed_actions.into_iter().map(str::to_owned).collect(),
    })
}

fn option(layout: ReportLayout) -> ReportLayoutOption {
    let template = layout.template();
    ReportLayoutOption {
        layout,
        name: layout.name().to_owned(),
        description: layout.description().to_owned(),
        template_id: template.id,
        template_version: i64::from(template.version),
    }
}

fn version_view(
    conn: &Connection,
    row: &LayoutVersionRow,
) -> Result<LayoutVersionView, ReportingError> {
    Ok(LayoutVersionView {
        version_id: row.id,
        version_no: row.version_no,
        layout: row.layout,
        name: row.layout.name().to_owned(),
        state: row.state,
        drafted_by: user_view(conn, row.drafted_by)?,
        drafted_at: row.drafted_at,
        decided_by: row.decided_by.map(|by| user_view(conn, by)).transpose()?,
        decided_at: row.decided_at,
        rejection_reason: row.rejection_reason.clone(),
        superseded_at: row.superseded_at,
        revision: row.revision,
    })
}
