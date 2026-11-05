//! MLLP framing and the partner's HL7 acknowledgement (ticket #4555 step 4; REQ-INT-002;
//! ADR 0007 §7). A message travels as a start block (VT), its bytes, an end block (FS) and
//! a carriage return. The partner answers on the same connection with an ACK whose MSA
//! segment says whether it accepted the message (AA), found an error in it (AE) or
//! rejected it (AR). Pure; the engine owns the connections.

use super::hl7_read::{Reader, find, message_text, segments, separators};
use super::model::AckCode;

/// VT: where a message starts.
pub const START_BLOCK: u8 = 0x0b;
/// FS: where a message ends, before a carriage return.
pub const END_BLOCK: u8 = 0x1c;
/// The partner's reason, kept at most this many characters.
const TEXT_MAX: usize = 200;
/// The attempt code of a message sent without an acknowledgement in time: it left, and is
/// sent again with the same control ID.
pub const NO_ACK: &str = "no_ack";

/// `message` framed for MLLP.
#[must_use]
pub fn frame(message: &[u8]) -> Vec<u8> {
    let mut framed = Vec::with_capacity(message.len() + 3);
    framed.push(START_BLOCK);
    framed.extend_from_slice(message);
    framed.extend_from_slice(&[END_BLOCK, b'\r']);
    framed
}

/// Why received bytes are not a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProblem {
    /// Something other than a start block came first.
    NotFramed,
    /// No end block within the size limit.
    TooLarge,
}

/// The first whole message in `buffer` and how many bytes it took; `None` while it is
/// still arriving. Line breaks before the start block are skipped, as some systems send
/// them between messages.
pub fn unframe(buffer: &[u8], max_len: usize) -> Result<Option<(Vec<u8>, usize)>, FrameProblem> {
    let Some(start) = buffer
        .iter()
        .position(|byte| !matches!(byte, b'\r' | b'\n'))
    else {
        return Ok(None);
    };
    if buffer[start] != START_BLOCK {
        return Err(FrameProblem::NotFramed);
    }
    let body = &buffer[start + 1..];
    match body.windows(2).position(|pair| pair == [END_BLOCK, b'\r']) {
        Some(end) if end <= max_len => Ok(Some((body[..end].to_vec(), start + end + 3))),
        Some(_) => Err(FrameProblem::TooLarge),
        None if body.len() > max_len + 1 => Err(FrameProblem::TooLarge),
        None => Ok(None),
    }
}

/// The partner's answer to one message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Acknowledgement {
    pub code: AckCode,
    /// The partner's reason: ERR-8 when given, else MSA-3; `None` when it gives none.
    pub text: Option<String>,
}

/// Why an answer is not the acknowledgement of the message sent; the codes show in Logs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckProblem {
    /// Not an HL7 message with an MSA segment holding AA, AE or AR.
    Unreadable,
    /// MSA-2 names another message.
    OtherMessage,
}

impl AckProblem {
    /// The stored and displayed code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Unreadable => "ack_unreadable",
            Self::OtherMessage => "ack_other_message",
        }
    }
}

/// MSH-10 of a message: the control ID its acknowledgement must carry in MSA-2.
#[must_use]
pub fn control_id(message: &[u8]) -> Option<String> {
    let text = message_text(message)?;
    let separators = separators(text)?;
    let segments = segments(text, separators);
    let id = Reader { separators }.text(segments.first()?.field(10));
    (!id.is_empty()).then_some(id)
}

/// The acknowledgement in `bytes`, of the message whose control ID is `control_id`.
pub fn read_acknowledgement(bytes: &[u8], control_id: &str) -> Result<Acknowledgement, AckProblem> {
    let text = message_text(bytes).ok_or(AckProblem::Unreadable)?;
    let separators = separators(text).ok_or(AckProblem::Unreadable)?;
    let segments = segments(text, separators);
    let reader = Reader { separators };
    let msa = find(&segments, "MSA").ok_or(AckProblem::Unreadable)?;
    let code = match reader.component(msa.field(1), 1).as_str() {
        "AA" => AckCode::Accepted,
        "AE" => AckCode::Error,
        "AR" => AckCode::Rejected,
        _ => return Err(AckProblem::Unreadable),
    };
    if reader.text(msa.field(2)) != control_id {
        return Err(AckProblem::OtherMessage);
    }
    let reason = find(&segments, "ERR")
        .map(|err| reader.text(err.field(8)))
        .filter(|reason| !reason.is_empty())
        .unwrap_or_else(|| reader.text(msa.field(3)));
    Ok(Acknowledgement {
        code,
        text: shown(&reason),
    })
}

/// The partner's reason as Logs show it: one line, without control characters, cut short.
fn shown(reason: &str) -> Option<String> {
    let line: String = reason
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .take(TEXT_MAX)
        .collect();
    let line = line.trim();
    (!line.is_empty()).then(|| line.to_owned())
}

#[cfg(test)]
#[path = "mllp_tests.rs"]
mod tests;
