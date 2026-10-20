//! Approved numeric AU test definitions (REQ-INT-001, REQ-INT-004).

use std::collections::BTreeSet;

use lis_au::AuConfig;
use lis_core::FieldError;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

/// The exact wire configuration and numeric test definitions approved together
/// with the source's analyte map. AU messages carry no units or software identity.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AuSourceProfile {
    /// Must match the connection and each receipt's frozen wire configuration.
    pub protocol: AuConfig,
    /// Only declared numeric assays. Packed LIH and unmapped tests stay held.
    pub tests: Vec<AuTestDefinition>,
}

/// A laboratory-declared numeric assay; the unit is configuration evidence,
/// never a claim that an AU message supplied it.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct AuTestDefinition {
    /// Exact two-digit instrument code, 01–99, matched to the source analyte map.
    pub test_code: String,
    /// Explicit unit of this numeric assay; `None` declares a unitless assay.
    pub unit: Option<String>,
}

impl AuSourceProfile {
    /// Validates the supported wire format and unique numeric declarations.
    pub fn validate(&self) -> Result<(), FieldError> {
        self.protocol.validate().map_err(|_| {
            invalid(
                "auProfile.protocol",
                "unsupported",
                "Check the AU wire settings.",
            )
        })?;
        if self.tests.is_empty() || self.tests.len() > 99 {
            return Err(invalid(
                "auProfile.tests",
                "out_of_range",
                "Declare between one and 99 numeric tests.",
            ));
        }
        let mut seen = BTreeSet::new();
        for (index, test) in self.tests.iter().enumerate() {
            let field = format!("auProfile.tests[{index}].testCode");
            if test.test_code.len() != 2
                || !test.test_code.bytes().all(|b| b.is_ascii_digit())
                || test.test_code == "00"
            {
                return Err(invalid(
                    &field,
                    "invalid",
                    "Use a two-digit code from 01 to 99.",
                ));
            }
            if !seen.insert(&test.test_code) {
                return Err(invalid(
                    &field,
                    "duplicate",
                    "Declare each numeric test once.",
                ));
            }
            let Some(unit) = &test.unit else {
                continue;
            };
            let field = format!("auProfile.tests[{index}].unit");
            if lis_core::required_text(&field, unit, 64)? != *unit {
                return Err(invalid(&field, "untrimmed", "Remove surrounding spaces."));
            }
        }
        Ok(())
    }
}

fn invalid(field: &str, code: &str, message: &str) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    }
}
