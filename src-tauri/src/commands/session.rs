//! Setup, sign-in and session commands (M1 IPC contract). Each forwards to one
//! engine method; the engine decides everything.

use lis_auth::{
    ChangeInitialPasswordInput, ChangePasswordInput, CreateFirstAdministratorInput, SetupStatus,
    SignInInput, SignInStep, UnlockSessionInput, VerifyMfaInput,
};
use lis_core::CommandError;
use lis_engine::{
    DisableUnlockPinInput, Engine, SessionInfo, SetUnlockPinInput, UnlockPinStatus,
    UnlockWithPinInput,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn setup_status(engine: State<'_, Engine>) -> Result<SetupStatus, CommandError> {
    engine.setup_status().await
}

#[tauri::command]
pub(crate) async fn create_first_administrator(
    engine: State<'_, Engine>,
    input: CreateFirstAdministratorInput,
) -> Result<(), CommandError> {
    engine.create_first_administrator(input).await
}

#[tauri::command]
pub(crate) async fn sign_in(
    engine: State<'_, Engine>,
    input: SignInInput,
) -> Result<SignInStep, CommandError> {
    engine.sign_in(input).await
}

#[tauri::command]
pub(crate) async fn change_initial_password(
    engine: State<'_, Engine>,
    input: ChangeInitialPasswordInput,
) -> Result<SignInStep, CommandError> {
    engine.change_initial_password(input).await
}

#[tauri::command]
pub(crate) async fn verify_mfa(
    engine: State<'_, Engine>,
    input: VerifyMfaInput,
) -> Result<SessionInfo, CommandError> {
    engine.verify_mfa(input).await
}

#[tauri::command]
pub(crate) async fn current_session(
    engine: State<'_, Engine>,
) -> Result<Option<SessionInfo>, CommandError> {
    engine.current_session().await
}

#[tauri::command]
pub(crate) async fn touch_session(engine: State<'_, Engine>) -> Result<(), CommandError> {
    engine.touch_session().await
}

#[tauri::command]
pub(crate) async fn lock_session(engine: State<'_, Engine>) -> Result<(), CommandError> {
    engine.lock_session().await
}

#[tauri::command]
pub(crate) async fn unlock_session(
    engine: State<'_, Engine>,
    input: UnlockSessionInput,
) -> Result<SessionInfo, CommandError> {
    engine.unlock_session(input).await
}

#[tauri::command]
pub(crate) async fn sign_out(engine: State<'_, Engine>) -> Result<(), CommandError> {
    engine.sign_out().await
}

#[tauri::command]
pub(crate) async fn change_password(
    engine: State<'_, Engine>,
    input: ChangePasswordInput,
) -> Result<(), CommandError> {
    engine.change_password(input).await
}

#[tauri::command]
pub(crate) async fn get_unlock_pin_status(
    engine: State<'_, Engine>,
) -> Result<UnlockPinStatus, CommandError> {
    engine.get_unlock_pin_status().await
}

#[tauri::command]
pub(crate) async fn set_unlock_pin(
    engine: State<'_, Engine>,
    input: SetUnlockPinInput,
) -> Result<(), CommandError> {
    engine.set_unlock_pin(input).await
}

#[tauri::command]
pub(crate) async fn disable_unlock_pin(
    engine: State<'_, Engine>,
    input: DisableUnlockPinInput,
) -> Result<(), CommandError> {
    engine.disable_unlock_pin(input).await
}

#[tauri::command]
pub(crate) async fn unlock_with_pin(
    engine: State<'_, Engine>,
    input: UnlockWithPinInput,
) -> Result<SessionInfo, CommandError> {
    engine.unlock_with_pin(input).await
}
