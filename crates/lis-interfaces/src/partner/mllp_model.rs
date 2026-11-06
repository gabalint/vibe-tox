//! How an HL7 connection travels over MLLP (ticket #4555 step 4; REQ-INT-002, REQ-INT-006):
//! to a partner's interface on the lab network, which answers each message with an
//! application acknowledgement; or from a partner to an address the LIS listens on, where
//! the LIS answers. MLLP is framing, not encryption: a partner outside the lab network is
//! reached through the lab's VPN, never over the internet.

use lis_core::text_enum;
use serde::{Deserialize, Serialize};
use ts_rs::TS;

text_enum! {
    /// The acknowledgement each message gets on the same connection.
    #[ts(export)]
    MllpAcknowledgement {
        /// The receiver's application ACK: AA accepted, AE error, AR rejected.
        Application = "application",
    }
}

text_enum! {
    /// How the bytes of a received message are read.
    #[ts(export)]
    CharacterSet {
        /// UTF-8, as MSH-18 `UNICODE UTF-8` declares it; plain ASCII reads the same.
        Utf8 = "utf-8",
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

/// A partner address allowed to connect to an MLLP listener, and whose it is.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct AllowedPartner {
    /// An IP address on the lab network, such as 192.168.1.50.
    pub address: String,
    /// Such as Hospital B interface engine; may be empty.
    pub note: String,
}

/// Where the LIS listens for a partner's HL7 messages: an address of this computer on the
/// lab network, open only to the partner addresses allowed, answering each message with an
/// application acknowledgement once it is saved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[ts(export)]
pub struct MllpListener {
    /// This computer's address on the lab network, such as 192.168.1.20.
    pub address: String,
    pub port: u16,
    pub allowed_partners: Vec<AllowedPartner>,
    pub acknowledgement: MllpAcknowledgement,
    pub character_set: CharacterSet,
    /// A partner's connection that sends nothing for this long is closed, in seconds.
    pub idle_timeout_seconds: u16,
}
