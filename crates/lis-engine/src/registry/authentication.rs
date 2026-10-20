//! Authentication, session and administration command policy.

use super::{Access, CommandSpec, spec};
use lis_auth::actions;
use lis_core::Capability;

const MANAGE_USERS: Access = Access::Capability(Capability::ManageUsers);
const VIEW_AUDIT: Access = Access::Capability(Capability::ViewAudit);

pub(crate) const APP_INFO: CommandSpec = spec("app_info", "app.info", Access::Public);
pub(crate) const SETUP_STATUS: CommandSpec =
    spec("setup_status", "auth.setup_status", Access::Public);
pub(crate) const CREATE_FIRST_ADMINISTRATOR: CommandSpec = spec(
    "create_first_administrator",
    actions::FIRST_ADMINISTRATOR_CREATE,
    Access::Public,
);
pub(crate) const SIGN_IN: CommandSpec = spec("sign_in", actions::SIGN_IN, Access::Public);
pub(crate) const CHANGE_INITIAL_PASSWORD: CommandSpec = spec(
    "change_initial_password",
    actions::PASSWORD_CHANGE,
    Access::Public,
);
pub(crate) const VERIFY_MFA: CommandSpec = spec("verify_mfa", actions::MFA_VERIFY, Access::Public);
pub(crate) const CURRENT_SESSION: CommandSpec =
    spec("current_session", "auth.current_session", Access::Public);
pub(crate) const TOUCH_SESSION: CommandSpec =
    spec("touch_session", "auth.session_touch", Access::Session);
pub(crate) const LOCK_SESSION: CommandSpec =
    spec("lock_session", actions::LOCK, Access::AnySession);
pub(crate) const UNLOCK_SESSION: CommandSpec =
    spec("unlock_session", actions::UNLOCK, Access::AnySession);
pub(crate) const GET_UNLOCK_PIN_STATUS: CommandSpec = spec(
    "get_unlock_pin_status",
    "auth.get_unlock_pin_status",
    Access::AnySession,
);
pub(crate) const SET_UNLOCK_PIN: CommandSpec =
    spec("set_unlock_pin", actions::UNLOCK_PIN_SET, Access::Session);
pub(crate) const DISABLE_UNLOCK_PIN: CommandSpec = spec(
    "disable_unlock_pin",
    actions::UNLOCK_PIN_DISABLED,
    Access::Session,
);
pub(crate) const UNLOCK_WITH_PIN: CommandSpec = spec(
    "unlock_with_pin",
    actions::UNLOCK_WITH_PIN,
    Access::AnySession,
);
pub(crate) const SIGN_OUT: CommandSpec = spec("sign_out", actions::SIGN_OUT, Access::AnySession);
pub(crate) const CHANGE_PASSWORD: CommandSpec =
    spec("change_password", actions::PASSWORD_CHANGE, Access::Session);
pub(crate) const LIST_ROLES: CommandSpec = spec("list_roles", "auth.list_roles", MANAGE_USERS);
pub(crate) const LIST_USERS: CommandSpec = spec("list_users", "auth.list_users", MANAGE_USERS);
pub(crate) const CREATE_USER: CommandSpec = spec("create_user", actions::USER_CREATE, MANAGE_USERS);
pub(crate) const SET_USER_ROLES: CommandSpec =
    spec("set_user_roles", actions::USER_ROLES_SET, MANAGE_USERS);
pub(crate) const DISABLE_USER: CommandSpec =
    spec("disable_user", actions::USER_DISABLE, MANAGE_USERS);
pub(crate) const ENABLE_USER: CommandSpec = spec("enable_user", actions::USER_ENABLE, MANAGE_USERS);
pub(crate) const RESET_USER_CREDENTIALS: CommandSpec = spec(
    "reset_user_credentials",
    actions::CREDENTIALS_RESET,
    MANAGE_USERS,
);
pub(crate) const DESIGNATE_ONE_PERSON_LABORATORY: CommandSpec = spec(
    "designate_one_person_laboratory",
    actions::DESIGNATE_ONE_PERSON_LABORATORY,
    MANAGE_USERS,
);
pub(crate) const LIST_AUDIT_EVENTS: CommandSpec =
    spec("list_audit_events", "audit.list_events", VIEW_AUDIT);
pub(crate) const VERIFY_AUDIT_CHAIN: CommandSpec =
    spec("verify_audit_chain", "audit.verify_chain", VIEW_AUDIT);
pub(crate) const LIST_LISTENER_STATUS: CommandSpec = spec(
    "list_listener_status",
    "interfaces.listener_status",
    Access::Public,
);
