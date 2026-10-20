//! Snapshot page membership survives appended incidents; clocks do not choose chronology.
use crate::{STAFF, fixture, read, run};
use lis_core::Capability;

#[test]
fn incident_snapshot_excludes_new_rows_until_refresh() {
    let f = fixture();
    let location = run(&f.t, &[Capability::ManageConfiguration], |ctx| {
        lis_specimens::configure_location(
            ctx,
            lis_specimens::ConfigureStorageLocationInput {
                location_id: None,
                expected_revision: None,
                code: "syn-page".into(),
                name: "Synthetic storage".into(),
                storage_condition: None,
                active: true,
            },
        )
    })
    .unwrap();
    let location_id = location.location_id;
    let mut ids = Vec::new();
    for _ in 0..51 {
        let view = run(&f.t, STAFF, move |ctx| {
            let observed_from = ctx.now();
            lis_specimens::record_excursion(
                ctx,
                lis_specimens::RecordExcursionInput {
                    location_id,
                    observed_from,
                    observed_to: None,
                    description: "Synthetic interruption".into(),
                },
            )
        })
        .unwrap();
        ids.push(view.excursion_id);
    }
    let first = read(&f.t, |conn| lis_specimens::list_excursions(conn, 0, None));
    assert_eq!(first.items.len(), 50);
    let new = run(&f.t, STAFF, move |ctx| {
        let observed_from = ctx.now();
        lis_specimens::record_excursion(
            ctx,
            lis_specimens::RecordExcursionInput {
                location_id,
                observed_from,
                observed_to: None,
                description: "Synthetic later interruption".into(),
            },
        )
    })
    .unwrap();
    let snapshot = first.snapshot;
    let last = read(&f.t, move |conn| {
        lis_specimens::list_excursions(conn, 50, Some(snapshot))
    });
    assert_eq!(last.items.len(), 1);
    assert!(!last.has_more);
    assert_eq!(last.items[0].excursion_id, ids[0]);
    let refreshed = read(&f.t, |conn| lis_specimens::list_excursions(conn, 0, None));
    assert_eq!(refreshed.items[0].excursion_id, new.excursion_id);
    let invalid =
        f.t.db
            .read_blocking(|conn| lis_specimens::list_excursions(conn, 0, Some("-1".into())));
    assert_eq!(invalid.unwrap_err().code(), lis_core::ErrorCode::Validation);
}
