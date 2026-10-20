//! Runtime operations commands (REQ-OPS-002, 006, 008, 013; NFR-19, NFR-24, NFR-26; M4
//! design brief §2.4, §6, §7, §8; contract "Health and runs", "Settings", "Updates"):
//! host health and its history, attestations, app runs and listening gaps, operations
//! settings and the security policy, update records, media disposals, and the
//! installation key's state and restore.
//!
//! Every write checks its capability in the domain function and audits in the same
//! transaction; update approval and version acceptance re-authenticate inside it. The
//! package file is read from the path an OS dialog returned, outside any transaction,
//! and only its name and SHA-256 are recorded.

mod health;
mod keys;
mod settings;
mod updates;

use lis_auth::AuthError;
use lis_core::{Capability, ErrorCode, FieldError};
use lis_ops::RuntimeError;
use lis_store::StoreError;

use crate::pipeline::{Failure, ModuleError, domain_error};
use crate::registry::{Access, CommandSpec, spec};

pub use health::{
    AcknowledgeListeningGapInput, AttestHealthCheckInput, ListAppRunsInput, ListHealthHistoryInput,
};
pub use keys::RestoreInstallationKeyInput;
pub(crate) use keys::reconcile_key_restores;
pub use updates::{AcceptInstalledVersionInput, RecordUpdateApprovalInput};

const CONFIGURE: Access = Access::Capability(Capability::ManageConfiguration);

pub(crate) const GET_HOST_HEALTH: CommandSpec =
    spec("get_host_health", "ops.host_health", Access::Session);
pub(crate) const REFRESH_HOST_HEALTH: CommandSpec = spec(
    "refresh_host_health",
    "ops.refresh_host_health",
    Access::Session,
);
pub(crate) const LIST_HEALTH_HISTORY: CommandSpec = spec(
    "list_health_history",
    "ops.list_health_history",
    Access::Session,
);
pub(crate) const ATTEST_HEALTH_CHECK: CommandSpec = spec(
    "attest_health_check",
    lis_ops::health::ATTEST_CHECK,
    CONFIGURE,
);
pub(crate) const LIST_APP_RUNS: CommandSpec =
    spec("list_app_runs", "ops.list_app_runs", Access::Session);
pub(crate) const ACKNOWLEDGE_LISTENING_GAP: CommandSpec = spec(
    "acknowledge_listening_gap",
    lis_ops::runs::ACKNOWLEDGE_GAP,
    Access::Capability(Capability::PerformAnalysis),
);
pub(crate) const GET_OPERATIONS_SETTINGS: CommandSpec =
    spec("get_operations_settings", "ops.get_settings", CONFIGURE);
pub(crate) const SET_OPERATIONS_SETTINGS: CommandSpec = spec(
    "set_operations_settings",
    lis_ops::settings::SET_SETTINGS,
    CONFIGURE,
);
pub(crate) const GET_SECURITY_POLICY: CommandSpec = spec(
    "get_security_policy",
    "auth.get_security_policy",
    Access::Capability(Capability::ManageUsers),
);
pub(crate) const SET_SECURITY_POLICY: CommandSpec = spec(
    "set_security_policy",
    lis_auth::actions::SET_SECURITY_POLICY,
    Access::Capability(Capability::ManageUsers),
);
pub(crate) const GET_SEPARATION_POLICY: CommandSpec = spec(
    "get_separation_policy",
    "auth.get_separation_policy",
    Access::EitherCapability(Capability::ApproveConfiguration, Capability::ManageQuality),
);
pub(crate) const SET_SEPARATION_POLICY: CommandSpec = spec(
    "set_separation_policy",
    lis_auth::actions::SET_SEPARATION_POLICY,
    Access::Capability(Capability::ApproveConfiguration),
);
pub(crate) const LIST_INSTALLED_VERSIONS: CommandSpec = spec(
    "list_installed_versions",
    "ops.list_installed_versions",
    Access::Session,
);
pub(crate) const LIST_UPDATE_APPROVALS: CommandSpec = spec(
    "list_update_approvals",
    "ops.list_update_approvals",
    CONFIGURE,
);
pub(crate) const RECORD_UPDATE_APPROVAL: CommandSpec = spec(
    "record_update_approval",
    lis_ops::updates::APPROVE_UPDATE,
    CONFIGURE,
);
pub(crate) const WITHDRAW_UPDATE_APPROVAL: CommandSpec = spec(
    "withdraw_update_approval",
    lis_ops::updates::WITHDRAW_UPDATE_APPROVAL,
    CONFIGURE,
);
pub(crate) const ACCEPT_INSTALLED_VERSION: CommandSpec = spec(
    "accept_installed_version",
    lis_ops::updates::ACCEPT_VERSION,
    Access::Capability(Capability::ApproveConfiguration),
);
pub(crate) const RECORD_MEDIA_DISPOSAL: CommandSpec = spec(
    "record_media_disposal",
    lis_ops::disposals::RECORD_MEDIA_DISPOSAL,
    Access::Capability(Capability::BackupRestore),
);
pub(crate) const LIST_MEDIA_DISPOSALS: CommandSpec = spec(
    "list_media_disposals",
    "ops.list_media_disposals",
    Access::Capability(Capability::BackupRestore),
);
pub(crate) const GET_INSTALLATION_KEY_STATE: CommandSpec = spec(
    "get_installation_key_state",
    "auth.installation_key_state",
    Access::Public,
);
pub(crate) const RESTORE_INSTALLATION_KEY: CommandSpec = spec(
    "restore_installation_key",
    keys::RESTORE_INSTALLATION_KEY_ACTION,
    Access::Public,
);

domain_error!(RuntimeError, |error: &RuntimeError| error
    .field_errors()
    .to_vec());

/// A runtime command that also re-authenticates or changes the security policy.
#[derive(Debug, thiserror::Error)]
enum OperationsError {
    #[error(transparent)]
    Auth(#[from] AuthError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl From<StoreError> for OperationsError {
    fn from(error: StoreError) -> Self {
        Self::Runtime(error.into())
    }
}

impl ModuleError for OperationsError {
    fn denied(&self) -> Option<lis_core::Denied> {
        match self {
            Self::Auth(error) => ModuleError::denied(error),
            Self::Runtime(error) => ModuleError::denied(error),
        }
    }
}

impl From<OperationsError> for Failure {
    fn from(error: OperationsError) -> Self {
        match error {
            OperationsError::Auth(error) => error.into(),
            OperationsError::Runtime(error) => error.into(),
        }
    }
}

/// A validation refusal of one input field.
fn invalid(field: &str, code: &str, message: &str) -> Failure {
    Failure::Refused {
        code: ErrorCode::Validation,
        message: "Check the highlighted fields.".to_owned(),
        field_errors: vec![FieldError {
            field: field.to_owned(),
            code: code.to_owned(),
            message: message.to_owned(),
        }],
        gate_reasons: Vec::new(),
    }
}
