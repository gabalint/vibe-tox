//! The demo laboratory (proposed REQ-OPS-016): the engine's demo status and loader, and the
//! shell-only switches between the laboratory and the demo laboratory. A switch records the
//! next start's choice and restarts the app through the ordinary exit path, which shuts the
//! engine down (`main.rs`); the other installation opens on the next start.

use lis_core::{CommandError, CorrelationId, ErrorCode};
use lis_engine::{DemoDataLoaded, DemoStatus, Engine, InstallationKind};
use tauri::{AppHandle, Manager, Runtime, State};

use crate::installation::{Choice, Folders, Installation};

#[tauri::command]
pub(crate) async fn complete_demo_setup(engine: State<'_, Engine>) -> Result<(), CommandError> {
    engine.complete_demo_setup().await
}

#[tauri::command]
pub(crate) async fn demo_status(engine: State<'_, Engine>) -> Result<DemoStatus, CommandError> {
    engine.demo_status().await
}

#[tauri::command]
pub(crate) async fn load_demo_data(
    engine: State<'_, Engine>,
) -> Result<DemoDataLoaded, CommandError> {
    engine.load_demo_data().await
}

/// "Explore the demo laboratory": only from a laboratory installation that is not set up
/// yet, so a working laboratory's listeners and screens are never left for the demo.
#[tauri::command]
pub(crate) async fn open_demo_laboratory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), CommandError> {
    ensure_current(&app, InstallationKind::Laboratory)?;
    let Some(engine) = app.try_state::<Engine>() else {
        return Err(refused(
            "The app did not start, so the demo laboratory cannot open.",
        ));
    };
    if !engine.setup_status().await?.needs_administrator {
        return Err(refused(
            "The demo laboratory can be opened only before this laboratory is set up.",
        ));
    }
    switch(&app, Choice::Demo).await
}

/// "Leave the demo laboratory": the laboratory's installation opens on the restart.
#[tauri::command]
pub(crate) async fn leave_demo_laboratory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), CommandError> {
    ensure_current(&app, InstallationKind::Demo)?;
    switch(&app, Choice::Laboratory).await
}

/// "Reset the demo laboratory": the restart deletes the demo folder, and only it, before
/// a fresh demo laboratory opens. The UI asks for confirmation first.
#[tauri::command]
pub(crate) async fn reset_demo_laboratory<R: Runtime>(
    app: AppHandle<R>,
) -> Result<(), CommandError> {
    ensure_current(&app, InstallationKind::Demo)?;
    switch(&app, Choice::DemoReset).await
}

fn refused(message: &str) -> CommandError {
    CommandError::new(ErrorCode::Conflict, message, CorrelationId::generate())
}

fn ensure_current<R: Runtime>(
    app: &AppHandle<R>,
    kind: InstallationKind,
) -> Result<(), CommandError> {
    match app.try_state::<Installation>() {
        Some(current) if current.0 == kind => Ok(()),
        Some(_) | None => Err(refused("This is not available in this installation.")),
    }
}

/// Records the next start's choice and restarts, unless work is still in progress.
async fn switch<R: Runtime>(app: &AppHandle<R>, choice: Choice) -> Result<(), CommandError> {
    if let Some(engine) = app.try_state::<Engine>() {
        let warnings = engine.quit_warnings().await;
        if !warnings.is_empty() {
            return Err(refused(&format!(
                "Wait until these finish, then try again: {}.",
                warnings.0.join("; ")
            )));
        }
    }
    let chosen = app
        .try_state::<Folders>()
        .map(|folders| folders.choose(choice));
    if let Some(Err(_)) | None = chosen {
        let correlation_id = CorrelationId::generate();
        let error = chosen.and_then(Result::err).map(|error| error.kind());
        tracing::error!(%correlation_id, ?error, "installation choice not recorded");
        return Err(CommandError::new(
            ErrorCode::Internal,
            "The app could not switch installations. Give support the reference shown.",
            correlation_id,
        ));
    }
    tracing::info!(
        installation = choice.kind().as_str(),
        "restarting to switch installation"
    );
    crate::confirm_exit(app);
    app.request_restart();
    Ok(())
}
