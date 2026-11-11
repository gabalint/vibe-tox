//! Reading one event's revisions and details, and the state an earlier event recorded.
use super::*;
use crate::{differing, event_facts, revision_details};

fn record(conn: &mut Connection, event: AuditEvent) -> ChainLink {
    let principal = user();
    let attribution = Attribution {
        principal: &principal,
        occurred_at: at("2026-10-12T10:00:00.000000000Z"),
        recorded_at: at("2026-10-12T10:00:00.000000000Z"),
        correlation_id: CorrelationId::generate(),
        authority: &[Capability::Accession],
    };
    let tx = conn.transaction().unwrap();
    let link = append(&tx, &attribution, &event).unwrap();
    tx.commit().unwrap();
    link
}

fn set_enabled(revision: i64, enabled: bool) -> AuditEvent {
    AuditEvent::new(
        Stream::Security,
        "interface.set_enabled",
        Outcome::Succeeded,
    )
    .record("instrument", "instrument-1")
    .old_revision(revision - 1)
    .new_revision(revision)
    .detail("enabled", AuditDetail::boolean(enabled))
}

#[test]
fn event_facts_read_revisions_and_details() {
    let (_dir, mut conn) = open();
    record(&mut conn, set_enabled(2, false));
    let facts = event_facts(&conn, Stream::Security, 1).unwrap().unwrap();
    assert_eq!(facts.action, "interface.set_enabled");
    assert_eq!(facts.outcome, "succeeded");
    assert_eq!(facts.record_type.as_deref(), Some("instrument"));
    assert_eq!((facts.old_revision, facts.new_revision), (Some(1), Some(2)));
    assert_eq!(facts.details["enabled"], serde_json::Value::Bool(false));
    assert_eq!(event_facts(&conn, Stream::Security, 2).unwrap(), None);
    assert_eq!(event_facts(&conn, Stream::Clinical, 1).unwrap(), None);
}

#[test]
fn revision_details_finds_the_event_that_produced_a_revision() {
    let (_dir, mut conn) = open();
    record(&mut conn, set_enabled(2, false));
    record(&mut conn, set_enabled(3, true));
    let record = ("instrument", "instrument-1");
    let second = revision_details(&conn, Stream::Security, record, 2)
        .unwrap()
        .unwrap();
    assert_eq!(second["enabled"], serde_json::Value::Bool(false));
    assert!(
        revision_details(&conn, Stream::Security, record, 1)
            .unwrap()
            .is_none()
    );
    assert!(
        revision_details(&conn, Stream::Security, ("instrument", "other"), 2)
            .unwrap()
            .is_none()
    );
}

#[test]
fn differing_keeps_changed_fields_in_order() {
    let fields = differing([
        (
            "Name".to_owned(),
            Some("A".to_owned()),
            Some("A".to_owned()),
        ),
        (
            "Phone".to_owned(),
            Some("1".to_owned()),
            Some("2".to_owned()),
        ),
        ("Fax".to_owned(), None, Some("3".to_owned())),
    ]);
    let names: Vec<_> = fields.iter().map(|field| field.field.as_str()).collect();
    assert_eq!(names, ["Phone", "Fax"]);
}
