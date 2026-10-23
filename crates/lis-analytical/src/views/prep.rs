//! Read models of material lots and preparation batches.

use lis_catalog::MethodVersionId;
use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use crate::model::{LotId, LotState, MaterialKind, PrepBatchId, PrepBatchState, PrepStepId};

/// A material lot. `expired` is derived from `expiresOn` at read time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LotView {
    /// The lot.
    pub lot_id: LotId,
    /// Reagent, calibrator, control or internal standard.
    pub material_kind: MaterialKind,
    /// The product name.
    pub product_name: String,
    /// The manufacturer's lot number.
    pub lot_number: String,
    /// The control or calibrator level, as labelled.
    pub level_text: Option<String>,
    /// The lab-defined sample label of a control or calibrator.
    pub sample_label: Option<String>,
    /// The last day of use, `YYYY-MM-DD`.
    pub expires_on: String,
    /// Whether the expiry date has passed (UTC date).
    pub expired: bool,
    /// Active or withdrawn.
    pub state: LotState,
    /// When it was recorded.
    pub recorded_at: UtcTimestamp,
    /// Optimistic-concurrency revision.
    #[ts(type = "number")]
    pub revision: i64,
}

/// A preparation batch with its steps (REQ-ANA-001).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepBatchView {
    /// The batch.
    pub prep_batch_id: PrepBatchId,
    /// The method version it prepares for.
    pub method_version_id: MethodVersionId,
    /// Open while steps are recorded; completed batches never change.
    pub state: PrepBatchState,
    /// Who created it.
    pub created_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub created_by_display: Option<String>,
    /// When it was created.
    pub created_at: UtcTimestamp,
    /// Who completed it.
    pub completed_by: Option<String>,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub completed_by_display: Option<String>,
    /// When it was completed.
    pub completed_at: Option<UtcTimestamp>,
    /// Optimistic-concurrency revision; each recorded step raises it.
    #[ts(type = "number")]
    pub revision: i64,
    /// The recorded steps, in order.
    pub steps: Vec<PrepStepView>,
}

/// One recorded preparation step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PrepStepView {
    /// The step.
    pub step_id: PrepStepId,
    /// Its order within the batch, from 1.
    #[ts(type = "number")]
    pub seq: i64,
    /// The lab-defined step code.
    pub step_code: String,
    /// The lot used, if any.
    pub lot_id: Option<LotId>,
    /// A note, as entered.
    pub note: Option<String>,
    /// Who performed it.
    pub performed_by: String,
    /// Current display name resolved by the engine from the account owner.
    #[ts(optional)]
    pub performed_by_display: Option<String>,
    /// When it was recorded.
    pub performed_at: UtcTimestamp,
}
