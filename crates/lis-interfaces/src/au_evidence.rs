//! AU artifacts retain complete wire frames, including delimiters and BCC.

use lis_au::{
    AssemblyOutcome, AuConfig, Frame, Message, ResultAssembler, StreamDecoder, WireEvent,
};
use rusqlite::Connection;

use crate::{InterfaceError, Receipt, ReceiptFrame, repo};

pub(crate) fn frame(profile: &AuConfig, bytes: &[u8]) -> Result<Frame, InterfaceError> {
    let mut decoder =
        StreamDecoder::new(profile.clone()).map_err(|_| InterfaceError::InvalidAuConfiguration)?;
    let mut events = decoder.push(bytes);
    if events.len() != 1 || decoder.is_pending() {
        return Err(InterfaceError::InvalidFrame);
    }
    match events.pop() {
        Some(Ok(WireEvent::Frame(frame))) => Ok(frame),
        _ => Err(InterfaceError::InvalidFrame),
    }
}

pub(crate) fn needs_import(conn: &Connection, receipt: &Receipt) -> Result<bool, InterfaceError> {
    if !receipt.transport.is_au() {
        return Ok(true);
    }
    let profile =
        repo::au_profile(conn, receipt.id)?.ok_or(InterfaceError::InvalidAuConfiguration)?;
    let frames = repo::frames_for_receipt(conn, receipt.id)?;
    let first = frames.first().ok_or(InterfaceError::ReceiptHasNoFrames)?;
    let frame = frame(&profile, &first.bytes)?;
    // Session boundaries and queries are retained as receipt evidence. A query
    // is not an order and must never be synthesized into patient result rows.
    Ok(!matches!(
        lis_au::parse_message(&profile, &frame),
        Ok(Message::Session { .. } | Message::Query(_) | Message::TriBlock(_))
    ))
}

/// Recover only unambiguous complete sample texts or single control frames.
pub(crate) fn recoverable(profile: &AuConfig, frames: &[ReceiptFrame]) -> Option<Vec<u8>> {
    let mut assembler = ResultAssembler::new();
    let mut wire = Vec::new();
    let mut complete = false;
    for (ordinal, stored) in frames.iter().enumerate() {
        if complete || stored.ordinal as usize != ordinal {
            return None;
        }
        let frame = frame(profile, &stored.bytes).ok()?;
        match lis_au::parse_message(profile, &frame).ok()? {
            Message::ResultBlock(block) => {
                complete = matches!(assembler.push(block).ok()?, AssemblyOutcome::Complete(_));
            }
            Message::Session { .. } | Message::Query(_) if frames.len() == 1 => complete = true,
            Message::Session { .. } | Message::Query(_) | Message::TriBlock(_) => return None,
        }
        wire.extend_from_slice(&stored.bytes);
    }
    complete.then_some(wire)
}
