//! User administration commands (M1 IPC contract; capability `manage_users`).

use lis_auth::{
    ChangeUserStatusInput, CreateUserInput, DesignateOnePersonLaboratoryInput,
    ResetUserCredentialsInput, RoleInfo, SetUserRolesInput, UserSummary,
};
use lis_core::CommandError;
use lis_engine::Engine;
use tauri::State;

#[tauri::command]
pub(crate) async fn list_roles(engine: State<'_, Engine>) -> Result<Vec<RoleInfo>, CommandError> {
    engine.list_roles().await
}

#[tauri::command]
pub(crate) async fn list_users(
    engine: State<'_, Engine>,
) -> Result<Vec<UserSummary>, CommandError> {
    engine.list_users().await
}

#[tauri::command]
pub(crate) async fn create_user(
    engine: State<'_, Engine>,
    input: CreateUserInput,
) -> Result<UserSummary, CommandError> {
    engine.create_user(input).await
}

#[tauri::command]
pub(crate) async fn set_user_roles(
    engine: State<'_, Engine>,
    input: SetUserRolesInput,
) -> Result<UserSummary, CommandError> {
    engine.set_user_roles(input).await
}

#[tauri::command]
pub(crate) async fn disable_user(
    engine: State<'_, Engine>,
    input: ChangeUserStatusInput,
) -> Result<UserSummary, CommandError> {
    engine.disable_user(input).await
}

#[tauri::command]
pub(crate) async fn enable_user(
    engine: State<'_, Engine>,
    input: ChangeUserStatusInput,
) -> Result<UserSummary, CommandError> {
    engine.enable_user(input).await
}

#[tauri::command]
pub(crate) async fn reset_user_credentials(
    engine: State<'_, Engine>,
    input: ResetUserCredentialsInput,
) -> Result<UserSummary, CommandError> {
    engine.reset_user_credentials(input).await
}

#[tauri::command]
pub(crate) async fn designate_one_person_laboratory(
    engine: State<'_, Engine>,
    input: DesignateOnePersonLaboratoryInput,
) -> Result<UserSummary, CommandError> {
    engine.designate_one_person_laboratory(input).await
}
