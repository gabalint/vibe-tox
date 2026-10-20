use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};

use lis_au::{AuError, StreamDecoder, WireEvent};

use super::AuSession;
use crate::listener::{IO_POLL, touch_status};
use crate::session::StreamContext;
use crate::{AbortCode, AuConnectionSettings, ListenerErrorCode};

pub(crate) fn run_stream<T: Read + Write + ?Sized>(
    stream: &mut T,
    context: &StreamContext<'_>,
    settings: &AuConnectionSettings,
) -> Result<(), ListenerErrorCode> {
    let mut session = AuSession::new(context, settings);
    let result = receive(stream, context, settings, &mut session);
    let reason = match result {
        Ok(()) if context.stop.load(Ordering::Acquire) => AbortCode::ListenerStopped,
        Ok(()) => AbortCode::ConnectionLost,
        Err(ListenerErrorCode::TransportFailed) => AbortCode::TransportError,
        Err(_) => AbortCode::ProtocolError,
    };
    session.abort(context, reason)?;
    result
}

fn receive<T: Read + Write + ?Sized>(
    stream: &mut T,
    context: &StreamContext<'_>,
    settings: &AuConnectionSettings,
    session: &mut AuSession,
) -> Result<(), ListenerErrorCode> {
    let mut decoder = StreamDecoder::new(settings.protocol.clone())
        .map_err(|_| ListenerErrorCode::ProtocolFailed)?;
    let mut events = VecDeque::new();
    let mut frame_started: Option<Instant> = None;
    let connected = Instant::now();
    let mut bytes = [0; 4096];
    while !context.stop.load(Ordering::Acquire) {
        let now = Instant::now();
        session.tick(now, stream, context)?;
        if session.response.is_some() {
            std::thread::park_timeout(IO_POLL);
            continue;
        }
        if let Some(event) = events.pop_front() {
            handle_event(session, event, now, context)?;
            continue;
        }
        if frame_started.is_some_and(|start| {
            now.duration_since(start) >= Duration::from_millis(u64::from(settings.frame_timeout_ms))
        }) {
            decoder.reset();
            frame_started = None;
            session.retry_wire(now, context)?;
        }
        if context.transport.is_tcp()
            && !session.established
            && now.duration_since(connected) >= context.establishment_timeout
        {
            return Err(ListenerErrorCode::EstablishmentTimeout);
        }
        match stream.read(&mut bytes) {
            Ok(0) if context.transport.is_serial() => {}
            Ok(0) => return Ok(()),
            Ok(count) => {
                touch_status(context.statuses, context.instrument_id);
                events.extend(decoder.push(&bytes[..count]));
                if !events.is_empty() {
                    frame_started = None;
                }
                if decoder.is_pending() {
                    // T1 ends at the next frame's start; T2 then bounds that
                    // frame, rather than expiring T1 during serial reception.
                    session.block_deadline = None;
                    frame_started.get_or_insert(now);
                } else {
                    frame_started = None;
                }
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::TimedOut
                        | io::ErrorKind::WouldBlock
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => return Err(ListenerErrorCode::TransportFailed),
        }
    }
    Ok(())
}

/// Decoder errors have not reached the application assembler. A class-B retry
/// preserves its previously durable prefix; semantic errors still abort it.
pub(super) fn handle_event(
    session: &mut AuSession,
    event: Result<WireEvent, AuError>,
    now: Instant,
    context: &StreamContext<'_>,
) -> Result<(), ListenerErrorCode> {
    match event {
        Ok(WireEvent::Frame(frame)) => session.receive(frame, now, context),
        Ok(WireEvent::Ack | WireEvent::Nak) => session.reject(now, context),
        Err(_) => session.retry_wire(now, context),
    }
}
