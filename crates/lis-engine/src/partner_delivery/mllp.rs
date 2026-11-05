//! Sending HL7 over MLLP to a partner's interface on the lab network (ticket #4555 step 4;
//! REQ-INT-002, REQ-INT-006). The LIS connects, sends one framed message, reads the
//! partner's acknowledgement on the same connection and closes it. The address is checked
//! again before each connection, so a changed record never sends to the internet. A
//! message without an acknowledgement in time is sent again later with the same control
//! ID. Codes are PHI-free; nothing here logs an address or content.

use std::time::Duration;

use lis_interfaces::partner::mllp::{
    AckProblem, Acknowledgement, NO_ACK, control_id, frame, read_acknowledgement, unframe,
};
use lis_interfaces::partner::{MllpPartner, valid_mllp_partner};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;

/// How long connecting may take.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The longest acknowledgement read.
const ACK_MAX: usize = 64 * 1024;

/// Why a message got no acknowledgement; the codes show in Logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MllpProblem {
    /// The address is not on the lab network.
    NotLabNetwork,
    /// Nothing answered at the address and port in time, or the connection was refused.
    Unreachable,
    /// The connection closed or broke before the acknowledgement.
    ConnectionLost,
    /// No acknowledgement within the connection's timeout.
    NoAck,
    /// The answer was not the acknowledgement of the message.
    Ack(AckProblem),
    /// The message has no control ID (MSH-10) to acknowledge.
    NoControlId,
}

impl MllpProblem {
    /// The stored and displayed code.
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::NotLabNetwork => "not_lab_network",
            Self::Unreachable => "partner_unreachable",
            Self::ConnectionLost => "connection_lost",
            Self::NoAck => NO_ACK,
            Self::Ack(problem) => problem.code(),
            Self::NoControlId => "no_control_id",
        }
    }
}

/// Sends `message` and returns the partner's acknowledgement of it.
pub(crate) async fn send(
    partner: &MllpPartner,
    message: &[u8],
) -> Result<Acknowledgement, MllpProblem> {
    let control_id = control_id(message).ok_or(MllpProblem::NoControlId)?;
    let mut stream = connect(partner).await?;
    stream
        .write_all(&frame(message))
        .await
        .map_err(|_| MllpProblem::ConnectionLost)?;
    let wait = Duration::from_secs(u64::from(partner.ack_timeout_seconds));
    let answer = timeout(wait, read_frame(&mut stream))
        .await
        .map_err(|_| MllpProblem::NoAck)??;
    read_acknowledgement(&answer, &control_id).map_err(MllpProblem::Ack)
}

/// Whether the partner's interface takes a connection; sends nothing.
pub(crate) async fn probe(partner: &MllpPartner) -> Result<(), MllpProblem> {
    connect(partner).await.map(drop)
}

async fn connect(partner: &MllpPartner) -> Result<TcpStream, MllpProblem> {
    let socket = valid_mllp_partner(partner.clone())
        .ok()
        .and_then(|partner| partner.socket())
        .ok_or(MllpProblem::NotLabNetwork)?;
    match timeout(CONNECT_TIMEOUT, TcpStream::connect(socket)).await {
        Ok(Ok(stream)) => Ok(stream),
        Ok(Err(_)) | Err(_) => Err(MllpProblem::Unreachable),
    }
}

/// The first framed message the partner sends back.
async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, MllpProblem> {
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 4096];
    loop {
        match unframe(&buffer, ACK_MAX) {
            Ok(Some((message, _))) => return Ok(message),
            Ok(None) => {}
            Err(_) => return Err(MllpProblem::Ack(AckProblem::Unreadable)),
        }
        let read = stream
            .read(&mut chunk)
            .await
            .map_err(|_| MllpProblem::ConnectionLost)?;
        if read == 0 {
            return Err(MllpProblem::ConnectionLost);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

#[cfg(test)]
#[path = "mllp_tests.rs"]
mod tests;
