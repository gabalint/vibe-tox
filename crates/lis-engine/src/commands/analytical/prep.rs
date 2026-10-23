//! Preparation batches, open → completed (REQ-ANA-001), all
//! `perform_analysis`. Batches and steps carry no patient sample labels.

use lis_analytical::{
    AnalyticalError, CompletePrepBatchInput, CreatePrepBatchInput, PrepBatchView,
    RecordPrepStepInput, actions,
};
use lis_analytical::{ListPrepBatchesInput, PrepBatchId, PrepBatchSummary};
use lis_core::CommandError;
use serde::Deserialize;
use ts_rs::TS;

#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
/// Open one saved preparation batch, including its retained steps.
pub struct GetPrepBatchInput {
    pub prep_batch_id: PrepBatchId,
}

pub(crate) const GET_PREP_BATCH: CommandSpec = spec(
    "get_prep_batch",
    "analytical.get_prep_batch",
    PERFORM_ANALYSIS,
);
pub(crate) const LIST_PREP_BATCHES: CommandSpec = spec(
    "list_prep_batches",
    "analytical.list_prep_batches",
    PERFORM_ANALYSIS,
);

use super::PERFORM_ANALYSIS;
use super::names::name_prep_batch;
use crate::engine::Engine;
use crate::registry::{CommandSpec, spec};

pub(crate) const CREATE_PREP_BATCH: CommandSpec = spec(
    "create_prep_batch",
    actions::CREATE_PREP_BATCH,
    PERFORM_ANALYSIS,
);
pub(crate) const RECORD_PREP_STEP: CommandSpec = spec(
    "record_prep_step",
    actions::RECORD_PREP_STEP,
    PERFORM_ANALYSIS,
);
pub(crate) const COMPLETE_PREP_BATCH: CommandSpec = spec(
    "complete_prep_batch",
    actions::COMPLETE_PREP_BATCH,
    PERFORM_ANALYSIS,
);

impl Engine {
    /// Open a saved batch after navigation or restart.
    pub async fn get_prep_batch(
        &self,
        input: GetPrepBatchInput,
    ) -> Result<PrepBatchView, CommandError> {
        self.run(&GET_PREP_BATCH, |call| async move {
            self.read(&call, move |conn| {
                let mut batch = lis_analytical::prep_batch(conn, input.prep_batch_id)?;
                name_prep_batch(conn, &mut batch)?;
                Ok::<_, AnalyticalError>(batch)
            })
            .await
        })
        .await
    }

    /// Stable bounded browsing of preparation batches for a pinned method.
    pub async fn list_prep_batches(
        &self,
        input: ListPrepBatchesInput,
    ) -> Result<Vec<PrepBatchSummary>, CommandError> {
        self.run(&LIST_PREP_BATCHES, |call| async move {
            self.read(&call, move |conn| {
                lis_analytical::list_prep_batches(conn, input)
            })
            .await
        })
        .await
    }

    /// `create_prep_batch`.
    pub async fn create_prep_batch(
        &self,
        input: CreatePrepBatchInput,
    ) -> Result<PrepBatchView, CommandError> {
        self.run(&CREATE_PREP_BATCH, |call| async move {
            self.write(&call, move |ctx| {
                let mut batch = lis_analytical::create_prep_batch(ctx, input)?;
                name_prep_batch(ctx.tx(), &mut batch)?;
                Ok::<_, AnalyticalError>(batch)
            })
            .await
        })
        .await
    }

    /// `record_prep_step`.
    pub async fn record_prep_step(
        &self,
        input: RecordPrepStepInput,
    ) -> Result<PrepBatchView, CommandError> {
        self.run(&RECORD_PREP_STEP, |call| async move {
            self.write(&call, move |ctx| {
                let mut batch = lis_analytical::record_prep_step(ctx, input)?;
                name_prep_batch(ctx.tx(), &mut batch)?;
                Ok::<_, AnalyticalError>(batch)
            })
            .await
        })
        .await
    }

    /// `complete_prep_batch`.
    pub async fn complete_prep_batch(
        &self,
        input: CompletePrepBatchInput,
    ) -> Result<PrepBatchView, CommandError> {
        self.run(&COMPLETE_PREP_BATCH, |call| async move {
            self.write(&call, move |ctx| {
                let mut batch = lis_analytical::complete_prep_batch(ctx, input)?;
                name_prep_batch(ctx.tx(), &mut batch)?;
                Ok::<_, AnalyticalError>(batch)
            })
            .await
        })
        .await
    }
}
