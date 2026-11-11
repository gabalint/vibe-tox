//! Days left and the lots expiring soon (#4557).

use lis_analytical::{LotState, MaterialKind, WithdrawLotInput};
use lis_core::UtcTimestamp;

use crate::{STAFF, fixture, read, run};

#[test]
fn lots_within_the_warning_period_are_expiring_soon() {
    let f = fixture();
    let today = f.lot(MaterialKind::Control, "C-TODAY", "2026-01-02");
    let at_limit = f.lot(MaterialKind::Reagent, "R-14", "2026-01-16");
    let beyond = f.lot(MaterialKind::Reagent, "R-15", "2026-01-17");
    let expired = f.lot(MaterialKind::Reagent, "R-OLD", "2026-01-01");
    let withdrawn = f.lot(MaterialKind::Calibrator, "K-OUT", "2026-01-05");
    let input = WithdrawLotInput {
        lot_id: withdrawn,
        expected_revision: 1,
        reason: "Synthetic recall".to_owned(),
    };
    run(&f, STAFF, move |ctx| {
        lis_analytical::withdraw_lot(ctx, input)
    })
    .unwrap();
    let now = lis_store::testing::test_start();

    let lots = read(&f, move |conn| {
        lis_analytical::list_lots(conn, false, now, Some(14))
    });
    let of = |id| lots.iter().find(|lot| lot.lot_id == id).unwrap();
    for (id, days_left, soon) in [
        (today, Some(0), true),
        (at_limit, Some(14), true),
        (beyond, Some(15), false),
        (expired, None, false),
        (withdrawn, Some(3), false),
    ] {
        assert_eq!((of(id).days_left, of(id).expiring_soon), (days_left, soon));
    }
    assert_eq!(of(withdrawn).state, LotState::Withdrawn);

    let unmarked = read(&f, move |conn| {
        lis_analytical::list_lots(conn, false, now, None)
    });
    assert!(unmarked.iter().all(|lot| !lot.expiring_soon));

    let soon = read(&f, move |conn| {
        lis_analytical::lots_expiring_soon(conn, now, 14)
    });
    let mut ids: Vec<_> = soon.iter().map(|item| item.lot.lot_id).collect();
    ids.sort_unstable();
    let mut expected = vec![today, at_limit];
    expected.sort_unstable();
    assert_eq!(ids, expected);
}

#[test]
fn a_lot_enters_the_queue_at_the_start_of_its_warning_period() {
    let f = fixture();
    let lot = f.lot(MaterialKind::Control, "C-MARCH", "2026-03-01");
    let start = lis_store::testing::test_start();
    let early = read(&f, move |conn| {
        lis_analytical::lots_expiring_soon(conn, start, 14)
    });
    assert!(early.is_empty());

    let later = UtcTimestamp::parse("2026-02-20T10:00:00.000000000Z").unwrap();
    let soon = read(&f, move |conn| {
        lis_analytical::lots_expiring_soon(conn, later, 14)
    });
    assert_eq!(soon.len(), 1);
    assert_eq!(soon[0].lot.lot_id, lot);
    assert_eq!(soon[0].lot.days_left, Some(9));
    assert_eq!(
        soon[0].since,
        UtcTimestamp::parse("2026-02-15T00:00:00.000000000Z").unwrap()
    );
}
