//! The module's error, the committed refusals of authentication attempts, and
//! field validation.

use lis_core::{ErrorCode, FieldError};
use lis_store::StoreError;

/// Why an auth command failed. Its transaction rolls back. `Display` is
/// PHI-free and secret-free: for codes other than `internal` it is the message
/// shown to the user.
#[derive(Debug, thiserror::Error)]
pub enum AuthError {
    /// Input fields break a rule; see [`AuthError::field_errors`].
    #[error("Check the highlighted fields.")]
    Invalid(Vec<FieldError>),
    /// `create_first_administrator` after a user exists.
    #[error("The first administrator has already been created.")]
    AlreadySetUp,
    /// Demo preparation belongs to the first account that requested it.
    #[error("Sign in with the account created during demo setup to finish preparing its data.")]
    DemoSetupAccount,
    /// Another account has this username (case-insensitively).
    #[error("This username is already in use.")]
    UsernameTaken,
    /// No account has this ID.
    #[error("The user was not found.")]
    UserNotFound,
    /// `expected_revision` is not the stored revision.
    #[error("Someone else changed this user. Reload and try again.")]
    StaleRevision,
    /// The change would leave no active administrator (lock-out guard).
    #[error("At least one active administrator must remain.")]
    LastAdministrator,
    /// The change would leave no active quality manager, which would let
    /// account administration grant that role alone again.
    #[error("At least one active quality manager must remain.")]
    LastQualityManager,
    /// The expected version is not the security policy version in force.
    #[error("Someone else changed the security policy. Reload and try again.")]
    PolicyVersionConflict,
    /// The expected version is not the separation policy version in force.
    #[error("Someone else changed the separation-of-duties policy. Reload and try again.")]
    SeparationPolicyVersionConflict,
    /// A one-person laboratory is set up only while the actor is its one active account.
    #[error(
        "Only the laboratory's one active account can set it up for one person. Disable the other accounts first."
    )]
    NotOnlyActiveAccount,
    /// The account already has the requested status.
    #[error("The account already has this status.")]
    StatusUnchanged,
    /// The sign-in step is unknown, expired, already used or no longer applies.
    #[error("This sign-in step has expired. Sign in again.")]
    ChallengeExpired,
    /// A key-only account cannot regain password or TOTP access through another API.
    #[error("This account uses a security key. Use its key or offline recovery code.")]
    SecurityKeyRequired,
    /// The selected key is absent, revoked or changed since the ceremony began.
    #[error("This security key is no longer available. Start again with a registered key.")]
    SecurityKeyUnavailable,
    /// Retain a working authenticator until a verified replacement is enrolled.
    #[error("Register another security key before revoking the last active key.")]
    LastSecurityKey,
    /// A credential is already assigned; even revoked credentials cannot be reassigned.
    #[error("This security-key credential is already registered.")]
    SecurityKeyExists,
    /// The verifier rejected registration or an assertion without revealing raw details.
    #[error("Security-key verification failed. Try again with your registered key and PIN.")]
    SecurityKeyVerification,
    /// Verifier setup or stored credential decoding failed.
    #[error("security-key verifier configuration or stored credential is invalid")]
    SecurityKeyConfiguration,
    /// This credential can only unlock an existing active account's open session.
    #[error("This session has ended. Sign in again.")]
    SessionUnavailable,
    /// The user has not enabled the optional session PIN.
    #[error("A personal unlock PIN is not enabled. Use your normal sign-in method.")]
    UnlockPinUnavailable,
    /// The OS random number generator failed.
    #[error("the OS random number generator failed")]
    Random,
    /// Argon2id hashing failed, or a stored hash is malformed.
    #[error("password hashing failed")]
    PasswordHash,
    /// An authentication secret could not be sealed or opened.
    #[error("an authentication secret could not be sealed or opened")]
    SecretCipher,
    /// The data key does not have 32 bytes.
    #[error("the data key has the wrong length")]
    DataKeyLength,
    /// The TOTP parameters or secret were refused.
    #[error("TOTP configuration refused")]
    Totp,
    /// The enrolment QR code could not be rendered.
    #[error("the enrolment QR code could not be rendered")]
    QrCode,
    /// Time arithmetic left the supported range.
    #[error("timestamp out of range")]
    TimeOutOfRange,
    /// A store operation failed, including authorization denials.
    #[error(transparent)]
    Store(#[from] StoreError),
}

impl From<rusqlite::Error> for AuthError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Store(error.into())
    }
}

impl AuthError {
    /// The `CommandError` code the engine reports.
    #[must_use]
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Invalid(_) => ErrorCode::Validation,
            Self::AlreadySetUp
            | Self::DemoSetupAccount
            | Self::UsernameTaken
            | Self::StaleRevision
            | Self::PolicyVersionConflict
            | Self::SeparationPolicyVersionConflict
            | Self::NotOnlyActiveAccount
            | Self::LastAdministrator
            | Self::LastQualityManager
            | Self::SecurityKeyRequired
            | Self::SecurityKeyUnavailable
            | Self::LastSecurityKey
            | Self::SecurityKeyExists
            | Self::StatusUnchanged
            | Self::UnlockPinUnavailable => ErrorCode::Conflict,
            Self::SecurityKeyVerification | Self::SessionUnavailable => ErrorCode::Unauthenticated,
            Self::UserNotFound | Self::ChallengeExpired => ErrorCode::NotFound,
            Self::Store(error) => error.code(),
            Self::Random
            | Self::PasswordHash
            | Self::SecretCipher
            | Self::DataKeyLength
            | Self::Totp
            | Self::QrCode
            | Self::SecurityKeyConfiguration
            | Self::TimeOutOfRange => ErrorCode::Internal,
        }
    }

    /// The field errors of [`AuthError::Invalid`]; empty otherwise.
    #[must_use]
    pub fn field_errors(&self) -> &[FieldError] {
        match self {
            Self::Invalid(errors) => errors,
            _ => &[],
        }
    }
}

/// Why an authentication attempt was refused. Unlike [`AuthError`], a
/// refusal commits: the failure counter and the audit event must persist.
///
/// Wrong usernames, wrong passwords and disabled accounts all give
/// [`Rejection::InvalidCredentials`], so the answer never reveals which factor
/// failed or whether an account exists.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum Rejection {
    /// The username or password is wrong, or the account is disabled.
    #[error("The username or password is incorrect.")]
    InvalidCredentials,
    /// Too many consecutive failures for this username; the laboratory's
    /// security policy sets the duration.
    #[error("Too many failed attempts. Try again later.")]
    LockedOut,
    /// The authenticator code is wrong, or was already used.
    #[error("The code is not valid. Enter the current code from your authenticator app.")]
    InvalidCode,
    /// The registered key could not verify this account or operation.
    #[error("The security key could not verify this account. Start again with a registered key.")]
    InvalidSecurityKey,
    /// Offline recovery material did not authorize credential replacement.
    #[error("The username or recovery code is incorrect, unavailable or already used.")]
    InvalidRecoveryCode,
    /// Wrong or malformed PIN; the remaining budget belongs to this session.
    #[error("The PIN is incorrect.")]
    WrongUnlockPin,
    /// Five failed PIN attempts ended this session; PIN cannot sign in again.
    #[error("Too many incorrect PIN attempts. Sign in again with your normal method.")]
    UnlockPinAttemptsExhausted,
    /// The current password given for a password change is wrong.
    #[error("The current password is incorrect.")]
    WrongCurrentPassword,
    /// The password re-entered to confirm a signed action is wrong.
    #[error("The password is incorrect.")]
    WrongPassword,
    /// Account administration aimed at the acting user's own roles or
    /// credentials; the denial is audited.
    #[error(
        "You cannot change your own roles or reset your own sign-in. Ask another administrator."
    )]
    OwnAccount,
}

impl Rejection {
    /// The `CommandError` code the engine reports.
    #[must_use]
    pub fn code(self) -> ErrorCode {
        match self {
            Self::InvalidCredentials
            | Self::LockedOut
            | Self::InvalidCode
            | Self::InvalidSecurityKey
            | Self::InvalidRecoveryCode
            | Self::UnlockPinAttemptsExhausted => ErrorCode::Unauthenticated,
            // Within a signed-in session: a field problem, not the end of the session.
            Self::WrongCurrentPassword | Self::WrongPassword | Self::WrongUnlockPin => {
                ErrorCode::Validation
            }
            Self::OwnAccount => ErrorCode::Forbidden,
        }
    }

    /// The field the user should correct, if any.
    #[must_use]
    pub fn field_errors(self) -> Vec<FieldError> {
        match self {
            Self::WrongUnlockPin => vec![FieldError {
                field: "pin".into(),
                code: "incorrect".into(),
                message: self.to_string(),
            }],
            Self::WrongCurrentPassword => vec![FieldError {
                field: "currentPassword".into(),
                code: "incorrect".into(),
                message: self.to_string(),
            }],
            Self::WrongPassword => vec![FieldError {
                field: "password".into(),
                code: "incorrect".into(),
                message: self.to_string(),
            }],
            Self::InvalidCredentials
            | Self::LockedOut
            | Self::InvalidCode
            | Self::OwnAccount
            | Self::InvalidSecurityKey
            | Self::InvalidRecoveryCode
            | Self::UnlockPinAttemptsExhausted => Vec::new(),
        }
    }
}

/// The result of an authentication attempt, committed either way.
#[derive(Debug)]
pub enum Attempt<T> {
    /// The attempt succeeded.
    Accepted(T),
    /// The attempt was refused; the failure was recorded.
    Rejected(Rejection),
}

/// A rule an input value breaks.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Problem {
    pub(crate) code: &'static str,
    pub(crate) message: &'static str,
}

/// Collects field problems so that one response lists every invalid field.
#[derive(Debug, Default)]
pub(crate) struct Fields(Vec<FieldError>);

impl Problem {
    /// This problem reported against `field`.
    pub(crate) fn at(self, field: &str) -> FieldError {
        FieldError {
            field: field.to_owned(),
            code: self.code.to_owned(),
            message: self.message.to_owned(),
        }
    }

    /// A validation error for this problem alone, reported against `field`.
    pub(crate) fn invalid(self, field: &str) -> AuthError {
        AuthError::Invalid(vec![self.at(field)])
    }
}

impl Fields {
    /// The parsed value, or `None` after recording the problem against `field`.
    pub(crate) fn check<T>(&mut self, field: &str, result: Result<T, Problem>) -> Option<T> {
        result
            .map_err(|problem| self.0.push(problem.at(field)))
            .ok()
    }

    /// The validation error listing every recorded problem.
    pub(crate) fn into_error(self) -> AuthError {
        AuthError::Invalid(self.0)
    }
}
