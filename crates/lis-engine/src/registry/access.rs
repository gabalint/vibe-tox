//! Session and capability requirements shared by command dispatch and authorization tests.

use lis_core::{Capability, Denied, Principal};

/// Who may call a command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    /// Anyone at the sign-in screen; no session needed.
    Public,
    /// A session, locked or not (lock, unlock, sign out).
    AnySession,
    /// An unlocked session.
    Session,
    /// An unlocked session whose user holds the capability. Commands check it
    /// in their domain function; queries in the engine's read path.
    Capability(Capability),
    /// An unlocked session whose user holds either named capability.
    EitherCapability(Capability, Capability),
}

impl Access {
    /// Checks and returns the capability that authorized access, for audit attribution.
    /// Session-only access has no capability; session resolution remains mandatory.
    pub(crate) fn require(self, principal: &Principal) -> Result<Option<Capability>, Denied> {
        match self {
            Self::Capability(capability) => {
                principal.require(capability).map(|()| Some(capability))
            }
            Self::EitherCapability(first, second) => principal
                .require(first)
                .map(|()| Some(first))
                .or_else(|_| principal.require(second).map(|()| Some(second))),
            Self::Public | Self::AnySession | Self::Session => Ok(None),
        }
    }
}
