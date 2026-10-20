//! What the poller remembers between scans within one run: files waiting to become
//! ready, files known to be received, oversize files found, and the last state
//! reported (§6).

use std::collections::{HashMap, HashSet};
use std::time::Duration;

use lis_core::{InstrumentId, UtcTimestamp};
use lis_interfaces::{ListenerDetailCode, ListenerState};

use super::files::Candidate;

/// A file is ready when its size and time are unchanged for at least this long.
const STABLE_AFTER: Duration = Duration::from_secs(10);
/// A file waiting longer than this raises the instrument's health signal.
const WAITING_ALERT: Duration = Duration::from_secs(5 * 60);
/// The wait before a file whose receipt failed is tried again; it doubles with
/// each further failure, up to [`RETRY_MAX`], so a full disk or a locked
/// database does not re-read every file at every scan.
const RETRY_FIRST: Duration = Duration::from_secs(10);
const RETRY_MAX: Duration = Duration::from_secs(5 * 60);

/// What the poller remembers within one run of the app.
#[derive(Default)]
pub(super) struct PollState {
    /// Files seen but not yet received, by instrument and file name.
    pub(super) pending: HashMap<(InstrumentId, String), Pending>,
    /// Files known to be in the ledger, so it is read once per file and run.
    pub(super) received: HashSet<(InstrumentId, String, u64, UtcTimestamp)>,
    /// Files over the size limit found per instrument in this run (F14). The
    /// health signal lasts until the app restarts; the receipt stays.
    pub(super) too_large: HashMap<InstrumentId, u32>,
    /// The last state reported per instrument, to log changes only.
    pub(super) reported: HashMap<InstrumentId, (ListenerState, Option<ListenerDetailCode>)>,
}

/// A file waiting to become ready.
pub(super) struct Pending {
    byte_len: u64,
    modified_at: UtcTimestamp,
    /// The first scan that saw this size and time.
    pub(super) unchanged_since: UtcTimestamp,
    /// The first scan that saw the file at all.
    first_seen: UtcTimestamp,
    /// The last receipt failed; logged once until the file changes.
    pub(super) failing: bool,
    /// The receipt was refused for good; the file waits until it changes.
    pub(super) refused: bool,
    /// Receipt failures in a row at this size and time.
    failures: u32,
    /// No receipt is tried before this time after a failure.
    retry_at: Option<UtcTimestamp>,
}

impl Pending {
    /// Records a failed receipt at `now`: the next try waits [`RETRY_FIRST`],
    /// doubled for each further failure, at most [`RETRY_MAX`].
    pub(super) fn back_off(&mut self, now: UtcTimestamp) {
        self.failures = self.failures.saturating_add(1);
        let doublings = self.failures.saturating_sub(1).min(16);
        let wait = RETRY_FIRST.saturating_mul(1 << doublings).min(RETRY_MAX);
        self.retry_at = now.checked_add(wait);
    }
}

impl PollState {
    /// Records this scan's sighting; true when the file had the same size and time at a
    /// scan at least [`STABLE_AFTER`] earlier, was not refused at that size and time, and
    /// is not backing off after a failed receipt.
    pub(super) fn observe(
        &mut self,
        instrument_id: InstrumentId,
        candidate: &Candidate,
        now: UtcTimestamp,
    ) -> bool {
        let pending = self
            .pending
            .entry((instrument_id, candidate.name.clone()))
            .or_insert(Pending {
                byte_len: candidate.byte_len,
                modified_at: candidate.modified_at,
                unchanged_since: now,
                first_seen: now,
                failing: false,
                refused: false,
                failures: 0,
                retry_at: None,
            });
        // A clock set back restarts the wait rather than stretching it.
        if pending.byte_len != candidate.byte_len
            || pending.modified_at != candidate.modified_at
            || now < pending.unchanged_since
        {
            pending.byte_len = candidate.byte_len;
            pending.modified_at = candidate.modified_at;
            pending.unchanged_since = now;
            pending.failing = false;
            pending.refused = false;
            pending.failures = 0;
            pending.retry_at = None;
            return false;
        }
        !pending.refused
            && pending.retry_at.is_none_or(|at| now >= at)
            && now.saturating_duration_since(pending.unchanged_since) >= STABLE_AFTER
    }

    /// Files of the instrument seen more than [`WAITING_ALERT`] ago and not received.
    pub(super) fn waiting(&self, instrument_id: InstrumentId, now: UtcTimestamp) -> u32 {
        let count = self
            .pending
            .iter()
            .filter(|((id, _), pending)| {
                *id == instrument_id
                    && now.saturating_duration_since(pending.first_seen) > WAITING_ALERT
            })
            .count();
        u32::try_from(count).unwrap_or(u32::MAX)
    }

    /// Oversize files of the instrument found in this run.
    pub(super) fn too_large(&self, instrument_id: InstrumentId) -> u32 {
        self.too_large.get(&instrument_id).copied().unwrap_or(0)
    }
}
