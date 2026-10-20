//! What health evaluation reads (M4 design brief D8, §6.1): plain data the engine
//! gathers from the owners (backups, anchors, listeners, outbox, runs, versions) and
//! from the host probes. `None` means unknown or never, as each field says.

use std::time::Duration;

use lis_core::UtcTimestamp;
use serde::Serialize;
use ts_rs::TS;

use crate::health::AttestationCode;
use crate::host::{DiskEncryption, OsIdentity};
use crate::runs::RunFacts;
use crate::updates::VersionFacts;

/// Everything [`crate::health::evaluate`] reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HealthInputs {
    /// Backup targets, copies, restore tests and the recovery kit (§3.5).
    pub backup: BackupFacts,
    /// Anchor export lag (§4).
    pub anchors: AnchorFacts,
    /// Findings of the newest chain and anchor verification; `None` before the first.
    pub audit_chain_findings: Option<u32>,
    /// Free space and database size.
    pub disk: DiskFacts,
    /// Encryption of the data folder's disk; `None` is unknown.
    pub disk_encryption: Option<DiskEncryption>,
    /// The operating system; `None` is unknown.
    pub os: Option<OsIdentity>,
    /// App runs: gaps, unclean stops and web-engine change.
    pub runs: RunFacts,
    /// Analyzer listeners and sleep prevention.
    pub listeners: ListenerFacts,
    /// Clock observations.
    pub clock: ClockFacts,
    /// Outbox state.
    pub outbox: OutboxFacts,
    /// Approval and acceptance of the running version.
    pub version: VersionFacts,
    /// A recovery reconciliation is open.
    pub recovery_open: bool,
    /// Attestations valid now ([`crate::health::valid_attestations`]).
    pub attestations: Vec<Attestation>,
}

/// Backup facts (§3.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupFacts {
    /// Active targets that take backups.
    pub active_targets: u32,
    /// Active removable targets.
    pub removable_targets: u32,
    /// Age of the oldest audit event newer than the newest verified set; zero when up
    /// to date; `None` when no set was ever verified.
    pub backup_age: Option<Duration>,
    /// The same for off-host (removable and network) targets only.
    pub offhost_age: Option<Duration>,
    /// Age of the newest verified set on a removable target that is disconnected now;
    /// `None` when there is none.
    pub offline_age: Option<Duration>,
    /// When the newest restore test passed; `None` if none ever did.
    pub last_passed_restore_test_at: Option<UtcTimestamp>,
    /// The recovery kit.
    pub kit: KitFacts,
    /// Missing or damaged artifacts found by the newest backup run.
    pub artifact_problems: u32,
    /// Failed runs since the newest succeeded run; interrupted runs do not count.
    pub failed_runs_in_a_row: u32,
    /// The longest streak, over active backup targets, of failed copies since the
    /// target's newest verified copy (§3.5).
    pub failed_copies_in_a_row: u32,
    /// Whether the newest snapshot that reached its integrity check passed it (§3.1
    /// step 4); `None` before the first.
    pub snapshot_intact: Option<bool>,
}

/// Recovery-kit facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KitFacts {
    /// When the newest kit was exported; `None` if never.
    pub last_export_at: Option<UtcTimestamp>,
    /// The key or the backup locations changed since the newest kit.
    pub outdated: bool,
    /// When storage of the newest kit was last confirmed; `None` if never.
    pub last_confirmation_at: Option<UtcTimestamp>,
}

/// Anchor facts (§4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnchorFacts {
    /// Active off-host (removable and network) targets.
    pub offhost_targets: u32,
    /// Age of the oldest audit event not covered by an anchor on an off-host target;
    /// `None` when every event is covered.
    pub lag: Option<Duration>,
}

/// Disk facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiskFacts {
    /// Bytes available on the data folder's disk; `None` is unknown.
    pub available_bytes: Option<u64>,
    /// Size of the database.
    pub database_bytes: u64,
    /// Average growth of backup snapshots per day over 7 days, for the projection.
    pub growth_bytes_per_day: Option<u64>,
}

/// Listener facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ListenerFacts {
    /// Enabled listeners in `error`, watched folders included.
    pub enabled_in_error: u32,
    /// Files in watched folders waiting more than 5 minutes to be read.
    pub files_waiting: u32,
    /// Files in watched folders over the size limit, not read, since the app started.
    pub files_too_large: u32,
    /// Listeners listening, connected or receiving.
    pub active: u32,
    /// The sleep inhibitor's state.
    pub keep_awake: KeepAwakeState,
}

lis_core::text_enum! {
    /// The sleep inhibitor's state (§6.2).
    #[ts(export)]
    KeepAwakeState {
        /// Sleep is prevented.
        Held = "held",
        /// No listener is active.
        NotNeeded = "not_needed",
        /// The OS refused or the inhibitor failed.
        Unavailable = "unavailable",
    }
}

/// Clock facts (NFR-26).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockFacts {
    /// When the newest backward wall-clock jump was detected.
    pub last_backward_jump_at: Option<UtcTimestamp>,
    /// The newest audit `recorded_at`; a clock behind it is wrong.
    pub newest_record_at: Option<UtcTimestamp>,
    /// Time synchronization; `None` is unknown (only Linux is probed).
    pub time_sync: Option<bool>,
}

/// Outbox facts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OutboxFacts {
    /// Dead-lettered items.
    pub dead_letters: u32,
    /// Age of the oldest pending or leased item.
    pub oldest_pending_age: Option<Duration>,
}

/// A valid attestation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Attestation {
    /// What was attested.
    #[serde(skip)]
    pub code: AttestationCode,
    /// Who (principal ID text).
    pub by: String,
    /// When.
    pub at: UtcTimestamp,
    /// Until when it counts.
    pub valid_until: UtcTimestamp,
}
