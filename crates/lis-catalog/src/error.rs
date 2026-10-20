//! The crate's error type. `Display` text is PHI-free and names no values.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

use crate::model::OrderableId;

/// Failure of a catalog command or query.
#[derive(Debug, thiserror::Error)]
pub enum CatalogError {
    /// An input field failed validation.
    #[error("invalid {}: {}", .0.field, .0.code)]
    Invalid(FieldError),
    /// The named record does not exist.
    #[error("{0} not found")]
    NotFound(&'static str),
    /// The caller's expected revision is stale.
    #[error("the record changed since it was read")]
    RevisionConflict,
    /// A record of this kind already uses the code.
    #[error("a {0} with this code already exists")]
    Duplicate(&'static str),
    /// Configuration is authored and approved by signed-in people only.
    #[error("configuration changes need a signed-in user")]
    UserRequired,
    /// Only a draft can be edited or approved; approved content is immutable.
    #[error("only a draft version can be edited or approved")]
    NotDraft,
    /// Only the draft's author may edit it, so the approver never edited it.
    #[error("only the author of a draft may edit it")]
    NotDraftAuthor,
    /// The approver authored the draft, and the laboratory's separation-of-duties
    /// policy requires a second person (ADR 0015).
    #[error(
        "you wrote this draft, and the laboratory's separation-of-duties policy requires \
         someone else to approve it; a quality manager can change that policy"
    )]
    SelfApproval,
    /// The parent already has an open draft.
    #[error("a draft version is already open")]
    DraftExists,
    /// Only an approved version can be retired.
    #[error("only an approved version can be retired")]
    NotApproved,
    /// The version is already retired.
    #[error("the version is already retired")]
    AlreadyRetired,
    /// A component no longer references a current approved method version
    /// (REQ-ANA-009 activation completeness).
    #[error("every component must use a current approved method version")]
    IncompleteConfiguration,
    /// Current approved orderables still use this method version.
    #[error("current orderables use this method version: {blocking_orderables:?}")]
    MethodInUse {
        /// Logical orderables whose current versions block the change.
        blocking_orderables: Vec<OrderableId>,
    },
    /// The bundled Standard Catalog file breaks its format; a build defect.
    #[error("the bundled standard catalog is invalid at {}: {}", .0.field, .0.code)]
    StandardFile(FieldError),
    /// The user chose items from another Standard Catalog version than this build's.
    #[error("the standard catalog version changed; reload the list")]
    StandardVersionMismatch,
    /// Codes of the selection are used by different items, or a test's
    /// method is already in the catalog (REQ-OPS-014).
    #[error("these items cannot be loaded: {}", .codes.join(", "))]
    StandardItemsUnavailable {
        /// The refused codes.
        codes: Vec<String>,
    },
    /// Omitting analytes would leave a method or a component with none.
    #[error("omitting these analytes leaves {code} with none")]
    OmittedAllAnalytes {
        /// The method or test code.
        code: String,
    },
    /// Drafts changed, stopped being drafts or appeared after the Catalog
    /// Preview was generated (REQ-OPS-015); generate a new report.
    #[error("the catalog changed after the report was generated: {}", .records.join(", "))]
    CatalogChanged {
        /// The changed or unlisted drafts.
        records: Vec<String>,
    },
    /// The Catalog Preview was already reviewed.
    #[error("this catalog preview was already reviewed")]
    PreviewAlreadyReviewed,
    /// The reviewer authored drafts of the report, and the laboratory's
    /// separation-of-duties policy requires a second person (ADR 0015).
    #[error(
        "you wrote these drafts, and the laboratory's separation-of-duties policy requires \
         someone else to approve them; a quality manager can change that policy: {}",
        .records.join(", ")
    )]
    ReviewerAuthoredDrafts {
        /// The drafts the reviewer authored.
        records: Vec<String>,
    },
    /// The confirmed blank cutoffs differ from the report's blank cutoffs
    /// (REQ-ANA-009: each must be confirmed as not applicable).
    #[error("confirm exactly the blank cutoffs: {}", .records.join(", "))]
    BlankCutoffsUnconfirmed {
        /// Blank cutoffs not confirmed, and confirmations of no blank cutoff.
        records: Vec<String>,
    },
    /// A review would leave current tests or panels on method versions it
    /// replaced (REQ-ANA-009); draft and list the tests too, or retire them.
    #[error("current tests or panels would use replaced method versions: {}", .records.join(", "))]
    OrderablesOnReplacedMethods {
        /// The tests and panels, as "CODE vN".
        records: Vec<String>,
    },
    /// The store failed or refused authorization.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl CatalogError {
    /// The `CommandError` code the engine reports for this error.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_)
            | Self::OmittedAllAnalytes { .. }
            | Self::BlankCutoffsUnconfirmed { .. } => ErrorCode::Validation,
            Self::NotFound(_) => ErrorCode::NotFound,
            Self::UserRequired | Self::NotDraftAuthor => ErrorCode::Forbidden,
            // ADR 0015: the separation-of-duties policy blocks it, not a missing capability.
            Self::SelfApproval
            | Self::ReviewerAuthoredDrafts { .. }
            | Self::IncompleteConfiguration
            | Self::MethodInUse { .. }
            | Self::OrderablesOnReplacedMethods { .. } => ErrorCode::GateBlocked,
            Self::RevisionConflict
            | Self::Duplicate(_)
            | Self::NotDraft
            | Self::DraftExists
            | Self::NotApproved
            | Self::AlreadyRetired
            | Self::StandardVersionMismatch
            | Self::StandardItemsUnavailable { .. }
            | Self::CatalogChanged { .. }
            | Self::PreviewAlreadyReviewed => ErrorCode::Conflict,
            Self::StandardFile(_) => ErrorCode::Internal,
            Self::Store(error) => error.code(),
        }
    }
}

impl From<rusqlite::Error> for CatalogError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

/// A validation error for `field` with a stable code and a PHI-free message.
pub(crate) fn invalid(field: &str, code: &str, message: &str) -> CatalogError {
    CatalogError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}
