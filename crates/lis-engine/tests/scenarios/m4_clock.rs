//! Recorded wall-clock jumps (NFR-26; M4 review M9): the clock alert of a backward jump
//! comes from its record, so it survives a restart of the app. Synthetic data only.

use lis_core::{CorrelationId, Principal};
use lis_engine::{Engine, HealthState, KeyProvider};
use lis_ops::health::{CheckCode, ClockStep};
use lis_ops::runs::AppRunId;
use lis_store::Database;

use crate::m4_anchors::MODULES;
use crate::m4_runtime::{Host, key};

async fn clock_check(engine: &Engine) -> (HealthState, Option<String>) {
    let health = engine.get_host_health().await.unwrap();
    let clock = health
        .checks
        .into_iter()
        .find(|check| check.check == CheckCode::Clock)
        .unwrap();
    (clock.state, clock.detail_code)
}

// M9: the light loop of the earlier run recorded a backward jump; the next run alerts.
#[tokio::test]
async fn a_backward_jump_still_alerts_after_a_restart() {
    let host = Host::new();
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    let totp = host.enrol_admin(&engine).await;
    assert_ne!(clock_check(&engine).await.0, HealthState::Alert);
    engine.shutdown().await;
    drop(engine);

    let run: AppRunId = host
        .conn()
        .query_row(
            "SELECT id FROM ops_app_runs ORDER BY started_at DESC LIMIT 1",
            [],
            |row| row.get(0),
        )
        .unwrap();
    let db = Database::open(host.dir.path(), MODULES, host.clock.clone()).unwrap();
    db.write(
        Principal::system("ops-runtime", []),
        CorrelationId::generate(),
        move |ctx| {
            lis_ops::health::record_clock_jump(
                ctx,
                run,
                ClockStep::Backward { seconds: 600 },
                false,
            )
        },
    )
    .await
    .unwrap();
    drop(db);

    host.advance(60 * 60);
    let engine = host.start(KeyProvider::Fixed(key(1))).await;
    host.admin_sign_in(&engine, &totp).await.unwrap();
    assert_eq!(
        clock_check(&engine).await,
        (HealthState::Alert, Some("backward_jump".to_owned()))
    );
}
