//! Connections to an MLLP listener from an address that is not allowed (ticket #4555 step
//! 4b; REQ-INT-006, NFR-08): closed at once, counted in memory, and audited as summaries at
//! most once a minute per address, so a flood of connections never becomes a flood of
//! audit events.

use lis_audit::{AuditDetail, AuditEvent, Outcome, Stream};
use lis_core::Capability;
use lis_store::{CommandContext, StoreError};

use super::error::PartnerError;
use super::model::PartnerConnectionId;
use crate::PeerRejections;

/// Audits the connections to the listener of `connection_id` refused from one address, or
/// from every untracked address.
pub fn record_rejected_partners(
    ctx: &mut CommandContext<'_>,
    connection_id: PartnerConnectionId,
    rejections: PeerRejections,
) -> Result<(), PartnerError> {
    ctx.require(Capability::EnterOrders)?;
    let event = AuditEvent::new(
        Stream::Security,
        "interfaces.reject_partner_connection",
        Outcome::Denied,
    )
    .record("partner_connection", connection_id)
    .reason("partner_not_allowed")
    .detail(
        "rejected_connections",
        AuditDetail::count(rejections.rejected_connections),
    );
    let event = match rejections.peer {
        Some(peer) => event.detail(
            "peer_ip",
            AuditDetail::machine_token(&peer.to_string()).map_err(StoreError::from)?,
        ),
        None => event.detail("peer_scope", AuditDetail::code("untracked")),
    };
    ctx.audit(event)?;
    Ok(())
}
