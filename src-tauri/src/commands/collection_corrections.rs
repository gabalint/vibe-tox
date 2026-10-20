//! Collection timing history and attributable quality decisions.

use lis_core::CommandError;
use lis_engine::{
    CollectionHistory, CorrectCollectionTimeInput, Engine, GetAccessionInput,
    ResolveCollectionCorrectionInput,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn get_collection_history(
    engine: State<'_, Engine>,
    input: GetAccessionInput,
) -> Result<CollectionHistory, CommandError> {
    engine.get_collection_history(input).await
}

#[tauri::command]
pub(crate) async fn correct_collection_time(
    engine: State<'_, Engine>,
    input: CorrectCollectionTimeInput,
) -> Result<CollectionHistory, CommandError> {
    engine.correct_collection_time(input).await
}

#[tauri::command]
pub(crate) async fn resolve_collection_correction(
    engine: State<'_, Engine>,
    input: ResolveCollectionCorrectionInput,
) -> Result<CollectionHistory, CommandError> {
    engine.resolve_collection_correction(input).await
}
