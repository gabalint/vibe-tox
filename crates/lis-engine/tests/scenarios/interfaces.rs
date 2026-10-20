//! Instrument configuration through the engine (REQ-INT-005, REQ-INT-009,
//! REQ-OPS-001): a change restarts the listeners so it takes effect at once.

use std::net::{IpAddr, Ipv4Addr, TcpListener};
use std::time::Duration;

use lis_core::{CommandError, ErrorCode, InstrumentId, UserId};
use lis_engine::{Engine, RegisterInstrument, SetInstrumentEnabled, UpdateInstrumentConnection};
use lis_interfaces::{ConnectionSettings, ListenerDetailCode, ListenerState};

use crate::fixture::{TestEngine, code_of};

fn tcp(port: u16, allowed_peers: Vec<IpAddr>) -> ConnectionSettings {
    ConnectionSettings::AstmTcp {
        bind_address: IpAddr::V4(Ipv4Addr::LOCALHOST),
        port,
        allowed_peers,
    }
}

fn free_port() -> u16 {
    TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}

fn registration(port: u16) -> RegisterInstrument {
    RegisterInstrument {
        code: "SYN-ANALYZER".to_owned(),
        name: "Synthetic analyzer".to_owned(),
        connection: tcp(port, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]),
        enabled: true,
    }
}

/// Calls the interfaces command `name` with plausible input; `None` for other areas.
pub(crate) async fn interfaces_call(
    engine: &Engine,
    name: &str,
    _target: UserId,
) -> Option<Result<(), CommandError>> {
    let instrument_id = InstrumentId::generate();
    Some(match name {
        "list_instruments" => engine.list_instruments().await.map(drop),
        "register_instrument" => engine.register_instrument(registration(1)).await.map(drop),
        "update_instrument_connection" => engine
            .update_instrument_connection(UpdateInstrumentConnection {
                instrument_id,
                expected_revision: 1,
                connection: tcp(1, vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]),
            })
            .await
            .map(drop),
        "set_instrument_enabled" => engine
            .set_instrument_enabled(SetInstrumentEnabled {
                instrument_id,
                expected_revision: 1,
                enabled: false,
            })
            .await
            .map(drop),
        _ => return None,
    })
}

/// The listener state of the only instrument, waiting up to two seconds for `wanted`.
async fn wait_for_state(
    t: &TestEngine,
    wanted: ListenerState,
) -> (ListenerState, Option<ListenerDetailCode>) {
    for _ in 0..200 {
        let status = t.engine.list_listener_status().await.unwrap();
        if status[0].state == wanted {
            return (status[0].state, status[0].detail);
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("the listener did not reach {wanted:?}");
}

#[tokio::test]
async fn configuration_changes_restart_the_listeners() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let registered = t
        .engine
        .register_instrument(registration(free_port()))
        .await
        .unwrap();
    assert!(registered.listeners_restarted);
    assert!(
        registered.message.contains("restarted"),
        "{}",
        registered.message
    );
    wait_for_state(&t, ListenerState::Listening).await;

    let moved = t
        .engine
        .update_instrument_connection(UpdateInstrumentConnection {
            instrument_id: registered.instrument.id,
            expected_revision: registered.instrument.revision,
            connection: tcp(free_port(), vec![IpAddr::V4(Ipv4Addr::LOCALHOST)]),
        })
        .await
        .unwrap();
    assert!(moved.listeners_restarted);
    wait_for_state(&t, ListenerState::Listening).await;

    let disabled = t
        .engine
        .set_instrument_enabled(SetInstrumentEnabled {
            instrument_id: registered.instrument.id,
            expected_revision: moved.instrument.revision,
            enabled: false,
        })
        .await
        .unwrap();
    assert!(!disabled.instrument.enabled);
    let state = wait_for_state(&t, ListenerState::Disabled).await;
    assert_eq!(state.1, Some(ListenerDetailCode::ConfigurationDisabled));

    let listed = t.engine.list_instruments().await.unwrap();
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].revision, 3);
    let security: Vec<String> = t.events("security").into_iter().map(|e| e.0).collect();
    for action in [
        "interface.register_instrument",
        "interface.update_connection",
        "interface.set_enabled",
    ] {
        assert!(security.iter().any(|a| a == action), "{action}");
    }
    t.engine.shutdown().await;
}

#[tokio::test]
async fn a_tcp_instrument_needs_a_lab_network_allow_list() {
    let t = TestEngine::start().await;
    t.admin_session().await;
    let mut input = registration(free_port());
    input.connection = tcp(free_port(), vec![]);
    let error = t.engine.register_instrument(input).await.unwrap_err();
    assert_eq!(error.code(), ErrorCode::Validation);
    let json = serde_json::to_value(&error).unwrap();
    assert_eq!(json["fieldErrors"][0]["field"], "connection.allowedPeers");
    let mut public = registration(free_port());
    public.connection = tcp(free_port(), vec![IpAddr::V4(Ipv4Addr::new(8, 8, 8, 8))]);
    assert_eq!(
        code_of(t.engine.register_instrument(public).await),
        ErrorCode::Validation
    );
    assert!(t.engine.list_instruments().await.unwrap().is_empty());
}
