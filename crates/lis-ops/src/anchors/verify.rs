//! Reading anchor files and verifying them against the local chains (M4 design brief §4;
//! `verify_audit_anchors` of the M4 contract). [`read_anchors`] does the file I/O and
//! [`verify_anchors`] the database reads, so the engine runs them apart and adds the
//! access audit event (`audit.verify_anchors`).
//!
//! Every `*.jsonl` file up to [`MAX_DEPTH`] folders below a chosen folder is read: the
//! chosen folder itself, a target's `VibeToxLIS/anchors/<day>/` files, or an exit export's
//! `audit/anchors.jsonl`. Each line is parsed strictly ([`lis_audit::parse_line`]); a line
//! that does not parse, an unreadable file and another installation's anchors are
//! reported, never compared or silently skipped. Symbolic links are not followed.

use std::collections::{BTreeSet, HashSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, Read};
use std::path::{Path, PathBuf};

use lis_audit::{Anchor, Stream};
use rusqlite::Connection;
use serde::Serialize;
use ts_rs::TS;

use super::AnchorError;
use super::classify::{AnchorResult, Classified, RestoreRecord, classify};
use super::export::ANCHOR_FILE_EXTENSION;
use crate::installation_id;

/// Folder levels below a chosen folder that are searched: enough for
/// `VibeToxLIS/anchors/<day>/` under a target's chosen folder.
const MAX_DEPTH: usize = 4;
/// An anchor line is about 330 bytes; a longer line is not one.
const MAX_LINE_BYTES: u64 = 1024;

/// What the anchor files under the chosen folders hold.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadAnchors {
    /// Valid lines read, of every installation, counting each copy.
    pub lines_read: u64,
    /// Distinct anchors, in the order first read.
    pub anchors: Vec<Anchor>,
    /// Lines that are not valid `vibe-tox-anchor-1` lines.
    pub invalid_lines: u64,
    /// Files and folders that could not be read.
    pub unreadable: u64,
}

/// Reads every anchor file under `folders` (see the module documentation). Fails only
/// when one of `folders` itself cannot be read.
pub fn read_anchors(folders: &[PathBuf]) -> Result<ReadAnchors, AnchorError> {
    let mut read = Reader::default();
    for folder in folders {
        let entries = sorted_entries(folder).map_err(|_| AnchorError::FolderUnreadable)?;
        read.walk_entries(entries, 1);
    }
    Ok(read.result)
}

#[derive(Default)]
struct Reader {
    result: ReadAnchors,
    seen: HashSet<Anchor>,
}

impl Reader {
    fn walk_entries(&mut self, entries: Vec<fs::DirEntry>, depth: usize) {
        for entry in entries {
            let Ok(kind) = entry.file_type() else {
                self.result.unreadable += 1;
                continue;
            };
            let path = entry.path();
            if kind.is_dir() && depth < MAX_DEPTH {
                match sorted_entries(&path) {
                    Ok(entries) => self.walk_entries(entries, depth + 1),
                    Err(_) => self.result.unreadable += 1,
                }
            } else if kind.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext == ANCHOR_FILE_EXTENSION)
            {
                self.read_file(&path);
            }
        }
    }

    fn read_file(&mut self, path: &Path) {
        let Ok(file) = File::open(path) else {
            self.result.unreadable += 1;
            return;
        };
        let mut reader = BufReader::new(file);
        let mut line = Vec::new();
        loop {
            line.clear();
            match (&mut reader)
                .take(MAX_LINE_BYTES)
                .read_until(b'\n', &mut line)
            {
                Ok(0) => return,
                Ok(_) => self.line(&line),
                Err(_) => {
                    self.result.unreadable += 1;
                    return;
                }
            }
        }
    }

    /// One line, with its `\n`; a line without one was cut short.
    fn line(&mut self, bytes: &[u8]) {
        let parsed = bytes
            .strip_suffix(b"\n")
            .and_then(|line| std::str::from_utf8(line).ok())
            .and_then(|line| lis_audit::parse_line(line).ok());
        let Some(anchor) = parsed else {
            self.result.invalid_lines += 1;
            return;
        };
        self.result.lines_read += 1;
        if self.seen.insert(anchor.clone()) {
            self.result.anchors.push(anchor);
        }
    }
}

/// The folder's entries in name order, so files are read oldest first.
fn sorted_entries(folder: &Path) -> std::io::Result<Vec<fs::DirEntry>> {
    let mut entries = fs::read_dir(folder)?.collect::<std::io::Result<Vec<_>>>()?;
    entries.sort_by_key(fs::DirEntry::file_name);
    Ok(entries)
}

/// Counts of a verification (`AnchorSummary` of the M4 contract).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnchorSummary {
    /// Distinct anchors of this installation compared.
    #[ts(type = "number")]
    pub compared: u64,
    /// Anchors the local chains reproduce.
    #[ts(type = "number")]
    pub matched: u64,
    /// Anchors lost with a verified restore of an older set.
    #[ts(type = "number")]
    pub lost_interval: u64,
    /// Anchors and conflicts a restore would explain, but the restore is not anchored yet.
    #[ts(type = "number")]
    pub unverified_restore_boundary: u64,
    /// Anchors the local chains do not reproduce.
    #[ts(type = "number")]
    pub mismatched: u64,
    /// Anchors whose last event does not exist locally.
    #[ts(type = "number")]
    pub local_events_missing: u64,
    /// Seal numbers with conflicting anchors that no restore explains.
    #[ts(type = "number")]
    pub conflicts: u64,
    /// Seal numbers below the newest anchor that no anchor covers.
    #[ts(type = "number")]
    pub gaps: u64,
    /// Local seals newer than the newest anchor.
    #[ts(type = "number")]
    pub unanchored: u64,
}

/// One stream's chain verification.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ChainView {
    /// The stream code.
    pub stream: String,
    /// The chain and its seals verified without findings.
    pub intact: bool,
}

/// One finding: a classified anchor, conflict, gap or unanchored range, another
/// installation's anchors, invalid lines or unreadable files.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnchorFinding {
    /// An [`AnchorResult`] code other than `matched`, or `foreign_installation`,
    /// `invalid_anchor_line` or `unreadable_file`.
    pub code: String,
    /// The stream, when the finding has one.
    pub stream: Option<String>,
    /// The seal number, or the first of a range.
    #[ts(type = "number | null")]
    pub seal_no: Option<i64>,
    /// The last seal number of a range; equal to `sealNo` for one seal.
    #[ts(type = "number | null")]
    pub last_seal_no: Option<i64>,
    /// The finding needs investigation (the `audit_chain` check alerts on it).
    pub problem: bool,
}

/// The result of `verify_audit_anchors`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct AnchorVerification {
    /// Valid anchor lines read, of every installation, counting each copy.
    #[ts(type = "number")]
    pub anchors_read: u64,
    /// The counts.
    pub summary: AnchorSummary,
    /// Each stream's chain verification.
    pub chains: Vec<ChainView>,
    /// Everything other than a match, in anchor order, then per stream.
    pub findings: Vec<AnchorFinding>,
}

impl AnchorVerification {
    /// Findings that need investigation plus broken chains: the `audit_chain` check's input.
    #[must_use]
    pub fn problems(&self) -> usize {
        self.findings
            .iter()
            .filter(|finding| finding.problem)
            .count()
            + self.chains.iter().filter(|chain| !chain.intact).count()
    }
}

/// Verifies `read` against this database's chains and explains restores with `restores`
/// (NFR-18). Anchors of another installation are reported, not compared.
pub fn verify_anchors(
    conn: &Connection,
    read: &ReadAnchors,
    restores: &[RestoreRecord],
) -> Result<AnchorVerification, AnchorError> {
    let installation = installation_id(conn)?.ok_or(AnchorError::NoInstallation)?;
    let (own, foreign): (Vec<Anchor>, Vec<Anchor>) = read
        .anchors
        .iter()
        .cloned()
        .partition(|anchor| anchor.installation == installation);
    let statuses = lis_audit::compare_anchors(conn, &own)?;
    let classified = classify(&own, &statuses, restores);
    let mut chains = Vec::with_capacity(Stream::ALL.len());
    for stream in Stream::ALL {
        chains.push(ChainView {
            stream: stream.as_str().to_owned(),
            intact: lis_audit::verify(conn, stream)?.is_intact(),
        });
    }
    let mut findings: Vec<AnchorFinding> = classified
        .iter()
        .filter(|entry| entry.result != AnchorResult::Matched)
        .map(classified_finding)
        .collect();
    let foreign_streams: BTreeSet<&'static str> = foreign
        .iter()
        .map(|anchor| anchor.seal.stream.as_str())
        .collect();
    findings.extend(foreign_streams.into_iter().map(|stream| AnchorFinding {
        stream: Some(stream.to_owned()),
        ..file_finding("foreign_installation", false)
    }));
    if read.invalid_lines > 0 {
        findings.push(file_finding("invalid_anchor_line", true));
    }
    if read.unreadable > 0 {
        findings.push(file_finding("unreadable_file", true));
    }
    Ok(AnchorVerification {
        anchors_read: read.lines_read,
        summary: summarize(own.len(), &classified),
        chains,
        findings,
    })
}

fn classified_finding(entry: &Classified) -> AnchorFinding {
    AnchorFinding {
        code: entry.result.as_str().to_owned(),
        stream: Some(entry.stream.as_str().to_owned()),
        seal_no: Some(entry.seal_no),
        last_seal_no: Some(entry.last_seal_no),
        problem: entry.result.is_problem(),
    }
}

fn file_finding(code: &str, problem: bool) -> AnchorFinding {
    AnchorFinding {
        code: code.to_owned(),
        stream: None,
        seal_no: None,
        last_seal_no: None,
        problem,
    }
}

fn summarize(compared: usize, classified: &[Classified]) -> AnchorSummary {
    let mut summary = AnchorSummary {
        compared: compared as u64,
        ..AnchorSummary::default()
    };
    for entry in classified {
        let seals = (entry.last_seal_no - entry.seal_no + 1).unsigned_abs();
        let count = match entry.result {
            AnchorResult::Matched => &mut summary.matched,
            AnchorResult::LostInterval => &mut summary.lost_interval,
            AnchorResult::UnverifiedRestoreBoundary => &mut summary.unverified_restore_boundary,
            AnchorResult::LocalEventsMissing => &mut summary.local_events_missing,
            AnchorResult::AnchorMismatch => &mut summary.mismatched,
            AnchorResult::AnchorConflict => &mut summary.conflicts,
            AnchorResult::AnchorGap => &mut summary.gaps,
            AnchorResult::Unanchored => &mut summary.unanchored,
        };
        *count += seals;
    }
    summary
}
