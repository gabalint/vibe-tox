//! Analyte corrections, retirement and their history (REQ-ANA-002).

use lis_catalog::{
    AnalyteHistoryEntry, AnalyteInput, AnalyteUsage, AnalyteView, CatalogError, UpdateAnalyte,
};
use lis_core::CommandError;

use super::{MANAGE, READ};
use crate::commands::people::name_recorders;
use crate::engine::Engine;
use crate::registry::{CommandSpec, spec};

pub(crate) const UPDATE_ANALYTE: CommandSpec =
    spec("update_analyte", "catalog.update_analyte", MANAGE);
pub(crate) const ANALYTE_HISTORY: CommandSpec =
    spec("analyte_history", "catalog.analyte_history", READ);
pub(crate) const ANALYTE_USAGE: CommandSpec = spec("analyte_usage", "catalog.analyte_usage", READ);

impl Engine {
    /// `update_analyte`.
    pub async fn update_analyte(&self, input: UpdateAnalyte) -> Result<AnalyteView, CommandError> {
        self.run(&UPDATE_ANALYTE, |call| async move {
            self.write(&call, move |ctx| lis_catalog::update_analyte(ctx, input))
                .await
        })
        .await
    }

    /// `analyte_history`, with who recorded each revision named on every read.
    pub async fn analyte_history(
        &self,
        input: AnalyteInput,
    ) -> Result<Vec<AnalyteHistoryEntry>, CommandError> {
        self.run(&ANALYTE_HISTORY, |call| async move {
            self.read(&call, move |conn| named_history(conn, input))
                .await
        })
        .await
    }

    /// `analyte_usage`.
    pub async fn analyte_usage(&self, input: AnalyteInput) -> Result<AnalyteUsage, CommandError> {
        self.run(&ANALYTE_USAGE, |call| async move {
            self.read(&call, move |conn| lis_catalog::analyte_usage(conn, input))
                .await
        })
        .await
    }
}

fn named_history(
    conn: &rusqlite::Connection,
    input: AnalyteInput,
) -> Result<Vec<AnalyteHistoryEntry>, CatalogError> {
    let mut rows = lis_catalog::analyte_history(conn, input)?;
    name_recorders(
        conn,
        &mut rows,
        |row| row.recorded_by.as_deref(),
        |row, name| row.recorded_by_display = name,
    )?;
    Ok(rows)
}
