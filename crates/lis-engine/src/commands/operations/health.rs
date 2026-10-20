//! Host health, its history and attestations; app runs and listening gaps.

use std::time::{Duration, Instant};

use lis_core::{CommandError, ErrorCode, GateReason};
use lis_ops::RuntimeError;
use lis_ops::health::{CheckCode, CheckResult, HealthEventView};
use lis_ops::runs::AppRunView;
use serde::Deserialize;
use ts_rs::TS;

use super::{
    ACKNOWLEDGE_LISTENING_GAP, ATTEST_HEALTH_CHECK, GET_HOST_HEALTH, LIST_APP_RUNS,
    LIST_HEALTH_HISTORY, REFRESH_HOST_HEALTH, invalid,
};
use crate::engine::Engine;
use crate::ops_worker::{HostHealthView, current_health, evaluate_and_record, locked};
use crate::pipeline::Failure;

pub use lis_ops::health::AttestCheckInput as AttestHealthCheckInput;
pub use lis_ops::runs::AcknowledgeGapInput as AcknowledgeListeningGapInput;

/// `refresh_host_health` probes the host at most this often.
const REFRESH_INTERVAL: Duration = Duration::from_secs(60);
/// The most history rows or runs one call lists.
const MAX_LIST: u32 = 500;

/// Input of `list_health_history`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListHealthHistoryInput {
    /// One check, or every check.
    pub check: Option<CheckCode>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub before_event_id: Option<lis_ops::health::HealthEventId>,
    /// How many state changes, newest first (1–500).
    #[ts(type = "number")]
    pub limit: u32,
}

/// Input of `list_app_runs`.
#[derive(Debug, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct ListAppRunsInput {
    /// Exact queue target, independent of history pagination.
    #[serde(default)]
    #[ts(optional = nullable)]
    pub run_id: Option<lis_ops::runs::AppRunId>,
    #[serde(default)]
    #[ts(optional = nullable)]
    pub before_run_id: Option<lis_ops::runs::AppRunId>,
    /// How many runs, newest first (1–200).
    #[ts(type = "number")]
    pub limit: u32,
}

impl Engine {
    /// `get_host_health` (session): every check evaluated now, with the host and the
    /// keep-awake state. Host probes are the daily ones; nothing is recorded.
    pub async fn get_host_health(&self) -> Result<HostHealthView, CommandError> {
        self.run(&GET_HOST_HEALTH, |_| async move {
            Ok(current_health(&self.ops, false).await?)
        })
        .await
    }

    /// `refresh_host_health` (session): probes the host again, evaluates and records state
    /// changes. Refused with `conflict` within a minute of the previous refresh.
    pub async fn refresh_host_health(&self) -> Result<HostHealthView, CommandError> {
        self.run(&REFRESH_HOST_HEALTH, |_| async move {
            self.claim_refresh()?;
            Ok(evaluate_and_record(&self.ops, true).await?)
        })
        .await
    }

    /// Takes this minute's refresh, or refuses with `conflict` and `refresh_rate_limited`.
    fn claim_refresh(&self) -> Result<(), Failure> {
        let mut health = locked(&self.ops.health);
        if health
            .last_refresh
            .is_some_and(|at| at.elapsed() < REFRESH_INTERVAL)
        {
            return Err(Failure::Refused {
                code: ErrorCode::Conflict,
                message: "Health was refreshed less than a minute ago. Try again in a minute."
                    .to_owned(),
                field_errors: Vec::new(),
                gate_reasons: vec![GateReason {
                    code: "refresh_rate_limited".to_owned(),
                    message: "Refresh is allowed once a minute.".to_owned(),
                }],
            });
        }
        health.last_refresh = Some(Instant::now());
        Ok(())
    }

    /// `list_health_history` (session): state changes, newest first.
    pub async fn list_health_history(
        &self,
        input: ListHealthHistoryInput,
    ) -> Result<Vec<HealthEventView>, CommandError> {
        self.run(&LIST_HEALTH_HISTORY, |call| async move {
            let limit = checked_limit(input.limit, MAX_LIST)?;
            self.read(&call, move |conn| {
                lis_ops::health::list_health_history_page(
                    conn,
                    input.check,
                    limit,
                    input.before_event_id,
                )
            })
            .await
        })
        .await
    }

    /// `attest_health_check` (`manage_configuration`): records an attestation of an
    /// attestable check and returns the check evaluated with it.
    pub async fn attest_health_check(
        &self,
        input: AttestHealthCheckInput,
    ) -> Result<CheckResult, CommandError> {
        self.run(&ATTEST_HEALTH_CHECK, |call| async move {
            let check = input.check;
            self.write(&call, move |ctx| lis_ops::health::attest_check(ctx, input))
                .await?;
            current_health(&self.ops, false)
                .await?
                .checks
                .into_iter()
                .find(|result| result.check == check)
                .ok_or_else(|| Failure::Internal("attested check not evaluated".to_owned()))
        })
        .await
    }

    /// `list_app_runs` (session): runs newest first, with gaps and stops.
    pub async fn list_app_runs(
        &self,
        input: ListAppRunsInput,
    ) -> Result<Vec<AppRunView>, CommandError> {
        self.run(&LIST_APP_RUNS, |call| async move {
            let limit = checked_limit(input.limit, 200)?;
            if input.run_id.is_some() && input.before_run_id.is_some() {
                return Err(invalid(
                    "runId",
                    "conflicting_cursor",
                    "Choose an exact run or a history page.",
                ));
            }
            self.read(&call, move |conn| match input.run_id {
                Some(id) => lis_ops::runs::get_app_run(conn, id).map(|row| vec![row]),
                None => lis_ops::runs::list_app_runs_page(conn, limit, input.before_run_id),
            })
            .await
        })
        .await
    }

    /// `acknowledge_listening_gap` (`perform_analysis`): records what was done about results
    /// that could not arrive while the app was closed (clinical stream).
    pub async fn acknowledge_listening_gap(
        &self,
        input: AcknowledgeListeningGapInput,
    ) -> Result<AppRunView, CommandError> {
        self.run(&ACKNOWLEDGE_LISTENING_GAP, |call| async move {
            self.write(&call, move |ctx| -> Result<_, RuntimeError> {
                lis_ops::runs::acknowledge_gap(ctx, input)
            })
            .await
        })
        .await
    }
}

/// `limit` when it is 1 to `max`; a validation refusal otherwise.
fn checked_limit(limit: u32, max: u32) -> Result<u32, Failure> {
    if (1..=max).contains(&limit) {
        Ok(limit)
    } else {
        Err(invalid(
            "limit",
            "out_of_range",
            &format!("Choose 1 to {max} records."),
        ))
    }
}
