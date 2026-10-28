//! Read models of the report layout choice (REQ-RPT-007).

use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use super::ReportUserView;
use crate::layouts::ReportLayout;
use crate::model::{LayoutVersionId, LayoutVersionState};

/// One ready-made layout the laboratory may choose.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportLayoutOption {
    pub layout: ReportLayout,
    pub name: String,
    pub description: String,
    /// The pinned template it renders with.
    pub template_id: String,
    #[ts(type = "number")]
    pub template_version: i64,
}

/// One drafted layout choice and its decision.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutVersionView {
    pub version_id: LayoutVersionId,
    #[ts(type = "number")]
    pub version_no: i64,
    pub layout: ReportLayout,
    pub name: String,
    pub state: LayoutVersionState,
    pub drafted_by: ReportUserView,
    pub drafted_at: UtcTimestamp,
    /// Who approved or rejected it; `None` while a draft.
    pub decided_by: Option<ReportUserView>,
    pub decided_at: Option<UtcTimestamp>,
    pub rejection_reason: Option<String>,
    pub superseded_at: Option<UtcTimestamp>,
    #[ts(type = "number")]
    pub revision: i64,
}

/// The layout candidates use now: the approved version, or the default
/// while none was approved (`version` is `None`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutInForce {
    pub layout: ReportLayout,
    pub name: String,
    pub version: Option<LayoutVersionView>,
}

/// The Report templates page: the choices, the layout in force, the draft
/// waiting for approval and every earlier version. Configuration, no PHI.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ReportLayoutsView {
    pub layouts: Vec<ReportLayoutOption>,
    pub in_force: LayoutInForce,
    pub draft: Option<LayoutVersionView>,
    /// Every version, newest first, including the draft and the active one.
    pub versions: Vec<LayoutVersionView>,
    /// What the caller may do now; the commands still check everything.
    pub allowed_actions: Vec<String>,
}
