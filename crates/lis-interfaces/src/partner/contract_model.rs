//! A connection's interface contract and acceptance tests (ticket #4555 step 5a;
//! REQ-INT-001), and their read models: what the partner and the laboratory agreed the
//! messages are, and whether the LIS's synthetic acceptance tests pass against the
//! connection as it is now.

use lis_core::{UtcTimestamp, text_enum};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use super::model::PartnerConnectionId;
use crate::model::ChangeStamp;

text_enum! {
    /// What sits between the LIS and the partner (the edge, REQ-INT-001).
    #[ts(export)]
    ContractEdge {
        /// The LIS writes, reads, connects or listens itself.
        Lis = "lis",
        /// An interface engine the laboratory runs.
        LabEngine = "lab_engine",
    }
}

/// The agreed terms of one connection, as a person recorded them. Free text, PHI-free.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct InterfaceContract {
    /// The message, version and profile, such as HL7 v2.5.1 ORU^R01, LRI Edition 5 based.
    pub profile: String,
    /// What the partner's acknowledgements mean, or how it confirms a file.
    pub acknowledgements: String,
    /// How corrections and withdrawals are sent and applied.
    pub corrections: String,
    pub edge: ContractEdge,
    /// The interface engine and its version, when one sits between.
    #[serde(default)]
    #[ts(optional)]
    pub engine: Option<String>,
    /// Who answers for the interface at the partner and at the laboratory, by role.
    pub owners: String,
    /// Agreed deviations from the profile; empty for none.
    #[serde(default)]
    pub deviations: String,
}

/// Input of `draft_partner_contract`: the next version, until it is approved.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct DraftPartnerContract {
    pub connection_id: PartnerConnectionId,
    pub contract: InterfaceContract,
}

/// Input of `approve_partner_contract`.
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ApprovePartnerContract {
    pub connection_id: PartnerConnectionId,
    #[ts(type = "number")]
    pub expected_revision: i64,
    pub reason: String,
    /// The person who drafted the contract confirms approving it themselves.
    #[serde(default)]
    pub self_approval: bool,
}

text_enum! {
    /// Whether the acceptance tests cover the connection as it is now.
    #[ts(export)]
    AcceptanceState {
        /// The latest run, for the connection as it is now, passed.
        Passed = "passed",
        /// The latest run, for the connection as it is now, failed.
        Failed = "failed",
        /// The contract, the code mappings or the message settings changed since the
        /// last run.
        OutOfDate = "out_of_date",
        /// No run yet.
        NotRun = "not_run",
    }
}

/// One acceptance test case.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AcceptanceCase {
    /// Such as `mapped_order` or `correction`.
    pub code: String,
    pub passed: bool,
    /// Why it failed, as a code; `None` when it passed.
    #[serde(default)]
    pub problem: Option<String>,
}

/// One run of the acceptance tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AcceptanceRun {
    pub ran: ChangeStamp,
    pub passed: bool,
    pub cases: Vec<AcceptanceCase>,
}

/// An approved contract version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContractVersion {
    #[ts(type = "number")]
    pub version: i64,
    pub contract: InterfaceContract,
    pub drafted: ChangeStamp,
    pub approved: ChangeStamp,
    pub reason: String,
    pub self_approval: bool,
}

/// The next version, drafted and waiting for approval.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContractDraft {
    pub contract: InterfaceContract,
    pub drafted: ChangeStamp,
}

/// A connection's contract, its draft and its acceptance tests.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PartnerContract {
    pub connection_id: PartnerConnectionId,
    /// The approved version in force; `None` before the first approval.
    pub current: Option<ContractVersion>,
    pub draft: Option<ContractDraft>,
    pub acceptance: AcceptanceState,
    /// The latest run, for the connection as it is now or as it was.
    pub last_run: Option<AcceptanceRun>,
    /// Commands that apply now.
    pub allowed_actions: Vec<String>,
}

/// What the Connections list and Overview show of the contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ContractSummary {
    /// The approved version in force; `None` before the first approval.
    #[ts(type = "number | null")]
    pub version: Option<i64>,
    pub acceptance: AcceptanceState,
    /// When the latest run ran.
    pub tested_at: Option<UtcTimestamp>,
}
