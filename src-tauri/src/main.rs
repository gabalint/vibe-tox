//! Desktop shell for the Vibe Toxicology LIS (ADR 0012): one Tauri window over the
//! bundled UI, thin command adapters, report windows and diagnostic logging setup.
//!
//! Invariants:
//! - The window reaches Rust only through the commands in [`commands`], each granted in a
//!   capability file of `capabilities/` (NFR-31). The CSP and window settings live in
//!   `tauri.conf.json`; no `dangerous*` option is used.
//! - One running instance: a second launch focuses the existing window (REQ-OPS-013).
//! - Diagnostic logs are PHI-free JSON lines in the app log directory ([`logging`]).
//! - The engine ([`lis_engine::Engine`]) starts in setup on the data folder of the chosen
//!   installation, the laboratory's or the demo laboratory's ([`installation`]), then its
//!   operations loops and keep-awake start; it is managed as state for the commands and
//!   shuts down when the app exits. A start failure is kept as [`startup::StartupState`]
//!   for the startup-failure screen instead of closing the app. Switching installations
//!   restarts the app through the same exit path.
//! - Report windows show a stored PDF and nothing else; they have no capability
//!   ([`report_window`]).
//! - Builds that CI configured for updates check, download and install signed updates from
//!   the project's GitHub releases on request ([`updater`], ADR 0020); installing restarts
//!   the app through the same exit path, or shuts the engine down before the Windows
//!   installer replaces the app.
//! - Closing the main window or quitting asks first while the engine reports work in
//!   progress or the UI reports an unsaved page form; Cancel is the default and quitting
//!   with work in progress is recorded (REQ-OPS-013, §6.3, [`quit`]). A second close or
//!   quit request while the dialog is open, such as an OS shutdown, is never blocked.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod installation;
mod logging;
mod quit;
mod report_window;
mod security_key_operations;
mod startup;
mod updater;

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use lis_core::SystemClock;
use lis_engine::{Engine, EngineConfig, Inhibitor, KeyProvider, PrinterDestinations};
use tauri::{AppHandle, Manager, RunEvent, WindowEvent};

use crate::installation::{Choice, Folders, Installation};
pub(crate) use crate::quit::confirm_exit;
use crate::quit::{QuitState, UnsavedForms, may_exit};
use crate::report_window::ReportWindows;
use crate::startup::StartupState;

/// How long a start waits for the previous process's database lock after a restart.
const RESTART_LOCK_WAIT: Duration = Duration::from_secs(5);

/// Label of the main window, matching `tauri.conf.json` and the capability files.
const MAIN_WINDOW: &str = "main";

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let context = context();
    let updates = updater::Updates::new(context.config());
    let app = plugins(&updates)
        .manage(updates)
        .manage(ReportWindows::default())
        .manage(security_key_operations::SecurityKeyOperations::default())
        .register_uri_scheme_protocol(report_window::SCHEME, report_window::serve)
        .setup(|app| {
            let logs = app.path().app_log_dir()?;
            let folders = app
                .path()
                .app_local_data_dir()
                .ok()
                .and_then(|data| Folders::new(data, logs.clone()));
            let chosen = folders.as_ref().map(Folders::chosen);
            let choice = match chosen {
                Some(Ok(choice)) => choice,
                Some(Err(_)) | None => Choice::Laboratory,
            };
            let log_dir = folders
                .as_ref()
                .map_or(logs.as_path(), |f| f.log_dir(choice.kind()));
            logging::init(log_dir)?;
            tracing::info!(
                app_version = %app.package_info().version,
                installation = choice.kind().as_str(),
                "app started"
            );
            if let Some(Err(error)) = chosen {
                tracing::warn!(%error, "opening the laboratory installation");
            }
            app.manage(QuitState::default());
            app.manage(UnsavedForms::default());
            app.manage(Installation(choice.kind()));
            let state = match start_engine(app.handle(), folders.as_ref(), choice) {
                Ok(engine) => {
                    engine.start_operations(Inhibitor::system());
                    app.manage(engine);
                    StartupState::Running
                }
                Err(failed) => failed,
            };
            if let Some(folders) = folders {
                app.manage(folders);
            }
            app.manage(state);
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::CloseRequested { api, .. }
                if window.label() == MAIN_WINDOW && !may_exit(window.app_handle()) =>
            {
                api.prevent_close();
            }
            WindowEvent::Destroyed => report_window::forget(window),
            _ => {}
        })
        .invoke_handler(commands::handler())
        .build(context)?;
    app.run(|app, event| match event {
        RunEvent::ExitRequested { api, .. } if !may_exit(app) => api.prevent_exit(),
        RunEvent::Exit => {
            if let Some(operations) =
                app.try_state::<security_key_operations::SecurityKeyOperations>()
            {
                operations.cancel_all();
            }
            if let Some(engine) = app.try_state::<Engine>() {
                tauri::async_runtime::block_on(engine.shutdown());
            }
            tracing::info!("app stopped");
        }
        _ => {}
    });
    Ok(())
}

/// The shell's plugins. The updater is registered only in builds CI configured for updates
/// (ADR 0020); its commands are never granted to the UI, and the update adapters call it
/// from Rust.
fn plugins(updates: &updater::Updates) -> tauri::Builder<tauri::Wry> {
    let builder = tauri::Builder::default()
        // Registered first so a second launch exits before it opens anything (REQ-OPS-013).
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            focus_main_window(app);
        }))
        // Dialogs open from Rust only; no `dialog:` permission is granted to the UI (M4 D10).
        .plugin(tauri_plugin_dialog::init());
    if updates.configured() {
        builder.plugin(tauri_plugin_updater::Builder::new().build())
    } else {
        builder
    }
}

/// Starts the engine on the chosen installation's data folder, after deleting the demo
/// folder when a reset is pending; a failure becomes the startup state.
fn start_engine(
    app: &AppHandle,
    folders: Option<&Folders>,
    choice: Choice,
) -> Result<Engine, StartupState> {
    // Local, never roaming: SQLite WAL fails on network file systems (M4 D15).
    let Some(folders) = folders else {
        tracing::error!("data folder not resolved");
        return Err(StartupState::Failed {
            code: "internal",
            data_folder: None,
        });
    };
    let kind = choice.kind();
    let data_dir = folders.data_dir(kind).to_path_buf();
    if choice == Choice::DemoReset {
        reset_demo(folders, &data_dir)?;
    } else if let Err(error) = folders.remove_tombstone() {
        tracing::warn!(error = ?error.kind(), "earlier demo reset not finished");
    }
    let config = || EngineConfig {
        data_dir: data_dir.clone(),
        clock: Arc::new(SystemClock),
        // The key-store entry is this bundle's own, and the demo's is separate, so a test
        // build or the demo never touches the production laboratory's key.
        keys: KeyProvider::OsKeyStore {
            identifier: installation::key_identifier(&app.config().identifier, kind),
        },
        printer_destinations: PrinterDestinations::LabNetwork,
        webview: webview(),
        outbox_handlers: Vec::new(),
        installation_kind: kind,
    };
    // After a switch the previous process may still hold the database lock briefly.
    let waiting_since = Instant::now();
    loop {
        match tauri::async_runtime::block_on(Engine::start(config())) {
            Err(error)
                if error.code() == "already_open"
                    && waiting_since.elapsed() < RESTART_LOCK_WAIT =>
            {
                std::thread::sleep(Duration::from_millis(100));
            }
            started => {
                return started.map_err(|error| {
                    tracing::error!(code = error.code(), %error, "engine did not start");
                    StartupState::Failed {
                        code: error.code(),
                        data_folder: Some(data_dir),
                    }
                });
            }
        }
    }
}

/// Empties the demo laboratory before its engine opens. The choice stays `demo-reset` until
/// the demo folder is gone, so a failure retries at the next start; the previous process
/// may still hold its files for a moment after a restart.
fn reset_demo(folders: &Folders, data_dir: &Path) -> Result<(), StartupState> {
    match folders
        .reset_demo(RESTART_LOCK_WAIT)
        .and_then(|deleted| folders.choose(Choice::Demo).map(|()| deleted))
    {
        Ok(deleted) => {
            if !deleted {
                tracing::warn!("demo laboratory reset; its old files are deleted at a later start");
            }
            tracing::info!("demo laboratory reset");
            Ok(())
        }
        Err(error) => {
            tracing::error!(error = ?error.kind(), "demo laboratory not reset");
            Err(StartupState::Failed {
                code: "demo_reset_failed",
                data_folder: Some(data_dir.to_path_buf()),
            })
        }
    }
}

/// The compiled configuration, assets and capabilities. Expanded once per binary, as the
/// macro embeds platform metadata.
fn context<R: tauri::Runtime>() -> tauri::Context<R> {
    tauri::generate_context!()
}

/// The OS web engine's name and version, for each session's audit context (ADR 0012).
fn webview() -> Option<String> {
    let name = match std::env::consts::OS {
        "windows" => "WebView2",
        "macos" => "WKWebView",
        _ => "WebKitGTK",
    };
    tauri::webview_version()
        .inspect_err(|error| tracing::warn!(%error, "web engine version not reported"))
        .ok()
        .map(|version| format!("{name} {version}"))
}

/// Brings the running window to the front when the app is launched again.
fn focus_main_window(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN_WINDOW) else {
        tracing::warn!("second launch: main window not found");
        return;
    };
    let focused = window
        .unminimize()
        .and_then(|()| window.show())
        .and_then(|()| window.set_focus());
    if let Err(error) = focused {
        tracing::warn!(%error, "second launch: could not focus the main window");
    }
}
