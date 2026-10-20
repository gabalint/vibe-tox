//! Bounded, in-memory aggregation of rejected analyzer-peer connections, so a
//! connection flood cannot amplify into one audit write per connection
//! (REQ-INT-009, NFR-08). The TCP accept loop closes a rejected socket at once,
//! records it here, and audits only the summaries [`RejectionTally::take_due`]
//! returns.

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::time::{Duration, Instant};

/// Minimum time between two audit events for the same peer (or for the
/// untracked-peer bucket).
pub(crate) const REJECTION_WINDOW: Duration = Duration::from_secs(60);
/// Peers tracked individually; rejections from further addresses share one
/// untracked-peer bucket, so memory stays bounded however many sources connect.
pub(crate) const MAX_TRACKED_PEERS: usize = 64;

/// Rejected connections to audit for one peer, or for every untracked peer
/// when `peer` is `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PeerRejections {
    pub(crate) peer: Option<IpAddr>,
    pub(crate) rejected_connections: usize,
}

/// Unaudited rejection counts per peer, released at most once per
/// [`REJECTION_WINDOW`] per tally. The first rejection of a quiet peer is due
/// at once, so a misconfigured analyzer becomes visible promptly.
#[derive(Debug, Default)]
pub(crate) struct RejectionTally {
    peers: BTreeMap<IpAddr, Counter>,
    untracked: Option<Counter>,
}

#[derive(Debug, Default)]
struct Counter {
    unaudited: usize,
    last_audit: Option<Instant>,
}

impl RejectionTally {
    /// Counts one rejected connection; does no I/O.
    pub(crate) fn record(&mut self, peer: IpAddr) {
        let counter = if self.peers.len() < MAX_TRACKED_PEERS || self.peers.contains_key(&peer) {
            self.peers.entry(peer).or_default()
        } else {
            self.untracked.get_or_insert_default()
        };
        counter.unaudited = counter.unaudited.saturating_add(1);
    }

    /// Returns the counts whose audit is due at `now`, and forgets peers that
    /// stayed quiet for a whole window after their last audit.
    pub(crate) fn take_due(&mut self, now: Instant) -> Vec<PeerRejections> {
        let mut due = Vec::new();
        self.peers
            .retain(|peer, counter| counter.settle(Some(*peer), now, &mut due));
        if self
            .untracked
            .as_mut()
            .is_some_and(|counter| !counter.settle(None, now, &mut due))
        {
            self.untracked = None;
        }
        due
    }

    /// Returns every unaudited count regardless of window, for listener stop.
    pub(crate) fn take_remaining(self) -> Vec<PeerRejections> {
        let tracked = self
            .peers
            .into_iter()
            .map(|(peer, counter)| (Some(peer), counter));
        tracked
            .chain(self.untracked.map(|counter| (None, counter)))
            .filter(|(_, counter)| counter.unaudited > 0)
            .map(|(peer, counter)| PeerRejections {
                peer,
                rejected_connections: counter.unaudited,
            })
            .collect()
    }

    #[cfg(test)]
    fn tracked_peers(&self) -> usize {
        self.peers.len()
    }
}

impl Counter {
    /// Moves the unaudited count into `due` unless an audit for this tally was
    /// written less than one window ago. Returns `false` when the tally is
    /// quiet and its window has closed, so it can be forgotten.
    fn settle(
        &mut self,
        peer: Option<IpAddr>,
        now: Instant,
        due: &mut Vec<PeerRejections>,
    ) -> bool {
        let window_open = self
            .last_audit
            .is_some_and(|at| now.saturating_duration_since(at) < REJECTION_WINDOW);
        if window_open {
            return true;
        }
        if self.unaudited == 0 {
            return false;
        }
        due.push(PeerRejections {
            peer,
            rejected_connections: self.unaudited,
        });
        self.unaudited = 0;
        self.last_audit = Some(now);
        true
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, Ipv6Addr};

    use super::*;

    const ONE: IpAddr = IpAddr::V4(Ipv4Addr::new(192, 168, 50, 10));

    fn synthetic_peer(index: u32) -> IpAddr {
        let [_, b, c, d] = index.to_be_bytes();
        IpAddr::V4(Ipv4Addr::new(10, b, c, d))
    }

    fn total(due: &[PeerRejections]) -> usize {
        due.iter().map(|item| item.rejected_connections).sum()
    }

    #[test]
    fn first_rejection_is_due_at_once_and_a_burst_waits_for_the_window() {
        let start = Instant::now();
        let mut tally = RejectionTally::default();
        tally.record(ONE);
        assert_eq!(
            tally.take_due(start),
            [PeerRejections {
                peer: Some(ONE),
                rejected_connections: 1
            }]
        );
        for _ in 0..999 {
            tally.record(ONE);
            assert!(tally.take_due(start + Duration::from_secs(1)).is_empty());
        }
        let just_before = start + REJECTION_WINDOW - Duration::from_millis(1);
        assert!(tally.take_due(just_before).is_empty());
        assert_eq!(
            tally.take_due(start + REJECTION_WINDOW),
            [PeerRejections {
                peer: Some(ONE),
                rejected_connections: 999
            }]
        );
    }

    #[test]
    fn quiet_peer_is_forgotten_after_one_window_without_another_audit() {
        let start = Instant::now();
        let mut tally = RejectionTally::default();
        tally.record(ONE);
        assert_eq!(total(&tally.take_due(start)), 1);
        assert!(tally.take_due(start + REJECTION_WINDOW).is_empty());
        assert_eq!(tally.tracked_peers(), 0);
        tally.record(ONE);
        assert_eq!(total(&tally.take_due(start + REJECTION_WINDOW)), 1);
    }

    #[test]
    fn many_distinct_addresses_stay_bounded_and_overflow_into_one_bucket() {
        let start = Instant::now();
        let mut tally = RejectionTally::default();
        let mut audits = Vec::new();
        for index in 0..10_000 {
            tally.record(synthetic_peer(index));
            tally.record(ONE);
            audits.extend(tally.take_due(start));
            assert!(tally.tracked_peers() <= MAX_TRACKED_PEERS);
        }
        assert_eq!(audits.len(), MAX_TRACKED_PEERS + 1);
        assert_eq!(audits.iter().filter(|item| item.peer.is_none()).count(), 1);
        let later = tally.take_due(start + REJECTION_WINDOW);
        assert!(later.len() <= MAX_TRACKED_PEERS + 1);
        assert_eq!(total(&audits) + total(&later), 20_000);
    }

    #[test]
    fn remaining_counts_are_released_at_stop_inside_the_window() {
        let start = Instant::now();
        let mut tally = RejectionTally::default();
        let v6 = IpAddr::V6(Ipv6Addr::new(0xfd00, 0, 0, 0, 0, 0, 0, 7));
        tally.record(ONE);
        tally.record(v6);
        assert_eq!(total(&tally.take_due(start)), 2);
        tally.record(ONE);
        tally.record(ONE);
        assert!(tally.take_due(start).is_empty());
        assert_eq!(
            tally.take_remaining(),
            [PeerRejections {
                peer: Some(ONE),
                rejected_connections: 2
            }]
        );
    }
}
