//! Engine startup state and the shell-only `startup_status` command (M4 design brief D11,
//! §6.4). A start failure no longer aborts the app without a window: the shell keeps the
//! PHI-free failure code and the data folder, and the startup-failure screen shows them
//! with guidance. Every other command then finds no engine and fails.

use std::path::PathBuf;

use lis_engine::InstallationKind;
use serde::Serialize;
use tauri::State;
use ts_rs::TS;

use crate::installation::Installation;

/// Whether the engine started.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum StartupState {
    /// The engine runs.
    Running,
    /// The engine did not start; `code` is [`lis_engine::EngineError::code`], or
    /// `internal` when the data folder could not be resolved.
    Failed {
        code: &'static str,
        data_folder: Option<PathBuf>,
    },
}

/// `startup_status()` answers this, even when the engine did not start.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub(crate) struct StartupStatus {
    /// `running` or `failed`.
    state: &'static str,
    /// Why the start failed.
    code: Option<&'static str>,
    /// The data folder, shown so the laboratory can find it; only on failure.
    data_folder: Option<String>,
    /// The installation this run opened, even when its engine did not start, so the demo
    /// laboratory is always marked and can always be left (proposed REQ-OPS-016).
    installation: InstallationKind,
}

impl StartupState {
    fn status(&self, installation: InstallationKind) -> StartupStatus {
        match self {
            Self::Running => StartupStatus {
                state: "running",
                code: None,
                data_folder: None,
                installation,
            },
            Self::Failed { code, data_folder } => StartupStatus {
                state: "failed",
                code: Some(code),
                data_folder: data_folder
                    .as_ref()
                    .map(|folder| folder.display().to_string()),
                installation,
            },
        }
    }
}

/// Shell-only and public: the screen asks it first, before any engine command.
#[tauri::command]
pub(crate) fn startup_status(
    state: State<'_, StartupState>,
    installation: State<'_, Installation>,
) -> StartupStatus {
    state.status(installation.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failure_reports_its_code_and_data_folder() {
        let failed = StartupState::Failed {
            code: "data_folder_on_network",
            data_folder: Some(PathBuf::from("/synthetic/data")),
        };
        assert_eq!(
            serde_json::to_value(failed.status(InstallationKind::Demo)).unwrap(),
            serde_json::json!({
                "state": "failed",
                "code": "data_folder_on_network",
                "dataFolder": "/synthetic/data",
                "installation": "demo"
            })
        );
        assert_eq!(
            serde_json::to_value(StartupState::Running.status(InstallationKind::Laboratory))
                .unwrap(),
            serde_json::json!({
                "state": "running",
                "code": null,
                "dataFolder": null,
                "installation": "laboratory"
            })
        );
    }
}
