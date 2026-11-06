//! One listener (REQ-INT-002, REQ-INT-003, REQ-INT-006, NFR-08). It serves its allowed
//! partners only: a connection from any other address is closed at once and counted, and
//! the counts are audited at most once a minute per address. At most [`CONNECTIONS_MAX`]
//! partners are served at once. Each framed message is stored, recorded with what it became
//! (as a read file is), and only then answered with its HL7 acknowledgement; a message that
//! cannot be saved gets none, so the partner sends it again. A connection idle longer than
//! the listener's idle timeout, sending anything but MLLP frames, or a message larger than
//! [`MESSAGE_MAX`] is closed. Logs carry IDs and codes, never addresses or content.

use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use jiff::tz::TimeZone;
use lis_core::{Capability, CorrelationId, Principal};
use lis_interfaces::partner::mllp::{FrameProblem, frame, unframe};
use lis_interfaces::partner::mllp_answer::acknowledge;
use lis_interfaces::partner::{IntakeSource, MllpListener};
use lis_interfaces::{PeerRejections, RejectionTally};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, watch};
use tokio::time::timeout;

use crate::ops_worker::Operations;
use crate::partner_delivery::hl7_message::dtm;
use crate::partner_delivery::lab_zone;
use crate::partner_intake::{ReadFile, receive};

/// The largest message taken, in bytes.
const MESSAGE_MAX: usize = 1024 * 1024;
/// Partners served at once by one listener; a further connection is closed.
const CONNECTIONS_MAX: usize = 4;
/// How often due rejection counts are audited while nobody connects.
const AUDIT_TICK: Duration = Duration::from_secs(10);
/// The pause after a failed accept, such as when the computer runs out of sockets.
const ACCEPT_PAUSE: Duration = Duration::from_secs(1);
/// The listener's name in audit events and logs.
const COMPONENT: &str = "partner-listener";

/// What every connection of one listener shares.
struct Served {
    ops: Arc<Operations>,
    source: IntakeSource,
    listener: MllpListener,
}

/// Accepts connections until the listener stops, then closes its port and audits the
/// rejections not audited yet.
pub(super) async fn listen(
    socket: TcpListener,
    ops: Arc<Operations>,
    source: IntakeSource,
    mut stopped: watch::Receiver<bool>,
) {
    let Some(listener) = source.destination.mllp_listener.clone() else {
        return;
    };
    let served = Arc::new(Served {
        ops,
        source,
        listener,
    });
    let slots = Arc::new(Semaphore::new(CONNECTIONS_MAX));
    let mut rejections = RejectionTally::default();
    let mut tick = tokio::time::interval(AUDIT_TICK);
    loop {
        tokio::select! {
            _ = stopped.changed() => break,
            accepted = socket.accept() => match accepted {
                Ok((stream, peer)) if served.listener.allows(peer.ip()) => {
                    admit(&served, &slots, &stopped, stream, peer.ip());
                }
                Ok((stream, peer)) => {
                    drop(stream);
                    rejections.record(peer.ip());
                }
                Err(_) => tokio::time::sleep(ACCEPT_PAUSE).await,
            },
            _ = tick.tick() => {}
        }
        audit(&served, rejections.take_due(Instant::now())).await;
    }
    drop(socket);
    audit(&served, rejections.take_remaining()).await;
}

/// Serves an allowed partner in its own task, or closes its connection when every slot is
/// taken.
fn admit(
    served: &Arc<Served>,
    slots: &Arc<Semaphore>,
    stopped: &watch::Receiver<bool>,
    stream: TcpStream,
    peer: IpAddr,
) {
    let Ok(slot) = Arc::clone(slots).try_acquire_owned() else {
        let connection_id = served.source.connection_id;
        tracing::warn!(%connection_id, error_code = "too_many_connections", "partner connection closed");
        return;
    };
    tokio::spawn(serve(
        stream,
        peer,
        Arc::clone(served),
        stopped.clone(),
        slot,
    ));
}

/// Answers each message on the connection in turn until it closes.
async fn serve(
    mut stream: TcpStream,
    peer: IpAddr,
    served: Arc<Served>,
    mut stopped: watch::Receiver<bool>,
    _slot: OwnedSemaphorePermit,
) {
    let mut buffer = Vec::new();
    while let Some(message) = next_message(&mut stream, &mut buffer, &served, &mut stopped).await {
        let answering = Arc::clone(&served);
        let answered = tokio::task::spawn_blocking(move || answering.answer(message, peer)).await;
        let Ok(Some(ack)) = answered else {
            return;
        };
        if stream.write_all(&frame(&ack)).await.is_err() {
            return;
        }
    }
}

/// The next whole message on the connection; `None` to close it: the partner closed it or
/// stayed idle, it sent anything but MLLP frames or too large a message, or the listener
/// stops.
async fn next_message(
    stream: &mut TcpStream,
    buffer: &mut Vec<u8>,
    served: &Served,
    stopped: &mut watch::Receiver<bool>,
) -> Option<Vec<u8>> {
    let idle = Duration::from_secs(u64::from(served.listener.idle_timeout_seconds));
    let mut chunk = [0_u8; 8192];
    loop {
        match unframe(buffer, MESSAGE_MAX) {
            Ok(Some((message, used))) => {
                buffer.drain(..used);
                return Some(message);
            }
            Ok(None) => {}
            Err(problem) => {
                let error_code = match problem {
                    FrameProblem::NotFramed => "not_mllp",
                    FrameProblem::TooLarge => "message_too_large",
                };
                let connection_id = served.source.connection_id;
                tracing::warn!(%connection_id, error_code, "partner connection closed");
                return None;
            }
        }
        let read = tokio::select! {
            _ = stopped.changed() => return None,
            read = timeout(idle, stream.read(&mut chunk)) => read,
        };
        match read {
            Ok(Ok(read)) if read > 0 => buffer.extend_from_slice(&chunk[..read]),
            _ => return None,
        }
    }
}

impl Served {
    /// Stores the message, records what it became, and returns its acknowledgement; `None`
    /// when it could not be saved. Blocking.
    fn answer(&self, message: Vec<u8>, peer: IpAddr) -> Option<Vec<u8>> {
        let db = &self.ops.db;
        let connection_id = self.source.connection_id;
        let sha256 = match db.artifacts().put(&message) {
            Ok(stored) => stored.digest.to_string(),
            Err(error) => {
                tracing::warn!(%connection_id, %error, "partner message not stored");
                return None;
            }
        };
        let file = ReadFile {
            name: String::new(),
            sha256,
            bytes: message.clone(),
            peer: Some(peer.to_string()),
        };
        let (id, _) = receive(db, &self.source, file)?;
        let acknowledged =
            db.read_blocking(move |conn| lis_interfaces::partner::received_ack(conn, id));
        let Ok(Some((code, reason))) = acknowledged else {
            tracing::error!(received_id = %id, "partner acknowledgement not read");
            return None;
        };
        let zone = lab_zone(db).unwrap_or(TimeZone::UTC);
        let at = dtm(self.ops.clock.now(), &zone);
        Some(acknowledge(&message, code, reason.as_deref(), &at))
    }
}

/// Audits the due rejection counts in one transaction, off the runtime.
async fn audit(served: &Arc<Served>, due: Vec<PeerRejections>) {
    if due.is_empty() {
        return;
    }
    let connection_id = served.source.connection_id;
    let writing = Arc::clone(served);
    let written = tokio::task::spawn_blocking(move || {
        writing.ops.db.write_blocking(
            Principal::system(COMPONENT, [Capability::EnterOrders]),
            CorrelationId::generate(),
            move |ctx| {
                due.into_iter().try_for_each(|rejections| {
                    lis_interfaces::partner::record_rejected_partners(
                        ctx,
                        connection_id,
                        rejections,
                    )
                })
            },
        )
    })
    .await;
    if matches!(written, Ok(Ok(()))) {
        tracing::warn!(%connection_id, error_code = "partner_not_allowed", "partner listener refused connections");
    } else {
        tracing::error!(%connection_id, "refused partner connections not audited");
    }
}
