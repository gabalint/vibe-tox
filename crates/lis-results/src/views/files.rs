//! Read models of result files: the file an import attempt read, the rows it
//! set aside, and measurements kept outside the ordered scope (result files
//! brief §7, F9, F17). Source values remain lexical text.

use lis_analytical::{ExecutionId, RunId};
use lis_catalog::AnalyteId;
use lis_core::{Phi, UserId, UtcTimestamp};
use lis_interfaces::FileOrigin;
use lis_orders::OrderComponentId;
use serde::Serialize;
use ts_rs::TS;

use crate::model::{MeasurementForm, MeasurementId, QuarantineReason, SetAsideKind, SetAsideRowId};

/// A file row an import attempt lists without quarantining it (result files
/// brief F9, F17). Its clinical text is returned only by the audited detail query.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SetAsideRowView {
    pub row_id: SetAsideRowId,
    pub record_path: String,
    pub sample_label: Option<String>,
    pub test_code: Option<String>,
    pub value_text: Option<String>,
    /// The sample-type word as read.
    pub type_word: Option<String>,
    pub kind: SetAsideKind,
    /// The check a not-ordered row failed; none for a batch row not placed.
    pub reason: Option<QuarantineReason>,
}

/// The result file an import attempt read (result files brief §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ImportFileView {
    /// Loaded by hand or picked up from the watched folder.
    pub origin: FileOrigin,
    /// The file's name as received; shown, never logged.
    pub file_name: Phi<String>,
    #[ts(type = "number")]
    pub byte_len: u64,
    /// SHA-256 of the exact bytes.
    pub sha256: Option<String>,
    /// The user who loaded it; absent for a folder pickup.
    pub loaded_by: Option<UserId>,
    /// That user's display name, which the engine adds from the accounts.
    #[serde(skip_serializing_if = "Option::is_none")]
    #[ts(optional)]
    pub loaded_by_display: Option<String>,
}

/// A current measurement no result line uses because its analyte was not
/// ordered for the execution (F17). No demographics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MeasuredNotOrderedView {
    pub measurement_id: MeasurementId,
    pub execution_id: ExecutionId,
    pub run_id: RunId,
    /// The run's instrument name and creation time: how the run list
    /// identifies the run.
    pub instrument_name: String,
    pub run_created_at: UtcTimestamp,
    pub analyte_id: AnalyteId,
    pub analyte_code: String,
    pub analyte_name: String,
    /// The value as received; a word stays a word.
    pub value_text: Option<String>,
    pub units_text: Option<String>,
    pub form: MeasurementForm,
    pub recorded_at: UtcTimestamp,
    /// Accepted components of the same order that `use_measured_result` can
    /// fill with this measurement.
    pub add_on_components: Vec<AddOnComponentView>,
}

/// An accepted order component a kept measurement can fill (F17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AddOnComponentView {
    pub component_id: OrderComponentId,
    /// The code of the orderable the component belongs to.
    pub orderable_code: String,
}
