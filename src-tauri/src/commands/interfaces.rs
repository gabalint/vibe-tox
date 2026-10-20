//! Analyzer configuration (M2 IPC contract; capability `manage_configuration`).
//! Each change restarts the listeners so it takes effect.

use lis_core::CommandError;
use lis_engine::{
    Engine, Instrument, InstrumentChangeResult, RegisterInstrument, SetInstrumentEnabled,
    UpdateInstrumentConnection,
};
use tauri::State;

#[tauri::command]
pub(crate) async fn list_instruments(
    engine: State<'_, Engine>,
) -> Result<Vec<Instrument>, CommandError> {
    engine.list_instruments().await
}

#[tauri::command]
pub(crate) async fn register_instrument(
    engine: State<'_, Engine>,
    input: RegisterInstrument,
) -> Result<InstrumentChangeResult, CommandError> {
    engine.register_instrument(input).await
}

#[tauri::command]
pub(crate) async fn update_instrument_connection(
    engine: State<'_, Engine>,
    input: UpdateInstrumentConnection,
) -> Result<InstrumentChangeResult, CommandError> {
    engine.update_instrument_connection(input).await
}

#[tauri::command]
pub(crate) async fn set_instrument_enabled(
    engine: State<'_, Engine>,
    input: SetInstrumentEnabled,
) -> Result<InstrumentChangeResult, CommandError> {
    engine.set_instrument_enabled(input).await
}
