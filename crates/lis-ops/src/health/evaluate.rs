//! The pure evaluation of every §6.1 rule (D8): [`evaluate`] turns [`HealthInputs`] and
//! the laboratory's thresholds into one [`CheckResult`] per check. No I/O, no clock:
//! `now` is passed in. Thresholds are breached at or above their value.

use std::time::Duration;

use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use super::checks::{ALL_CHECKS, AttestationCode, CheckCode, HealthState};
use super::inputs::{Attestation, HealthInputs};
use super::{backup_rules, host_rules};
use crate::policy::OperationsSettings;

/// One evaluated check, as the health screen shows it (`HealthCheckView`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export, rename = "HealthCheckView")]
pub struct CheckResult {
    /// The check.
    pub check: CheckCode,
    /// Its label.
    pub label: String,
    /// Its state.
    pub state: HealthState,
    /// Why it is not ok, as a machine code (kept in the history).
    pub detail_code: Option<String>,
    /// One PHI-free sentence.
    pub summary: String,
    /// The observed value, such as "35 min".
    pub value: Option<String>,
    /// The limit it was compared with.
    pub threshold: Option<String>,
    /// When it was evaluated.
    pub observed_at: UtcTimestamp,
    /// The code of the capability whose holders own the work item ([`CheckCode::owner`]).
    pub owner_capability: String,
    /// What to do, when the check needs attention.
    pub next_action: Option<String>,
    /// A person may attest it.
    pub attestable: bool,
    /// The valid attestation, if any.
    pub attestation: Option<Attestation>,
}

/// The outcome of one rule, before the check's metadata is added.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Finding {
    state: HealthState,
    detail: Option<&'static str>,
    summary: String,
    value: Option<String>,
    threshold: Option<String>,
}

impl Finding {
    pub(super) fn new(
        state: HealthState,
        detail: &'static str,
        summary: impl Into<String>,
    ) -> Self {
        Self {
            state,
            detail: (!state.is_fine() || state == HealthState::Attested).then_some(detail),
            summary: summary.into(),
            value: None,
            threshold: None,
        }
    }

    pub(super) fn ok(summary: impl Into<String>) -> Self {
        Self::new(HealthState::Ok, "", summary)
    }

    pub(super) fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    pub(super) fn threshold(mut self, threshold: impl Into<String>) -> Self {
        self.threshold = Some(threshold.into());
        self
    }
}

/// Evaluates every check at `now` (§6.1), in table order.
#[must_use]
pub fn evaluate(
    inputs: &HealthInputs,
    settings: &OperationsSettings,
    now: UtcTimestamp,
) -> Vec<CheckResult> {
    ALL_CHECKS
        .into_iter()
        .map(|check| {
            let finding = rule(check, inputs, settings, now);
            result(check, finding, inputs, now)
        })
        .collect()
}

fn rule(
    check: CheckCode,
    inputs: &HealthInputs,
    settings: &OperationsSettings,
    now: UtcTimestamp,
) -> Finding {
    match check {
        CheckCode::BackupConfigured => backup_rules::backup_configured(inputs),
        CheckCode::BackupAge => backup_rules::backup_age(inputs, settings),
        CheckCode::BackupFailures => backup_rules::backup_failures(inputs),
        CheckCode::OffhostCopyAge => backup_rules::offhost_copy_age(inputs, settings),
        CheckCode::OfflineCopyAge => backup_rules::offline_copy_age(inputs, settings),
        CheckCode::RestoreTestAge => backup_rules::restore_test_age(inputs, settings, now),
        CheckCode::RecoveryKit => backup_rules::recovery_kit(inputs, settings, now),
        CheckCode::AnchorLag => backup_rules::anchor_lag(inputs, settings),
        CheckCode::AuditChain => audit_chain(inputs),
        CheckCode::ArtifactStore => backup_rules::artifact_store(inputs),
        CheckCode::DatabaseIntegrity => backup_rules::database_integrity(inputs),
        CheckCode::ListeningGaps => listening_gaps(inputs),
        CheckCode::Outbox => outbox(inputs),
        CheckCode::Version => version(inputs),
        CheckCode::RecoveryReconciliation => recovery_reconciliation(inputs),
        CheckCode::CloudServices => Finding::ok("No cloud services configured."),
        CheckCode::DiskFree => host_rules::disk_free(inputs, settings),
        CheckCode::DiskEncryption => host_rules::disk_encryption(inputs, now),
        CheckCode::OsSupport => host_rules::os_support(inputs, settings, now),
        CheckCode::Webview => host_rules::webview(inputs, now),
        CheckCode::Listeners => host_rules::listeners(inputs),
        CheckCode::KeepAwake => host_rules::keep_awake(inputs),
        CheckCode::UncleanShutdowns => host_rules::unclean_shutdowns(inputs),
        CheckCode::Clock => host_rules::clock(inputs, now),
        CheckCode::Ups => host_rules::ups(inputs, now),
    }
}

fn result(
    check: CheckCode,
    finding: Finding,
    inputs: &HealthInputs,
    now: UtcTimestamp,
) -> CheckResult {
    let info = check.info();
    let attestation = info
        .attestation
        .and_then(|code| valid_attestation(inputs, code, now))
        .cloned();
    CheckResult {
        check,
        label: info.label.to_owned(),
        state: finding.state,
        detail_code: finding.detail.map(str::to_owned),
        summary: finding.summary,
        value: finding.value,
        threshold: finding.threshold,
        observed_at: now,
        owner_capability: info.owner.as_str().to_owned(),
        next_action: (!finding.state.is_fine()).then(|| info.next_action.to_owned()),
        attestable: info.attestation.is_some(),
        attestation,
    }
}

/// The newest attestation of `code` valid at `now`.
pub(super) fn valid_attestation(
    inputs: &HealthInputs,
    code: AttestationCode,
    now: UtcTimestamp,
) -> Option<&Attestation> {
    inputs
        .attestations
        .iter()
        .filter(|attestation| {
            attestation.code == code && attestation.at <= now && now < attestation.valid_until
        })
        .max_by_key(|attestation| attestation.at)
}

/// `attested` while a valid attestation of `code` exists, otherwise `unknown`.
pub(super) fn unknown_or_attested(
    inputs: &HealthInputs,
    code: AttestationCode,
    now: UtcTimestamp,
    unknown_summary: &str,
) -> Finding {
    if valid_attestation(inputs, code, now).is_some() {
        Finding::new(
            HealthState::Attested,
            "attested",
            "Attested by a person; the app cannot read it.",
        )
    } else {
        Finding::new(HealthState::Unknown, "not_readable", unknown_summary)
    }
}

pub(super) fn minutes(count: u32) -> Duration {
    Duration::from_secs(u64::from(count) * 60)
}

pub(super) fn hours(count: u32) -> Duration {
    minutes(count) * 60
}

pub(super) fn days(count: u32) -> Duration {
    hours(count) * 24
}

/// A duration as people read it: minutes up to 2 hours, hours up to 2 days, then days.
pub(super) fn age_text(age: Duration) -> String {
    let minutes = age.as_secs() / 60;
    match minutes {
        0..120 => format!("{minutes} min"),
        120..2880 => format!("{} h", minutes / 60),
        _ => format!("{} d", minutes / 1440),
    }
}

fn audit_chain(inputs: &HealthInputs) -> Finding {
    match inputs.audit_chain_findings {
        None => Finding::new(HealthState::Unknown, "not_verified", "Not verified yet."),
        Some(0) => Finding::ok("The audit chains and anchors verified without findings."),
        Some(count) => Finding::new(
            HealthState::Alert,
            "findings",
            format!("{count} verification finding(s)."),
        )
        .value(count.to_string()),
    }
}

fn listening_gaps(inputs: &HealthInputs) -> Finding {
    match inputs.runs.unacknowledged_gaps {
        0 => Finding::ok("No unacknowledged listening gap."),
        count => Finding::new(
            HealthState::Warning,
            "unacknowledged",
            format!("{count} listening gap(s) to acknowledge."),
        )
        .value(count.to_string()),
    }
}

fn outbox(inputs: &HealthInputs) -> Finding {
    let outbox = &inputs.outbox;
    if outbox.dead_letters > 0 {
        return Finding::new(
            HealthState::Warning,
            "dead_letters",
            format!("{} item(s) failed permanently.", outbox.dead_letters),
        )
        .value(outbox.dead_letters.to_string());
    }
    match outbox.oldest_pending_age {
        Some(age) if age >= hours(1) => Finding::new(
            HealthState::Warning,
            "old_pending",
            format!("The oldest waiting item is {} old.", age_text(age)),
        )
        .value(age_text(age))
        .threshold("warning at 1 h"),
        Some(_) | None => Finding::ok("No failed or long-waiting work."),
    }
}

fn version(inputs: &HealthInputs) -> Finding {
    if inputs.version.started_without_approval {
        return Finding::new(
            HealthState::Alert,
            "started_without_approval",
            "The running version started without an update approval.",
        );
    }
    if inputs.version.awaiting_acceptance {
        return Finding::new(
            HealthState::Warning,
            "awaiting_acceptance",
            "The running version is not yet accepted for clinical use.",
        );
    }
    Finding::ok("The running version is approved and accepted.")
}

fn recovery_reconciliation(inputs: &HealthInputs) -> Finding {
    if inputs.recovery_open {
        Finding::new(
            HealthState::Alert,
            "open",
            "Restored from a backup: the recovery reconciliation is open.",
        )
    } else {
        Finding::ok("No open recovery reconciliation.")
    }
}
