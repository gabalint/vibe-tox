use std::io;
use std::net::{IpAddr, SocketAddr, TcpListener, TcpStream};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use lis_core::{Capability, CorrelationId, InstrumentId, Principal};
use lis_store::Database;

use crate::ListenerErrorCode;
use crate::commands::audit_rejected_peer;
use crate::listener::{IO_POLL, StatusMap, detail_for_error, set_status};
use crate::model::InstrumentTransport;
use crate::peer_rejections::{PeerRejections, RejectionTally};
use crate::session::{StreamContext, run_protocol_stream};
use crate::status::{ListenerDetailCode, ListenerState};

pub(crate) struct TcpListenerConfig {
    pub(crate) database: Arc<Database>,
    pub(crate) statuses: StatusMap,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) bind_address: IpAddr,
    pub(crate) port: u16,
    pub(crate) allowed_peers: Vec<IpAddr>,
    pub(crate) stop: Arc<AtomicBool>,
    pub(crate) establishment_timeout: Duration,
    pub(crate) au: Option<crate::AuConnectionSettings>,
}

pub(crate) fn spawn(name: String, context: TcpListenerConfig) -> io::Result<JoinHandle<()>> {
    let listener = TcpListener::bind((context.bind_address, context.port))?;
    listener.set_nonblocking(true)?;
    thread::Builder::new()
        .name(name)
        .spawn(move || run(listener, context))
}

fn run(listener: TcpListener, context: TcpListenerConfig) {
    set_status(
        &context.statuses,
        context.instrument_id,
        ListenerState::Listening,
        None,
    );
    tracing::info!(instrument_id = %context.instrument_id, transport = if context.au.is_some() { "au_tcp" } else { "astm_tcp" }, "listener started");
    let mut rejections = RejectionTally::default();
    let accepted = accept_until_stopped(&listener, &context, &mut rejections);
    audit_rejections(&context, rejections.take_remaining());
    if accepted.is_ok() {
        tracing::info!(instrument_id = %context.instrument_id, "listener stopped");
        return;
    }
    set_status(
        &context.statuses,
        context.instrument_id,
        ListenerState::Error,
        Some(ListenerDetailCode::TransportFailed),
    );
    tracing::warn!(instrument_id = %context.instrument_id, error_code = ?ListenerErrorCode::TransportFailed, "listener stopped");
}

fn accept_until_stopped(
    listener: &TcpListener,
    context: &TcpListenerConfig,
    rejections: &mut RejectionTally,
) -> io::Result<()> {
    while !context.stop.load(Ordering::Acquire) {
        match listener.accept() {
            Ok((stream, peer)) => handle_connection(stream, peer, context, rejections),
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                thread::park_timeout(IO_POLL);
            }
            Err(error) => return Err(error),
        }
        audit_rejections(context, rejections.take_due(Instant::now()));
    }
    Ok(())
}

/// Serves an allowed analyzer; closes any other peer before ASTM establishment
/// and only counts it, so a flood costs no database work per connection
/// (REQ-INT-009, NFR-08).
pub(crate) fn handle_connection(
    mut stream: TcpStream,
    peer: SocketAddr,
    context: &TcpListenerConfig,
    rejections: &mut RejectionTally,
) {
    if context.allowed_peers.contains(&peer.ip()) {
        serve_peer(&mut stream, peer, context);
    } else {
        drop(stream);
        rejections.record(peer.ip());
    }
}

fn serve_peer(stream: &mut TcpStream, peer: SocketAddr, context: &TcpListenerConfig) {
    if configure_stream(stream).is_err() {
        set_status(
            &context.statuses,
            context.instrument_id,
            ListenerState::Error,
            Some(ListenerDetailCode::TransportFailed),
        );
        return;
    }
    set_status(
        &context.statuses,
        context.instrument_id,
        ListenerState::Connected,
        None,
    );
    let stream_context = StreamContext {
        database: &context.database,
        statuses: &context.statuses,
        instrument_id: context.instrument_id,
        transport: if context.au.is_some() {
            InstrumentTransport::AuTcp
        } else {
            InstrumentTransport::AstmTcp
        },
        peer: Some(peer),
        stop: &context.stop,
        establishment_timeout: context.establishment_timeout,
    };
    let result = run_protocol_stream(stream, &stream_context, context.au.as_ref());
    if let Err(code) = result {
        tracing::warn!(instrument_id = %context.instrument_id, error_code = ?code, "listener connection ended");
    }
    if !context.stop.load(Ordering::Acquire) {
        set_status(
            &context.statuses,
            context.instrument_id,
            ListenerState::Listening,
            result.err().map(detail_for_error),
        );
    }
}

fn configure_stream(stream: &TcpStream) -> io::Result<()> {
    stream.set_read_timeout(Some(IO_POLL))?;
    stream.set_write_timeout(Some(IO_POLL))
}

/// Writes every due summary in one transaction, so audit writes are bounded
/// by the tally's window and peer cap, not by the connection rate.
fn audit_rejections(context: &TcpListenerConfig, due: Vec<PeerRejections>) {
    if due.is_empty() {
        return;
    }
    let instrument_id = context.instrument_id;
    let rejected_connections: usize = due.iter().map(|item| item.rejected_connections).sum();
    let written = context.database.write_blocking(
        Principal::system("interface-tcp-listener", [Capability::InstrumentIntake]),
        CorrelationId::generate(),
        move |ctx| {
            due.into_iter()
                .try_for_each(|rejections| audit_rejected_peer(ctx, instrument_id, rejections))
        },
    );
    let (state, detail, code) = if written.is_ok() {
        (
            ListenerState::Listening,
            ListenerDetailCode::UnexpectedPeerRejected,
            ListenerErrorCode::UnexpectedPeer,
        )
    } else {
        (
            ListenerState::Error,
            ListenerDetailCode::PersistenceFailed,
            ListenerErrorCode::PersistenceFailed,
        )
    };
    set_status(&context.statuses, instrument_id, state, Some(detail));
    tracing::warn!(instrument_id = %instrument_id, error_code = ?code, rejected_connections, "listener rejected peers");
}
