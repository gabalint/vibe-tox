//! Anchor lag and the anchor screens' queries (M4 design brief §4, §6.1 `anchor_lag`;
//! `get_anchor_status` and `list_anchor_exports` of the M4 contract). Not audited: they
//! show sequence numbers, times, counts and target labels only.

use std::time::Duration;

use lis_audit::Stream;
use lis_core::UtcTimestamp;
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::repo;
use super::{AnchorError, AnchorExportId};
use crate::health::AnchorFacts;
use crate::targets::{self, Target};

/// Most anchor files one list returns.
const LIST_LIMIT_MAX: u32 = 200;

/// One stream's chain, seal and off-host anchor positions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StreamAnchorView {
    /// `clinical`, `security` or `access`.
    pub stream: String,
    /// The newest event's sequence number; 0 for an empty stream.
    #[ts(type = "number")]
    pub head_seq: i64,
    /// The newest seal's last sequence number; 0 before the first seal.
    #[ts(type = "number")]
    pub sealed_through_seq: i64,
    /// The last sequence number covered by a seal exported to an active off-host target.
    #[ts(type = "number")]
    pub anchored_through_seq: i64,
    /// When the oldest event not anchored off-host was recorded; `None` when every event is.
    pub oldest_unanchored_at: Option<UtcTimestamp>,
}

/// The newest anchor file written.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LastExportView {
    /// When it was written.
    pub at: UtcTimestamp,
    /// Its target's label.
    pub target_label: String,
}

/// The anchor status (`get_anchor_status`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnchorStatusView {
    /// One entry per stream, in stream order.
    pub streams: Vec<StreamAnchorView>,
    /// Whole minutes since the oldest event not anchored off-host; `None` when none is.
    #[ts(type = "number | null")]
    pub lag_minutes: Option<u64>,
    /// Active removable and network-share targets.
    #[ts(type = "number")]
    pub off_host_targets: u32,
    /// The newest anchor file written to any target.
    pub last_export: Option<LastExportView>,
    #[serde(skip)]
    #[ts(skip)]
    lag: Option<Duration>,
}

impl AnchorStatusView {
    /// The `anchor_lag` health check's input (§6.1).
    #[must_use]
    pub fn health_facts(&self) -> AnchorFacts {
        AnchorFacts {
            offhost_targets: self.off_host_targets,
            lag: self.lag,
        }
    }
}

/// Anchor positions per stream and the lag at `now` (NFR-18, REQ-OPS-008). An event is
/// anchored off-host when a seal covering it was exported to an active removable or
/// network-share target; a retired target no longer counts.
pub fn get_anchor_status(
    conn: &Connection,
    now: UtcTimestamp,
) -> Result<AnchorStatusView, AnchorError> {
    let off_host: Vec<Target> = targets::active(conn)?
        .into_iter()
        .filter(|target| target.kind.is_off_host())
        .collect();
    let mut streams = Vec::with_capacity(Stream::ALL.len());
    for head in lis_audit::heads(conn)? {
        let anchored = anchored_through_seq(conn, head.stream, &off_host)?;
        let oldest_unanchored_at = if head.seq > anchored {
            lis_audit::event_recorded_at(conn, head.stream, anchored + 1)?
        } else {
            None
        };
        streams.push(StreamAnchorView {
            stream: head.stream.as_str().to_owned(),
            head_seq: head.seq,
            sealed_through_seq: lis_audit::last_seal(conn, head.stream)?
                .map_or(0, |seal| seal.last_seq),
            anchored_through_seq: anchored,
            oldest_unanchored_at,
        });
    }
    let lag = streams
        .iter()
        .filter_map(|stream| stream.oldest_unanchored_at)
        .min()
        .map(|oldest| now.saturating_duration_since(oldest));
    Ok(AnchorStatusView {
        streams,
        lag_minutes: lag.map(|lag| lag.as_secs() / 60),
        off_host_targets: u32::try_from(off_host.len()).unwrap_or(u32::MAX),
        last_export: repo::last_export(conn)?,
        lag,
    })
}

/// The last sequence number of the newest seal of `stream` on any of `off_host`.
fn anchored_through_seq(
    conn: &Connection,
    stream: Stream,
    off_host: &[Target],
) -> Result<i64, AnchorError> {
    let mut newest = 0;
    for target in off_host {
        newest = newest.max(repo::last_exported_seal(conn, target.id, stream)?.unwrap_or(0));
    }
    if newest == 0 {
        return Ok(0);
    }
    let seal = lis_audit::seals(conn, stream, newest - 1)?
        .into_iter()
        .next();
    Ok(seal.map_or(0, |seal| seal.last_seq))
}

/// One stream's seals in an anchor file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct SealRangeView {
    /// The stream code.
    pub stream: String,
    /// First seal number in the file.
    #[ts(type = "number")]
    pub first_seal_no: i64,
    /// Last seal number in the file.
    #[ts(type = "number")]
    pub last_seal_no: i64,
}

/// One anchor file written to a target (`list_anchor_exports`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnchorExportView {
    /// The export.
    pub export_id: AnchorExportId,
    /// Its target's label.
    pub target_label: String,
    /// The file's name, without its folder.
    pub file_name: String,
    /// When it was written.
    pub exported_at: UtcTimestamp,
    /// Its seals, per stream.
    pub ranges: Vec<SealRangeView>,
}

/// The newest anchor files first, with one row beyond `limit` when it exists.
pub fn list_anchor_exports(
    conn: &Connection,
    limit: u32,
) -> Result<Vec<AnchorExportView>, AnchorError> {
    let query_limit = limit.clamp(1, LIST_LIMIT_MAX).saturating_add(1);
    let mut views = Vec::new();
    for row in repo::list_exports(conn, query_limit)? {
        let ranges = repo::export_ranges(conn, row.id)?;
        views.push(AnchorExportView {
            export_id: row.id,
            target_label: row.target_label,
            file_name: row.file_name,
            exported_at: row.exported_at,
            ranges,
        });
    }
    Ok(views)
}
