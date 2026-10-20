//! Patient identity, clients and practitioners (M2 IPC contract). The engine
//! authorizes each call and audits subject reads.

use lis_core::CommandError;
use lis_engine::{
    AddClientLocation, AddSubjectIdentifier, AssertDemographics, ClientId, ClientLocationId,
    ClientView, ConfirmDistinctSubjects, CreateClient, DeactivateSubjectIdentifier, Engine,
    GetSubjectInput, IdentifierAdded, IdentityHoldResolved, IdentityHoldView,
    LaboratoryProfileView, ListIdentityHoldsInput, PractitionerId, PractitionerView,
    RegisterPractitioner, RegisterSubject, SetLaboratoryProfile, SubjectCandidates,
    SubjectMatchQuery, SubjectRegistered, SubjectRevision, SubjectSearch, SubjectSummary,
    SubjectView,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn search_subjects(
    engine: State<'_, Engine>,
    input: SubjectSearch,
) -> Result<Vec<SubjectSummary>, CommandError> {
    engine.search_subjects(input).await
}

#[tauri::command]
pub(crate) async fn match_subject_candidates(
    engine: State<'_, Engine>,
    input: SubjectMatchQuery,
) -> Result<SubjectCandidates, CommandError> {
    engine.match_subject_candidates(input).await
}

#[tauri::command]
pub(crate) async fn register_subject(
    engine: State<'_, Engine>,
    input: RegisterSubject,
) -> Result<SubjectRegistered, CommandError> {
    engine.register_subject(input).await
}

#[tauri::command]
pub(crate) async fn get_subject(
    engine: State<'_, Engine>,
    input: GetSubjectInput,
) -> Result<SubjectView, CommandError> {
    engine.get_subject(input).await
}

#[tauri::command]
pub(crate) async fn assert_demographics(
    engine: State<'_, Engine>,
    input: AssertDemographics,
) -> Result<SubjectRevision, CommandError> {
    engine.assert_demographics(input).await
}

#[tauri::command]
pub(crate) async fn add_subject_identifier(
    engine: State<'_, Engine>,
    input: AddSubjectIdentifier,
) -> Result<IdentifierAdded, CommandError> {
    engine.add_subject_identifier(input).await
}

#[tauri::command]
pub(crate) async fn deactivate_subject_identifier(
    engine: State<'_, Engine>,
    input: DeactivateSubjectIdentifier,
) -> Result<SubjectRevision, CommandError> {
    engine.deactivate_subject_identifier(input).await
}

#[tauri::command]
pub(crate) async fn list_identity_holds(
    engine: State<'_, Engine>,
    input: ListIdentityHoldsInput,
) -> Result<Vec<IdentityHoldView>, CommandError> {
    engine.list_identity_holds(input).await
}

#[tauri::command]
pub(crate) async fn resolve_identity_hold(
    engine: State<'_, Engine>,
    input: ConfirmDistinctSubjects,
) -> Result<IdentityHoldResolved, CommandError> {
    engine.resolve_identity_hold(input).await
}

#[tauri::command]
pub(crate) async fn list_clients(
    engine: State<'_, Engine>,
) -> Result<Vec<ClientView>, CommandError> {
    engine.list_clients().await
}

#[tauri::command]
pub(crate) async fn create_client(
    engine: State<'_, Engine>,
    input: CreateClient,
) -> Result<ClientId, CommandError> {
    engine.create_client(input).await
}

#[tauri::command]
pub(crate) async fn add_client_location(
    engine: State<'_, Engine>,
    input: AddClientLocation,
) -> Result<ClientLocationId, CommandError> {
    engine.add_client_location(input).await
}

#[tauri::command]
pub(crate) async fn list_practitioners(
    engine: State<'_, Engine>,
) -> Result<Vec<PractitionerView>, CommandError> {
    engine.list_practitioners().await
}

#[tauri::command]
pub(crate) async fn register_practitioner(
    engine: State<'_, Engine>,
    input: RegisterPractitioner,
) -> Result<PractitionerId, CommandError> {
    engine.register_practitioner(input).await
}

#[tauri::command]
pub(crate) async fn get_laboratory_profile(
    engine: State<'_, Engine>,
) -> Result<Option<LaboratoryProfileView>, CommandError> {
    engine.get_laboratory_profile().await
}

#[tauri::command]
pub(crate) async fn set_laboratory_profile(
    engine: State<'_, Engine>,
    input: SetLaboratoryProfile,
) -> Result<LaboratoryProfileView, CommandError> {
    engine.set_laboratory_profile(input).await
}

#[tauri::command]
pub(crate) async fn list_laboratory_profiles(
    engine: State<'_, Engine>,
) -> Result<Vec<LaboratoryProfileView>, CommandError> {
    engine.list_laboratory_profiles().await
}

#[tauri::command]
pub(crate) async fn record_intake_deferral(
    engine: State<'_, Engine>,
    input: lis_engine::RecordIntakeDeferralInput,
) -> Result<lis_engine::IntakeDeferralView, CommandError> {
    engine.record_intake_deferral(input).await
}

#[tauri::command]
pub(crate) async fn resolve_intake_deferral(
    engine: State<'_, Engine>,
    input: lis_engine::ResolveIntakeDeferralInput,
) -> Result<lis_engine::IntakeDeferralView, CommandError> {
    engine.resolve_intake_deferral(input).await
}

#[tauri::command]
pub(crate) async fn list_intake_deferrals(
    engine: State<'_, Engine>,
    input: lis_engine::ListIntakeDeferralsInput,
) -> Result<Vec<lis_engine::IntakeDeferralView>, CommandError> {
    engine.list_intake_deferrals(input).await
}

#[tauri::command]
pub(crate) async fn update_client(
    engine: State<'_, Engine>,
    input: lis_engine::UpdateClient,
) -> Result<lis_engine::ClientView, CommandError> {
    engine.update_client(input).await
}

#[tauri::command]
pub(crate) async fn update_client_location(
    engine: State<'_, Engine>,
    input: lis_engine::UpdateClientLocation,
) -> Result<lis_engine::ClientLocationView, CommandError> {
    engine.update_client_location(input).await
}

#[tauri::command]
pub(crate) async fn update_practitioner(
    engine: State<'_, Engine>,
    input: lis_engine::UpdatePractitioner,
) -> Result<lis_engine::PractitionerView, CommandError> {
    engine.update_practitioner(input).await
}

#[tauri::command]
pub(crate) async fn directory_history(
    engine: State<'_, Engine>,
    input: lis_engine::DirectoryHistoryInput,
) -> Result<Vec<lis_engine::DirectoryHistoryEntry>, CommandError> {
    engine.directory_history(input).await
}
