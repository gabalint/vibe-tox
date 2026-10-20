//! AU receive-side application exchanges. ACK means durable receipt only.

use std::io::Write;
use std::time::{Duration, Instant};

use lis_au::{
    AssemblyOutcome, BlockId, Frame, Message, ResultAssembler, ResultBlock, ResultKind,
    SampleHeader, TransmissionClass,
};
use lis_core::Phi;

use crate::listener::set_status;
use crate::receipt_writer::ReceiptWriter;
use crate::session::StreamContext;
use crate::{
    AbortCode, AuConnectionSettings, ListenerDetailCode, ListenerErrorCode, ListenerState,
};

mod stream;
pub(crate) use stream::run_stream;

// AU400/AU640 manual pp20–22: the analyzer cannot receive a response before t3.
const RESPONSE_TURNAROUND: Duration = Duration::from_millis(500);

pub(super) struct AuSession {
    writer: ReceiptWriter,
    settings: AuConnectionSettings,
    assembler: ResultAssembler,
    wire: Phi<Vec<u8>>,
    header: Option<(ResultKind, SampleHeader)>,
    blocked: Option<(ResultKind, SampleHeader)>,
    last_acknowledged: Option<Phi<Vec<u8>>>,
    completed_multiblock: bool,
    response: Option<(Instant, u8)>,
    block_deadline: Option<Instant>,
    failures: u8,
    established: bool,
}

impl AuSession {
    pub(super) fn new(context: &StreamContext<'_>, settings: &AuConnectionSettings) -> Self {
        Self {
            writer: ReceiptWriter::new(context.instrument_id, context.transport, context.peer),
            settings: settings.clone(),
            assembler: ResultAssembler::new(),
            wire: Phi::new(Vec::new()),
            header: None,
            blocked: None,
            last_acknowledged: None,
            completed_multiblock: false,
            response: None,
            block_deadline: None,
            failures: 0,
            established: false,
        }
    }

    pub(super) fn receive(
        &mut self,
        frame: Frame,
        now: Instant,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        self.established = true;
        // A repeated final block of a multiblock sample is not a new complete
        // sample. Single-block repeats get their own duplicate receipt evidence.
        if (self.assembler.is_pending() || self.completed_multiblock)
            && self.last_acknowledged.as_ref() == Some(&frame.wire)
        {
            self.acknowledge(now);
            return Ok(());
        }
        let parsed = lis_au::parse_message(&self.settings.protocol, &frame);
        match parsed {
            Ok(Message::ResultBlock(block)) => self.result(frame, block, now, context),
            Ok(Message::Query(_)) => self.control(
                frame,
                now,
                context,
                Some(ListenerDetailCode::AuQueryUnsupported),
            ),
            Ok(Message::Session { .. }) => self.control(frame, now, context, None),
            Ok(Message::TriBlock(_)) => self.reject(now, context),
            Err(_) => {
                // Retain well-framed but unsupported/malformed application text;
                // semantic import records a failed attempt without losing bytes.
                self.abort(context, AbortCode::ProtocolError)?;
                self.persist(&frame, context)?;
                self.finish(context)?;
                self.last_acknowledged = Some(frame.wire);
                self.acknowledge(now);
                set_status(
                    context.statuses,
                    context.instrument_id,
                    ListenerState::Connected,
                    Some(ListenerDetailCode::ProtocolFailed),
                );
                Ok(())
            }
        }
    }

    fn result(
        &mut self,
        frame: Frame,
        block: ResultBlock,
        now: Instant,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        let identity = (block.kind, block.header.clone());
        let needs_restart = if (self.assembler.is_pending()
            && self.header.as_ref() == Some(&identity))
            || block.block == BlockId::Intermediate(0)
        {
            false
        } else {
            let instrument = context.instrument_id;
            let header = block.header.clone();
            let kind = block.kind;
            context
                .database
                .read_blocking(move |conn| {
                    crate::repo::requires_au_restart(conn, instrument, kind, &header)
                })
                .map_err(|_| ListenerErrorCode::PersistenceFailed)?
        };
        if (needs_restart || self.blocked.as_ref() == Some(&identity))
            && block.block != BlockId::Intermediate(0)
        {
            return self.reject(now, context);
        }
        if self
            .header
            .as_ref()
            .is_some_and(|header| *header != identity)
        {
            self.abort(context, AbortCode::IncompleteRecord)?;
        }
        let was_pending = self.assembler.is_pending();
        let outcome = self.assembler.push(block);
        if matches!(outcome, Ok(AssemblyOutcome::Duplicate)) {
            self.acknowledge(now);
            return Ok(());
        }
        self.persist(&frame, context)?;
        self.header = Some(identity);
        match outcome {
            Ok(AssemblyOutcome::Pending) => {
                self.blocked = None;
                // Class A starts T1 now; class B restarts it when ACK is sent.
                self.block_deadline =
                    Some(now + Duration::from_millis(u64::from(self.settings.block_timeout_ms)));
                set_status(
                    context.statuses,
                    context.instrument_id,
                    ListenerState::Receiving,
                    None,
                );
            }
            Ok(AssemblyOutcome::Complete(_)) => {
                self.finish(context)?;
                self.completed_multiblock = was_pending;
                self.blocked = None;
                set_status(
                    context.statuses,
                    context.instrument_id,
                    ListenerState::Connected,
                    None,
                );
            }
            Ok(AssemblyOutcome::Duplicate) => return Err(ListenerErrorCode::ProtocolFailed),
            Err(_) => return self.reject(now, context),
        }
        self.last_acknowledged = Some(frame.wire);
        self.failures = 0;
        self.acknowledge(now);
        Ok(())
    }

    fn control(
        &mut self,
        frame: Frame,
        now: Instant,
        context: &StreamContext<'_>,
        detail: Option<ListenerDetailCode>,
    ) -> Result<(), ListenerErrorCode> {
        let interrupted = self.writer.is_active();
        self.abort(context, AbortCode::IncompleteRecord)?;
        self.persist(&frame, context)?;
        self.finish(context)?;
        self.last_acknowledged = Some(frame.wire);
        self.acknowledge(now);
        set_status(
            context.statuses,
            context.instrument_id,
            ListenerState::Connected,
            detail.or(interrupted.then_some(ListenerDetailCode::ProtocolFailed)),
        );
        Ok(())
    }

    fn persist(
        &mut self,
        frame: &Frame,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        if !self.writer.is_active() {
            self.writer
                .begin(context.database, Some(&self.settings.protocol))?;
            self.wire = Phi::new(Vec::new());
        }
        self.writer
            .persist(context.database, frame.wire.expose().clone())?;
        let mut wire = std::mem::replace(&mut self.wire, Phi::new(Vec::new())).into_inner();
        wire.extend_from_slice(frame.wire.expose());
        self.wire = Phi::new(wire);
        Ok(())
    }

    fn finish(&mut self, context: &StreamContext<'_>) -> Result<(), ListenerErrorCode> {
        self.writer.complete(context.database, self.wire.expose())?;
        self.wire = Phi::new(Vec::new());
        self.header = None;
        self.block_deadline = None;
        Ok(())
    }

    pub(super) fn abort(
        &mut self,
        context: &StreamContext<'_>,
        reason: AbortCode,
    ) -> Result<(), ListenerErrorCode> {
        self.writer.abort(context.database, reason)?;
        if let Some(header) = self.header.take() {
            self.blocked = Some(header);
        }
        self.assembler.reset();
        self.wire = Phi::new(Vec::new());
        self.block_deadline = None;
        self.last_acknowledged = None;
        self.completed_multiblock = false;
        Ok(())
    }

    pub(super) fn reject(
        &mut self,
        now: Instant,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        self.abort(context, AbortCode::ProtocolError)?;
        self.failures += 1;
        set_status(
            context.statuses,
            context.instrument_id,
            ListenerState::Connected,
            Some(ListenerDetailCode::ProtocolFailed),
        );
        if self.failures > 4 {
            return Err(ListenerErrorCode::ProtocolFailed);
        }
        self.respond(now, 0x15);
        Ok(())
    }

    /// A class-B transport retry must retain the already acknowledged prefix.
    pub(super) fn retry_wire(
        &mut self,
        now: Instant,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        if self.settings.protocol.framing.class == TransmissionClass::A {
            return self.reject(now, context);
        }
        self.failures += 1;
        if self.failures > 4 {
            self.abort(context, AbortCode::RetryLimitExceeded)?;
            return Err(ListenerErrorCode::ProtocolFailed);
        }
        set_status(
            context.statuses,
            context.instrument_id,
            ListenerState::Receiving,
            Some(ListenerDetailCode::ProtocolFailed),
        );
        self.respond(now, 0x15);
        self.block_deadline = Some(
            now + RESPONSE_TURNAROUND
                + Duration::from_millis(u64::from(self.settings.block_timeout_ms)),
        );
        Ok(())
    }

    fn acknowledge(&mut self, now: Instant) {
        self.respond(now, 0x06);
    }

    fn respond(&mut self, now: Instant, byte: u8) {
        if self.settings.protocol.framing.class == TransmissionClass::B {
            self.response = Some((now + RESPONSE_TURNAROUND, byte));
        }
    }

    pub(super) fn tick<T: Write + ?Sized>(
        &mut self,
        now: Instant,
        stream: &mut T,
        context: &StreamContext<'_>,
    ) -> Result<(), ListenerErrorCode> {
        if let Some((due, byte)) = self.response
            && now >= due
        {
            stream
                .write_all(&[byte])
                .and_then(|()| stream.flush())
                .map_err(|_| ListenerErrorCode::TransportFailed)?;
            self.response = None;
            if self.assembler.is_pending() {
                self.block_deadline =
                    Some(now + Duration::from_millis(u64::from(self.settings.block_timeout_ms)));
            }
        }
        if self.block_deadline.is_some_and(|deadline| now >= deadline) && self.response.is_none() {
            self.abort(context, AbortCode::ReceiveTimeout)?;
            set_status(
                context.statuses,
                context.instrument_id,
                ListenerState::Connected,
                Some(ListenerDetailCode::ProtocolFailed),
            );
        }
        Ok(())
    }
}

#[cfg(test)]
pub(crate) mod tests;
