//! Pure AU400/AU640 online codec based on the supplied Olympus specification,
//! issued 1999-02-03. The protocol is proprietary fixed-width text, not ASTM.
//! No I/O, persistence, acknowledgements, clinical interpretation or inferred
//! orders occur here. A caller must persist exact wire receipts before sending
//! an ACK (REQ-INT-008/009). Only explicit caller-provided TRI is encoded.
//!
//! The initial character profile is printable ASCII. The manual contradicts
//! itself about extended character and zero-width sample-ID settings; those
//! profiles require separately validated support. Raw fields retain padding,
//! values and flags without inventing units, cutoffs or decimal interpretations.
//! Identical final frames cannot be distinguished from legitimate repeated
//! samples without receipt/session context; completed-sample deduplication is
//! deliberately the caller's responsibility.
#![warn(clippy::wildcard_enum_match_arm)]
mod assemble;
mod config;
mod encode;
mod error;
mod fields;
mod frame;
mod model;
mod parse;

pub use assemble::{AssemblyOutcome, ResultAssembler};
pub use config::{AuConfig, FormatConfig, FramingConfig, TransmissionClass};
pub use encode::{Tri, TriTest, encode_stop, encode_tri};
pub use error::AuError;
pub use frame::{Frame, FrameEnd, StreamDecoder, WireEvent, encode_frame};
pub use model::{
    BlockId, Demographics, Message, Query, ResultBlock, ResultKind, ResultTest, SampleHeader,
    SampleResults, SessionKind, TriBlock,
};
pub use parse::parse_message;
