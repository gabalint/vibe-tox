//! Who acts, and what they may do.
//!
//! Authorization is engine-side and deny-by-default: every command calls
//! `require` for the capability it needs before reading gates or changing
//! state. See `docs/architecture/security-and-privacy.md` (Authorization) and
//! `docs/governance/security-audit-logging.md` (Authorization).

use std::collections::BTreeSet;
use std::fmt;

use crate::id::{InstrumentId, SessionId, UserId, WorkstationId};

/// A permission to perform one kind of action.
///
/// The set follows the separate duties in the security design. Application
/// administration (`ManageUsers`, `ManageConfiguration`,
/// `ApproveConfiguration`) never implies clinical authority (`ReviewResults`,
/// `ReleaseReports`), and OS administration confers none of these. New
/// variants are added through the coordinator when a command needs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// Receive specimens and create accessions.
    Accession,
    /// Enter orders as supplied by the ordering source.
    EnterOrders,
    /// Run analyses: runs, positions and analytical data.
    PerformAnalysis,
    /// Technical review of results.
    ReviewResults,
    /// Release and correct reports.
    ReleaseReports,
    /// Deliver released reports: record delivery attempts, add or redirect
    /// delivery obligations and export issued reports (REQ-RPT-003).
    DeliverReports,
    /// QC assessments, deviations and other quality decisions.
    ManageQuality,
    /// Billing records.
    Billing,
    /// Read the audit trail.
    ViewAudit,
    /// Export records out of the application.
    Export,
    /// Create backups and restore them.
    BackupRestore,
    /// User accounts, roles and grants.
    ManageUsers,
    /// Draft configuration changes.
    ManageConfiguration,
    /// Approve configuration versions for use.
    ApproveConfiguration,
    /// Deliver analyzer data into durable receipts.
    InstrumentIntake,
}

impl Capability {
    /// The stable lowercase code recorded in audit events.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accession => "accession",
            Self::EnterOrders => "enter_orders",
            Self::PerformAnalysis => "perform_analysis",
            Self::ReviewResults => "review_results",
            Self::ReleaseReports => "release_reports",
            Self::DeliverReports => "deliver_reports",
            Self::ManageQuality => "manage_quality",
            Self::Billing => "billing",
            Self::ViewAudit => "view_audit",
            Self::Export => "export",
            Self::BackupRestore => "backup_restore",
            Self::ManageUsers => "manage_users",
            Self::ManageConfiguration => "manage_configuration",
            Self::ApproveConfiguration => "approve_configuration",
            Self::InstrumentIntake => "instrument_intake",
        }
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A refused authorization check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("capability required: {capability}")]
pub struct Denied {
    /// The capability the principal lacks.
    pub capability: Capability,
}

/// The kind and identity of whoever acts.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Actor {
    /// A person signed in at the application's own sign-in screen.
    User {
        /// The user account.
        user_id: UserId,
        /// The signed-in session.
        session_id: SessionId,
        /// The computer the session runs on.
        workstation_id: WorkstationId,
    },
    /// An analyzer listener, never whoever happens to be signed in.
    Instrument {
        /// The configured instrument.
        instrument_id: InstrumentId,
    },
    /// An engine component such as an outbox worker.
    System {
        /// A fixed, code-defined component name, such as `outbox-worker`.
        component: &'static str,
    },
}

impl Actor {
    /// Stable stored kind of this actor.
    #[must_use]
    pub fn kind(&self) -> &'static str {
        match self {
            Self::User { .. } => "user",
            Self::Instrument { .. } => "instrument",
            Self::System { .. } => "system",
        }
    }

    /// Canonical stored actor identifier: a typed ID or fixed system component name.
    #[must_use]
    pub fn id_text(&self) -> String {
        match self {
            Self::User { user_id, .. } => user_id.to_string(),
            Self::Instrument { instrument_id } => instrument_id.to_string(),
            Self::System { component } => (*component).to_owned(),
        }
    }
}

/// An actor with its resolved capability set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    actor: Actor,
    capabilities: BTreeSet<Capability>,
}

impl Principal {
    /// A signed-in user holding the capabilities their roles resolve to.
    pub fn user(
        user_id: UserId,
        session_id: SessionId,
        workstation_id: WorkstationId,
        capabilities: impl IntoIterator<Item = Capability>,
    ) -> Self {
        let actor = Actor::User {
            user_id,
            session_id,
            workstation_id,
        };
        Self::new(actor, capabilities)
    }

    /// An instrument listener. It holds only [`Capability::InstrumentIntake`].
    #[must_use]
    pub fn instrument(instrument_id: InstrumentId) -> Self {
        Self::new(
            Actor::Instrument { instrument_id },
            [Capability::InstrumentIntake],
        )
    }

    /// An engine component acting on its own behalf.
    pub fn system(
        component: &'static str,
        capabilities: impl IntoIterator<Item = Capability>,
    ) -> Self {
        Self::new(Actor::System { component }, capabilities)
    }

    fn new(actor: Actor, capabilities: impl IntoIterator<Item = Capability>) -> Self {
        Self {
            actor,
            capabilities: capabilities.into_iter().collect(),
        }
    }

    /// Who acts.
    #[must_use]
    pub fn actor(&self) -> &Actor {
        &self.actor
    }

    /// Succeeds only when the principal holds `capability`.
    pub fn require(&self, capability: Capability) -> Result<(), Denied> {
        if self.capabilities.contains(&capability) {
            Ok(())
        } else {
            Err(Denied { capability })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn user(capabilities: &[Capability]) -> Principal {
        Principal::user(
            UserId::generate(),
            SessionId::generate(),
            WorkstationId::generate(),
            capabilities.iter().copied(),
        )
    }

    #[test]
    fn require_grants_held_capabilities() {
        let principal = user(&[Capability::Accession]);
        assert_eq!(principal.require(Capability::Accession), Ok(()));
    }

    #[test]
    fn require_denies_missing_capabilities() {
        let principal = user(&[Capability::Accession]);
        assert_eq!(
            principal.require(Capability::ReleaseReports),
            Err(Denied {
                capability: Capability::ReleaseReports
            })
        );
        assert_eq!(
            Denied {
                capability: Capability::ReleaseReports
            }
            .to_string(),
            "capability required: release_reports"
        );
    }

    #[test]
    fn administration_does_not_imply_clinical_authority() {
        let admin = user(&[
            Capability::ManageUsers,
            Capability::ManageConfiguration,
            Capability::ApproveConfiguration,
        ]);
        assert!(admin.require(Capability::ReviewResults).is_err());
        assert!(admin.require(Capability::ReleaseReports).is_err());
    }

    #[test]
    fn instruments_hold_only_intake() {
        let instrument = Principal::instrument(InstrumentId::generate());
        assert_eq!(instrument.require(Capability::InstrumentIntake), Ok(()));
        assert!(instrument.require(Capability::ReviewResults).is_err());
        assert!(instrument.require(Capability::Accession).is_err());
    }

    #[test]
    fn actors_share_one_stable_kind_and_identifier_mapping() {
        let user_id = UserId::generate();
        let user = Actor::User {
            user_id,
            session_id: SessionId::generate(),
            workstation_id: WorkstationId::generate(),
        };
        let instrument_id = InstrumentId::generate();
        let instrument = Actor::Instrument { instrument_id };
        let system = Actor::System {
            component: "outbox-worker",
        };
        assert_eq!((user.kind(), user.id_text()), ("user", user_id.to_string()));
        assert_eq!(
            (instrument.kind(), instrument.id_text()),
            ("instrument", instrument_id.to_string())
        );
        assert_eq!(
            (system.kind(), system.id_text()),
            ("system", "outbox-worker".to_owned())
        );
    }
}
