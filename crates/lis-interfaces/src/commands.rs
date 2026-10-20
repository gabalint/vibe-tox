use std::net::SocketAddr;

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::{Actor, Capability, InstrumentId};
use lis_store::{CommandContext, OutboxItem, StoreError, StoredArtifact};
use serde_json::json;
use sha2::{Digest, Sha256};

use crate::model::{
    AbortCode, ConnectionSettings, Instrument, InstrumentTransport, Receipt, ReceiptFrame,
    ReceiptId, ReceiptState,
};
use crate::peer_rejections::PeerRejections;
use crate::{InterfaceError, RECEIPT_COMPLETED_TOPIC, repo};

/// Opens a durable transfer receipt before the listener acknowledges ENQ.
pub(crate) fn begin_receipt(
    ctx: &mut CommandContext<'_>,
    instrument_id: InstrumentId,
    transport: InstrumentTransport,
    peer: Option<SocketAddr>,
) -> Result<Receipt, InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    require_matching_instrument(ctx, instrument_id)?;
    let instrument = required_instrument(ctx, instrument_id)?;
    if !instrument.enabled {
        return Err(InterfaceError::InstrumentDisabled);
    }
    if instrument.connection.transport() != transport || peer.is_some() != transport.is_tcp() {
        return Err(InterfaceError::TransportMismatch);
    }
    if let (
        ConnectionSettings::AstmTcp { allowed_peers, .. }
        | ConnectionSettings::AuTcp { allowed_peers, .. },
        Some(peer),
    ) = (&instrument.connection, peer)
        && !allowed_peers.contains(&peer.ip())
    {
        return Err(InterfaceError::UnexpectedTcpPeer);
    }
    let receipt = Receipt {
        id: ReceiptId::generate(),
        instrument_id,
        transport,
        peer: peer.map(|value| value.to_string()),
        started_at: ctx.now(),
        completed_at: None,
        state: ReceiptState::Receiving,
        abort_reason: None,
        frame_count: 0,
        message_sha256: None,
        artifact_digest: None,
        duplicate_of: None,
    };
    repo::insert_receipt(ctx.tx(), &receipt)?;
    if let Some(profile) = instrument.connection.au_profile() {
        repo::insert_au_profile(ctx.tx(), receipt.id, profile)?;
    }
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interface.begin_receipt",
            Outcome::Succeeded,
        )
        .record("interface_receipt", receipt.id)
        .detail("instrument_id", AuditDetail::id(instrument_id))
        .detail("transport", AuditDetail::code(transport.as_str())),
    )?;
    Ok(receipt)
}

/// Commits one exact accepted frame before the driver may emit its ACK (REQ-INT-009).
pub(crate) fn persist_frame(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
    ordinal: u32,
    bytes: Vec<u8>,
) -> Result<(), InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    let receipt = required_receipt(ctx, receipt_id)?;
    require_matching_instrument(ctx, receipt.instrument_id)?;
    if receipt.transport.is_au() {
        let profile = repo::au_profile(ctx.tx(), receipt_id)?
            .ok_or(InterfaceError::InvalidAuConfiguration)?;
        let frame = crate::au_evidence::frame(&profile, &bytes)?;
        if let Ok(lis_au::Message::ResultBlock(block)) = lis_au::parse_message(&profile, &frame) {
            repo::guard_au_multiblock(ctx.tx(), &receipt, ordinal, &block)?;
        }
    } else if lis_astm::parse_frame(&bytes).is_err() {
        return Err(InterfaceError::InvalidFrame);
    }
    if receipt.state != ReceiptState::Receiving {
        return Err(InterfaceError::InvalidReceiptState);
    }
    if receipt.frame_count != ordinal {
        return Err(InterfaceError::FrameOutOfSequence);
    }
    repo::insert_frame(
        ctx.tx(),
        &ReceiptFrame {
            receipt_id,
            ordinal,
            bytes,
        },
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interface.persist_frame",
            Outcome::Succeeded,
        )
        .record("interface_receipt", receipt_id)
        .detail("frame_ordinal", AuditDetail::unsigned(u64::from(ordinal))),
    )?;
    Ok(())
}

/// Marks a receipt complete after its assembled artifact has been synced.
pub(crate) fn complete_receipt(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
    artifact: StoredArtifact,
) -> Result<Receipt, InterfaceError> {
    complete_receipt_from(ctx, receipt_id, artifact, CompletionOrigin::LiveInstrument)
}

fn complete_receipt_from(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
    artifact: StoredArtifact,
    origin: CompletionOrigin,
) -> Result<Receipt, InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    let receipt = required_receipt(ctx, receipt_id)?;
    if origin == CompletionOrigin::LiveInstrument {
        require_matching_instrument(ctx, receipt.instrument_id)?;
    }
    ensure_receiving_with_frames(&receipt)?;
    let digest = artifact.digest.to_string();
    let (frame_digest, frame_size) = digest_frames(ctx, receipt_id)?;
    if digest != frame_digest || artifact.size != frame_size {
        return Err(InterfaceError::ArtifactMismatch);
    }
    let duplicate_of = if receipt.transport.is_au() {
        repo::first_au_duplicate(ctx.tx(), &receipt, &digest)?
    } else {
        repo::first_duplicate(ctx.tx(), receipt.instrument_id, &digest)?
    };
    repo::complete_receipt(ctx.tx(), receipt_id, ctx.now(), &digest, duplicate_of)?;
    let message_digest = AuditDetail::sha256(&digest).map_err(StoreError::from)?;
    let mut audit = AuditEvent::new(
        Stream::Clinical,
        "interface.complete_receipt",
        Outcome::Succeeded,
    )
    .record("interface_receipt", receipt_id)
    .detail("instrument_id", AuditDetail::id(receipt.instrument_id))
    .detail(
        "frame_count",
        AuditDetail::unsigned(u64::from(receipt.frame_count)),
    )
    .detail("message_sha256", message_digest)
    .detail("duplicate", AuditDetail::boolean(duplicate_of.is_some()));
    if origin == CompletionOrigin::StartupRecovery {
        audit = audit.detail("recovered_at_startup", AuditDetail::boolean(true));
    }
    ctx.audit(audit)?;
    if duplicate_of.is_none() && crate::au_evidence::needs_import(ctx.tx(), &receipt)? {
        enqueue_import(ctx, receipt_id)?;
    }
    Ok(Receipt {
        completed_at: Some(ctx.now()),
        state: ReceiptState::Complete,
        message_sha256: Some(digest.clone()),
        artifact_digest: Some(digest),
        duplicate_of,
        ..receipt
    })
}

pub(crate) fn complete_recovered_receipt(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
    artifact: StoredArtifact,
) -> Result<Receipt, InterfaceError> {
    complete_receipt_from(ctx, receipt_id, artifact, CompletionOrigin::StartupRecovery)
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum CompletionOrigin {
    LiveInstrument,
    StartupRecovery,
}

/// Terminates an incomplete receipt without creating a partial message artifact.
pub(crate) fn abort_receipt(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
    reason: AbortCode,
) -> Result<Receipt, InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    let receipt = required_receipt(ctx, receipt_id)?;
    require_matching_instrument(ctx, receipt.instrument_id)?;
    if receipt.state != ReceiptState::Receiving {
        return Err(InterfaceError::InvalidReceiptState);
    }
    repo::terminate_receipt(
        ctx.tx(),
        receipt_id,
        ReceiptState::Aborted,
        ctx.now(),
        reason,
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interface.abort_receipt",
            Outcome::Succeeded,
        )
        .record("interface_receipt", receipt_id)
        .detail("instrument_id", AuditDetail::id(receipt.instrument_id))
        .detail("reason_code", AuditDetail::code(reason.as_str())),
    )?;
    Ok(Receipt {
        completed_at: Some(ctx.now()),
        state: ReceiptState::Aborted,
        abort_reason: Some(reason),
        ..receipt
    })
}

/// Audits one aggregated count of rejected connections (REQ-INT-009, NFR-08).
/// A summary without a peer covers every peer beyond the tracked-peer cap.
pub(crate) fn audit_rejected_peer(
    ctx: &mut CommandContext<'_>,
    instrument_id: InstrumentId,
    rejections: PeerRejections,
) -> Result<(), InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    required_instrument(ctx, instrument_id)?;
    let event = AuditEvent::new(Stream::Security, "interface.reject_peer", Outcome::Denied)
        .record("instrument", instrument_id)
        .reason("unexpected_peer")
        .detail(
            "rejected_connections",
            AuditDetail::count(rejections.rejected_connections),
        );
    let event = match rejections.peer {
        Some(peer_ip) => event.detail(
            "peer_ip",
            AuditDetail::machine_token(&peer_ip.to_string()).map_err(StoreError::from)?,
        ),
        None => event.detail("peer_scope", AuditDetail::code("untracked")),
    };
    ctx.audit(event)?;
    Ok(())
}

pub(crate) fn interrupt_receipt(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
) -> Result<(), InterfaceError> {
    ctx.require(Capability::InstrumentIntake)?;
    let receipt = required_receipt(ctx, receipt_id)?;
    if receipt.state != ReceiptState::Receiving {
        return Err(InterfaceError::InvalidReceiptState);
    }
    repo::terminate_receipt(
        ctx.tx(),
        receipt_id,
        ReceiptState::Interrupted,
        ctx.now(),
        AbortCode::StartupInterrupted,
    )?;
    ctx.audit(
        AuditEvent::new(
            Stream::Clinical,
            "interface.interrupt_receipt",
            Outcome::Succeeded,
        )
        .record("interface_receipt", receipt_id)
        .detail(
            "reason_code",
            AuditDetail::code(AbortCode::StartupInterrupted.as_str()),
        ),
    )?;
    Ok(())
}

/// Queues the import of a complete receipt that is not a duplicate (REQ-INT-009:
/// semantic processing only after the durable receipt commits).
pub(crate) fn enqueue_import(
    ctx: &mut CommandContext<'_>,
    receipt_id: ReceiptId,
) -> Result<(), InterfaceError> {
    ctx.enqueue(OutboxItem {
        topic: RECEIPT_COMPLETED_TOPIC,
        dedup_key: receipt_id.to_string(),
        payload: json!({ "receipt_id": receipt_id }),
    })?;
    Ok(())
}

pub(crate) fn required_instrument(
    ctx: &CommandContext<'_>,
    instrument_id: InstrumentId,
) -> Result<Instrument, InterfaceError> {
    repo::instrument_by_id(ctx.tx(), instrument_id)?.ok_or(InterfaceError::InstrumentNotFound)
}

fn required_receipt(
    ctx: &CommandContext<'_>,
    receipt_id: ReceiptId,
) -> Result<Receipt, InterfaceError> {
    repo::receipt_by_id(ctx.tx(), receipt_id)?.ok_or(InterfaceError::ReceiptNotFound)
}

fn ensure_receiving_with_frames(receipt: &Receipt) -> Result<(), InterfaceError> {
    if receipt.state != ReceiptState::Receiving {
        return Err(InterfaceError::InvalidReceiptState);
    }
    if receipt.frame_count == 0 {
        return Err(InterfaceError::ReceiptHasNoFrames);
    }
    Ok(())
}

fn digest_frames(
    ctx: &CommandContext<'_>,
    receipt_id: ReceiptId,
) -> Result<(String, u64), InterfaceError> {
    let frames = repo::frames_for_receipt(ctx.tx(), receipt_id)?;
    let receipt = required_receipt(ctx, receipt_id)?;
    let mut hasher = Sha256::new();
    let mut size = 0_u64;
    for stored in frames {
        let bytes = if receipt.transport.is_au() {
            stored.bytes
        } else {
            lis_astm::parse_frame(&stored.bytes)
                .map_err(|_| InterfaceError::InvalidFrame)?
                .text()
                .to_vec()
        };
        hasher.update(&bytes);
        size = size
            .checked_add(u64::try_from(bytes.len()).map_err(|_| InterfaceError::ArtifactMismatch)?)
            .ok_or(InterfaceError::ArtifactMismatch)?;
    }
    Ok((hex::encode(hasher.finalize()), size))
}

/// Only the instrument's own principal may act for it (REQ-INT-009).
pub(crate) fn require_matching_instrument(
    ctx: &CommandContext<'_>,
    instrument_id: InstrumentId,
) -> Result<(), InterfaceError> {
    match ctx.principal().actor() {
        Actor::Instrument {
            instrument_id: principal_id,
        } if *principal_id == instrument_id => Ok(()),
        Actor::Instrument { .. } | Actor::User { .. } | Actor::System { .. } => {
            Err(InterfaceError::Store(StoreError::from(lis_core::Denied {
                capability: Capability::InstrumentIntake,
            })))
        }
    }
}
