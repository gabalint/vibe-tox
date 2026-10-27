//! The update commands' failures. Messages are PHI-free; the updater's own error, with
//! addresses and versions only, goes to the diagnostic log.

use lis_core::{CommandError, CorrelationId, ErrorCode};

/// Logs the updater's failure (no PHI: addresses and versions only) and reports it.
pub(super) fn updater_error(error: &tauri_plugin_updater::Error, what: &str) -> CommandError {
    use tauri_plugin_updater::Error;
    let correlation_id = CorrelationId::generate();
    tracing::warn!(%correlation_id, %error, "{what}");
    let message = match error {
        Error::Reqwest(_) | Error::Network(_) => {
            "The update server could not be reached. Check the internet connection and try again."
        }
        Error::Minisign(_)
        | Error::Base64(_)
        | Error::SignatureUtf8(_)
        | Error::SignedVersionMismatch { .. }
        | Error::MissingSignedVersion => {
            "The update package failed its signature check and was not kept. Report this to the project."
        }
        Error::TargetNotFound(_) | Error::TargetsNotFound(_) => {
            "This update has no package for this computer's system."
        }
        Error::UnsupportedOs | Error::UnsupportedArch => {
            "This computer's system cannot install updates from the app."
        }
        _ => "The update could not be checked. Give support the reference shown.",
    };
    CommandError::new(ErrorCode::Unavailable, message, correlation_id)
}

pub(super) fn unavailable() -> CommandError {
    refused("This build cannot install updates. Install a build from the project's release page.")
}

pub(super) fn not_staged() -> CommandError {
    refused("Download this update again before approving or installing it.")
}

pub(super) fn refused(message: &str) -> CommandError {
    CommandError::new(ErrorCode::Conflict, message, CorrelationId::generate())
}

pub(super) fn internal(what: &str) -> CommandError {
    let correlation_id = CorrelationId::generate();
    tracing::error!(%correlation_id, "{what}");
    CommandError::new(
        ErrorCode::Internal,
        "The update could not be prepared. Give support the reference shown.",
        correlation_id,
    )
}
