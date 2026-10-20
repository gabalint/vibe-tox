//! Typed reporting failures. Display text is PHI-free; gate and conflict
//! codes travel to the UI as gate reasons.

use lis_core::{ErrorCode, FieldError, GateReason};
use lis_store::StoreError;

/// Failure of a reporting command or query.
#[derive(Debug, thiserror::Error)]
pub enum ReportingError {
    /// Invalid command input.
    #[error("Check the highlighted fields.")]
    Invalid(FieldError),
    /// A named record does not exist.
    #[error("The {0} was not found.")]
    NotFound(&'static str),
    /// A stale expected revision, or the report moved on since it was shown.
    #[error("This record changed since it was shown. Reload and try again.")]
    RevisionConflict,
    /// The record's state refuses this action.
    #[error("The {0} is not in a state that allows this action.")]
    WrongState(&'static str),
    /// The candidate is not ready for release.
    #[error("This candidate is not ready for release. Prepare a new candidate.")]
    CandidateNotReady,
    /// The candidate is older than the release window (L5).
    #[error("This candidate has expired. Prepare a new candidate.")]
    CandidateExpired,
    /// The content rebuilt now differs from what the candidate shows.
    #[error(
        "The report content changed since the candidate was prepared. Prepare a new candidate."
    )]
    CandidateStale(Vec<GateReason>),
    /// Only the candidate's signatory may release it.
    #[error("Only the person who prepared this candidate may release it.")]
    NotSignatory,
    /// The report is withdrawn; nothing more may be issued for it.
    #[error("This report was withdrawn.")]
    ReportWithdrawn,
    /// The new content equals the current revision.
    #[error("Nothing changed since the current revision.")]
    NothingChanged,
    /// The candidate's stored file could not be read back intact before
    /// release; an incident is recorded (brief §3.4 row 6).
    #[error("The candidate's report file is missing or damaged; this is a preservation incident.")]
    ArtifactMissing,
    /// An issued file could not be read back intact for export; an incident
    /// is recorded (brief §4.6).
    #[error("The issued report file is missing or damaged; this is a preservation incident.")]
    ArtifactUnavailable,
    /// Stored report content or data no longer matches its digest; the
    /// engine records a preservation incident (NFR-07).
    #[error("The stored report content is damaged; this is a preservation incident.")]
    ContentMismatch(crate::stored::ContentIncident),
    /// Safety gates block the action.
    #[error("This action is blocked by one or more gates.")]
    GateBlocked(Vec<GateReason>),
    /// The revision has open impact; delivery is paused (L13).
    #[error("Delivery is paused until the impact on this revision is assessed.")]
    ImpactOpen,
    /// The obligation, destination or revision already has this record.
    #[error("This {0} already exists.")]
    Duplicate(&'static str),
    /// A signed-in user must act.
    #[error("A signed-in user must perform this action.")]
    UserRequired,
    /// The laboratory profile's time zone is unusable.
    #[error("the laboratory time zone is not available")]
    LaboratoryTimeZone,
    /// Stored data contradicts an invariant.
    #[error("inconsistent stored {0}")]
    Inconsistent(&'static str),
    /// Rendering failed.
    #[error(transparent)]
    Render(#[from] lis_report::RenderError),
    /// Report data could not be serialized.
    #[error(transparent)]
    Data(#[from] lis_report::DataError),
    /// Account lookup or fresh authentication failed.
    #[error(transparent)]
    Auth(#[from] lis_auth::AuthError),
    /// An identity lookup failed.
    #[error(transparent)]
    Identity(#[from] lis_identity::IdentityError),
    /// A catalog lookup failed.
    #[error(transparent)]
    Catalog(#[from] lis_catalog::CatalogError),
    /// An order lookup failed.
    #[error(transparent)]
    Orders(#[from] lis_orders::OrderError),
    /// A specimen lookup failed.
    #[error(transparent)]
    Specimens(#[from] lis_specimens::SpecimenError),
    /// A run-provenance lookup failed.
    #[error(transparent)]
    Analytical(#[from] lis_analytical::AnalyticalError),
    /// A result lookup failed.
    #[error(transparent)]
    Results(#[from] lis_results::ResultError),
    /// Database, audit or authorization failure.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl ReportingError {
    /// Stable UI error classification.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::NotFound(_) | Self::ArtifactUnavailable => ErrorCode::NotFound,
            Self::RevisionConflict
            | Self::WrongState(_)
            | Self::CandidateNotReady
            | Self::CandidateExpired
            | Self::CandidateStale(_)
            | Self::ReportWithdrawn
            | Self::NothingChanged
            | Self::ArtifactMissing
            | Self::ContentMismatch(_)
            | Self::ImpactOpen
            | Self::Duplicate(_) => ErrorCode::Conflict,
            Self::GateBlocked(_) => ErrorCode::GateBlocked,
            Self::NotSignatory | Self::UserRequired => ErrorCode::Forbidden,
            Self::LaboratoryTimeZone | Self::Inconsistent(_) | Self::Render(_) | Self::Data(_) => {
                ErrorCode::Internal
            }
            Self::Auth(error) => error.code(),
            Self::Identity(error) => error.code(),
            Self::Catalog(error) => error.code(),
            Self::Orders(error) => error.code(),
            Self::Specimens(error) => error.code(),
            Self::Results(error) => error.code(),
            Self::Analytical(error) => error.code(),
            Self::Store(error) => error.code(),
        }
    }

    /// Structured input errors.
    #[must_use]
    pub fn field_errors(&self) -> Vec<FieldError> {
        match self {
            Self::Invalid(error) => vec![error.clone()],
            Self::Auth(error) => error.field_errors().to_vec(),
            Self::NotFound(_)
            | Self::RevisionConflict
            | Self::WrongState(_)
            | Self::CandidateNotReady
            | Self::CandidateExpired
            | Self::CandidateStale(_)
            | Self::NotSignatory
            | Self::ReportWithdrawn
            | Self::NothingChanged
            | Self::ArtifactMissing
            | Self::ArtifactUnavailable
            | Self::ContentMismatch(_)
            | Self::GateBlocked(_)
            | Self::ImpactOpen
            | Self::Duplicate(_)
            | Self::UserRequired
            | Self::LaboratoryTimeZone
            | Self::Inconsistent(_)
            | Self::Render(_)
            | Self::Data(_)
            | Self::Identity(_)
            | Self::Catalog(_)
            | Self::Orders(_)
            | Self::Specimens(_)
            | Self::Analytical(_)
            | Self::Results(_)
            | Self::Store(_) => Vec::new(),
        }
    }

    /// Why the action was refused, as stable codes the UI can branch on.
    #[must_use]
    pub fn gate_reasons(&self) -> Vec<GateReason> {
        let single = |code: &str| vec![gate(code, &self.to_string())];
        match self {
            Self::GateBlocked(reasons) => reasons.clone(),
            Self::CandidateStale(reasons) => {
                let mut all = single("candidate_stale");
                all.extend(reasons.iter().cloned());
                all
            }
            Self::CandidateNotReady => single("candidate_not_ready"),
            Self::CandidateExpired => single("candidate_expired"),
            Self::RevisionConflict => single("revision_conflict"),
            Self::ReportWithdrawn => single("report_withdrawn"),
            Self::NothingChanged => single("nothing_changed"),
            Self::ArtifactMissing | Self::ArtifactUnavailable => single("artifact_missing"),
            Self::ContentMismatch(_) => single("stored_content_mismatch"),
            Self::ImpactOpen => single("revision_impact_open"),
            Self::Results(error) => error.gate_reasons().to_vec(),
            Self::Invalid(_)
            | Self::NotFound(_)
            | Self::WrongState(_)
            | Self::NotSignatory
            | Self::Duplicate(_)
            | Self::UserRequired
            | Self::LaboratoryTimeZone
            | Self::Inconsistent(_)
            | Self::Render(_)
            | Self::Data(_)
            | Self::Auth(_)
            | Self::Identity(_)
            | Self::Catalog(_)
            | Self::Orders(_)
            | Self::Specimens(_)
            | Self::Analytical(_)
            | Self::Store(_) => Vec::new(),
        }
    }
}

impl From<rusqlite::Error> for ReportingError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl From<lis_audit::AuditError> for ReportingError {
    fn from(error: lis_audit::AuditError) -> Self {
        Self::Store(error.into())
    }
}

impl From<serde_json::Error> for ReportingError {
    fn from(_: serde_json::Error) -> Self {
        Self::Inconsistent("report content")
    }
}

/// A gate reason with a stable code and PHI-free text.
pub(crate) fn gate(code: &str, message: &str) -> GateReason {
    GateReason {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

pub(crate) fn invalid(field: &str, code: &str, message: &str) -> ReportingError {
    ReportingError::Invalid(FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    })
}

/// Bounded required text from user input, as a field error.
pub(crate) fn required(field: &str, value: &str, max: usize) -> Result<String, ReportingError> {
    lis_core::required_text(field, value, max).map_err(ReportingError::Invalid)
}

/// Bounded optional text from user input, as a field error.
pub(crate) fn optional(
    field: &str,
    value: Option<&str>,
    max: usize,
) -> Result<Option<String>, ReportingError> {
    lis_core::optional_text(field, value, max).map_err(ReportingError::Invalid)
}
