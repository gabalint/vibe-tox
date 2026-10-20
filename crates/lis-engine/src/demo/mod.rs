//! The demo laboratory's data (proposed REQ-OPS-016; starter data brief, decisions 1, 1a, 3
//! and 6). A demo installation is fixed when its database is created
//! ([`lis_ops::InstallationKind::Demo`]) and lives in its own data folder, which the
//! desktop shell chooses.
//!
//! - **Loader** ([`load`]): in one transaction, through the identity and reporting
//!   commands, so each record gets its capability check and audit event, it sets the
//!   laboratory profile (only when none exists) and creates the file's clients, locations,
//!   delivery destinations, practitioners and patients, acknowledging the identity-hold
//!   candidates the file lists. Loading again creates nothing new: the parties load only
//!   when none of the file's client codes exists, and a partial match is refused.
//! - **Demo cutoffs**: the same transaction fills the blank cutoffs of the caller's own
//!   method drafts with the file's demo values, labeled "demo value, not a recommended
//!   cutoff", through `update_method_draft`; only a draft's author may edit it.
//! - It is refused outside a demo installation, and records one `demo.load_demo_data`
//!   event with the file's digest and the counts.

mod cutoffs;
mod file;
mod load;
mod setup;

use lis_catalog::{CatalogError, DemoValues};
use lis_core::{Denied, ErrorCode};
use lis_identity::IdentityError;
use lis_ops::InstallationKind;
use lis_reporting::ReportingError;
use lis_store::StoreError;

use crate::pipeline::{Failure, ModuleError};
use file::{DemoFile, DemoFileError};

pub use load::DemoDataLoaded;
pub(crate) use load::{LOAD_DEMO_DATA, demo_data_loaded, load};
pub(crate) use setup::complete_setup;

/// Why demo data could not be loaded.
#[derive(Debug, thiserror::Error)]
pub(crate) enum DemoError {
    /// Demo data never enters a laboratory's own database.
    #[error("Demo data loads only in the demo laboratory.")]
    NotDemo,
    /// Some, but not all, of the file's client codes exist, so loading would mix records.
    #[error(
        "Some demo client codes are already in use, so the demo data was not loaded. Reset the demo laboratory to start again."
    )]
    PartlyLoaded,
    #[error(transparent)]
    Auth(#[from] lis_auth::AuthError),
    /// The shipped file is invalid: a build defect.
    #[error(transparent)]
    File(#[from] DemoFileError),
    #[error(transparent)]
    Identity(#[from] IdentityError),
    #[error(transparent)]
    Reporting(#[from] ReportingError),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<lis_audit::AuditError> for DemoError {
    fn from(error: lis_audit::AuditError) -> Self {
        Self::Store(error.into())
    }
}

impl ModuleError for DemoError {
    fn denied(&self) -> Option<Denied> {
        match self {
            Self::Identity(error) => error.denied(),
            Self::Reporting(error) => error.denied(),
            Self::Catalog(error) => error.denied(),
            Self::Store(error) => error.denied(),
            Self::Auth(error) => error.denied(),
            Self::NotDemo | Self::PartlyLoaded | Self::File(_) => None,
        }
    }
}

impl From<DemoError> for Failure {
    fn from(error: DemoError) -> Self {
        match error {
            DemoError::NotDemo | DemoError::PartlyLoaded => {
                Self::refused(ErrorCode::Conflict, &error.to_string())
            }
            DemoError::File(error) => Self::Internal(error.to_string()),
            DemoError::Identity(error) => error.into(),
            DemoError::Reporting(error) => error.into(),
            DemoError::Catalog(error) => error.into(),
            DemoError::Store(error) => error.into(),
            DemoError::Auth(error) => error.into(),
        }
    }
}

/// How the installation marks its report PDFs: the demo laboratory's are watermarked.
pub(crate) fn report_marking(kind: InstallationKind) -> lis_report::Marking {
    match kind {
        InstallationKind::Laboratory => lis_report::Marking::Unmarked,
        InstallationKind::Demo => lis_report::Marking::Demo,
    }
}

/// How the installation marks its specimen labels: the demo laboratory's say "DEMO".
pub(crate) fn label_marking(kind: InstallationKind) -> lis_labels::Marking {
    match kind {
        InstallationKind::Laboratory => lis_labels::Marking::Unmarked,
        InstallationKind::Demo => lis_labels::Marking::Demo,
    }
}

/// The demo values for the Standard Catalog's blank cutoffs in a demo installation, which
/// the Catalog Preview labels "demo value"; empty in a laboratory installation.
pub(crate) fn demo_cutoff_values(kind: InstallationKind) -> DemoValues {
    match kind {
        InstallationKind::Laboratory => DemoValues::new(),
        InstallationKind::Demo => DemoFile::embedded()
            .map(|(file, _)| file.demo_values())
            .unwrap_or_else(|error| {
                tracing::error!(%error, "demo data file unusable");
                DemoValues::new()
            }),
    }
}

#[cfg(test)]
mod tests;
