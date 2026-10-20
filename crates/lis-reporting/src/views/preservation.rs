//! Evidence and attributable closure of damaged report records.
use super::ReportUserView;
use crate::model::{
    CandidateId, IncidentResolution, PreservationIncidentId, PreservationIncidentKind, ReportId,
    ReportRevisionId, StoredRecord,
};
use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

/// A preservation incident (NFR-07), open or closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreservationIncidentView {
    pub incident_id: PreservationIncidentId,
    /// 1 while open, 2 once closed.
    #[ts(type = "number")]
    pub revision: i64,
    pub kind: PreservationIncidentKind,
    /// Which stored record failed.
    pub record: StoredRecord,
    /// The digest that record should have.
    pub artifact_sha256: String,
    pub report_id: ReportId,
    pub revision_id: Option<ReportRevisionId>,
    pub candidate_id: Option<CandidateId>,
    /// The withdrawal notice failed.
    pub withdrawal_notice: bool,
    pub detected_at: UtcTimestamp,
    pub closure: Option<IncidentClosureView>,
    pub allowed_actions: Vec<String>,
}

/// How and why an incident was closed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct IncidentClosureView {
    pub resolution: IncidentResolution,
    pub reason: String,
    pub closed_by: ReportUserView,
    pub closed_at: UtcTimestamp,
}
