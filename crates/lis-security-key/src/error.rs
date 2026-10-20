use thiserror::Error;

/// PHI-free errors from native hardware interaction; never include a device PIN.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SecurityKeyError {
    /// The caller or native dialog cancelled the operation.
    #[error("Security key operation cancelled.")]
    Cancelled,
    /// A ceremony has a hard limit of sixty seconds, including PIN entry.
    #[error("Security key operation timed out. Try again.")]
    TimedOut,
    /// No readable USB key is connected; OS permissions can cause this too.
    #[error("No accessible security key was found. Insert a key and check device permissions.")]
    NoKey,
    /// Native USB selection is deliberately unambiguous.
    #[error("Connect only the security key you want to use, then try again.")]
    MultipleKeys,
    /// The Windows backend currently accepts one selected credential.
    #[error("Select one registered security key and try again.")]
    SelectCredential,
    /// The connected key does not support the requested FIDO2 ceremony.
    #[error("This key does not support the requested FIDO2 authentication.")]
    UnsupportedKey,
    /// The authenticator rejected the supplied PIN without an automatic retry.
    #[error("The security key PIN was not accepted. Check it before trying again.")]
    InvalidPin,
    /// A key needs to be configured using its supported management software.
    #[error("Set a PIN on this security key using its management software, then try again.")]
    PinNotSet,
    /// A key may require reinsertion or its vendor's recovery procedure.
    #[error("The security key has blocked PIN verification. Follow its recovery instructions.")]
    PinBlocked,
    /// Refuse PIN input unless a live operation explicitly requested it.
    #[error("This operation is not waiting for a PIN.")]
    PinNotRequested,
    /// A selected key does not hold an allowed credential.
    #[error("This security key is not registered for the selected account.")]
    CredentialNotFound,
    /// Engine-generated options need an explicit verification policy and fixed RP.
    #[error("Security key request is invalid.")]
    InvalidRequest,
    /// Native data did not conform to the expected WebAuthn shape or bounds.
    #[error("The security key returned an invalid response.")]
    InvalidResponse,
    /// The build target has no supported native backend.
    #[error("Security key authentication is unavailable on this operating system.")]
    UnsupportedPlatform,
    /// Native libraries can collapse failure or OS-dialog cancellation to this.
    #[error("Security key operation failed or was cancelled in the operating system dialog.")]
    NativeFailure,
}
