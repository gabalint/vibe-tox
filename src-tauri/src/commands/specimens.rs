//! Accessioning, specimen, container, hold, excursion and label commands
//! (M2 IPC contract; capabilities `accession`, `manage_quality` and
//! `manage_configuration`, checked by the engine).

use lis_core::CommandError;
use lis_engine::{
    AccessionDetail, AssessExcursionInput, ConfigureLabelPrinterInput,
    ConfigureStorageLocationInput, ContainerHoldView, ContainerLookup, CreateAccessionInput,
    CreateAliquotInput, DisposeContainerInput, Engine, ExcursionView, FindContainerInput,
    GetAccessionInput, GetOrderInput, LabelJobView, LabelPrinterView, MarkContainerDepletedInput,
    MoveContainerInput, OrderAccessionContext, PlaceContainerHoldInput, PrintLabelsInput,
    RecordExcursionInput, RecordReceiptDecisionInput, ReleaseContainerHoldInput,
    StorageLocationView,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn create_accession(
    engine: State<'_, Engine>,
    input: CreateAccessionInput,
) -> Result<AccessionDetail, CommandError> {
    engine.create_accession(input).await
}

#[tauri::command]
pub(crate) async fn get_accession(
    engine: State<'_, Engine>,
    input: GetAccessionInput,
) -> Result<AccessionDetail, CommandError> {
    engine.get_accession(input).await
}

#[tauri::command]
pub(crate) async fn get_order_accession_context(
    engine: State<'_, Engine>,
    input: GetOrderInput,
) -> Result<OrderAccessionContext, CommandError> {
    engine.get_order_accession_context(input).await
}

#[tauri::command]
pub(crate) async fn find_container(
    engine: State<'_, Engine>,
    input: FindContainerInput,
) -> Result<ContainerLookup, CommandError> {
    engine.find_container(input).await
}

#[tauri::command]
pub(crate) async fn record_receipt_decision(
    engine: State<'_, Engine>,
    input: RecordReceiptDecisionInput,
) -> Result<AccessionDetail, CommandError> {
    engine.record_receipt_decision(input).await
}

#[tauri::command]
pub(crate) async fn create_aliquot(
    engine: State<'_, Engine>,
    input: CreateAliquotInput,
) -> Result<AccessionDetail, CommandError> {
    engine.create_aliquot(input).await
}

#[tauri::command]
pub(crate) async fn mark_container_depleted(
    engine: State<'_, Engine>,
    input: MarkContainerDepletedInput,
) -> Result<AccessionDetail, CommandError> {
    engine.mark_container_depleted(input).await
}

#[tauri::command]
pub(crate) async fn move_container(
    engine: State<'_, Engine>,
    input: MoveContainerInput,
) -> Result<AccessionDetail, CommandError> {
    engine.move_container(input).await
}

#[tauri::command]
pub(crate) async fn dispose_container(
    engine: State<'_, Engine>,
    input: DisposeContainerInput,
) -> Result<AccessionDetail, CommandError> {
    engine.dispose_container(input).await
}

#[tauri::command]
pub(crate) async fn print_labels(
    engine: State<'_, Engine>,
    input: PrintLabelsInput,
) -> Result<Vec<LabelJobView>, CommandError> {
    engine.print_labels(input).await
}

#[tauri::command]
pub(crate) async fn list_printers(
    engine: State<'_, Engine>,
) -> Result<Vec<LabelPrinterView>, CommandError> {
    engine.list_printers().await
}

#[tauri::command]
pub(crate) async fn list_locations(
    engine: State<'_, Engine>,
) -> Result<Vec<StorageLocationView>, CommandError> {
    engine.list_locations().await
}

#[tauri::command]
pub(crate) async fn place_hold(
    engine: State<'_, Engine>,
    input: PlaceContainerHoldInput,
) -> Result<ContainerHoldView, CommandError> {
    engine.place_hold(input).await
}

#[tauri::command]
pub(crate) async fn release_hold(
    engine: State<'_, Engine>,
    input: ReleaseContainerHoldInput,
) -> Result<ContainerHoldView, CommandError> {
    engine.release_hold(input).await
}

#[tauri::command]
pub(crate) async fn record_excursion(
    engine: State<'_, Engine>,
    input: RecordExcursionInput,
) -> Result<ExcursionView, CommandError> {
    engine.record_excursion(input).await
}

#[tauri::command]
pub(crate) async fn assess_excursion(
    engine: State<'_, Engine>,
    input: AssessExcursionInput,
) -> Result<ExcursionView, CommandError> {
    engine.assess_excursion(input).await
}

#[tauri::command]
pub(crate) async fn configure_location(
    engine: State<'_, Engine>,
    input: ConfigureStorageLocationInput,
) -> Result<StorageLocationView, CommandError> {
    engine.configure_location(input).await
}

#[tauri::command]
pub(crate) async fn configure_printer(
    engine: State<'_, Engine>,
    input: ConfigureLabelPrinterInput,
) -> Result<LabelPrinterView, CommandError> {
    engine.configure_printer(input).await
}

#[tauri::command]
pub(crate) async fn resolve_receipt_chronology(
    engine: State<'_, Engine>,
    input: lis_engine::ResolveReceiptChronologyInput,
) -> Result<AccessionDetail, CommandError> {
    engine.resolve_receipt_chronology(input).await
}

#[tauri::command]
pub(crate) async fn get_container_history(
    engine: State<'_, Engine>,
    input: lis_engine::GetContainerHistoryInput,
) -> Result<lis_engine::MaterialHistoryPage, CommandError> {
    engine.get_container_history(input).await
}

#[tauri::command]
pub(crate) async fn list_excursions(
    engine: State<'_, Engine>,
    input: lis_engine::ListExcursionsInput,
) -> Result<lis_engine::ExcursionPage, CommandError> {
    engine.list_excursions(input).await
}

#[tauri::command]
pub(crate) async fn get_excursion(
    engine: State<'_, Engine>,
    input: lis_engine::GetExcursionInput,
) -> Result<lis_engine::ExcursionView, CommandError> {
    engine.get_excursion(input).await
}
