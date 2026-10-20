//! Diagnostic logging setup (docs/governance/security-audit-logging.md#diagnostic-logging).
//!
//! JSON lines go to a daily-rotating file in the app log directory; debug builds also
//! print readable lines to stderr. Only `lis_*` crates log at `info` (`debug` in debug
//! builds); dependencies log at `warn` and above, so framework internals such as IPC
//! payloads never reach the file. Log statements carry no PHI, payloads or secrets.

use std::path::Path;

use tracing_appender::rolling::{self, Rotation};
use tracing_subscriber::filter::{LevelFilter, Targets};
use tracing_subscriber::fmt;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

const LOG_FILE_PREFIX: &str = "lis";
const LOG_FILE_SUFFIX: &str = "jsonl";
/// Daily files kept before the oldest is deleted. A lab-configurable retention comes with
/// the operations settings (M4).
const RETAINED_LOG_FILES: usize = 30;

/// Why diagnostic logging could not start; the app does not start without it.
#[derive(Debug, thiserror::Error)]
pub(crate) enum LoggingError {
    #[error("the log file could not be opened")]
    File(#[from] rolling::InitError),
    #[error("a diagnostic logger is already installed")]
    AlreadyInstalled(#[from] tracing_subscriber::util::TryInitError),
}

/// Installs the global subscriber writing to `log_dir`.
pub(crate) fn init(log_dir: &Path) -> Result<(), LoggingError> {
    let file = rolling::Builder::new()
        .rotation(Rotation::DAILY)
        .filename_prefix(LOG_FILE_PREFIX)
        .filename_suffix(LOG_FILE_SUFFIX)
        .max_log_files(RETAINED_LOG_FILES)
        .build(log_dir)?;
    let file_layer = fmt::layer().json().with_writer(file);
    let stderr_layer =
        cfg!(debug_assertions).then(|| fmt::layer().pretty().with_writer(std::io::stderr));
    tracing_subscriber::registry()
        .with(file_layer)
        .with(stderr_layer)
        .with(filter())
        .try_init()?;
    Ok(())
}

/// Targets match by prefix, so `lis` covers every workspace crate (`lis_desktop`, `lis_store`, …).
fn filter() -> Targets {
    let own_level = if cfg!(debug_assertions) {
        LevelFilter::DEBUG
    } else {
        LevelFilter::INFO
    };
    Targets::new()
        .with_target("lis", own_level)
        .with_default(LevelFilter::WARN)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tracing::Level;

    #[test]
    fn workspace_crates_log_at_info() {
        assert!(filter().would_enable("lis_desktop", &Level::INFO));
        assert!(filter().would_enable("lis_store::outbox", &Level::INFO));
    }

    #[test]
    fn dependencies_are_limited_to_warnings() {
        assert!(!filter().would_enable("tauri::ipc", &Level::INFO));
        assert!(!filter().would_enable("wry", &Level::DEBUG));
        assert!(filter().would_enable("tauri", &Level::WARN));
    }
}
