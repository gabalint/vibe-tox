//! Interfaces: connections to outside systems, the message log and Unrouted
//! messages (REQ-INT-001–003, REQ-RPT-003, REQ-RPT-011–013, REQ-BIL-002).
//! Every command needs `manage_configuration`; the domain rules live in
//! `lis_interfaces::partner`.
//!
//! A connection's folder comes from a native dialog opened by the desktop
//! shell, never from the UI: [`Engine::choose_partner_folder`] keeps the
//! checked folder for the signed-in session and returns a handle, which the
//! save and test commands take instead of a path.

mod connections;
mod contracts;
mod folder_choices;
mod folders;
mod mappings;
mod messages;
mod mllp;
mod routing;
mod sftp;
mod test_files;

use lis_core::Capability;
use lis_interfaces::partner::PartnerError;

pub use self::connections::{
    CreatePartnerConnectionInput, PartnerConnectionInput, UpdatePartnerConnectionInput,
};
pub(crate) use self::folder_choices::FolderChoices;
pub use self::folders::{ChosenPartnerFolder, FolderTestResult, TestPartnerFolderInput};
pub use self::messages::ExportedMessageLog;
pub use self::mllp::TestPartnerMllpInput;
pub(crate) use self::routing::{
    HandoffError, ReportRoutingError, route_billing_handoff, route_report_revision,
    route_report_withdrawal,
};
pub use self::sftp::TestPartnerSftpInput;
use crate::pipeline::domain_error;
use crate::registry::{Access, CommandSpec, spec};

const MANAGE: Access = Access::Capability(Capability::ManageConfiguration);

pub(crate) const LIST_PARTNER_CONNECTIONS: CommandSpec = spec(
    "list_partner_connections",
    "interfaces.list_connections",
    MANAGE,
);
pub(crate) const GET_PARTNER_CONNECTION: CommandSpec = spec(
    "get_partner_connection",
    "interfaces.get_connection",
    MANAGE,
);
pub(crate) const CHOOSE_PARTNER_FOLDER: CommandSpec =
    spec("choose_partner_folder", "interfaces.choose_folder", MANAGE);
pub(crate) const TEST_PARTNER_FOLDER: CommandSpec =
    spec("test_partner_folder", "interfaces.test_folder", MANAGE);
pub(crate) const TEST_PARTNER_SFTP: CommandSpec =
    spec("test_partner_sftp", "interfaces.test_sftp", MANAGE);
pub(crate) const TEST_PARTNER_MLLP: CommandSpec =
    spec("test_partner_mllp", "interfaces.test_mllp", MANAGE);
pub(crate) const GET_LIS_SFTP_KEY: CommandSpec =
    spec("get_lis_sftp_key", "interfaces.get_sftp_key", MANAGE);
pub(crate) const CREATE_PARTNER_CONNECTION: CommandSpec = spec(
    "create_partner_connection",
    "interfaces.create_connection",
    MANAGE,
);
pub(crate) const UPDATE_PARTNER_CONNECTION: CommandSpec = spec(
    "update_partner_connection",
    "interfaces.update_connection",
    MANAGE,
);
pub(crate) const SET_PARTNER_CONNECTION_ACTIVE: CommandSpec = spec(
    "set_partner_connection_active",
    "interfaces.set_connection_active",
    MANAGE,
);
pub(crate) const RECORD_PARTNER_DESTINATION_CHECK: CommandSpec = spec(
    "record_partner_destination_check",
    "interfaces.verify_destination",
    MANAGE,
);
pub(crate) const SEND_PARTNER_TEST_MESSAGE: CommandSpec = spec(
    "send_partner_test_message",
    "interfaces.send_test_message",
    MANAGE,
);
pub(crate) const LIST_PARTNER_MESSAGES: CommandSpec =
    spec("list_partner_messages", "interfaces.list_messages", MANAGE);
pub(crate) const EXPORT_PARTNER_MESSAGES: CommandSpec = spec(
    "export_partner_messages",
    "interfaces.export_messages",
    MANAGE,
);
pub(crate) const LIST_UNROUTED_PARTNER_MESSAGES: CommandSpec = spec(
    "list_unrouted_partner_messages",
    "interfaces.list_unrouted",
    MANAGE,
);
pub(crate) const RESOLVE_UNROUTED_MESSAGE: CommandSpec = spec(
    "resolve_unrouted_message",
    "interfaces.resolve_unrouted",
    MANAGE,
);
pub(crate) const RESEND_PARTNER_DELIVERY: CommandSpec = spec(
    "resend_partner_delivery",
    "interfaces.resend_delivery",
    MANAGE,
);
pub(crate) const LIST_PARTNER_MAPPINGS: CommandSpec =
    spec("list_partner_mappings", "interfaces.list_mappings", MANAGE);
pub(crate) const DRAFT_PARTNER_MAPPING: CommandSpec =
    spec("draft_partner_mapping", "interfaces.draft_mapping", MANAGE);
pub(crate) const DISCARD_PARTNER_MAPPING_DRAFT: CommandSpec = spec(
    "discard_partner_mapping_draft",
    "interfaces.discard_mapping_draft",
    MANAGE,
);
pub(crate) const APPROVE_PARTNER_MAPPINGS: CommandSpec = spec(
    "approve_partner_mappings",
    "interfaces.approve_mappings",
    MANAGE,
);
pub(crate) const DRAFT_PARTNER_CONTRACT: CommandSpec = spec(
    "draft_partner_contract",
    "interfaces.draft_contract",
    MANAGE,
);
pub(crate) const APPROVE_PARTNER_CONTRACT: CommandSpec = spec(
    "approve_partner_contract",
    "interfaces.approve_contract",
    MANAGE,
);
pub(crate) const RUN_PARTNER_ACCEPTANCE_TESTS: CommandSpec = spec(
    "run_partner_acceptance_tests",
    "interfaces.run_acceptance_tests",
    MANAGE,
);

domain_error!(
    PartnerError,
    PartnerError::field_errors,
    PartnerError::gate_reasons
);
