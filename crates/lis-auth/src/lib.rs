//! Security module: per-person local accounts on the shared host computer,
//! Password/TOTP or personal security keys under laboratory policy, sessions, fixed
//! roles mapped to capabilities, lockout and user administration
//! (REQ-IDN-003, REQ-IDN-004, REQ-OPS-001, NFR-08, NFR-19). Table prefix `auth_`.
//!
//! **Sign-in** uses the account's selected method. A correct password yields a
//! [`ChallengeSlot`] challenge (replace a temporary password, enrol an
//! authenticator, or enter a code); [`complete_sign_in`] starts that session.
//! Security-key accounts require a fresh library-verified assertion and
//! have no password/TOTP fallback. PIN/biometric verification is the default; a
//! lab may opt into touch-only sign-in for new keys. Sensitive actions, first
//! setup and offline recovery always require verification. Every ceremony binds
//! the current laboratory policy plus account
//! and credential revisions; later changes invalidate queued completion.
//! Failed attempts commit their failure count and audit event
//! ([`Attempt`]); consecutive failures per normalized username, known or
//! not, lock it. Wrong usernames, wrong passwords and disabled accounts get
//! the same answer.
//!
//! **Security policy** ([`SecurityPolicy`]): the idle timeout and the lockout
//! (failures and minutes) are versioned, append-only laboratory settings
//! (`manage_users`, NFR-19, L11). Until the laboratory sets them, 15 minutes
//! idle and 5 failures locking for 15 minutes apply. The throttle reads them
//! inside each attempt's transaction.
//!
//! **Separation of duties** ([`SeparationPolicy`], ADR 0015): whether one person
//! may approve configuration they wrote, release a report whose results they
//! reviewed, or verify a delivery destination they created. Versioned,
//! append-only, set with current administrative authority, fresh confirmation and a reason; until
//! the laboratory sets it, each needs a second person. The owning modules call
//! [`SeparationPolicy::authorize_self`], which also needs the person's
//! confirmation for each action, and their schema guards read
//! `auth_v_separation_policy`. A laboratory that one person runs gives its one
//! active account every role, at first-run setup or with
//! [`designate_one_person_laboratory`]: the one case of granting one's own roles.
//!
//! **Installation key checks** ([`has_sealed_secrets`],
//! [`key_opens_sealed_secrets`]) tell the engine whether a key may be created
//! and whether a loaded or restored key matches this database (M4 §3.9).
//!
//! **Fresh authentication** ([`reauthenticate`]): each release or withdrawal
//! checks the selected method inside the command's transaction. Passwords are
//! verified directly; key assertions issue one-use session/purpose-bound receipts
//! retained by the engine's frozen action gateway (REQ-IDN-004, NFR-19).
//!
//! **Optional session PIN.** A six-digit personal PIN may unlock only an existing
//! same-user session, with five persisted attempts. It cannot sign in, switch users,
//! recover an account or confirm a sensitive action. Setup/change/removal requires
//! fresh normal authentication; credential reset/recovery removes it.
//!
//! **Secrets.** Passwords are Argon2id PHC strings (OWASP minimum
//! parameters). PIN verifiers use the same salted Argon2id before encryption.
//! TOTP secrets and PIN verifiers are sealed with purpose-separated associated data and
//! ChaCha20-Poly1305 under a [`DataKey`] the engine keeps in the OS key store.
//! Security keys store public verifier material only. Offline recovery codes
//! are random and Argon2id-hashed; they authorize replacement enrollment only.
//! No password, code, secret or hash appears in audit events, logs or errors.
//!
//! **Roles** are fixed in code ([`RoleCode`]). A user may hold several, or none.
//! A role granting any clinical capability (the first five below) is granted
//! or revoked only with `manage_quality`, other roles with `manage_users`; while
//! no active quality manager exists, `manage_users` may appoint one. Nobody
//! changes their own roles (except a one-person laboratory's one active account,
//! above) or resets their own credentials, and creating an
//! account, changing roles or resetting credentials needs fresh confirmation
//! of the actor's selected method ([`list_roles`] tells the screen what the caller may assign).
//!
//! | Role | Capabilities |
//! | --- | --- |
//! | `accessioner` | `accession`, `enter_orders`, `deliver_reports` |
//! | `analyst` | `perform_analysis` |
//! | `technical_reviewer` | `review_results` |
//! | `releaser` | `release_reports`, `deliver_reports` |
//! | `quality_manager` | `manage_quality` |
//! | `billing` | `billing` |
//! | `auditor` | `view_audit` |
//! | `administrator` | `manage_users`, `manage_configuration`, `approve_configuration`, `backup_restore`, `export` (no clinical capability) |
//!
//! `instrument_intake` belongs to instrument principals only.
//!
//! **Invariants:** grants and sessions are never rewritten or deleted
//! (triggers); at least one active administrator always remains, and so does
//! one active quality manager once appointed; a disabled
//! account has no capabilities; every state change writes a `security`
//! audit event in its transaction. Broader client/site scoping (REQ-IDN-003)
//! remains future work.

mod account;
pub mod actions;
mod admin;
mod challenge;
mod cipher;
mod credentials;
mod duties;
mod error;
mod key_authentication;
mod key_credential;
mod key_policy;
mod key_proofs;
mod key_recovery;
mod key_registration;
mod key_repo;
mod key_secrets;
mod key_verifier;
mod one_person;
mod password;
mod policy;
mod queries;
mod reauthenticate;
mod repo;
mod roles;
mod separation;
mod sessions;
mod setup;
mod sign_in;
mod sign_in_failure;
mod throttle;
mod totp;
mod unlock_pin;
mod unlock_pin_repo;

use lis_core::{Migration, ModuleSchema};

pub use admin::{
    ChangeUserStatusInput, CreateUserInput, ResetUserCredentialsInput, SetUserRolesInput,
    create_user, disable_user, enable_user, reset_user_credentials, set_user_roles,
};
pub use challenge::{
    CHALLENGE_TTL, Challenge, ChallengeId, ChallengeSlot, ChallengeStep, LiveChallenge,
    PasswordAccepted, PendingSecret, SignInStep,
};
pub use cipher::{DataKey, has_sealed_secrets, key_opens_sealed_secrets};
pub use credentials::{
    AuthenticationMethod, AuthenticationStatus, SecurityKeyCredentialView, SecurityKeyId,
    authentication_method, get_authentication_status, list_key_sign_in_choices, list_security_keys,
    revoke_security_key,
};
pub use error::{Attempt, AuthError, Rejection};
pub use key_authentication::{
    KeyAuthenticationAccount, KeyAuthenticationPurpose, complete_key_sign_in, complete_key_unlock,
    prepare_key_authentication, prepare_key_authentication_for_purpose, record_key_failure,
};
pub use key_proofs::issue_key_action_proof;
pub use key_recovery::{complete_key_recovery, prepare_key_recovery, rotate_key_recovery_code};
pub use key_registration::{
    FirstKeyAdministratorInput, KeyEnrollment, KeyRegistrationAccount,
    complete_first_key_registration, complete_key_enrollment_session,
    complete_own_key_registration, prepare_first_key_registration, prepare_own_key_registration,
};
pub use key_verifier::{
    PendingKeyAuthentication, PendingKeyRegistration, SecurityKeyVerifier,
    VerifiedKeyAuthentication, VerifiedKeyRegistration,
};
pub use one_person::{DesignateOnePersonLaboratoryInput, designate_one_person_laboratory};
pub use policy::{
    DEFAULT_SECURITY_POLICY, SecurityPolicy, SecurityPolicyView, SetSecurityPolicyInput,
    current_security_policy, get_security_policy, set_security_policy,
};
pub use queries::{
    SessionUser, SetupStatus, UserSummary, display_names, list_users, session_user, setup_status,
};
pub use reauthenticate::{ReauthPurpose, Reauthentication, reauthenticate};
pub use repo::UserStatus;
pub use roles::{RoleCode, RoleInfo, list_roles};
pub use separation::{
    DEFAULT_SEPARATION_POLICY, Duty, SELF_APPROVAL_FIELD, SelfApprovalRefused, SeparationPolicy,
    SeparationPolicyView, SetSeparationPolicyInput, current_separation_policy,
    get_separation_policy, set_separation_policy,
};
pub use sessions::session_is_open;
pub use sessions::{
    EndReason, LockTrigger, NewSession, end_abandoned_sessions, end_session, record_lock,
    register_workstation,
};
pub use setup::{
    CreateFirstAdministratorInput, complete_demo_setup, create_first_administrator,
    demo_setup_pending_for,
};
pub use sign_in::{
    ChangeInitialPasswordInput, ChangePasswordInput, SignInInput, UnlockSessionInput,
    VerifyMfaInput, change_password, check_password, complete_sign_in, set_initial_password,
    unlock,
};
pub use totp::TotpEnrollment;
pub use unlock_pin::{
    DisableUnlockPinInput, SetUnlockPinInput, UnlockPinStatus, UnlockWithPinInput,
    disable_unlock_pin, get_unlock_pin_status, set_unlock_pin, unlock_with_pin,
};

/// This module's migrations, in version order.
pub const MIGRATIONS: &[Migration] = &[
    Migration {
        version: 1,
        name: "auth",
        sql: include_str!("../migrations/0001_auth.sql"),
    },
    Migration {
        version: 2,
        name: "security_policy",
        sql: include_str!("../migrations/0002_security_policy.sql"),
    },
    Migration {
        version: 3,
        name: "credentials_reset_end",
        sql: include_str!("../migrations/0003_credentials_reset_end.sql"),
    },
    Migration {
        version: 4,
        name: "policy_session_ends",
        sql: include_str!("../migrations/0004_policy_session_ends.sql"),
    },
    Migration {
        version: 5,
        name: "separation_policy",
        sql: include_str!("../migrations/0005_separation_policy.sql"),
    },
    Migration {
        version: 6,
        name: "demo_setup",
        sql: include_str!("../migrations/0006_demo_setup.sql"),
    },
    Migration {
        version: 7,
        name: "security_keys",
        sql: include_str!("../migrations/0007_security_keys.sql"),
    },
    Migration {
        version: 8,
        name: "unlock_pin",
        sql: include_str!("../migrations/0008_unlock_pin.sql"),
    },
    Migration {
        version: 9,
        name: "security_key_policy",
        sql: include_str!("../migrations/0009_security_key_policy.sql"),
    },
];

/// This module's schema descriptor.
pub const SCHEMA: ModuleSchema = ModuleSchema {
    module: "auth",
    table_prefix: "auth_",
    migrations: MIGRATIONS,
};
