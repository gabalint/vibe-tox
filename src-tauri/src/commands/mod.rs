//! Thin IPC adapters, one file per area. Each command calls exactly one engine
//! command or query (`lis_engine::Engine`); the engine authorizes, audits and logs.
//!
//! NFR-31: a command is reachable only when it is in [`handler`], in `APP_COMMANDS` in
//! `build.rs` and granted in a capability file (`capabilities/<area>.json`). Add new commands to all three and
//! to the engine's registry (`lis_engine::registry::COMMANDS`); the tests below check
//! that the four agree.

mod app_info;
mod audit;
mod listeners;
mod security_keys;
mod session;
mod users;
// Wave 3+: one adapter module per area. Edit only your block.
// identity: begin
mod identity;
// identity: end
// catalog: begin
mod catalog;
mod catalog_review;
// catalog: end
// interfaces: begin
mod interfaces;
// interfaces: end
// orders: begin
mod orders;
// orders: end
// specimens: begin
mod specimens;
// specimens: end
// analytical: begin
mod analytical;
// analytical: end
// results: begin
mod results;
// results: end
// reporting: begin
mod reporting;
// reporting: end
// billing: begin
mod billing;
mod billing_intake;
mod collection_corrections;
// billing: end
// work_queues: begin
mod work_queues;
// work_queues: end
// backup: begin
mod backup;
// backup: end
// runtime: begin
mod operations;
mod updates;
// runtime: end
// anchors: begin
mod anchors;
// anchors: end
// demo: begin
pub(crate) mod demo;
// demo: end
// result_files: begin
mod result_files;
// result_files: end
// file_templates: begin
mod file_templates;
// file_templates: end

mod handler;
pub(crate) use handler::handler;

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::sync::Arc;

    use lis_auth::DataKey;
    use lis_core::SystemClock;
    use lis_engine::registry::COMMANDS;
    use lis_engine::{Engine, EngineConfig, KeyProvider};
    use tauri::ipc::{CallbackFn, InvokeBody};
    use tauri::test::{INVOKE_KEY, MockRuntime, get_ipc_response, mock_builder};
    use tauri::webview::InvokeRequest;
    use tauri::{App, Manager, WebviewWindow, WebviewWindowBuilder};
    use tempfile::TempDir;

    fn app() -> (App<MockRuntime>, TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let engine = tauri::async_runtime::block_on(Engine::start(EngineConfig {
            data_dir: dir.path().to_owned(),
            clock: Arc::new(SystemClock),
            keys: KeyProvider::Fixed(DataKey::from_bytes([3; 32])),
            printer_destinations: lis_engine::PrinterDestinations::LabNetwork,
            webview: None,
            outbox_handlers: Vec::new(),
            installation_kind: lis_engine::InstallationKind::Laboratory,
        }))
        .unwrap();
        let app = mock_builder()
            .invoke_handler(super::handler())
            .build(crate::context())
            .unwrap();
        app.manage(engine);
        app.manage(crate::security_key_operations::SecurityKeyOperations::default());
        app.manage(crate::startup::StartupState::Running);
        app.manage(crate::installation::Installation(
            lis_engine::InstallationKind::Laboratory,
        ));
        app.manage(crate::updater::Updates::new(app.config()));
        (app, dir)
    }

    /// Shell-only commands: granted and handled, but not engine commands, so not in the
    /// registry (`app_info` is in it as a public command).
    const SHELL_ONLY: [&str; 8] = [
        "security_key_status",
        "security_key_pin",
        "security_key_cancel",
        "security_key_continue",
        "startup_status",
        "open_demo_laboratory",
        "leave_demo_laboratory",
        "reset_demo_laboratory",
    ];

    fn window(app: &App<MockRuntime>, label: &str) -> WebviewWindow<MockRuntime> {
        WebviewWindowBuilder::new(app, label, Default::default())
            .build()
            .unwrap()
    }

    fn invoke(window: &WebviewWindow<MockRuntime>, cmd: &str) -> Result<(), String> {
        let request = InvokeRequest {
            cmd: cmd.into(),
            callback: CallbackFn(0),
            error: CallbackFn(1),
            url: window.url().unwrap(),
            body: InvokeBody::default(),
            headers: Default::default(),
            invoke_key: INVOKE_KEY.into(),
        };
        get_ipc_response(window, request)
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[test]
    fn main_window_may_call_granted_command() {
        let (app, _dir) = app();
        let main = window(&app, "main");
        assert_eq!(invoke(&main, "app_info"), Ok(()));
        assert_eq!(invoke(&main, "setup_status"), Ok(()));
        assert_eq!(invoke(&main, "startup_status"), Ok(()));
    }

    // NFR-31: every registered command is wired and granted; refusals come
    // from the engine or from input validation, never "not found" or "not allowed".
    #[test]
    fn every_registry_command_reaches_its_adapter() {
        let (app, _dir) = app();
        let main = window(&app, "main");
        for spec in COMMANDS {
            if let Err(error) = invoke(&main, spec.name) {
                assert!(
                    !error.contains("not found") && !error.contains("not allowed"),
                    "{}: {error}",
                    spec.name
                );
            }
        }
    }

    /// Every permission the files in `capabilities/` grant; none may be granted twice.
    fn granted_permissions() -> BTreeSet<String> {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
        let files = std::fs::read_dir(dir)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
            });
        let mut granted = BTreeSet::new();
        for path in files {
            let file: serde_json::Value =
                serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
            let permissions = file["permissions"].as_array().unwrap();
            for permission in permissions.iter().map(|p| p.as_str().unwrap().to_owned()) {
                assert!(
                    granted.insert(permission.clone()),
                    "{permission} granted twice"
                );
            }
        }
        granted
    }

    // NFR-31: the capability files together grant exactly the registry's commands and
    // the shell-only ones. Each area owns one file in `capabilities/`; Tauri loads every
    // file there.
    #[test]
    fn capability_files_match_the_registry() {
        let granted = granted_permissions();
        let registered: BTreeSet<String> = COMMANDS
            .iter()
            .map(|spec| spec.name)
            .chain(SHELL_ONLY)
            .map(|name| format!("allow-{}", name.replace('_', "-")))
            .collect();
        assert_eq!(granted, registered);
    }

    // NFR-31: the UI's command names type-check against the registry. Written during
    // `cargo test`, as ts-rs writes the DTO bindings; CI checks generated files.
    #[test]
    fn export_command_names() {
        let names: BTreeSet<&str> = COMMANDS
            .iter()
            .map(|spec| spec.name)
            .chain(SHELL_ONLY)
            .collect();
        let union = names
            .iter()
            .map(|name| format!("\"{name}\""))
            .collect::<Vec<_>>()
            .join(" | ");
        let file = format!(
            "// This file was generated by [ts-rs](https://github.com/Aleph-Alpha/ts-rs). \
             Do not edit this file manually.\n\n\
             /**\n * Every IPC command name in the engine's registry (NFR-31).\n */\n\
             export type CommandName = {union};\n"
        );
        let path = std::path::Path::new(env!("TS_RS_EXPORT_DIR")).join("CommandName.ts");
        std::fs::write(path, file).unwrap();
    }

    // NFR-31: a command outside the capability allow-list is refused.
    #[test]
    fn unknown_command_is_refused() {
        let (app, _dir) = app();
        let main = window(&app, "main");
        let refused = invoke(&main, "export_all_records").unwrap_err();
        assert!(refused.contains("not found"), "{refused}");
        let refused = invoke(&main, "plugin:shell|execute").unwrap_err();
        assert!(refused.contains("not"), "{refused}");
    }

    // NFR-31: the grant is per window; a window without a capability gets nothing.
    #[test]
    fn window_without_capability_is_refused() {
        let (app, _dir) = app();
        let other = window(&app, "other");
        for command in ["app_info", "sign_in", "list_users", "security_key_status"] {
            let refused = invoke(&other, command).unwrap_err();
            assert!(refused.contains("not allowed"), "{refused}");
        }
    }
}
