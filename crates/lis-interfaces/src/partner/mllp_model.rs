//! How an HL7 connection travels over MLLP (ticket #4555 step 4; REQ-INT-002, REQ-INT-006):
//! to a partner's interface on the lab network, which answers each message with an
//! application acknowledgement. MLLP is framing, not encryption: a partner outside the lab
//! network is reached through the lab's VPN, never over the internet.

use lis_core::text_enum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

text_enum! {
    /// What the LIS waits for after each message it sends.
    #[ts(export)]
    MllpAcknowledgement {
        /// The partner's application ACK: AA accepted, AE error, AR rejected.
        Application = "application",
    }
}

text_enum! {
    /// The partner's application acknowledgement code (MSA-1).
    #[ts(export)]
    AckCode {
        /// Accepted.
        Accepted = "AA",
        /// Not accepted: an error in the message's content.
        Error = "AE",
        /// Rejected: the message, its type or its sender is not accepted.
        Rejected = "AR",
    }
}

/// Where a connection the LIS opens sends its HL7 messages: the partner's interface on the
/// lab network.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MllpPartner {
    /// The partner's address on the lab network, such as 10.20.0.15.
    pub address: String,
    pub port: u16,
    pub acknowledgement: MllpAcknowledgement,
    /// How long the LIS waits for the acknowledgement, in seconds.
    pub ack_timeout_seconds: u16,
}
