//! The health checks (M4 design brief §6.1, binding): codes, states, labels, owners, next
//! actions and which checks a person may attest. One table for every check.

use lis_core::Capability;

lis_core::text_enum! {
    /// One health check (§6.1). The startup refusal of a network data folder is not a
    /// check: the app does not start there ([`crate::host::data_folder_on_network`]).
    #[ts(export)]
    CheckCode {
        /// An active backup target exists.
        BackupConfigured = "backup_configured",
        /// Age of the oldest audit event not in a verified backup.
        BackupAge = "backup_age",
        /// Failed backup runs, and failed copies per target, since the last success.
        BackupFailures = "backup_failures",
        /// The same, counting only off-host (removable and network) targets.
        OffhostCopyAge = "offhost_copy_age",
        /// Age of the newest verified set on a disconnected removable target (NFR-20).
        OfflineCopyAge = "offline_copy_age",
        /// Age of the last passed restore test.
        RestoreTestAge = "restore_test_age",
        /// The recovery kit is exported, current and confirmed stored.
        RecoveryKit = "recovery_kit",
        /// Lag of audit anchors on off-host targets (NFR-18).
        AnchorLag = "anchor_lag",
        /// Audit chain and anchor verification.
        AuditChain = "audit_chain",
        /// Artifact problems found by the last backup run.
        ArtifactStore = "artifact_store",
        /// The newest backup snapshot's integrity check (§3.1 step 4).
        DatabaseIntegrity = "database_integrity",
        /// Free space on the data folder's disk.
        DiskFree = "disk_free",
        /// Encryption of the data folder's disk.
        DiskEncryption = "disk_encryption",
        /// The operating system is within vendor support (NFR-25).
        OsSupport = "os_support",
        /// The web engine changed since the previous run (NFR-33).
        Webview = "webview",
        /// No enabled analyzer listener is in error.
        Listeners = "listeners",
        /// Sleep is prevented while listeners are active (REQ-OPS-013).
        KeepAwake = "keep_awake",
        /// Listening gaps not yet acknowledged (REQ-OPS-002).
        ListeningGaps = "listening_gaps",
        /// Unclean stops in the last 30 days.
        UncleanShutdowns = "unclean_shutdowns",
        /// Clock jumps, a clock behind the records, and time synchronization (NFR-26).
        Clock = "clock",
        /// Dead letters and old pending items in the outbox.
        Outbox = "outbox",
        /// The running version is approved and accepted (REQ-OPS-006).
        Version = "version",
        /// A recovery reconciliation is open after a restore.
        RecoveryReconciliation = "recovery_reconciliation",
        /// Power protection, which the app cannot read.
        Ups = "ups",
        /// Cloud services (none in this version).
        CloudServices = "cloud_services",
    }
}

lis_core::text_enum! {
    /// The state of a check. `unknown` counts as a warning in the overall state, so
    /// green never hides a backlog; `attested` replaces `unknown` while an attestation
    /// is valid.
    #[ts(export)]
    HealthState {
        /// Within limits.
        Ok = "ok",
        /// Needs attention.
        Warning = "warning",
        /// Needs action now.
        Alert = "alert",
        /// The app could not find out.
        Unknown = "unknown",
        /// A person attested what the app cannot read.
        Attested = "attested",
    }
}

lis_core::text_enum! {
    /// What a person may attest (`ops_attestations.check_code`).
    #[ts(export)]
    AttestationCode {
        /// The data folder's disk is encrypted.
        DiskEncryption = "disk_encryption",
        /// The computer is on a working UPS.
        Ups = "ups",
        /// The clock is synchronized to a time source.
        TimeSync = "time_sync",
        /// The app was smoke-tested on the changed web engine.
        WebviewSmoke = "webview_smoke",
        /// The operating system is supported, for example under Extended Security Updates.
        OsSupport = "os_support",
    }
}

impl HealthState {
    /// Severity for the overall state: unknown counts as a warning.
    fn severity(self) -> u8 {
        match self {
            Self::Ok | Self::Attested => 0,
            Self::Warning | Self::Unknown => 1,
            Self::Alert => 2,
        }
    }

    /// True when the check needs no attention.
    #[must_use]
    pub fn is_fine(self) -> bool {
        self.severity() == 0
    }
}

/// The overall state: the worst check, with `unknown` counted as a warning.
#[must_use]
pub fn overall(states: impl IntoIterator<Item = HealthState>) -> HealthState {
    match states.into_iter().map(HealthState::severity).max() {
        Some(2) => HealthState::Alert,
        Some(1) => HealthState::Warning,
        _ => HealthState::Ok,
    }
}

/// Per-check metadata: label, owner, next action and attestation code.
pub(crate) struct CheckInfo {
    pub(crate) code: CheckCode,
    pub(crate) label: &'static str,
    pub(crate) owner: Capability,
    pub(crate) next_action: &'static str,
    pub(crate) attestation: Option<AttestationCode>,
}

const fn row(
    code: CheckCode,
    label: &'static str,
    owner: Capability,
    next_action: &'static str,
    attestation: Option<AttestationCode>,
) -> CheckInfo {
    CheckInfo {
        code,
        label,
        owner,
        next_action,
        attestation,
    }
}

/// The §6.1 table, in [`CheckCode`] declaration order.
const CHECKS: [CheckInfo; 25] = {
    use Capability::{
        ApproveConfiguration, BackupRestore, ManageConfiguration, ManageQuality, PerformAnalysis,
        ViewAudit,
    };
    [
        row(
            CheckCode::BackupConfigured,
            "Backup target",
            BackupRestore,
            "Add a backup target.",
            None,
        ),
        row(
            CheckCode::BackupAge,
            "Backup age",
            BackupRestore,
            "Check the backup targets and run a backup.",
            None,
        ),
        row(
            CheckCode::BackupFailures,
            "Backup failures",
            BackupRestore,
            "Open the backup runs, fix the cause they show, then run a backup.",
            None,
        ),
        row(
            CheckCode::OffhostCopyAge,
            "Off-host copy",
            BackupRestore,
            "Connect a removable or network target and run a backup.",
            None,
        ),
        row(
            CheckCode::OfflineCopyAge,
            "Offline copy",
            BackupRestore,
            "Rotate a removable target: back up to it, then disconnect it.",
            None,
        ),
        row(
            CheckCode::RestoreTestAge,
            "Restore test",
            BackupRestore,
            "Run a restore test.",
            None,
        ),
        row(
            CheckCode::RecoveryKit,
            "Recovery kit",
            BackupRestore,
            "Export the recovery kit and confirm where it is stored.",
            None,
        ),
        row(
            CheckCode::AnchorLag,
            "Audit anchors",
            BackupRestore,
            "Connect an off-host target so anchors can be exported.",
            None,
        ),
        row(
            CheckCode::AuditChain,
            "Audit trail integrity",
            ViewAudit,
            "Review the verification findings and follow the incident procedure.",
            None,
        ),
        row(
            CheckCode::ArtifactStore,
            "Stored files",
            BackupRestore,
            "Review the files the last backup could not read.",
            None,
        ),
        row(
            CheckCode::DatabaseIntegrity,
            "Database integrity",
            ManageConfiguration,
            "Keep the earlier backups safe and follow the incident procedure to check the database.",
            None,
        ),
        row(
            CheckCode::DiskFree,
            "Free disk space",
            ManageConfiguration,
            "Free disk space or move to a larger disk.",
            None,
        ),
        row(
            CheckCode::DiskEncryption,
            "Disk encryption",
            ManageConfiguration,
            "Turn on disk encryption, or attest it if the app cannot read it.",
            Some(AttestationCode::DiskEncryption),
        ),
        row(
            CheckCode::OsSupport,
            "Operating system support",
            ManageConfiguration,
            "Plan an upgrade to a supported operating system.",
            Some(AttestationCode::OsSupport),
        ),
        row(
            CheckCode::Webview,
            "Web engine",
            ManageConfiguration,
            "Run the smoke check on the new web engine and attest it.",
            Some(AttestationCode::WebviewSmoke),
        ),
        row(
            CheckCode::Listeners,
            "Analyzer listeners",
            ManageConfiguration,
            "Check the analyzer connections.",
            None,
        ),
        row(
            CheckCode::KeepAwake,
            "Sleep prevention",
            ManageConfiguration,
            "Set the computer never to sleep while analyzers send results.",
            None,
        ),
        row(
            CheckCode::ListeningGaps,
            "Listening gaps",
            PerformAnalysis,
            "Check for results that could not arrive, then acknowledge the gap.",
            None,
        ),
        row(
            CheckCode::UncleanShutdowns,
            "Unclean shutdowns",
            ManageConfiguration,
            "Check the UPS and how the app is closed.",
            None,
        ),
        row(
            CheckCode::Clock,
            "Clock",
            ManageConfiguration,
            "Check the clock and its time synchronization.",
            Some(AttestationCode::TimeSync),
        ),
        row(
            CheckCode::Outbox,
            "Pending work",
            ManageConfiguration,
            "Review failed and waiting background work.",
            None,
        ),
        row(
            CheckCode::Version,
            "App version",
            ApproveConfiguration,
            "Record the update approval or accept the installed version.",
            None,
        ),
        row(
            CheckCode::RecoveryReconciliation,
            "Recovery reconciliation",
            ManageQuality,
            "Work through the recovery checklist and close it.",
            None,
        ),
        row(
            CheckCode::Ups,
            "Power protection",
            ManageConfiguration,
            "Attest that the computer is on a working UPS.",
            Some(AttestationCode::Ups),
        ),
        row(
            CheckCode::CloudServices,
            "Cloud services",
            ManageConfiguration,
            "",
            None,
        ),
    ]
};

/// Every check, in table order.
pub const ALL_CHECKS: [CheckCode; 25] = {
    let mut codes = [CheckCode::BackupConfigured; 25];
    let mut index = 0;
    while index < CHECKS.len() {
        codes[index] = CHECKS[index].code;
        index += 1;
    }
    codes
};

impl CheckCode {
    /// The attestation that applies to this check, if it is attestable.
    #[must_use]
    pub fn attestation(self) -> Option<AttestationCode> {
        self.info().attestation
    }

    /// The capability whose holders own this check's work items.
    #[must_use]
    pub fn owner(self) -> Capability {
        self.info().owner
    }

    pub(crate) fn info(self) -> &'static CheckInfo {
        // The table follows the declaration order; a test checks it.
        &CHECKS[self as usize]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_overall_state_is_the_worst_with_unknown_as_a_warning() {
        use HealthState::{Alert, Attested, Ok, Unknown, Warning};
        assert_eq!(overall([]), Ok);
        assert_eq!(overall([Ok, Attested]), Ok);
        assert_eq!(overall([Ok, Unknown, Attested]), Warning);
        assert_eq!(overall([Warning, Ok]), Warning);
        assert_eq!(overall([Unknown, Alert, Ok]), Alert);
    }

    #[test]
    fn the_table_follows_the_declaration_order() {
        for (index, check) in ALL_CHECKS.into_iter().enumerate() {
            assert_eq!(check as usize, index);
            assert_eq!(check.info().code, check);
        }
    }

    #[test]
    fn exactly_the_five_attestable_checks_map_to_attestation_codes() {
        let attestable: Vec<(CheckCode, AttestationCode)> = ALL_CHECKS
            .into_iter()
            .filter_map(|check| check.attestation().map(|code| (check, code)))
            .collect();
        assert_eq!(
            attestable,
            [
                (CheckCode::DiskEncryption, AttestationCode::DiskEncryption),
                (CheckCode::OsSupport, AttestationCode::OsSupport),
                (CheckCode::Webview, AttestationCode::WebviewSmoke),
                (CheckCode::Clock, AttestationCode::TimeSync),
                (CheckCode::Ups, AttestationCode::Ups),
            ]
        );
    }
}
