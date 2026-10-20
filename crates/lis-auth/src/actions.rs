//! Audit action names this module writes. The engine's command registry uses
//! the same constants for its log lines and denial events.

/// A password step: failures and lockouts. Success is recorded by [`SIGN_IN`] after MFA.
pub const SIGN_IN: &str = "auth.sign_in";
/// A second-factor step: failures, and the sign-in when it succeeds.
pub const MFA_VERIFY: &str = "auth.mfa_verify";
/// An authenticator enrolment confirmed by its first code.
pub const MFA_ENROLL: &str = "auth.mfa_enroll";
/// A username locked after too many consecutive failures.
pub const LOCKOUT: &str = "auth.lockout";
/// A session locked by the user or by the idle timeout.
pub const LOCK: &str = "auth.lock";
/// A locked session unlocked with the same user's password.
pub const UNLOCK: &str = "auth.unlock";
/// The signed-in user's password re-entered to confirm a signed action, such
/// as a report release (REQ-IDN-004, NFR-19).
pub const REAUTHENTICATE: &str = "auth.reauthenticate";
/// A session ended by its user.
pub const SIGN_OUT: &str = "auth.sign_out";
/// A session ended by a user switch, app exit or crash recovery.
pub const SESSION_END: &str = "auth.session_end";
/// A password changed by its owner, including the replacement of a temporary password.
pub const PASSWORD_CHANGE: &str = "auth.password_change";
/// The first administrator created at first-run setup.
pub const FIRST_ADMINISTRATOR_CREATE: &str = "auth.first_administrator_create";
/// A user account created by an administrator.
pub const USER_CREATE: &str = "auth.user_create";
/// A user's roles changed.
pub const USER_ROLES_SET: &str = "auth.user_roles_set";
/// A user account disabled.
pub const USER_DISABLE: &str = "auth.user_disable";
/// A user account enabled again.
pub const USER_ENABLE: &str = "auth.user_enable";
/// A temporary password set, and optionally the authenticator removed, by an administrator.
pub const CREDENTIALS_RESET: &str = "auth.credentials_reset";
/// This computer registered as a workstation on first start.
pub const WORKSTATION_REGISTER: &str = "auth.workstation_register";
/// A new security policy version: idle timeout and lockout (NFR-19).
pub const SET_SECURITY_POLICY: &str = "auth.set_security_policy";
/// A new separation-of-duties policy version (ADR 0015).
pub const SET_SEPARATION_POLICY: &str = "auth.set_separation_policy";
/// The one active account set up as a one-person laboratory (ADR 0015); it writes
/// [`USER_ROLES_SET`] and [`SET_SEPARATION_POLICY`].
pub const DESIGNATE_ONE_PERSON_LABORATORY: &str = "auth.designate_one_person_laboratory";

/// An optional personal session PIN set or replaced after fresh authentication.
pub const UNLOCK_PIN_SET: &str = "auth.unlock_pin_set";
/// The personal PIN removed explicitly or during credential reset/recovery.
pub const UNLOCK_PIN_DISABLED: &str = "auth.unlock_pin_disabled";
/// A same-user session PIN attempt, including failures and budget exhaustion.
pub const UNLOCK_WITH_PIN: &str = "auth.unlock_with_pin";
