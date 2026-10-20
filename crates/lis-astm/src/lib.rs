//! Pure ASTM E1381 (CLSI LIS1-A) framing, ASTM E1394 (CLSI LIS2-A2) records,
//! and a receive-side state machine for analyzer result intake.
//!
//! The crate performs no I/O, scheduling, storage, semantic result acceptance,
//! or sender-side contention handling. Callers feed bytes and timer events into
//! [`ReceiveDriver`], carry out its actions, and report successful durable writes
//! with [`ReceiveEvent::Persisted`]. The driver cannot emit a frame ACK before
//! that matching event (REQ-INT-009), and it safely recognizes a retransmission
//! of the last durable frame without persisting or appending it twice
//! (REQ-INT-003). Malformed frames use a bounded six-attempt receive policy
//! (REQ-INT-005).
//!
//! Record values remain text, including result quantities, units, comparators,
//! precision, and timestamps (NFR-10). The parser preserves each raw record and
//! raw field/component while exposing escape-decoded views. The supported wire
//! profile is seven-bit printable ASCII plus record-separating carriage returns,
//! 240 text bytes per frame, standard delimiter escapes, and even-length hex
//! escapes. A transfer accepts at most [`DEFAULT_MAX_FRAMES_PER_TRANSFER`] frames
//! and [`DEFAULT_MAX_WIRE_BYTES_PER_TRANSFER`] exact wire bytes, independent of
//! the application-text limit. Vendor dialects, sockets, serial ports, clocks,
//! persistence, and host-query/download state machines are outside this crate.

#![warn(clippy::wildcard_enum_match_arm)]

mod accessors;
mod driver;
mod driver_contract;
mod escape;
mod frame;
mod message;
mod record;
mod record_model;

pub use accessors::{CommentRecord, HeaderRecord, OrderRecord, PatientRecord, ResultRecord};
pub use driver::ReceiveDriver;
pub use driver_contract::{
    AbortReason, Action, ControlResponse, DEFAULT_MAX_FRAMES_PER_TRANSFER,
    DEFAULT_MAX_MESSAGE_BYTES, DEFAULT_MAX_WIRE_BYTES_PER_TRANSFER, DriverError, FrameId,
    RECEIVE_TIMEOUT, ReceiveEvent, TimerId,
};
pub use frame::{
    ACK, CR, ENQ, EOT, ETB, ETX, Frame, FrameEnd, FrameError, LF, NAK, STX, checksum, encode_frame,
    encode_records, parse_frame,
};
pub use message::{Message, MessageError, OrderNode, PatientNode, ResultNode, assemble_message};
pub use record::{parse_message_records, parse_record};
pub use record_model::{Component, Delimiters, Field, Record, RecordError, RecordType, Repeat};
