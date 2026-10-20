//! Schema migration evidence, including conflicting physical/audit order.
//! Synthetic placeholders isolate chronology from clinical gate fixtures.
#![cfg(test)]

use lis_results::{ResultId, ResultRevisionId, ReviewStatus, TechnicalReviewId};
use rusqlite::{Connection, OptionalExtension, params};

const EARLY: &str = "2026-10-14T08:00:00.000000000Z";
const LATE: &str = "2026-10-14T09:00:00.000000000Z";
const ACTOR_A: &str = "00000000-0000-7000-8000-000000000001";
const ACTOR_B: &str = "00000000-0000-7000-8000-000000000002";

struct Fixture {
    conn: Connection,
    result: ResultId,
    revision: ResultRevisionId,
}

fn fixture() -> Fixture {
    let conn = Connection::open_in_memory().unwrap();
    conn.pragma_update(None, "foreign_keys", false).unwrap();
    for schema in [
        lis_audit::SCHEMA,
        lis_store::SCHEMA,
        lis_auth::SCHEMA,
        lis_identity::SCHEMA,
        lis_catalog::SCHEMA,
        lis_interfaces::SCHEMA,
        lis_orders::SCHEMA,
        lis_specimens::SCHEMA,
        lis_analytical::SCHEMA,
    ] {
        for migration in schema.migrations {
            conn.execute_batch(migration.sql).unwrap();
        }
    }
    for migration in &lis_results::MIGRATIONS[..7] {
        conn.execute_batch(migration.sql).unwrap();
    }
    let result = ResultId::generate();
    let revision = ResultRevisionId::generate();
    conn.execute(
        "INSERT INTO res_results(id,order_component_id,analyte_id,purpose,revision) \
         VALUES (?1,'synthetic-component','synthetic-analyte','analytical',1)",
        [result],
    )
    .unwrap();
    let f = Fixture {
        conn,
        result,
        revision,
    };
    f.add_revision(revision, 1);
    f
}

impl Fixture {
    fn add_revision(&self, id: ResultRevisionId, number: i64) {
        self.conn.execute(
            "INSERT INTO res_result_revisions \
             (id,result_id,revision_no,cause,outcome,classification,method_version_id, \
              method_certainty,output_scale,sequence_role,specimen_id,created_by,created_at,reason) \
             VALUES (?1,?2,?3,'disposition','not_performed','not_applicable', \
              'synthetic-method','definitive','quantitative','first_line', \
              'synthetic-specimen',?4,?5,'Synthetic disposition')",
            params![id,self.result,number,ACTOR_A,EARLY],
        ).unwrap();
        self.conn
            .execute(
                "UPDATE res_results SET current_revision_id=?1,revision=revision+1 WHERE id=?2",
                params![id, self.result],
            )
            .unwrap();
    }

    fn review(&self, sequence: i64, actor: &str, at: &str, decision: &str) -> TechnicalReviewId {
        let id = TechnicalReviewId::generate();
        self.conn
            .execute(
                "INSERT INTO res_technical_reviews \
             (id,specimen_id,decision,reason,reviewed_by,reviewed_at,sequence) \
             VALUES (?1,'synthetic-specimen',?2,'Synthetic review evidence',?3,?4,?5)",
                params![id, decision, actor, at, sequence],
            )
            .unwrap();
        self.conn
            .execute(
                "INSERT INTO res_review_items(review_id,result_revision_id) VALUES (?1,?2)",
                params![id, self.revision],
            )
            .unwrap();
        id
    }

    fn audit(&self, id: TechnicalReviewId, seq: i64, actor: &str, at: &str) {
        // Real schema; chain placeholders deliberately do not claim audit-chain validation.
        self.conn
            .execute(
                "INSERT INTO audit_events \
             (stream,seq,prev_hash,hash,hash_alg,occurred_at,recorded_at,principal_kind, \
              principal_id,action,outcome,record_type,record_id,authority,correlation_id,details) \
             VALUES ('clinical',?1,?2,?2,'sha256',?3,?3,'user',?4,'result.submit_review', \
              'succeeded','technical_review',?5,'review_results','synthetic','{}')",
                params![seq, "0".repeat(64), at, actor, id],
            )
            .unwrap();
    }

    fn migrate(&self) {
        self.conn
            .execute_batch(lis_results::MIGRATIONS[7].sql)
            .unwrap();
    }

    fn latest(&self) -> Option<(String, String)> {
        self.conn
            .query_row(
                "SELECT latest.review_id,review.reviewed_by FROM res_v_latest_reviews latest \
             JOIN res_technical_reviews review ON review.id=latest.review_id \
             WHERE latest.result_id=?1",
                [self.result],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()
            .unwrap()
    }

    fn approved_lines(&self) -> i64 {
        self.conn
            .query_row("SELECT count(*) FROM res_v_reviewed_lines", [], |r| {
                r.get(0)
            })
            .unwrap()
    }

    fn assert_unresolved(&self) {
        assert_eq!(self.latest(), None);
        assert_eq!(self.approved_lines(), 0);
        assert!(matches!(
            lis_results::review_status(&self.conn,self.revision).unwrap(),
            ReviewStatus::Stale { reasons } if reasons.iter().any(|r|r.code=="review_chronology_unresolved")
        ));
    }
}

#[test]
fn audit_order_overrides_physical_sequence_and_keeps_exact_latest_reviewer() {
    for (first_time, last_time) in [(LATE, EARLY), (EARLY, EARLY)] {
        let f = fixture();
        // Physically first B was causally last. Neither row order nor clocks decide.
        let b = f.review(1, ACTOR_B, last_time, "approved");
        let a = f.review(2, ACTOR_A, first_time, "approved");
        f.audit(a, 10, ACTOR_A, first_time);
        f.audit(b, 20, ACTOR_B, last_time);
        let before = original_rows(&f.conn);
        f.migrate();
        assert_eq!(f.latest(), Some((b.to_string(), ACTOR_B.to_owned())));
        assert_eq!(f.approved_lines(), 1);
        assert_eq!(original_rows(&f.conn), before);
        // Reporting consumes this exact reviewer for separation of duties.
        let reviewer: String = f
            .conn
            .query_row(
                "SELECT r.reviewed_by FROM res_v_reviewed_lines l \
             JOIN res_technical_reviews r ON r.id=l.review_id",
                [],
                |r| r.get(0),
            )
            .unwrap();
        assert_eq!(reviewer, ACTOR_B);
        f.conn.execute_batch("VACUUM").unwrap();
        assert_eq!(f.latest(), Some((b.to_string(), ACTOR_B.to_owned())));
    }
}

#[test]
fn missing_duplicate_and_mismatched_audits_never_choose_other_legacy_approval() {
    for problem in ["missing", "duplicate", "actor", "timestamp"] {
        let f = fixture();
        let known = f.review(1, ACTOR_A, EARLY, "approved");
        f.audit(known, 10, ACTOR_A, EARLY);
        let uncertain = f.review(2, ACTOR_B, LATE, "approved");
        match problem {
            "duplicate" => {
                f.audit(uncertain, 20, ACTOR_B, LATE);
                f.audit(uncertain, 21, ACTOR_B, LATE);
            }
            "actor" => f.audit(uncertain, 20, ACTOR_A, LATE),
            "timestamp" => f.audit(uncertain, 20, ACTOR_B, EARLY),
            _ => {}
        }
        let before = original_rows(&f.conn);
        f.migrate();
        f.assert_unresolved();
        assert_eq!(original_rows(&f.conn), before);
        let rows: i64 = f
            .conn
            .query_row("SELECT count(*) FROM res_legacy_review_evidence", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(rows, 2, "unknown legacy rows remain explicit");
        assert!(
            f.conn
                .execute("DELETE FROM res_legacy_review_evidence", [])
                .is_err()
        );
        assert!(
            f.conn
                .execute("UPDATE res_legacy_review_evidence SET clinical_seq=100", [])
                .is_err()
        );
        assert!(
            f.conn
                .execute(
                    "INSERT INTO res_legacy_review_evidence VALUES ('invented',1,100)",
                    []
                )
                .is_err()
        );
    }
}

#[test]
fn new_review_recovers_without_inventing_legacy_order_and_rejection_still_wins() {
    let f = fixture();
    f.review(1, ACTOR_A, LATE, "approved"); // No recoverable audit mapping.
    f.migrate();
    f.assert_unresolved();
    let b = f.review(2, ACTOR_B, EARLY, "approved");
    f.audit(b, 30, ACTOR_B, EARLY);
    assert_eq!(f.latest(), Some((b.to_string(), ACTOR_B.to_owned())));
    assert_eq!(f.approved_lines(), 1);
    let rejected = f.review(3, ACTOR_A, EARLY, "rejected");
    f.audit(rejected, 31, ACTOR_A, EARLY);
    assert_eq!(f.approved_lines(), 0);
    assert!(
        matches!(lis_results::review_status(&f.conn,f.revision).unwrap(),
        ReviewStatus::Stale{reasons} if reasons[0].code=="review_rejected")
    );
    let approved = f.review(4, ACTOR_B, EARLY, "approved");
    f.audit(approved, 32, ACTOR_B, EARLY);
    assert_eq!(f.approved_lines(), 1);
    let next = ResultRevisionId::generate();
    f.add_revision(next, 2);
    assert_eq!(
        f.approved_lines(),
        0,
        "approval of an older revision is not current"
    );
    assert_eq!(f.latest(), Some((approved.to_string(), ACTOR_B.to_owned())));
}

#[test]
fn post_migration_review_without_unique_audit_is_stale() {
    for duplicate in [false, true] {
        let f = fixture();
        f.migrate();
        let review = f.review(1, ACTOR_A, EARLY, "approved");
        if duplicate {
            f.audit(review, 1, ACTOR_A, EARLY);
            f.audit(review, 2, ACTOR_A, EARLY);
        }
        assert!(
            matches!(lis_results::review_status(&f.conn,f.revision).unwrap(),
            ReviewStatus::Stale{reasons} if reasons[0].code=="review_chronology_unresolved")
        );
    }
}

fn original_rows(conn: &Connection) -> Vec<(String, String, String, String, i64)> {
    conn.prepare("SELECT id,decision,reviewed_by,reviewed_at,sequence FROM res_technical_reviews ORDER BY id")
        .unwrap().query_map([],|r|Ok((r.get(0)?,r.get(1)?,r.get(2)?,r.get(3)?,r.get(4)?)))
        .unwrap().collect::<Result<_,_>>().unwrap()
}
