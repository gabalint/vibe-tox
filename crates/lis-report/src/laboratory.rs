//! The performing-laboratory block printed by every report template.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The performing laboratory as printed on reports and notices (REQ-RPT-008).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
pub struct ReportLaboratory {
    /// Laboratory name.
    pub name: String,
    /// Postal address, one line per entry.
    pub address_lines: Vec<String>,
    /// CLIA certificate number.
    pub clia_id: String,
    /// Laboratory director's name.
    pub director: String,
    /// Telephone number as printed.
    pub phone: String,
}
