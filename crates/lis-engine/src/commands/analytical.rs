//! Instrument readiness, source registrations, lots, the worklist, runs and
//! positions (REQ-ANA-001, REQ-ANA-003 registration part, REQ-ANA-006,
//! REQ-ANA-007, REQ-OPS-001). Preparation batches are in [`prep`], QC
//! commands in [`qc`].
//!
//! No analytical view carries demographics, but the worklist and run detail
//! disclose container labels and accession numbers that lead to a patient,
//! so they are audited reads, and every command returning a run appends an
//! access event in its transaction before returning it (security standard,
//! sensitive reads).
//! Readiness, lots and registrations are laboratory configuration that
//! analysts, quality staff and configuration authors all need, so any
//! signed-in user may list them; the same holds for reading runs, which QC
//! review starts from.

pub(crate) mod names;
pub(crate) mod performing_site;
pub(crate) mod prep;
pub(crate) mod qc;

pub(crate) use prep::{CREATE_PREP_BATCH, RECORD_PREP_STEP};

use lis_analytical::{
    AddPatientPositionInput, AddQcPositionInput, AnalyticalError, ApproveSourceInput,
    CloseRunInput, CreateRunInput, FailExecutionInput, InstrumentReadinessView, LotView,
    RegisterLotInput, RegisterSourceInput, RunDetail, RunId, RunState, RunSummary,
    SourceRegistrationView, WithdrawLotInput, WorklistEntry, actions,
};
use lis_audit::AuditDetail;
use lis_catalog::MethodVersionId;
use lis_core::{Capability, CommandError, InstrumentId};
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use crate::engine::Engine;
use crate::pipeline::{access, domain_error};
use crate::registry::{Access, CommandSpec, spec};

const PERFORM_ANALYSIS: Access = Access::Capability(Capability::PerformAnalysis);
const MANAGE_QUALITY: Access = Access::Capability(Capability::ManageQuality);

pub(crate) const LIST_INSTRUMENT_READINESS: CommandSpec = spec(
    "list_instrument_readiness",
    "analytical.list_instrument_readiness",
    Access::Session,
);
pub(crate) const LIST_SOURCE_REGISTRATIONS: CommandSpec = spec(
    "list_source_registrations",
    "analytical.list_source_registrations",
    Access::Session,
);
pub(crate) const REGISTER_SOURCE: CommandSpec = spec(
    "register_source",
    actions::REGISTER_SOURCE,
    Access::Capability(Capability::ManageConfiguration),
);
pub(crate) const APPROVE_SOURCE: CommandSpec = spec(
    "approve_source",
    actions::APPROVE_SOURCE,
    Access::Capability(Capability::ApproveConfiguration),
);
pub(crate) const LIST_LOTS: CommandSpec =
    spec("list_lots", "analytical.list_lots", Access::Session);
pub(crate) const REGISTER_LOT: CommandSpec =
    spec("register_lot", actions::REGISTER_LOT, MANAGE_QUALITY);
pub(crate) const WITHDRAW_LOT: CommandSpec =
    spec("withdraw_lot", actions::WITHDRAW_LOT, MANAGE_QUALITY);
pub(crate) const LIST_WORKLIST: CommandSpec = spec(
    "list_worklist",
    "analytical.list_worklist",
    PERFORM_ANALYSIS,
);
pub(crate) const CREATE_RUN: CommandSpec =
    spec("create_run", actions::CREATE_RUN, PERFORM_ANALYSIS);
// Runs carry container label codes but no demographics, and quality users
// reach QC review through them, so any signed-in user may read them; each
// read is audited.
pub(crate) const GET_RUN: CommandSpec = spec("get_run", "analytical.get_run", Access::Session);
pub(crate) const LIST_RUNS: CommandSpec =
    spec("list_runs", "analytical.list_runs", Access::Session);
pub(crate) const ADD_PATIENT_POSITION: CommandSpec = spec(
    "add_patient_position",
    actions::ADD_PATIENT_POSITION,
    PERFORM_ANALYSIS,
);
pub(crate) const ADD_QC_POSITION: CommandSpec = spec(
    "add_qc_position",
    actions::ADD_QC_POSITION,
    PERFORM_ANALYSIS,
);
pub(crate) const FAIL_EXECUTION: CommandSpec =
    spec("fail_execution", actions::FAIL_EXECUTION, PERFORM_ANALYSIS);
pub(crate) const CLOSE_RUN: CommandSpec = spec("close_run", actions::CLOSE_RUN, PERFORM_ANALYSIS);

/// Input of `list_source_registrations`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListSourceRegistrationsInput {
    /// The instrument.
    pub instrument_id: InstrumentId,
}

/// Input of `list_lots`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListLotsInput {
    /// Only lots that are not withdrawn.
    pub active_only: bool,
}

/// Input of `list_worklist`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListWorklistInput {
    /// The method version whose pending tests to list.
    pub method_version_id: MethodVersionId,
}

/// Input of `get_run`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct GetRunInput {
    /// The run.
    pub run_id: RunId,
}

/// Input of `list_runs`.
#[derive(Debug, Clone, Copy, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListRunsInput {
    /// Only runs in this state.
    #[serde(default)]
    #[ts(optional)]
    pub state: Option<RunState>,
    /// Only runs of this instrument.
    #[serde(default)]
    #[ts(optional)]
    pub instrument_id: Option<InstrumentId>,
    /// Frozen method filter used for repeat placement.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub method_version_id: Option<MethodVersionId>,
    /// Last row of the previous stable page.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub after_run_id: Option<RunId>,
    /// Page size (1–500); omitted preserves the historical 500-row request.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub limit: Option<u32>,
}

domain_error!(
    AnalyticalError,
    AnalyticalError::field_errors,
    |error: &AnalyticalError| error.gate_reasons().to_vec()
);

/// The access event for a run disclosed to the caller.
fn run_access(action: &'static str, run: &RunDetail) -> lis_audit::AuditEvent {
    access(action)
        .record("run", run.run_id)
        .detail("positions", AuditDetail::count(run.positions.len()))
}

/// Appends the access event for the run a command returns, in its transaction.
fn audit_run_response(
    ctx: &mut CommandContext<'_>,
    action: &'static str,
    mut run: RunDetail,
) -> Result<RunDetail, AnalyticalError> {
    ctx.audit(run_access(action, &run))?;
    names::name_run(ctx.tx(), &mut run)?;
    Ok(run)
}

impl Engine {
    /// `list_instrument_readiness`.
    pub async fn list_instrument_readiness(
        &self,
    ) -> Result<Vec<InstrumentReadinessView>, CommandError> {
        self.run(&LIST_INSTRUMENT_READINESS, |call| async move {
            self.read(&call, lis_analytical::list_instrument_readiness)
                .await
        })
        .await
    }

    /// `list_source_registrations`.
    pub async fn list_source_registrations(
        &self,
        input: ListSourceRegistrationsInput,
    ) -> Result<Vec<SourceRegistrationView>, CommandError> {
        self.run(&LIST_SOURCE_REGISTRATIONS, |call| async move {
            self.read(&call, move |conn| {
                let mut sources =
                    lis_analytical::list_source_registrations(conn, input.instrument_id)?;
                names::name_sources(conn, &mut sources)?;
                Ok::<_, AnalyticalError>(sources)
            })
            .await
        })
        .await
    }

    /// `register_source`.
    pub async fn register_source(
        &self,
        input: RegisterSourceInput,
    ) -> Result<SourceRegistrationView, CommandError> {
        self.run(&REGISTER_SOURCE, |call| async move {
            self.write(&call, move |ctx| {
                lis_analytical::register_source(ctx, input)
            })
            .await
        })
        .await
    }

    /// `approve_source`.
    pub async fn approve_source(
        &self,
        input: ApproveSourceInput,
    ) -> Result<SourceRegistrationView, CommandError> {
        self.run(&APPROVE_SOURCE, |call| async move {
            self.write(&call, move |ctx| lis_analytical::approve_source(ctx, input))
                .await
        })
        .await
    }

    /// `list_lots`, with expiry derived at the engine's current time.
    pub async fn list_lots(&self, input: ListLotsInput) -> Result<Vec<LotView>, CommandError> {
        let now = self.clock.now();
        self.run(&LIST_LOTS, |call| async move {
            self.read(&call, move |conn| {
                lis_analytical::list_lots(conn, input.active_only, now)
            })
            .await
        })
        .await
    }

    /// `register_lot`.
    pub async fn register_lot(&self, input: RegisterLotInput) -> Result<LotView, CommandError> {
        self.run(&REGISTER_LOT, |call| async move {
            self.write(&call, move |ctx| lis_analytical::register_lot(ctx, input))
                .await
        })
        .await
    }

    /// `withdraw_lot`.
    pub async fn withdraw_lot(&self, input: WithdrawLotInput) -> Result<LotView, CommandError> {
        self.run(&WITHDRAW_LOT, |call| async move {
            self.write(&call, move |ctx| lis_analytical::withdraw_lot(ctx, input))
                .await
        })
        .await
    }

    /// `list_worklist` (audited read).
    pub async fn list_worklist(
        &self,
        input: ListWorklistInput,
    ) -> Result<Vec<WorklistEntry>, CommandError> {
        self.run(&LIST_WORKLIST, |call| async move {
            self.audited_read(
                &call,
                move |conn| lis_analytical::list_worklist(conn, input.method_version_id),
                move |entries| {
                    access(LIST_WORKLIST.action)
                        .detail("method_version", AuditDetail::id(input.method_version_id))
                        .detail("returned", AuditDetail::count(entries.len()))
                },
            )
            .await
        })
        .await
    }

    /// `create_run`.
    pub async fn create_run(&self, input: CreateRunInput) -> Result<RunDetail, CommandError> {
        self.run(&CREATE_RUN, |call| async move {
            self.write(&call, move |ctx| {
                let run = lis_analytical::create_run(ctx, input)?;
                audit_run_response(ctx, CREATE_RUN.action, run)
            })
            .await
        })
        .await
    }

    /// `get_run` (audited read), with lot expiry derived at the engine's current time.
    pub async fn get_run(&self, input: GetRunInput) -> Result<RunDetail, CommandError> {
        let now = self.clock.now();
        self.run(&GET_RUN, |call| async move {
            self.audited_read(
                &call,
                move |conn| {
                    let mut run = lis_analytical::get_run(conn, input.run_id, now)?;
                    names::name_run(conn, &mut run)?;
                    Ok::<_, AnalyticalError>(run)
                },
                |run| run_access(GET_RUN.action, run),
            )
            .await
        })
        .await
    }

    /// `list_runs`.
    pub async fn list_runs(&self, input: ListRunsInput) -> Result<Vec<RunSummary>, CommandError> {
        self.run(&LIST_RUNS, |call| async move {
            self.read(&call, move |conn| {
                lis_analytical::list_runs_page(
                    conn,
                    lis_analytical::ListRunsPageInput {
                        state: input.state,
                        instrument_id: input.instrument_id,
                        method_version_id: input.method_version_id,
                        after_run_id: input.after_run_id,
                        limit: input.limit.unwrap_or(500),
                    },
                )
            })
            .await
        })
        .await
    }

    /// `add_patient_position`.
    pub async fn add_patient_position(
        &self,
        input: AddPatientPositionInput,
    ) -> Result<RunDetail, CommandError> {
        self.run(&ADD_PATIENT_POSITION, |call| async move {
            self.write(&call, move |ctx| {
                let run = lis_analytical::add_patient_position(ctx, input)?;
                audit_run_response(ctx, ADD_PATIENT_POSITION.action, run)
            })
            .await
        })
        .await
    }

    /// `add_qc_position`.
    pub async fn add_qc_position(
        &self,
        input: AddQcPositionInput,
    ) -> Result<RunDetail, CommandError> {
        self.run(&ADD_QC_POSITION, |call| async move {
            self.write(&call, move |ctx| {
                let run = lis_analytical::add_qc_position(ctx, input)?;
                audit_run_response(ctx, ADD_QC_POSITION.action, run)
            })
            .await
        })
        .await
    }

    /// `fail_execution`.
    pub async fn fail_execution(
        &self,
        input: FailExecutionInput,
    ) -> Result<RunDetail, CommandError> {
        self.run(&FAIL_EXECUTION, |call| async move {
            self.write(&call, move |ctx| {
                let run = lis_analytical::fail_execution(ctx, input)?;
                audit_run_response(ctx, FAIL_EXECUTION.action, run)
            })
            .await
        })
        .await
    }

    /// `close_run`.
    pub async fn close_run(&self, input: CloseRunInput) -> Result<RunDetail, CommandError> {
        self.run(&CLOSE_RUN, |call| async move {
            self.write(&call, move |ctx| {
                let run = lis_analytical::close_run(ctx, input)?;
                audit_run_response(ctx, CLOSE_RUN.action, run)
            })
            .await
        })
        .await
    }
}
