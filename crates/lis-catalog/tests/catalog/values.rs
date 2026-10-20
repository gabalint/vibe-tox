//! Lexical decimals round-trip exactly (NFR-10); codes and duplicates.

use lis_catalog::{CatalogError, audit_record_label, create_specimen_type, get_method_version};
use lis_core::ErrorCode;

use crate::{Catalog, coded};

#[test]
fn decimal_limits_round_trip_exactly() {
    let c = Catalog::open();
    let id = c.approved_method("SYN-M1");
    let view = c.read(move |conn| get_method_version(conn, id));
    let a = &view.analytes[0].limits;
    let texts = [
        a.cutoff.as_ref().unwrap().as_str(),
        a.limit_of_detection.as_ref().unwrap().as_str(),
        a.limit_of_quantitation.as_ref().unwrap().as_str(),
        a.reportable_low.as_ref().unwrap().as_str(),
        a.reportable_high.as_ref().unwrap().as_str(),
    ];
    assert_eq!(texts, ["111.10", "0.000100", "2.500", "5.0", "1000.00"]);
    assert_eq!(a.unit.as_deref(), Some("ng/mL"));
    assert_eq!(a.display_scale, Some(2));

    // Stored as TEXT, never as a number.
    let stored: Vec<(String, String)> = c.read(move |conn| {
        let mut statement = conn.prepare(
            "SELECT typeof(cutoff), cutoff FROM cat_method_version_analytes \
             WHERE method_version_id = ?1",
        )?;
        let rows = statement.query_map([id], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    });
    assert!(
        stored
            .iter()
            .all(|(kind, text)| kind == "text" && text == "111.10")
    );

    // Decimals cross the UI boundary as strings with the same text.
    let json = serde_json::to_value(&view).unwrap();
    assert_eq!(json["analytes"][0]["cutoff"], "111.10");
    assert_eq!(json["analytes"][0]["limitOfDetection"], "0.000100");
    assert_eq!(json["status"], "approved");
    assert_eq!(json["thresholdRule"], "at_or_above");
}

#[test]
fn duplicate_and_malformed_codes_are_refused() {
    let c = Catalog::open();
    let before = c.counts();
    let duplicate = c.as_author(|ctx| create_specimen_type(ctx, coded("SYN-UR", "Another")));
    assert!(matches!(
        duplicate,
        Err(CatalogError::Duplicate("specimen type"))
    ));
    let malformed = c.as_author(|ctx| create_specimen_type(ctx, coded("bad code", "Another")));
    assert_eq!(malformed.unwrap_err().code(), ErrorCode::Validation);
    assert!(matches!(
        c.analyte("SYN-A"),
        Err(CatalogError::Duplicate("analyte"))
    ));
    assert_eq!(c.counts(), before);
}

#[test]
fn reference_creation_audits_record_ids_without_dynamic_codes() {
    let c = Catalog::open();
    assert_eq!(
        c.audit_actions(),
        [
            "catalog.create_specimen_type",
            "catalog.create_analyte",
            "catalog.create_analyte"
        ]
    );
    let events: Vec<(String, String)> = c.read(|conn| {
        let mut statement =
            conn.prepare("SELECT record_id, details FROM audit_events ORDER BY seq")?;
        let rows = statement.query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?;
        Ok(rows.collect::<rusqlite::Result<_>>()?)
    });
    assert!(
        events
            .iter()
            .all(|(record_id, details)| !record_id.is_empty() && details == "{}")
    );
}

#[test]
fn audit_labels_are_resolved_from_current_catalog_reads() {
    let c = Catalog::open();
    let id = c.approved_method("SYN-M1");
    let view = c.read(move |conn| get_method_version(conn, id));
    let label = c.read(move |conn| audit_record_label(conn, "method_version", &id.to_string()));
    assert_eq!(
        label,
        Some(format!("{} v{}", view.method_name, view.version))
    );
    assert_eq!(
        c.read(|conn| audit_record_label(conn, "method_version", "not-an-id")),
        None
    );
}

#[test]
fn approved_report_content_and_range_policy_round_trip_and_freeze() {
    let c = Catalog::open();
    let method = c.method("SYN-REPORT");
    let mut content = c.content();
    content.report_notes = Some("Synthetic method limitations α".to_owned());
    content.analytes[0].reference_interval =
        Some("Synthetic reference: see supplied context".to_owned());
    content.analytes[0].range_policy = lis_catalog::RangePolicy::RequireWithinRange;
    let draft = c
        .as_author(move |ctx| {
            lis_catalog::draft_method_version(
                ctx,
                lis_catalog::DraftMethodVersion {
                    method_id: method,
                    content,
                },
            )
        })
        .unwrap();
    c.approve_method(draft.id, draft.revision).unwrap();
    let id = draft.id;
    let view = c.read(move |conn| get_method_version(conn, id));
    assert_eq!(
        view.report_notes.as_deref(),
        Some("Synthetic method limitations α")
    );
    assert_eq!(
        view.analytes[0].limits.range_policy,
        lis_catalog::RangePolicy::RequireWithinRange
    );
    assert_eq!(
        view.analytes[0].limits.reference_interval.as_deref(),
        Some("Synthetic reference: see supplied context")
    );
    for sql in [
        "UPDATE cat_method_versions SET report_notes = 'changed' WHERE id = ?1",
        "UPDATE cat_method_version_analytes SET reference_interval = 'changed' WHERE method_version_id = ?1",
        "UPDATE cat_method_version_analytes SET range_policy = 'report_only' WHERE method_version_id = ?1",
    ] {
        assert!(c.raw().execute(sql, [id]).is_err());
    }
}
