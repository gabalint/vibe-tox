//! The checks on an opened scratch database and on a set's artifacts (M4 design brief
//! §3.6 steps 4–7).

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{self, BufReader};
use std::path::PathBuf;
use std::sync::atomic::Ordering;

use lis_audit::chain::GENESIS_HASH;
use lis_audit::{Finding as ChainFinding, Stream};
use lis_core::FieldError;
use lis_store::{ARTIFACTS_DIR, ArtifactDigest, ArtifactStore, Database, MAX_ARTIFACT_BYTES};
use rusqlite::Connection;

use super::anchors::{self, SetPosition};
use super::findings::{FindingCode, Findings};
use super::stage::{StageInput, StagedRestore};
use crate::anchors::read_anchors;
use crate::backup::{ArtifactEntry, BackupError, HeadEntry, Manifest, SealEntry, artifact_path};
use crate::format::{self, BackupKey, Content, FormatError};

/// One audit stream of a staged database.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChainCheck {
    /// The stream.
    pub stream: Stream,
    /// Events read.
    pub events: u64,
    /// Verification found nothing.
    pub intact: bool,
    /// Its head equals the manifest's.
    pub head_matches_manifest: bool,
}

/// Steps 4, 5 and the first half of 7 on the opened scratch database; returns the
/// artifact digests it references.
pub(super) fn check_opened(
    db: &Database,
    input: &StageInput<'_>,
    manifest: &Manifest,
    staged: &mut StagedRestore,
) -> Result<BTreeSet<String>, BackupError> {
    let heads = manifest.heads.clone();
    let seals = manifest.seals.clone();
    let (chains, findings) = db.read_blocking(move |conn| check_chains(conn, &heads, &seals))?;
    staged.chains = chains;
    for (code, count) in findings {
        staged.findings.add(code, count);
    }
    let mut roots = vec![input.source_root.to_path_buf()];
    roots.extend(input.anchor_roots.iter().cloned());
    let read = read_anchors(&roots)
        .map_err(|_| BackupError::Io(io::Error::other("anchor folder unreadable")))?;
    let position = SetPosition {
        installation: manifest.installation_id,
        created_at: manifest.created_at,
        heads: manifest_heads(&manifest.heads),
    };
    let check = db.read_blocking(move |conn| anchors::compare(conn, &position, &read))?;
    staged.anchors = Some(check.summary);
    staged.lost_interval_until = check.lost_interval_until;
    for finding in check.findings.list() {
        staged.findings.add(finding.code, finding.count);
    }
    let columns = owned_columns(input.artifact_columns)?;
    db.read_blocking(move |conn| referenced_artifacts(conn, &columns))
}

fn manifest_heads(heads: &[HeadEntry]) -> Vec<(Stream, i64)> {
    heads
        .iter()
        .filter_map(|head| super::stream_named(&head.stream).map(|stream| (stream, head.seq)))
        .collect()
}

type ChainFindings = Vec<(FindingCode, u64)>;

/// Verifies every stream and compares its head and newest seal with the manifest.
fn check_chains(
    conn: &Connection,
    heads: &[HeadEntry],
    seals: &[SealEntry],
) -> Result<(Vec<ChainCheck>, ChainFindings), BackupError> {
    let mut chains = Vec::new();
    let mut findings = Vec::new();
    for stream in Stream::ALL {
        let Ok(report) = lis_audit::verify(conn, stream) else {
            findings.push((FindingCode::ChainBroken, 1));
            chains.push(ChainCheck {
                stream,
                events: 0,
                intact: false,
                head_matches_manifest: false,
            });
            continue;
        };
        let seal_problems = report
            .findings
            .iter()
            .filter(|finding| matches!(finding, ChainFinding::SealMismatch { .. }))
            .count();
        let chain_problems = report.findings.len() - seal_problems;
        let (seq, hash) = report
            .head
            .as_ref()
            .map_or((0, GENESIS_HASH), |head| (head.seq, head.hash.as_str()));
        let head_matches_manifest = heads
            .iter()
            .any(|head| head.stream == stream.as_str() && head.seq == seq && head.hash == hash);
        let seal_agrees = newest_seal_agrees(conn, stream, seals)?;
        findings.push((FindingCode::ChainBroken, chain_problems as u64));
        findings.push((FindingCode::HeadMismatch, u64::from(!head_matches_manifest)));
        findings.push((
            FindingCode::SealInconsistent,
            seal_problems as u64 + u64::from(!seal_agrees),
        ));
        chains.push(ChainCheck {
            stream,
            events: report.events_checked,
            intact: report.is_intact(),
            head_matches_manifest,
        });
    }
    Ok((chains, findings))
}

/// The stream's newest seal is the one the manifest recorded, or both have none.
fn newest_seal_agrees(
    conn: &Connection,
    stream: Stream,
    seals: &[SealEntry],
) -> Result<bool, BackupError> {
    let recorded = seals.iter().find(|seal| seal.stream == stream.as_str());
    Ok(match (lis_audit::last_seal(conn, stream)?, recorded) {
        (None, None) => true,
        (Some(local), Some(recorded)) => {
            local.seal_no == recorded.seal_no
                && local.last_seq == recorded.last_seq
                && local.head_hash == recorded.head_hash
        }
        (Some(_), None) | (None, Some(_)) => false,
    })
}

/// Checks the column names, which are interpolated into SQL as quoted identifiers.
fn owned_columns(columns: &[(&str, &str)]) -> Result<Vec<(String, String)>, BackupError> {
    let identifier = |name: &str| {
        !name.is_empty()
            && name
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
    };
    columns
        .iter()
        .map(|&(table, column)| {
            if identifier(table) && identifier(column) {
                Ok((table.to_owned(), column.to_owned()))
            } else {
                Err(BackupError::Invalid(FieldError {
                    field: "artifactColumns".to_owned(),
                    code: "invalid".to_owned(),
                    message: "An artifact column is not a table and column name.".to_owned(),
                }))
            }
        })
        .collect()
}

/// Every distinct digest the columns hold.
fn referenced_artifacts(
    conn: &Connection,
    columns: &[(String, String)],
) -> Result<BTreeSet<String>, BackupError> {
    let mut digests = BTreeSet::new();
    for (table, column) in columns {
        let sql =
            format!("SELECT DISTINCT \"{column}\" FROM \"{table}\" WHERE \"{column}\" IS NOT NULL");
        let mut statement = conn.prepare(&sql)?;
        let rows = statement.query_map([], |row| row.get::<_, String>(0))?;
        for digest in rows {
            digests.insert(digest?);
        }
    }
    Ok(digests)
}

/// Step 6: decrypts and digest-checks every manifest artifact, keeping it in the scratch
/// artifact folder when asked.
pub(super) fn check_artifacts(
    input: &StageInput<'_>,
    manifest: &Manifest,
    findings: &mut Findings,
) -> Result<(), BackupError> {
    let store = if input.keep_artifacts {
        let dir: PathBuf = input.scratch_dir.join(ARTIFACTS_DIR);
        Some(ArtifactStore::open(dir, MAX_ARTIFACT_BYTES)?)
    } else {
        None
    };
    for entry in &manifest.artifacts {
        if input.cancel.load(Ordering::Relaxed) {
            return Err(FormatError::Cancelled.into());
        }
        match read_artifact(input, entry)? {
            Artifact::Missing => findings.add(FindingCode::ArtifactMissing, 1),
            Artifact::Corrupt => findings.add(FindingCode::ArtifactCorrupt, 1),
            Artifact::Verified(bytes) => {
                if let Some(store) = &store {
                    store.put(&bytes)?;
                }
            }
        }
    }
    Ok(())
}

enum Artifact {
    Missing,
    Corrupt,
    Verified(Vec<u8>),
}

fn read_artifact(input: &StageInput<'_>, entry: &ArtifactEntry) -> Result<Artifact, BackupError> {
    let Ok(digest) = entry.sha256.parse::<ArtifactDigest>() else {
        return Ok(Artifact::Corrupt);
    };
    let file = match File::open(artifact_path(input.source_root, &entry.sha256)) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Artifact::Missing),
        Err(error) => return Err(error.into()),
    };
    let expected_len = format::encrypted_len(entry.bytes);
    if entry.bytes > MAX_ARTIFACT_BYTES || file.metadata()?.len() != expected_len {
        return Ok(Artifact::Corrupt);
    }
    match decrypt_artifact(input.key, file, input) {
        Ok(bytes) => {
            let (actual, len) = ArtifactDigest::of_reader(bytes.as_slice())?;
            Ok(if actual == digest && len == entry.bytes {
                Artifact::Verified(bytes)
            } else {
                Artifact::Corrupt
            })
        }
        Err(FormatError::Cancelled) => Err(FormatError::Cancelled.into()),
        Err(FormatError::Io(error)) => Err(error.into()),
        Err(_) => Ok(Artifact::Corrupt),
    }
}

fn decrypt_artifact(
    key: &BackupKey,
    file: File,
    input: &StageInput<'_>,
) -> Result<Vec<u8>, FormatError> {
    let mut bytes = Vec::new();
    format::decrypt(
        key,
        Content::Artifact,
        BufReader::new(file),
        &mut bytes,
        input.cancel,
    )?;
    Ok(bytes)
}
