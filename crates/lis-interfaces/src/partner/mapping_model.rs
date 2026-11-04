//! Code mappings of an HL7 connection: the partner's code, LOINC and UCUM unit
//! for each LIS test and analyte (REQ-INT-004), and their read models.

use lis_core::{Id, text_enum};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::PartnerConnectionId;
use crate::model::ChangeStamp;

/// Marker for the LIS record a mapping maps: an orderable or an analyte.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LisRecordMarker {}
/// The ID of the LIS orderable or analyte a mapping maps.
pub type LisRecordId = Id<LisRecordMarker>;

text_enum! {
    /// What a code mapping maps.
    #[ts(export)]
    MappingKind {
        /// A LIS test or panel (orderable): the HL7 OBR-4 code.
        Test = "test",
        /// A LIS analyte: the HL7 OBX-3 code.
        Analyte = "analyte",
    }
}

/// The partner's side of one mapping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MappingValues {
    /// The partner's code for the test or analyte.
    pub partner_code: String,
    /// The LOINC code; `None` uses the analyte's LOINC from the catalog, if any.
    #[serde(default)]
    #[ts(optional)]
    pub loinc: Option<String>,
    /// The UCUM unit of an analyte's results. A quantity in another unit is held.
    #[serde(default)]
    #[ts(optional)]
    pub ucum_unit: Option<String>,
    /// For an HL7 Orders connection: the sequence role the partner's contract gives its
    /// order code, `first_line` or `confirmation`; it is never inferred.
    #[serde(default)]
    #[ts(optional)]
    pub sequence_role: Option<String>,
}

/// Input of `draft_partner_mapping`: a change the next approval applies.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DraftPartnerMapping {
    pub connection_id: PartnerConnectionId,
    pub kind: MappingKind,
    /// The LIS orderable or analyte ID.
    pub lis_id: LisRecordId,
    /// `None` removes the mapping.
    pub mapping: Option<MappingValues>,
}

/// A draft with the LIS code and name the engine looked up.
#[derive(Debug, Clone)]
pub struct NewMappingDraft {
    pub input: DraftPartnerMapping,
    pub lis_code: String,
    pub lis_name: String,
}

/// Input of `discard_partner_mapping_draft`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DiscardPartnerMappingDraft {
    pub connection_id: PartnerConnectionId,
    pub kind: MappingKind,
    pub lis_id: LisRecordId,
}

/// Input of `approve_partner_mappings`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApprovePartnerMappings {
    pub connection_id: PartnerConnectionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
    /// The person who drafted a change confirms approving it themselves.
    #[serde(default)]
    pub self_approval: bool,
}

/// A proposed change and who made it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MappingDraft {
    /// `None`: the mapping is removed.
    pub values: Option<MappingValues>,
    pub drafted: ChangeStamp,
}

/// One LIS test or analyte of a connection's code mappings.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MappingRow {
    pub kind: MappingKind,
    pub lis_id: LisRecordId,
    pub lis_code: String,
    pub lis_name: String,
    /// In the approved version; `None` when not mapped.
    pub approved: Option<MappingValues>,
    pub draft: Option<MappingDraft>,
    /// A waiting message needs it and is held until it is approved.
    pub held: bool,
}

/// A connection's code mappings: the approved version, drafts and what is held.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerMappings {
    pub connection_id: PartnerConnectionId,
    /// The approved version in force; `None` before the first approval.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
    pub approved: Option<ChangeStamp>,
    /// Tests first, then analytes, by LIS name.
    pub rows: Vec<MappingRow>,
    /// Commands that apply now.
    pub allowed_actions: Vec<String>,
}

/// One approved mapping, as an HL7 message uses it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeMapping {
    pub kind: MappingKind,
    pub lis_id: LisRecordId,
    pub values: MappingValues,
}

/// The approved version in force and its mappings.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovedMappings {
    pub version: i64,
    pub mappings: Vec<CodeMapping>,
}

impl ApprovedMappings {
    /// The mapping of `lis_id` of `kind`, if approved.
    #[must_use]
    pub fn get(&self, kind: MappingKind, lis_id: LisRecordId) -> Option<&MappingValues> {
        self.mappings
            .iter()
            .find(|mapping| mapping.kind == kind && mapping.lis_id == lis_id)
            .map(|mapping| &mapping.values)
    }
}
