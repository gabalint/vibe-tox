//! The fixed role catalog, its mapping to capabilities, and who may assign
//! each role. The table in the crate documentation mirrors
//! [`RoleCode::definition`], which is the only place the mapping lives.

use std::collections::BTreeSet;

use lis_core::{Capability, Principal};
use rusqlite::Connection;
use rusqlite::types::{FromSql, FromSqlError, FromSqlResult, ToSql, ToSqlOutput, ValueRef};
use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::error::AuthError;
use crate::repo;

/// Capabilities that act on patients' specimens, results or reports, or on the
/// quality decisions behind them. A role granting any of them is clinical.
const CLINICAL_CAPABILITIES: [Capability; 7] = [
    Capability::Accession,
    Capability::EnterOrders,
    Capability::PerformAnalysis,
    Capability::ReviewResults,
    Capability::ReleaseReports,
    Capability::DeliverReports,
    Capability::ManageQuality,
];

/// A role a user can hold. Roles are fixed in code; laboratory policy decides
/// who holds which (security-and-privacy.md, "Authorization").
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum RoleCode {
    /// Receives specimens and enters orders as supplied.
    Accessioner,
    /// Runs analyses.
    Analyst,
    /// Reviews results technically.
    TechnicalReviewer,
    /// Releases and corrects reports.
    Releaser,
    /// Decides quality matters.
    QualityManager,
    /// Works on billing records.
    Billing,
    /// Reads the audit trail.
    Auditor,
    /// Manages users and approves configuration; no clinical authority.
    Administrator,
}

/// One role's label, description and capabilities.
struct Definition {
    label: &'static str,
    description: &'static str,
    capabilities: &'static [Capability],
}

impl RoleCode {
    /// Every role, in display order.
    pub const ALL: [Self; 8] = [
        Self::Accessioner,
        Self::Analyst,
        Self::TechnicalReviewer,
        Self::Releaser,
        Self::QualityManager,
        Self::Billing,
        Self::Auditor,
        Self::Administrator,
    ];

    /// The stored and serialized code.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Accessioner => "accessioner",
            Self::Analyst => "analyst",
            Self::TechnicalReviewer => "technical_reviewer",
            Self::Releaser => "releaser",
            Self::QualityManager => "quality_manager",
            Self::Billing => "billing",
            Self::Auditor => "auditor",
            Self::Administrator => "administrator",
        }
    }

    /// The capabilities this role grants.
    #[must_use]
    pub fn capabilities(self) -> &'static [Capability] {
        self.definition().capabilities
    }

    fn definition(self) -> Definition {
        use Capability as C;
        let (label, description, capabilities): (_, _, &'static [Capability]) = match self {
            Self::Accessioner => (
                "Accessioner",
                "Receives specimens, enters orders as supplied by the ordering source and \
                 delivers released reports.",
                &[C::Accession, C::EnterOrders, C::DeliverReports],
            ),
            Self::Analyst => ("Analyst", "Runs analyses.", &[C::PerformAnalysis]),
            Self::TechnicalReviewer => (
                "Technical reviewer",
                "Reviews results technically.",
                &[C::ReviewResults],
            ),
            Self::Releaser => (
                "Authorized releaser",
                "Releases, corrects and delivers reports.",
                &[C::ReleaseReports, C::DeliverReports],
            ),
            Self::QualityManager => (
                "Quality manager",
                "Records quality decisions.",
                &[C::ManageQuality],
            ),
            Self::Billing => ("Billing", "Works on billing records.", &[C::Billing]),
            Self::Auditor => ("Auditor", "Reads the audit trail.", &[C::ViewAudit]),
            // Application administration never confers clinical authority. Backups and the
            // exit export are operations duties (M4 brief D14, laboratory question L1); the
            // most sensitive steps also need fresh authentication.
            Self::Administrator => (
                "Administrator",
                "Manages user accounts, drafts and approves configuration, runs backups and restore tests, \
                 and exports the laboratory's records. Grants no clinical authority.",
                &[
                    C::ManageUsers,
                    C::ManageConfiguration,
                    C::ApproveConfiguration,
                    C::BackupRestore,
                    C::Export,
                ],
            ),
        };
        Definition {
            label,
            description,
            capabilities,
        }
    }

    fn parse(code: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|role| role.as_str() == code)
    }

    /// Whether the role grants any clinical capability.
    pub(crate) fn is_clinical(self) -> bool {
        self.capabilities()
            .iter()
            .any(|capability| CLINICAL_CAPABILITIES.contains(capability))
    }

    /// The capability needed to grant or revoke this role (REQ-IDN-004):
    /// `manage_users` for a non-clinical role, `manage_quality` for a clinical
    /// one, so a clinical credential needs an account administrator and a
    /// quality manager. While the laboratory has no active quality manager
    /// (`bootstrap`), `manage_users` may grant that one role, or nobody could.
    pub(crate) fn authority(self, bootstrap: bool) -> Capability {
        if !self.is_clinical() || (bootstrap && self == Self::QualityManager) {
            Capability::ManageUsers
        } else {
            Capability::ManageQuality
        }
    }
}

/// True while no active account holds the quality manager role, the one state
/// in which `manage_users` may grant it.
pub(crate) fn quality_bootstrap(conn: &Connection) -> Result<bool, AuthError> {
    Ok(repo::active_holders(conn, RoleCode::QualityManager, None)? == 0)
}

/// The union of the capabilities `roles` grant.
pub(crate) fn capabilities_of(roles: &[RoleCode]) -> BTreeSet<Capability> {
    roles
        .iter()
        .flat_map(|role| role.capabilities().iter().copied())
        .collect()
}

impl ToSql for RoleCode {
    fn to_sql(&self) -> rusqlite::Result<ToSqlOutput<'_>> {
        Ok(ToSqlOutput::from(self.as_str()))
    }
}

impl FromSql for RoleCode {
    fn column_result(value: ValueRef<'_>) -> FromSqlResult<Self> {
        Self::parse(value.as_str()?).ok_or(FromSqlError::InvalidType)
    }
}

/// A role as shown in user administration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct RoleInfo {
    /// The role.
    pub code: RoleCode,
    /// Short name.
    pub label: String,
    /// What the role is for.
    pub description: String,
    /// Capability codes the role grants.
    pub capabilities: Vec<String>,
    /// Whether `caller` may grant or revoke the role now; the screen offers
    /// only these. The engine checks again on every change.
    pub assignable: bool,
}

/// The role catalog, with what `caller` may assign.
pub fn list_roles(conn: &Connection, caller: &Principal) -> Result<Vec<RoleInfo>, AuthError> {
    let bootstrap = quality_bootstrap(conn)?;
    Ok(RoleCode::ALL
        .into_iter()
        .map(|code| {
            let definition = code.definition();
            RoleInfo {
                code,
                label: definition.label.to_owned(),
                description: definition.description.to_owned(),
                capabilities: definition
                    .capabilities
                    .iter()
                    .map(|capability| capability.as_str().to_owned())
                    .collect(),
                assignable: caller.require(code.authority(bootstrap)).is_ok(),
            }
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn administrator_has_no_clinical_capability() {
        let admin = capabilities_of(&[RoleCode::Administrator]);
        assert_eq!(
            admin,
            BTreeSet::from([
                Capability::ManageUsers,
                Capability::ManageConfiguration,
                Capability::ApproveConfiguration,
                Capability::BackupRestore,
                Capability::Export,
            ])
        );
        for clinical in CLINICAL_CAPABILITIES {
            assert!(!admin.contains(&clinical), "{clinical}");
        }
    }

    #[test]
    fn only_administrators_approve_application_configuration() {
        let approvers: Vec<_> = RoleCode::ALL
            .into_iter()
            .filter(|role| {
                role.capabilities()
                    .contains(&Capability::ApproveConfiguration)
            })
            .collect();
        assert_eq!(approvers, [RoleCode::Administrator]);
    }

    #[test]
    fn accessioners_and_releasers_deliver_reports() {
        let delivering: Vec<RoleCode> = RoleCode::ALL
            .into_iter()
            .filter(|role| role.capabilities().contains(&Capability::DeliverReports))
            .collect();
        assert_eq!(delivering, [RoleCode::Accessioner, RoleCode::Releaser]);
    }

    #[test]
    fn auditor_only_views_the_audit_trail() {
        assert_eq!(
            capabilities_of(&[RoleCode::Auditor]),
            BTreeSet::from([Capability::ViewAudit])
        );
    }

    #[test]
    fn instrument_intake_is_never_a_user_capability() {
        assert!(!capabilities_of(&RoleCode::ALL).contains(&Capability::InstrumentIntake));
    }

    #[test]
    fn codes_round_trip_through_text_and_json() {
        for role in RoleCode::ALL {
            assert_eq!(RoleCode::parse(role.as_str()), Some(role));
            assert_eq!(
                serde_json::to_value(role).unwrap(),
                serde_json::json!(role.as_str())
            );
        }
        assert_eq!(RoleCode::parse("root"), None);
    }

    #[test]
    fn only_billing_auditor_and_administrator_are_not_clinical() {
        let plain: Vec<RoleCode> = RoleCode::ALL
            .into_iter()
            .filter(|role| !role.is_clinical())
            .collect();
        assert_eq!(
            plain,
            [
                RoleCode::Billing,
                RoleCode::Auditor,
                RoleCode::Administrator
            ]
        );
    }

    #[test]
    fn clinical_roles_need_manage_quality_except_the_first_quality_manager() {
        for role in RoleCode::ALL {
            let expected = if role.is_clinical() {
                Capability::ManageQuality
            } else {
                Capability::ManageUsers
            };
            assert_eq!(role.authority(false), expected, "{role:?}");
        }
        assert_eq!(
            RoleCode::QualityManager.authority(true),
            Capability::ManageUsers
        );
        assert_eq!(
            RoleCode::Releaser.authority(true),
            Capability::ManageQuality
        );
    }
}
