//! QC review, QC assessments, QC conflicts, deviations and instrument status
//! (REQ-ANA-004, REQ-ANA-005, REQ-ANA-007), all `manage_quality`. QC data and
//! aggregate QC reads omit patient sample labels. Exact deviation reads retain
//! free-text source records and are access-audited.

use lis_analytical::{
    AnalyticalError, AssessQcInput, CloseDeviationInput, DeviationView, InstrumentReadinessView,
    OpenDeviationInput, QcAssessmentView, QcConflictView, ResolveQcConflictInput, RunId, RunQcView,
    SetInstrumentStatusInput, actions,
};
use lis_core::{Capability, CommandError};
use serde::Deserialize;
use ts_rs::TS;

use super::names::{name_deviation, name_run_qc};
use crate::engine::Engine;
use crate::registry::{Access, CommandSpec, spec};

const MANAGE_QUALITY: Access = Access::Capability(Capability::ManageQuality);

pub(crate) const GET_RUN_QC: CommandSpec =
    spec("get_run_qc", "analytical.get_run_qc", MANAGE_QUALITY);
pub(crate) const ASSESS_QC: CommandSpec = spec("assess_qc", actions::ASSESS_QC, MANAGE_QUALITY);
pub(crate) const RESOLVE_QC_CONFLICT: CommandSpec = spec(
    "resolve_qc_conflict",
    actions::RESOLVE_QC_CONFLICT,
    MANAGE_QUALITY,
);
pub(crate) const OPEN_DEVIATION: CommandSpec =
    spec("open_deviation", actions::OPEN_DEVIATION, MANAGE_QUALITY);
pub(crate) const CLOSE_DEVIATION: CommandSpec =
    spec("close_deviation", actions::CLOSE_DEVIATION, MANAGE_QUALITY);
pub(crate) const SET_INSTRUMENT_STATUS: CommandSpec = spec(
    "set_instrument_status",
    actions::SET_INSTRUMENT_STATUS,
    MANAGE_QUALITY,
);

/// Input of `get_run_qc`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetRunQcInput {
    /// The run.
    pub run_id: RunId,
}

impl Engine {
    /// `get_run_qc`.
    pub async fn get_run_qc(&self, input: GetRunQcInput) -> Result<RunQcView, CommandError> {
        self.run(&GET_RUN_QC, |call| async move {
            self.read(&call, move |conn| {
                let mut view = lis_analytical::get_run_qc(conn, input.run_id)?;
                name_run_qc(conn, &mut view)?;
                Ok::<_, AnalyticalError>(view)
            })
            .await
        })
        .await
    }

    /// `assess_qc`.
    pub async fn assess_qc(&self, input: AssessQcInput) -> Result<QcAssessmentView, CommandError> {
        self.run(&ASSESS_QC, |call| async move {
            self.write(&call, move |ctx| lis_analytical::assess_qc(ctx, input))
                .await
        })
        .await
    }

    /// `resolve_qc_conflict`.
    pub async fn resolve_qc_conflict(
        &self,
        input: ResolveQcConflictInput,
    ) -> Result<QcConflictView, CommandError> {
        self.run(&RESOLVE_QC_CONFLICT, |call| async move {
            self.write(&call, move |ctx| {
                lis_analytical::resolve_qc_conflict(ctx, input)
            })
            .await
        })
        .await
    }

    /// `open_deviation`.
    pub async fn open_deviation(
        &self,
        input: OpenDeviationInput,
    ) -> Result<DeviationView, CommandError> {
        self.run(&OPEN_DEVIATION, |call| async move {
            self.write(&call, move |ctx| lis_analytical::open_deviation(ctx, input))
                .await
        })
        .await
    }

    /// `close_deviation`.
    pub async fn close_deviation(
        &self,
        input: CloseDeviationInput,
    ) -> Result<DeviationView, CommandError> {
        self.run(&CLOSE_DEVIATION, |call| async move {
            self.write(&call, move |ctx| {
                lis_analytical::close_deviation(ctx, input)
            })
            .await
        })
        .await
    }

    /// `set_instrument_status`.
    pub async fn set_instrument_status(
        &self,
        input: SetInstrumentStatusInput,
    ) -> Result<InstrumentReadinessView, CommandError> {
        self.run(&SET_INSTRUMENT_STATUS, |call| async move {
            self.write(&call, move |ctx| {
                lis_analytical::set_instrument_status(ctx, input)
            })
            .await
        })
        .await
    }
}

/// Open the exact deviation independently of its run, position or lot scope.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetDeviationInput {
    pub deviation_id: lis_analytical::DeviationId,
}
pub(crate) const GET_DEVIATION: CommandSpec =
    spec("get_deviation", "analytical.get_deviation", MANAGE_QUALITY);
impl Engine {
    pub async fn get_deviation(
        &self,
        input: GetDeviationInput,
    ) -> Result<DeviationView, CommandError> {
        self.run(&GET_DEVIATION, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let mut deviation = lis_analytical::get_deviation(conn, input.deviation_id)?;
                    name_deviation(conn, &mut deviation)?;
                    Ok::<_, AnalyticalError>(deviation)
                },
                move |_| {
                    crate::pipeline::access(GET_DEVIATION.action)
                        .record("deviation", input.deviation_id)
                },
            )
            .await
        })
        .await
    }
}
