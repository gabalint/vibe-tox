//! One durable-receipt writer shared by the ASTM and AU stream sessions.
use crate::commands::{abort_receipt, begin_receipt, complete_receipt, persist_frame};
use crate::{AbortCode, InstrumentTransport, ListenerErrorCode, ReceiptId};
use lis_core::{CorrelationId, InstrumentId, Principal};
use lis_store::Database;
use std::net::SocketAddr;

pub(crate) struct ReceiptWriter {
    instrument_id: InstrumentId,
    transport: InstrumentTransport,
    peer: Option<SocketAddr>,
    active: Option<ActiveReceipt>,
}

struct ActiveReceipt {
    id: ReceiptId,
    next_ordinal: u32,
}

impl ReceiptWriter {
    pub(crate) fn new(
        instrument_id: InstrumentId,
        transport: InstrumentTransport,
        peer: Option<SocketAddr>,
    ) -> Self {
        Self {
            instrument_id,
            transport,
            peer,
            active: None,
        }
    }
    pub(crate) fn is_active(&self) -> bool {
        self.active.is_some()
    }
    pub(crate) fn begin(
        &mut self,
        database: &Database,
        expected_profile: Option<&lis_au::AuConfig>,
    ) -> Result<(), ListenerErrorCode> {
        let instrument_id = self.instrument_id;
        let transport = self.transport;
        let peer = self.peer;
        let expected_profile = expected_profile.cloned();
        let receipt = database
            .write_blocking(
                Principal::instrument(instrument_id),
                CorrelationId::generate(),
                move |ctx| {
                    let instrument = crate::commands::required_instrument(ctx, instrument_id)?;
                    if instrument.connection.au_profile() != expected_profile.as_ref() {
                        return Err(crate::InterfaceError::TransportMismatch);
                    }
                    begin_receipt(ctx, instrument_id, transport, peer)
                },
            )
            .map_err(|_| ListenerErrorCode::PersistenceFailed)?;
        self.active = Some(ActiveReceipt {
            id: receipt.id,
            next_ordinal: 0,
        });
        Ok(())
    }

    pub(crate) fn persist(
        &mut self,
        database: &Database,
        bytes: Vec<u8>,
    ) -> Result<(), ListenerErrorCode> {
        let active = self
            .active
            .as_mut()
            .ok_or(ListenerErrorCode::ProtocolFailed)?;
        let receipt_id = active.id;
        let ordinal = active.next_ordinal;
        database
            .write_blocking(
                Principal::instrument(self.instrument_id),
                CorrelationId::generate(),
                move |ctx| persist_frame(ctx, receipt_id, ordinal, bytes),
            )
            .map_err(|_| ListenerErrorCode::PersistenceFailed)?;
        active.next_ordinal = active
            .next_ordinal
            .checked_add(1)
            .ok_or(ListenerErrorCode::ProtocolFailed)?;
        Ok(())
    }

    pub(crate) fn complete(
        &mut self,
        database: &Database,
        bytes: &[u8],
    ) -> Result<(), ListenerErrorCode> {
        let receipt_id = self
            .active
            .as_ref()
            .ok_or(ListenerErrorCode::ProtocolFailed)?
            .id;
        let artifact = database
            .artifacts()
            .put(bytes)
            .map_err(|_| ListenerErrorCode::PersistenceFailed)?;
        let instrument_id = self.instrument_id;
        database
            .write_blocking(
                Principal::instrument(instrument_id),
                CorrelationId::generate(),
                move |ctx| complete_receipt(ctx, receipt_id, artifact),
            )
            .map_err(|_| ListenerErrorCode::PersistenceFailed)?;
        self.active = None;
        Ok(())
    }

    pub(crate) fn abort(
        &mut self,
        database: &Database,
        reason: AbortCode,
    ) -> Result<(), ListenerErrorCode> {
        let Some(receipt_id) = self.active.as_ref().map(|active| active.id) else {
            return Ok(());
        };
        let instrument_id = self.instrument_id;
        database
            .write_blocking(
                Principal::instrument(instrument_id),
                CorrelationId::generate(),
                move |ctx| abort_receipt(ctx, receipt_id, reason),
            )
            .map_err(|_| ListenerErrorCode::PersistenceFailed)?;
        self.active = None;
        Ok(())
    }
}
