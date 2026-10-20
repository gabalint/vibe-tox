//! Native, offline FIDO2 ceremonies (NFR-19). The engine owns credential policy,
//! challenge validation, authentication, authorization, persistence and audit.

mod ceremony;
mod error;
mod interaction;
mod registration;
mod request;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod usb;
#[cfg(any(target_os = "macos", target_os = "linux"))]
mod usb_token;

pub use ceremony::{authenticate, register};
pub use error::SecurityKeyError;
pub use interaction::{KeyControl, KeyInteraction, KeyStatus, interaction};
