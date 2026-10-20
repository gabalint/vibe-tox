//! The delivery queue is bounded and paged (REQ-RPT-013, NFR-05): the limit
//! is 1–500, pages follow the queue order by the last obligation shown
//! without repeats or gaps, and a revision with damaged stored content is
//! left out without shortening a page (NFR-07). Synthetic rows, written
//! straight to a real SQLite schema with foreign keys off.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_core::{FieldError, UserId};
use lis_identity::{ClientId, SubjectId};
use lis_orders::OrderId;
use lis_reporting::snapshot::sha256_hex;
use lis_reporting::{
    CandidateId, DeliveryQueueFilter, DeliveryQueuePage, DestinationId,
    ListDeliveryObligationsInput, ObligationId, ReportId, ReportRevisionId, ReportingError,
    delivery_queue_page,
};
use lis_specimens::{AccessionId, SpecimenId};
use rusqlite::{Connection, params};

const SHA: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AT: &str = "2026-10-12T14:00:00.000000000Z";

struct Lab {
    conn: Connection,
    user: UserId,
    client: ClientId,
    release_seq: i64,
}

impl Lab {
    fn new() -> Self {
        let conn = Connection::open_in_memory().unwrap();
        conn.pragma_update(None, "foreign_keys", false).unwrap();
        for migration in [
            lis_store::MIGRATIONS,
            lis_audit::MIGRATIONS,
            lis_auth::MIGRATIONS,
            lis_identity::MIGRATIONS,
            lis_catalog::MIGRATIONS,
            lis_interfaces::MIGRATIONS,
            lis_orders::MIGRATIONS,
            lis_specimens::MIGRATIONS,
            lis_analytical::MIGRATIONS,
            lis_results::MIGRATIONS,
            lis_reporting::MIGRATIONS,
        ]
        .concat()
        {
            conn.execute_batch(migration.sql).unwrap();
        }
        let user = UserId::generate();
        conn.execute(
            "INSERT INTO auth_users (id, username, display_name, status, password_hash, \
             must_change_password, created_at, updated_at, revision) VALUES (?1, 'synthetic1', \
             'Synthetic User', 'active', '$argon2id$synthetic', 0, ?2, ?2, 1)",
            params![user, AT],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO idn_laboratory_profiles (id, version, name, address_line_1, clia_id, \
             director_name, phone, time_zone, set_by, set_at) VALUES (?1, 1, 'Synthetic Lab', \
             '1 Test Way', '00D0000000', 'Test Director', '555-0100', 'America/Chicago', ?2, ?3)",
            params![UserId::generate().to_string(), user, AT],
        )
        .unwrap();
        Self {
            conn,
            user,
            client: ClientId::generate(),
            release_seq: 0,
        }
    }

    /// A report on its own accepted specimen and order of the client.
    fn report(&self, number: &str) -> ReportId {
        let (order, subject) = (OrderId::generate(), SubjectId::generate());
        let (accession, specimen) = (AccessionId::generate(), SpecimenId::generate());
        let collection = UserId::generate().to_string();
        self.conn
            .execute(
                "INSERT INTO ord_orders (id, subject_id, client_id, practitioner_id, source_kind, \
                 billing_responsibility, template_use, status, version, received_at, \
                 received_by_kind, received_by_id, decided_at, decided_by, revision) \
                 VALUES (?1, ?2, ?3, ?4, 'paper_requisition', 'client', 'none', 'accepted', 1, \
                 ?5, 'user', ?6, ?5, ?6, 1)",
                params![
                    order,
                    subject,
                    self.client,
                    UserId::generate().to_string(),
                    AT,
                    self.user
                ],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO spc_accessions (id, accession_number, order_id, subject_id, \
                 created_by, created_at) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                params![accession, number, order, subject, self.user, AT],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO spc_collections (id, accession_id, precision, recorded_by, \
                 recorded_at) VALUES (?1, ?2, 'unknown', ?3, ?4)",
                params![collection, accession, self.user, AT],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO spc_specimens (id, collection_id, specimen_type_code, acceptance, \
                 received_at, received_by, revision) VALUES (?1, ?2, 'UR', 'accepted', ?3, ?4, 1)",
                params![specimen, collection, AT, self.user],
            )
            .unwrap();
        let report = ReportId::generate();
        self.conn
            .execute(
                "INSERT INTO rpt_reports (id, specimen_id, report_number, created_by, created_at) \
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![report, specimen, format!("{number}-UR"), self.user, AT],
            )
            .unwrap();
        report
    }

    /// The initial released revision of `report`; its stored content fails
    /// its digest when `damaged`.
    fn revision(&mut self, report: ReportId, damaged: bool) -> ReportRevisionId {
        let content_sha = if damaged {
            SHA.to_owned()
        } else {
            sha256_hex("{}")
        };
        let (candidate, revision) = (CandidateId::generate(), ReportRevisionId::generate());
        self.conn
            .execute(
                "INSERT INTO rpt_candidates (id, report_id, proposed_revision_no, relationship, \
                 completeness, signatory_user_id, prepared_at, content_json, content_sha256, \
                 data_json, data_sha256, template_id, template_version, state, revision) \
                 VALUES (?1, ?2, 1, 'initial', 'final', ?3, ?4, '{}', ?5, '{}', ?6, 'tox-report', \
                 2, 'rendering', 1)",
                params![
                    candidate,
                    report,
                    self.user,
                    AT,
                    content_sha,
                    sha256_hex("{}")
                ],
            )
            .unwrap();
        self.conn
            .execute(
                "UPDATE rpt_candidates SET state = 'ready', artifact_sha256 = ?2, \
                 artifact_size = 10, template_sha256 = ?2, renderer_version = 'typst 0.15.1', \
                 fonts_json = '[]', rendered_at = ?3, revision = 2 WHERE id = ?1",
                params![candidate, SHA, AT],
            )
            .unwrap();
        self.conn
            .execute(
                "UPDATE rpt_candidates SET state = 'released', revision = 3 WHERE id = ?1",
                [candidate],
            )
            .unwrap();
        self.release_seq += 1;
        self.conn
            .execute(
                "INSERT INTO audit_events (stream, seq, prev_hash, hash, hash_alg, occurred_at, \
                 recorded_at, principal_kind, principal_id, action, outcome, record_type, \
                 record_id, new_revision, authority, correlation_id, details) \
                 VALUES ('clinical', ?1, ?2, ?2, 'sha256', ?3, ?3, 'user', ?4, 'report.release', \
                 'succeeded', 'report_revision', ?5, 1, 'release_reports', 'k1', '{}')",
                params![self.release_seq, SHA, AT, self.user, revision],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO rpt_revisions (id, report_id, revision_no, candidate_id, \
                 relationship, completeness, content_sha256, artifact_sha256, released_by, \
                 released_at) VALUES (?1, ?2, 1, ?3, 'initial', 'final', ?4, ?5, ?6, ?7)",
                params![revision, report, candidate, content_sha, SHA, self.user, AT],
            )
            .unwrap();
        revision
    }

    /// A report obligation to a new, unverified courier destination,
    /// created `second` seconds after `AT`.
    fn obligation(&self, revision: ReportRevisionId, second: u32) -> ObligationId {
        let (destination, obligation) = (DestinationId::generate(), ObligationId::generate());
        let created_at = format!("2026-10-12T14:00:{second:02}.000000000Z");
        self.conn
            .execute(
                "INSERT INTO rpt_destinations (id, client_id, channel, label, created_by, \
                 created_at, revision) VALUES (?1, ?2, 'courier', 'Synthetic courier', ?3, ?4, 1)",
                params![destination, self.client, self.user, AT],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO rpt_delivery_obligations (id, revision_id, destination_id, kind, \
                 origin, created_by, created_at) VALUES (?1, ?2, ?3, 'report', 'release', ?4, ?5)",
                params![obligation, revision, destination, self.user, created_at],
            )
            .unwrap();
        obligation
    }

    fn page(
        &self,
        filter: DeliveryQueueFilter,
        limit: u32,
        after: Option<ObligationId>,
    ) -> Result<Vec<ObligationId>, ReportingError> {
        Ok(self
            .page_result(filter, limit, after)?
            .items
            .into_iter()
            .map(|view| view.obligation_id)
            .collect())
    }

    fn page_result(
        &self,
        filter: DeliveryQueueFilter,
        limit: u32,
        after: Option<ObligationId>,
    ) -> Result<DeliveryQueuePage, ReportingError> {
        let input = ListDeliveryObligationsInput {
            filter,
            limit,
            after,
        };
        delivery_queue_page(&self.conn, input)
    }
}

fn assert_invalid(result: Result<Vec<ObligationId>, ReportingError>, field: &str) {
    match result {
        Err(ReportingError::Invalid(FieldError { field: got, .. })) => assert_eq!(got, field),
        other => panic!("expected an invalid {field}, got {other:?}"),
    }
}

/// Five obligations of a sound revision, interleaved with four of a damaged one.
fn interleaved(lab: &mut Lab) -> Vec<ObligationId> {
    let sound = lab.report("A26-0001");
    let sound = lab.revision(sound, false);
    let damaged = lab.report("A26-0002");
    let damaged = lab.revision(damaged, true);
    let mut shown = Vec::new();
    for (second, is_damaged) in [true, true, false, true, false, false, true, false, false]
        .into_iter()
        .enumerate()
    {
        let second = u32::try_from(second).unwrap();
        if is_damaged {
            lab.obligation(damaged, second);
        } else {
            shown.push(lab.obligation(sound, second));
        }
    }
    shown
}

#[test]
fn a_limit_outside_1_to_500_is_refused() {
    let lab = Lab::new();
    for limit in [0, 501, u32::MAX] {
        assert_invalid(lab.page(DeliveryQueueFilter::All, limit, None), "limit");
    }
    assert_eq!(lab.page(DeliveryQueueFilter::All, 500, None).unwrap(), []);
}

#[test]
fn an_unknown_cursor_is_refused() {
    let lab = Lab::new();
    let unknown = Some(ObligationId::generate());
    assert_invalid(lab.page(DeliveryQueueFilter::All, 10, unknown), "after");
}

#[test]
fn pages_hold_exactly_the_limit_and_continue_without_repeats_or_gaps() {
    let mut lab = Lab::new();
    let shown = interleaved(&mut lab);

    let first = lab.page(DeliveryQueueFilter::All, 2, None).unwrap();
    assert_eq!(first, shown[0..2]);
    let second = lab
        .page(DeliveryQueueFilter::All, 2, first.last().copied())
        .unwrap();
    assert_eq!(second, shown[2..4]);
    let third = lab
        .page(DeliveryQueueFilter::All, 2, second.last().copied())
        .unwrap();
    assert_eq!(third, shown[4..]);
    let past_end = lab
        .page(DeliveryQueueFilter::All, 2, third.last().copied())
        .unwrap();
    assert_eq!(past_end, []);
    // One large page shows the same rows in the same order.
    assert_eq!(
        lab.page(DeliveryQueueFilter::All, 500, None).unwrap(),
        shown
    );
}

#[test]
fn filters_apply_before_the_limit() {
    let mut lab = Lab::new();
    let shown = interleaved(&mut lab);
    // Every destination is unverified, so every sound obligation is blocked.
    assert_eq!(
        lab.page(DeliveryQueueFilter::Blocked, 3, None).unwrap(),
        shown[0..3]
    );
    for filter in [
        DeliveryQueueFilter::Awaiting,
        DeliveryQueueFilter::Attention,
        DeliveryQueueFilter::Paused,
    ] {
        assert_eq!(lab.page(filter, 3, None).unwrap(), []);
    }
}

#[test]
fn one_request_reads_at_most_four_pages_of_left_out_rows() {
    let mut lab = Lab::new();
    let damaged = lab.report("A26-0002");
    let damaged = lab.revision(damaged, true);
    for second in 0..5 {
        lab.obligation(damaged, second);
    }
    let sound = lab.report("A26-0001");
    let sound = lab.revision(sound, false);
    let shown = lab.obligation(sound, 5);
    // Limit 1: four one-row pages of damaged content end the request.
    let bounded = lab.page_result(DeliveryQueueFilter::All, 1, None).unwrap();
    assert_eq!(bounded.items, []);
    assert!(bounded.next_after.is_some());
    assert_eq!(
        lab.page(DeliveryQueueFilter::All, 1, bounded.next_after)
            .unwrap(),
        [shown]
    );
    // Limit 2: the third two-row page reaches the sound obligation.
    assert_eq!(
        lab.page(DeliveryQueueFilter::All, 2, None).unwrap(),
        [shown]
    );
}

#[test]
fn queue_items_omit_free_text_attempt_notes() {
    let mut lab = Lab::new();
    let report = lab.report("A26-0001");
    let revision = lab.revision(report, false);
    let obligation = lab.obligation(revision, 0);
    lab.conn
        .execute(
            "INSERT INTO rpt_delivery_attempts (id, obligation_id, evidence, signal_source, \
             failure_code, note, observed_at, recorded_by, recorded_at) VALUES (?1, ?2, 'failed', \
             'staff_attestation', 'no_answer_or_busy', ?3, ?4, ?5, ?4)",
            params![
                lis_core::CorrelationId::generate().to_string(),
                obligation,
                "Sensitive synthetic note",
                AT,
                lab.user,
            ],
        )
        .unwrap();
    let exact = lis_reporting::get_delivery_obligation(&lab.conn, obligation).unwrap();
    assert_eq!(
        exact.attempts[0].note.as_deref(),
        Some("Sensitive synthetic note")
    );
    let page = lab.page_result(DeliveryQueueFilter::All, 10, None).unwrap();
    assert!(
        page.items[0]
            .attempts
            .iter()
            .all(|attempt| attempt.note.is_none())
    );
    assert!(
        !serde_json::to_string(&page)
            .unwrap()
            .contains("Sensitive synthetic note")
    );
}

#[test]
fn composite_queue_reaches_all_obligations_beyond_the_desktop_page_limit() {
    let mut lab = Lab::new();
    let report = lab.report("A26-SYN-503");
    let revision = lab.revision(report, false);
    let mut expected: Vec<_> = (0..503).map(|_| lab.obligation(revision, 0)).collect();
    expected.sort();
    let list = |limit| {
        lis_reporting::delivery_queue(
            &lab.conn,
            ListDeliveryObligationsInput {
                filter: DeliveryQueueFilter::All,
                limit,
                after: None,
            },
        )
        .unwrap()
        .into_iter()
        .map(|row| row.obligation_id)
        .collect::<Vec<_>>()
    };
    assert_eq!(list(501), expected[..501]);
    assert_eq!(list(u32::MAX), expected);
    // The external desktop contract remains bounded.
    assert_invalid(lab.page(DeliveryQueueFilter::All, 501, None), "limit");
}
