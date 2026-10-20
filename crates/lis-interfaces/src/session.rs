use std::collections::VecDeque;
use std::io::{self, Read, Write};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use lis_astm::{
    ACK, AbortReason, Action, ControlResponse, FrameId, NAK, ReceiveDriver, ReceiveEvent, TimerId,
};
use lis_core::InstrumentId;
use lis_store::Database;

use crate::ListenerErrorCode;
use crate::listener::{StatusMap, set_status, touch_status};
use crate::model::{AbortCode, InstrumentTransport};
use crate::receipt_writer::ReceiptWriter;
use crate::status::ListenerState;

pub(crate) struct StreamContext<'a> {
    pub(crate) database: &'a Database,
    pub(crate) statuses: &'a StatusMap,
    pub(crate) instrument_id: InstrumentId,
    pub(crate) transport: InstrumentTransport,
    pub(crate) peer: Option<SocketAddr>,
    pub(crate) stop: &'a AtomicBool,
    pub(crate) establishment_timeout: Duration,
}

pub(crate) fn run_protocol_stream<T: Read + Write + ?Sized>(
    stream: &mut T,
    context: &StreamContext<'_>,
    au: Option<&crate::AuConnectionSettings>,
) -> Result<(), ListenerErrorCode> {
    match au {
        Some(settings) => crate::au_session::run_stream(stream, context, settings),
        None => run_stream(stream, context),
    }
}

pub(crate) fn run_stream<T: Read + Write + ?Sized>(
    stream: &mut T,
    context: &StreamContext<'_>,
) -> Result<(), ListenerErrorCode> {
    let mut session = Session::new(context.instrument_id, context.transport, context.peer);
    let establishment_deadline = Instant::now() + context.establishment_timeout;
    let mut buffer = vec![0_u8; 4096];
    loop {
        if context.stop.load(Ordering::Acquire) {
            session
                .writer
                .abort(context.database, AbortCode::ListenerStopped)?;
            return Ok(());
        }
        if context.transport == InstrumentTransport::AstmTcp
            && !session.established
            && Instant::now() >= establishment_deadline
        {
            return Err(ListenerErrorCode::EstablishmentTimeout);
        }
        if let Some(timer_id) = session.expired_timer() {
            handle_or_abort(
                &mut session,
                ReceiveEvent::TimerExpired(timer_id),
                stream,
                context.database,
                context.statuses,
            )?;
        }
        match stream.read(&mut buffer) {
            Ok(0) if context.transport == InstrumentTransport::AstmSerial => {}
            Ok(0) => {
                session
                    .writer
                    .abort(context.database, AbortCode::ConnectionLost)?;
                return Ok(());
            }
            Ok(read) => {
                touch_status(context.statuses, context.instrument_id);
                handle_or_abort(
                    &mut session,
                    ReceiveEvent::Bytes(buffer[..read].to_vec()),
                    stream,
                    context.database,
                    context.statuses,
                )?;
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock
                        | io::ErrorKind::TimedOut
                        | io::ErrorKind::Interrupted
                ) => {}
            Err(_) => {
                session
                    .writer
                    .abort(context.database, AbortCode::TransportError)?;
                return Err(ListenerErrorCode::TransportFailed);
            }
        }
    }
}

fn handle_or_abort<T: Write + ?Sized>(
    session: &mut Session,
    event: ReceiveEvent,
    stream: &mut T,
    database: &Database,
    statuses: &StatusMap,
) -> Result<(), ListenerErrorCode> {
    let Err(code) = session.handle_event(event, stream, database, statuses) else {
        return Ok(());
    };
    let reason = match code {
        ListenerErrorCode::TransportFailed => AbortCode::TransportError,
        ListenerErrorCode::EstablishmentTimeout => AbortCode::ProtocolError,
        ListenerErrorCode::OpenFailed
        | ListenerErrorCode::PersistenceFailed
        | ListenerErrorCode::ProtocolFailed
        | ListenerErrorCode::UnexpectedPeer => AbortCode::ProtocolError,
    };
    session.writer.abort(database, reason)?;
    Err(code)
}

pub(crate) struct Session {
    instrument_id: InstrumentId,
    writer: ReceiptWriter,
    driver: ReceiveDriver,
    timer: Option<(TimerId, Instant)>,
    established: bool,
}

impl Session {
    pub(crate) fn new(
        instrument_id: InstrumentId,
        transport: InstrumentTransport,
        peer: Option<SocketAddr>,
    ) -> Self {
        Self {
            instrument_id,
            writer: ReceiptWriter::new(instrument_id, transport, peer),
            driver: ReceiveDriver::default(),
            timer: None,
            established: false,
        }
    }

    pub(crate) fn handle_event<T: Write + ?Sized>(
        &mut self,
        event: ReceiveEvent,
        stream: &mut T,
        database: &Database,
        statuses: &StatusMap,
    ) -> Result<(), ListenerErrorCode> {
        let mut events = VecDeque::from([event]);
        while let Some(event) = events.pop_front() {
            let actions = self
                .driver
                .handle(event)
                .map_err(|_| ListenerErrorCode::ProtocolFailed)?;
            let mut persisted = Vec::new();
            for action in actions {
                self.perform(action, stream, database, statuses, &mut persisted)?;
            }
            events.extend(persisted.into_iter().map(ReceiveEvent::Persisted));
        }
        Ok(())
    }

    fn perform<T: Write + ?Sized>(
        &mut self,
        action: Action,
        stream: &mut T,
        database: &Database,
        statuses: &StatusMap,
        persisted: &mut Vec<FrameId>,
    ) -> Result<(), ListenerErrorCode> {
        match action {
            Action::Send(response) => {
                if response == ControlResponse::Ack && !self.writer.is_active() {
                    self.writer.begin(database, None)?;
                    self.established = true;
                    set_status(statuses, self.instrument_id, ListenerState::Receiving, None);
                }
                send_response(stream, response)?;
            }
            Action::PersistFrame { frame_id, bytes } => {
                self.writer.persist(database, bytes)?;
                persisted.push(frame_id);
            }
            Action::MessageComplete(records) => {
                self.writer
                    .complete(database, &assembled_message(records))?;
                set_status(statuses, self.instrument_id, ListenerState::Connected, None);
            }
            Action::TransferAborted(reason) => {
                self.writer.abort(database, abort_reason(reason))?;
                set_status(statuses, self.instrument_id, ListenerState::Connected, None);
            }
            Action::StartTimer { timer_id, duration } => {
                self.timer = Some((timer_id, Instant::now() + duration));
            }
            Action::StopTimer => self.timer = None,
        }
        Ok(())
    }

    fn expired_timer(&self) -> Option<TimerId> {
        self.timer
            .filter(|(_, deadline)| Instant::now() >= *deadline)
            .map(|(timer_id, _)| timer_id)
    }

    #[cfg(test)]
    pub(crate) fn current_timer(&self) -> Option<TimerId> {
        self.timer.map(|(timer_id, _)| timer_id)
    }
}

fn send_response(
    stream: &mut (impl Write + ?Sized),
    response: ControlResponse,
) -> Result<(), ListenerErrorCode> {
    let byte = match response {
        ControlResponse::Ack => ACK,
        ControlResponse::Nak => NAK,
    };
    stream
        .write_all(&[byte])
        .and_then(|()| stream.flush())
        .map_err(|_| ListenerErrorCode::TransportFailed)
}

pub(crate) fn assembled_message(records: Vec<Vec<u8>>) -> Vec<u8> {
    let capacity = records.iter().map(|record| record.len() + 1).sum();
    let mut bytes = Vec::with_capacity(capacity);
    for record in records {
        bytes.extend_from_slice(&record);
        bytes.push(lis_astm::CR);
    }
    bytes
}

fn abort_reason(reason: AbortReason) -> AbortCode {
    match reason {
        AbortReason::RetryLimitExceeded => AbortCode::RetryLimitExceeded,
        AbortReason::ReceiveTimeout => AbortCode::ReceiveTimeout,
        AbortReason::IncompleteRecord => AbortCode::IncompleteRecord,
        AbortReason::EmptyTransfer => AbortCode::EmptyTransfer,
        AbortReason::MessageTooLarge => AbortCode::MessageTooLarge,
        AbortReason::InputBufferExceeded => AbortCode::InputBufferExceeded,
        AbortReason::EmptyFrame => AbortCode::EmptyFrame,
        AbortReason::FrameLimitExceeded => AbortCode::FrameLimitExceeded,
        AbortReason::WireBytesExceeded => AbortCode::WireBytesExceeded,
    }
}
