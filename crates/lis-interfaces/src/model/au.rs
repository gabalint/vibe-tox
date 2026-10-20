//! The qualified AU wire profile and bounded receive timers.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::InterfaceError;

/// Settings copied from the AU400/AU640 online parameter sheet (REQ-INT-009).
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AuConnectionSettings {
    /// Exact framing and field-width settings; never detected from patient data.
    pub protocol: lis_au::AuConfig,
    /// T2: time allowed for one frame, from its first byte (100–9900 ms).
    pub frame_timeout_ms: u32,
    /// T1/T7: wait for the next block of an unfinished sample (100–9900 ms).
    /// This does not time out an idle real-time analyzer between samples.
    pub block_timeout_ms: u32,
}

impl AuConnectionSettings {
    pub(crate) fn validate(&self) -> Result<(), InterfaceError> {
        self.protocol
            .validate()
            .map_err(|_| InterfaceError::InvalidAuConfiguration)?;
        if ![self.frame_timeout_ms, self.block_timeout_ms]
            .iter()
            .all(|value| (100..=9900).contains(value) && value % 100 == 0)
        {
            return Err(InterfaceError::InvalidAuConfiguration);
        }
        Ok(())
    }
}
