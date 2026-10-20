//! SQL for destinations, verifications, obligations, closures and attempts.
//! All are append-only except a destination's single retirement.

use lis_core::{UserId, UtcTimestamp};
use lis_identity::{ClientId, ClientLocationId};
use rusqlite::{Connection, OptionalExtension, Row, params};

use crate::model::{
    DeliveryAttemptId, DeliveryChannel, DeliveryEvidence, DeliveryFailureCode, DestinationId,
    ObligationClosureCause, ObligationId, ObligationKind, ObligationOrigin, ReportId,
    ReportRevisionId,
};
use crate::queries::delivery::DeliveryQueueFilter;

/// A destination with its verification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DestinationRow {
    pub(crate) id: DestinationId,
    pub(crate) client_id: ClientId,
    pub(crate) client_location_id: Option<ClientLocationId>,
    pub(crate) channel: DeliveryChannel,
    pub(crate) address: Option<String>,
    pub(crate) label: String,
    pub(crate) replaces_id: Option<DestinationId>,
    pub(crate) created_by: UserId,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) retired_at: Option<UtcTimestamp>,
    pub(crate) retire_reason: Option<String>,
    pub(crate) revision: i64,
    pub(crate) verification: Option<Verification>,
    /// When an attempt first recorded the address as wrong (REQ-RPT-012).
    pub(crate) reported_wrong_at: Option<UtcTimestamp>,
}

/// Who verified a destination, how and when.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Verification {
    pub(crate) method: String,
    pub(crate) verified_by: UserId,
    pub(crate) verified_at: UtcTimestamp,
}

const DESTINATION_COLUMNS: &str = "d.id, d.client_id, d.client_location_id, d.channel, \
     d.address, d.label, d.replaces_id, d.created_by, d.created_at, d.retired_at, \
     d.retire_reason, d.revision, v.method, v.verified_by, v.verified_at, \
     (SELECT min(a.recorded_at) FROM rpt_delivery_attempts a \
      JOIN rpt_delivery_obligations o ON o.id = a.obligation_id \
      WHERE o.destination_id = d.id AND a.failure_code = 'wrong_destination') \
     FROM rpt_destinations d LEFT JOIN rpt_destination_verifications v ON v.destination_id = d.id";

fn destination_row(row: &Row<'_>) -> rusqlite::Result<DestinationRow> {
    let method: Option<String> = row.get(12)?;
    let verification = match method {
        Some(method) => Some(Verification {
            method,
            verified_by: row.get(13)?,
            verified_at: row.get(14)?,
        }),
        None => None,
    };
    Ok(DestinationRow {
        id: row.get(0)?,
        client_id: row.get(1)?,
        client_location_id: row.get(2)?,
        channel: row.get(3)?,
        address: row.get(4)?,
        label: row.get(5)?,
        replaces_id: row.get(6)?,
        created_by: row.get(7)?,
        created_at: row.get(8)?,
        retired_at: row.get(9)?,
        retire_reason: row.get(10)?,
        revision: row.get(11)?,
        verification,
        reported_wrong_at: row.get(15)?,
    })
}

pub(crate) fn destination(
    conn: &Connection,
    id: DestinationId,
) -> rusqlite::Result<Option<DestinationRow>> {
    conn.prepare_cached(&format!("SELECT {DESTINATION_COLUMNS} WHERE d.id = ?1"))?
        .query_row([id], destination_row)
        .optional()
}

/// Destinations, optionally of one client, by client and label.
pub(crate) fn destinations(
    conn: &Connection,
    client_id: Option<ClientId>,
) -> rusqlite::Result<Vec<DestinationRow>> {
    conn.prepare_cached(&format!(
        "SELECT {DESTINATION_COLUMNS} WHERE ?1 IS NULL OR d.client_id = ?1 \
         ORDER BY d.client_id, d.label, d.id"
    ))?
    .query_map([client_id], destination_row)?
    .collect()
}

/// L11: the active destinations of the location, or the client-level ones
/// when the location has none or the order names no location.
pub(crate) fn routing(
    conn: &Connection,
    client_id: ClientId,
    location_id: Option<ClientLocationId>,
) -> rusqlite::Result<Vec<DestinationRow>> {
    let mut statement = conn.prepare_cached(&format!(
        "SELECT {DESTINATION_COLUMNS} WHERE d.client_id = ?1 AND d.retired_at IS NULL \
         AND d.client_location_id IS ?2 ORDER BY d.created_at, d.id"
    ))?;
    if location_id.is_some() {
        let at_location: Vec<DestinationRow> = statement
            .query_map(params![client_id, location_id], destination_row)?
            .collect::<rusqlite::Result<_>>()?;
        if !at_location.is_empty() {
            return Ok(at_location);
        }
    }
    statement
        .query_map(
            params![client_id, None::<ClientLocationId>],
            destination_row,
        )?
        .collect()
}

pub(crate) fn insert_destination(conn: &Connection, d: &DestinationRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_destinations (id, client_id, client_location_id, channel, address, label, \
         replaces_id, created_by, created_at, revision) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, 1)",
    )?
    .execute(params![
        d.id,
        d.client_id,
        d.client_location_id,
        d.channel,
        d.address,
        d.label,
        d.replaces_id,
        d.created_by,
        d.created_at
    ])?;
    Ok(())
}

pub(crate) fn insert_verification(
    conn: &Connection,
    id: DestinationId,
    v: &Verification,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_destination_verifications (destination_id, method, verified_by, \
         verified_at) VALUES (?1, ?2, ?3, ?4)",
    )?
    .execute(params![id, v.method, v.verified_by, v.verified_at])?;
    Ok(())
}

/// Retires an active destination at `revision`; `false` if it changed meanwhile.
pub(crate) fn retire_destination(
    conn: &Connection,
    id: DestinationId,
    revision: i64,
    by: UserId,
    at: UtcTimestamp,
    reason: &str,
) -> rusqlite::Result<bool> {
    let changed = conn
        .prepare_cached(
            "UPDATE rpt_destinations SET retired_by = ?3, retired_at = ?4, retire_reason = ?5, \
             revision = revision + 1 WHERE id = ?1 AND revision = ?2 AND retired_at IS NULL",
        )?
        .execute(params![id, revision, by, at, reason])?;
    Ok(changed == 1)
}

/// An obligation with its closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ObligationRow {
    pub(crate) id: ObligationId,
    pub(crate) revision_id: ReportRevisionId,
    pub(crate) destination_id: DestinationId,
    pub(crate) kind: ObligationKind,
    pub(crate) origin: ObligationOrigin,
    pub(crate) origin_reason: Option<String>,
    pub(crate) redirected_from_id: Option<ObligationId>,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) closure: Option<ObligationClosureCause>,
}

const OBLIGATION_COLUMNS: &str = "o.id, o.revision_id, o.destination_id, o.kind, o.origin, \
     o.origin_reason, o.redirected_from_id, o.created_at, c.cause \
     FROM rpt_delivery_obligations o \
     LEFT JOIN rpt_obligation_closures c ON c.obligation_id = o.id";

fn obligation_row(row: &Row<'_>) -> rusqlite::Result<ObligationRow> {
    Ok(ObligationRow {
        id: row.get(0)?,
        revision_id: row.get(1)?,
        destination_id: row.get(2)?,
        kind: row.get(3)?,
        origin: row.get(4)?,
        origin_reason: row.get(5)?,
        redirected_from_id: row.get(6)?,
        created_at: row.get(7)?,
        closure: row.get(8)?,
    })
}

pub(crate) fn obligation(
    conn: &Connection,
    id: ObligationId,
) -> rusqlite::Result<Option<ObligationRow>> {
    conn.prepare_cached(&format!("SELECT {OBLIGATION_COLUMNS} WHERE o.id = ?1"))?
        .query_row([id], obligation_row)
        .optional()
}

/// Every obligation of a report's revisions; withdrawal notices first, then
/// newest revision first (REQ-RPT-013).
pub(crate) fn report_obligations(
    conn: &Connection,
    report_id: ReportId,
) -> rusqlite::Result<Vec<ObligationRow>> {
    conn.prepare_cached(&format!(
        "SELECT {OBLIGATION_COLUMNS} JOIN rpt_revisions r ON r.id = o.revision_id \
         WHERE r.report_id = ?1 \
         ORDER BY o.kind = 'report', r.revision_no DESC, o.created_at, o.id"
    ))?
    .query_map([report_id], obligation_row)?
    .collect()
}

/// One page of the delivery queue (REQ-RPT-013, NFR-05): at most `limit`
/// obligations not closed, in queue order (withdrawal notices first, then
/// corrections, then oldest first, then by ID), after the obligation `after`.
/// The sort key never changes, so pages neither repeat nor skip a row.
///
/// Rows are narrowed to those whose stored facts can give `filter`'s status
/// (`queries::delivery::status`): blocked destinations, then attempts. A pause
/// on open impact is computed, not stored, so the caller still drops paused
/// rows from the other filters and keeps only paused ones for `paused`.
/// The attempt subqueries are uncorrelated: each is built once per query.
pub(crate) fn queue_page(
    conn: &Connection,
    filter: DeliveryQueueFilter,
    after: Option<ObligationId>,
    limit: u32,
) -> rusqlite::Result<Vec<ObligationRow>> {
    conn.prepare_cached(&format!(
        "SELECT {OBLIGATION_COLUMNS} \
         JOIN rpt_v_revisions r ON r.revision_id = o.revision_id \
         JOIN rpt_destinations d ON d.id = o.destination_id \
         LEFT JOIN rpt_destination_verifications v ON v.destination_id = d.id \
         WHERE c.obligation_id IS NULL \
         AND (?1 IS NULL OR (o.kind = 'report', r.relationship <> 'correction', o.created_at, o.id) \
          > (SELECT k.kind = 'report', kr.relationship <> 'correction', k.created_at, k.id \
             FROM rpt_delivery_obligations k JOIN rpt_revisions kr ON kr.id = k.revision_id \
             WHERE k.id = ?1)) \
         AND CASE WHEN d.retired_at IS NOT NULL OR v.destination_id IS NULL OR d.id IN \
          (SELECT w.destination_id FROM rpt_delivery_attempts a \
           JOIN rpt_delivery_obligations w ON w.id = a.obligation_id \
           WHERE a.failure_code = 'wrong_destination') \
         THEN ?2 IN ('blocked', 'all') \
         ELSE CASE ?2 \
          WHEN 'awaiting' THEN o.id NOT IN (SELECT obligation_id FROM rpt_delivery_attempts) \
          WHEN 'attention' THEN o.id IN (SELECT obligation_id FROM rpt_delivery_attempts) \
           AND o.id NOT IN (SELECT obligation_id FROM rpt_delivery_attempts \
                            WHERE evidence = 'handed_off') \
          WHEN 'paused' THEN o.kind = 'report' AND r.availability = 'current' \
          ELSE ?2 = 'all' END END \
         ORDER BY o.kind = 'report', r.relationship <> 'correction', o.created_at, o.id \
         LIMIT ?3"
    ))?
    .query_map(params![after, filter, limit], obligation_row)?
    .collect()
}

pub(crate) fn insert_obligation(
    conn: &Connection,
    o: &ObligationRow,
    created_by: &str,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_delivery_obligations (id, revision_id, destination_id, kind, origin, \
         origin_reason, redirected_from_id, created_by, created_at) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
    )?
    .execute(params![
        o.id,
        o.revision_id,
        o.destination_id,
        o.kind,
        o.origin,
        o.origin_reason,
        o.redirected_from_id,
        created_by,
        o.created_at
    ])?;
    Ok(())
}

pub(crate) fn close_obligation(
    conn: &Connection,
    id: ObligationId,
    cause: ObligationClosureCause,
    by_obligation_id: Option<ObligationId>,
    closed_by: &str,
    at: UtcTimestamp,
) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_obligation_closures (obligation_id, cause, by_obligation_id, closed_by, \
         closed_at) VALUES (?1, ?2, ?3, ?4, ?5)",
    )?
    .execute(params![id, cause, by_obligation_id, closed_by, at])?;
    Ok(())
}

/// One attested attempt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AttemptRow {
    pub(crate) id: DeliveryAttemptId,
    pub(crate) obligation_id: ObligationId,
    pub(crate) evidence: DeliveryEvidence,
    pub(crate) observed_at: UtcTimestamp,
    pub(crate) failure_code: Option<DeliveryFailureCode>,
    pub(crate) note: Option<String>,
    pub(crate) recorded_by: UserId,
    pub(crate) recorded_at: UtcTimestamp,
}

/// An obligation's attempts, oldest first.
pub(crate) fn attempts(
    conn: &Connection,
    obligation_id: ObligationId,
) -> rusqlite::Result<Vec<AttemptRow>> {
    conn.prepare_cached(
        "SELECT id, obligation_id, evidence, observed_at, failure_code, note, recorded_by, \
         recorded_at FROM rpt_delivery_attempts WHERE obligation_id = ?1 \
         ORDER BY recorded_at, id",
    )?
    .query_map([obligation_id], |row| {
        Ok(AttemptRow {
            id: row.get(0)?,
            obligation_id: row.get(1)?,
            evidence: row.get(2)?,
            observed_at: row.get(3)?,
            failure_code: row.get(4)?,
            note: row.get(5)?,
            recorded_by: row.get(6)?,
            recorded_at: row.get(7)?,
        })
    })?
    .collect()
}

pub(crate) fn insert_attempt(conn: &Connection, a: &AttemptRow) -> rusqlite::Result<()> {
    conn.prepare_cached(
        "INSERT INTO rpt_delivery_attempts (id, obligation_id, evidence, signal_source, \
         observed_at, failure_code, note, recorded_by, recorded_at) \
         VALUES (?1, ?2, ?3, 'staff_attestation', ?4, ?5, ?6, ?7, ?8)",
    )?
    .execute(params![
        a.id,
        a.obligation_id,
        a.evidence,
        a.observed_at,
        a.failure_code,
        a.note,
        a.recorded_by,
        a.recorded_at
    ])?;
    Ok(())
}

#[cfg(test)]
mod tests {
    //! `queue_page` against the real `rpt_` schema with foreign keys off:
    //! each filter keeps exactly the rows whose stored facts can give its
    //! status (`queries::delivery::status`), in queue order, page by page.

    use std::collections::BTreeSet;

    use super::*;
    use crate::model::{CandidateId, ReportId};

    const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    const AT: &str = "2026-10-12T14:00:00.000000000Z";

    fn at(second: u32) -> String {
        format!("2026-10-12T14:00:{second:02}.000000000Z")
    }

    fn schema() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        // The verification guard reads the separation-of-duties policy view.
        for migration in [
            lis_audit::MIGRATIONS,
            lis_auth::MIGRATIONS,
            crate::MIGRATIONS,
        ]
        .concat()
        {
            conn.execute_batch(migration.sql).unwrap();
        }
        conn
    }

    /// Releases revision `no` of `report`: a correction of the previous one
    /// when `no > 1`.
    fn revision(
        conn: &Connection,
        report: ReportId,
        no: i64,
        supersedes: Option<ReportRevisionId>,
    ) -> ReportRevisionId {
        let (candidate, revision) = (CandidateId::generate(), ReportRevisionId::generate());
        let (relationship, reason) = match supersedes {
            Some(_) => ("correction", Some("Synthetic correction")),
            None => ("initial", None),
        };
        conn.execute(
            "INSERT INTO rpt_candidates (id, report_id, proposed_revision_no, relationship, \
             supersedes_revision_id, amendment_reason, completeness, signatory_user_id, \
             prepared_at, content_json, content_sha256, data_json, data_sha256, template_id, \
             template_version, state, revision) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'final', 'u1', \
             ?7, '{}', ?8, '{}', ?8, 'tox-report', 2, 'rendering', 1)",
            params![
                candidate,
                report,
                no,
                relationship,
                supersedes,
                reason,
                AT,
                SHA
            ],
        )
        .unwrap();
        conn.execute(
            "UPDATE rpt_candidates SET state = 'ready', artifact_sha256 = ?2, artifact_size = 10, \
             template_sha256 = ?2, renderer_version = 'typst 0.15.1', fonts_json = '[]', \
             rendered_at = ?3, revision = 2 WHERE id = ?1",
            params![candidate, SHA, AT],
        )
        .unwrap();
        conn.execute(
            "UPDATE rpt_candidates SET state = 'released', revision = 3 WHERE id = ?1",
            [candidate],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO audit_events (stream, seq, prev_hash, hash, hash_alg, occurred_at, \
             recorded_at, principal_kind, principal_id, action, outcome, record_type, record_id, \
             new_revision, authority, correlation_id, details) VALUES ('clinical', ?1, ?2, ?2, \
             'sha256', ?3, ?3, 'user', 'u1', 'report.release', 'succeeded', 'report_revision', \
             ?4, ?1, 'release_reports', 'k1', '{}')",
            params![no, SHA, AT, revision],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO rpt_revisions (id, report_id, revision_no, candidate_id, relationship, \
             completeness, supersedes_revision_id, amendment_reason, content_sha256, \
             artifact_sha256, released_by, released_at) \
             VALUES (?1, ?2, ?3, ?4, ?5, 'final', ?6, ?7, ?8, ?8, 'u1', ?9)",
            params![
                revision,
                report,
                no,
                candidate,
                relationship,
                supersedes,
                reason,
                SHA,
                AT
            ],
        )
        .unwrap();
        revision
    }

    /// A courier destination, verified by a second person when `verified`.
    fn destination(conn: &Connection, verified: bool) -> DestinationId {
        let id = DestinationId::generate();
        conn.execute(
            "INSERT INTO rpt_destinations (id, client_id, channel, label, created_by, created_at, \
             revision) VALUES (?1, 'client', 'courier', 'Synthetic courier', 'u1', ?2, 1)",
            params![id, AT],
        )
        .unwrap();
        if verified {
            conn.execute(
                "INSERT INTO rpt_destination_verifications (destination_id, method, verified_by, \
                 verified_at) VALUES (?1, 'callback', 'u2', ?2)",
                params![id, AT],
            )
            .unwrap();
        }
        id
    }

    fn obligation(
        conn: &Connection,
        revision: ReportRevisionId,
        destination: DestinationId,
        kind: ObligationKind,
        second: u32,
    ) -> ObligationId {
        let id = ObligationId::generate();
        let origin = match kind {
            ObligationKind::Report => "release",
            ObligationKind::WithdrawalNotice => "withdrawal",
        };
        conn.execute(
            "INSERT INTO rpt_delivery_obligations (id, revision_id, destination_id, kind, origin, \
             created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, 'u1', ?6)",
            params![id, revision, destination, kind, origin, at(second)],
        )
        .unwrap();
        id
    }

    fn attempt(conn: &Connection, obligation: ObligationId, evidence: &str, failure: Option<&str>) {
        conn.execute(
            "INSERT INTO rpt_delivery_attempts (id, obligation_id, evidence, signal_source, \
             observed_at, failure_code, recorded_by, recorded_at) \
             VALUES (?1, ?2, ?3, 'staff_attestation', ?4, ?5, 'u1', ?4)",
            params![
                DeliveryAttemptId::generate(),
                obligation,
                evidence,
                AT,
                failure
            ],
        )
        .unwrap();
    }

    /// Every row of `filter`, read `limit` at a time by the last one shown.
    fn all_pages(conn: &Connection, filter: DeliveryQueueFilter, limit: u32) -> Vec<ObligationId> {
        let mut ids = Vec::new();
        loop {
            let page = queue_page(conn, filter, ids.last().copied(), limit).unwrap();
            assert!(page.len() <= usize::try_from(limit).unwrap());
            if page.is_empty() {
                return ids;
            }
            ids.extend(page.iter().map(|row| row.id));
            assert!(ids.len() <= 20, "pages must end");
        }
    }

    #[test]
    fn each_filter_keeps_the_rows_its_status_can_come_from_in_queue_order() {
        use ObligationKind::{Report, WithdrawalNotice};
        let conn = schema();
        let report = ReportId::generate();
        conn.execute(
            "INSERT INTO rpt_reports VALUES (?1, 's1', 'A26-0001-UR', 'u1', ?2)",
            params![report, AT],
        )
        .unwrap();
        let initial = revision(&conn, report, 1, None);
        let correction = revision(&conn, report, 2, Some(initial));
        let ok = |verified| destination(&conn, verified);

        // The superseded initial revision's rows are the oldest yet come last.
        let old = obligation(&conn, initial, ok(true), Report, 0);
        let wrong_dest = ok(true);
        let wrong_elsewhere = obligation(&conn, initial, wrong_dest, Report, 1);
        let closed = obligation(&conn, initial, ok(true), Report, 2);
        conn.execute(
            "INSERT INTO rpt_obligation_closures (obligation_id, cause, closed_by, closed_at) \
             VALUES (?1, 'newer_revision', 'u1', ?2)",
            params![closed, AT],
        )
        .unwrap();
        let unverified = obligation(&conn, correction, ok(false), Report, 10);
        let retired_dest = ok(true);
        conn.execute(
            "UPDATE rpt_destinations SET retired_by = 'u1', retired_at = ?2, \
             retire_reason = 'Synthetic', revision = 2 WHERE id = ?1",
            params![retired_dest, AT],
        )
        .unwrap();
        let retired = obligation(&conn, correction, retired_dest, Report, 11);
        let wrong = obligation(&conn, correction, wrong_dest, Report, 12);
        attempt(&conn, wrong, "failed", Some("wrong_destination"));
        let awaiting = obligation(&conn, correction, ok(true), Report, 13);
        let failed = obligation(&conn, correction, ok(true), Report, 14);
        attempt(&conn, failed, "failed", Some("no_answer_or_busy"));
        let unknown = obligation(&conn, correction, ok(true), Report, 15);
        attempt(&conn, unknown, "outcome_unknown", None);
        let handed = obligation(&conn, correction, ok(true), Report, 16);
        attempt(&conn, handed, "failed", Some("refused"));
        attempt(&conn, handed, "handed_off", None);
        let notice = obligation(&conn, correction, ok(true), WithdrawalNotice, 20);

        let order = [
            notice,
            unverified,
            retired,
            wrong,
            awaiting,
            failed,
            unknown,
            handed,
            old,
            wrong_elsewhere,
        ];
        assert_eq!(all_pages(&conn, DeliveryQueueFilter::All, 3), order);
        assert_eq!(all_pages(&conn, DeliveryQueueFilter::All, 500), order);
        let expected = [
            (
                DeliveryQueueFilter::Blocked,
                vec![unverified, retired, wrong, wrong_elsewhere],
            ),
            (DeliveryQueueFilter::Awaiting, vec![notice, awaiting, old]),
            (DeliveryQueueFilter::Attention, vec![failed, unknown]),
            // Only report obligations of a current revision can be paused.
            (
                DeliveryQueueFilter::Paused,
                vec![awaiting, failed, unknown, handed],
            ),
        ];
        for (filter, rows) in expected {
            let got = all_pages(&conn, filter, 1);
            assert_eq!(got, rows, "{filter:?}");
            assert_eq!(got.iter().collect::<BTreeSet<_>>().len(), got.len());
        }
    }
}
