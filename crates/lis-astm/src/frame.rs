use thiserror::Error;

/// Enquiry control byte.
pub const ENQ: u8 = 0x05;
/// Positive acknowledgement control byte.
pub const ACK: u8 = 0x06;
/// Negative acknowledgement control byte.
pub const NAK: u8 = 0x15;
/// End-of-transmission control byte.
pub const EOT: u8 = 0x04;
/// Start-of-text control byte.
pub const STX: u8 = 0x02;
/// End-of-text control byte for the last frame in a record.
pub const ETX: u8 = 0x03;
/// End-of-block control byte for an intermediate frame.
pub const ETB: u8 = 0x17;
/// Carriage return byte.
pub const CR: u8 = 0x0d;
/// Line feed byte.
pub const LF: u8 = 0x0a;

const MAX_TEXT_BYTES: usize = 240;
const FRAME_OVERHEAD_BYTES: usize = 7;

/// Whether a frame continues or ends the current record text.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FrameEnd {
    /// More text follows in another frame.
    Intermediate,
    /// This is the final frame for the current record text.
    Final,
}

impl FrameEnd {
    fn byte(self) -> u8 {
        match self {
            Self::Intermediate => ETB,
            Self::Final => ETX,
        }
    }
}

/// A validated low-level frame with its exact received bytes.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Frame {
    number: u8,
    text: Vec<u8>,
    end: FrameEnd,
    raw: Vec<u8>,
}

impl Frame {
    /// Returns the numeric frame sequence in the range 0 through 7.
    #[must_use]
    pub const fn number(&self) -> u8 {
        self.number
    }

    /// Returns the validated application text exactly as received.
    #[must_use]
    pub fn text(&self) -> &[u8] {
        &self.text
    }

    /// Reports whether the frame is intermediate or final.
    #[must_use]
    pub const fn end(&self) -> FrameEnd {
        self.end
    }

    /// Returns the complete frame bytes, including control bytes and checksum.
    #[must_use]
    pub fn raw(&self) -> &[u8] {
        &self.raw
    }
}

/// A framing or encoding failure that never includes received payload text.
#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum FrameError {
    /// The frame is too short to contain its required fields.
    #[error("ASTM frame is shorter than the required framing bytes")]
    TooShort,
    /// The leading start-of-text byte is absent.
    #[error("ASTM frame is missing STX")]
    MissingStart,
    /// The trailing carriage-return/line-feed pair is absent.
    #[error("ASTM frame is missing its CR/LF terminator")]
    MissingTerminator,
    /// Neither ETB nor ETX occurs in the framing position.
    #[error("ASTM frame is missing ETB or ETX")]
    MissingEndMarker,
    /// The frame sequence byte is not an ASCII digit from zero through seven.
    #[error("ASTM frame number is outside 0 through 7")]
    BadFrameNumber,
    /// The frame text exceeds the selected 240-byte profile.
    #[error("ASTM frame text exceeds 240 bytes")]
    Oversize,
    /// Text contains a byte outside printable ASCII and carriage return.
    #[error("ASTM frame text contains a disallowed byte at offset {offset}")]
    DisallowedTextByte {
        /// Zero-based byte position inside the frame text.
        offset: usize,
    },
    /// Checksum characters are not two uppercase hexadecimal digits.
    #[error("ASTM frame checksum is not two uppercase hexadecimal digits")]
    InvalidChecksumDigits,
    /// The transmitted checksum does not match the computed checksum.
    #[error("ASTM frame checksum does not match")]
    BadChecksum,
    /// A record passed to the encoder already contains a record terminator.
    #[error("ASTM record encoder input contains a carriage return")]
    EmbeddedRecordTerminator,
}

/// Computes the low byte of the sum from frame number through ETB/ETX.
#[must_use]
pub fn checksum(bytes: &[u8]) -> u8 {
    bytes.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte))
}

/// Encodes one validated frame using uppercase hexadecimal checksum digits.
pub fn encode_frame(number: u8, text: &[u8], end: FrameEnd) -> Result<Vec<u8>, FrameError> {
    validate_number(number)?;
    validate_text(text)?;

    let number_byte = b'0' + number;
    let end_byte = end.byte();
    let mut framed = Vec::with_capacity(text.len() + FRAME_OVERHEAD_BYTES);
    framed.extend_from_slice(&[STX, number_byte]);
    framed.extend_from_slice(text);
    framed.push(end_byte);
    let sum = checksum(&framed[1..]);
    framed.extend_from_slice(&hex_pair(sum));
    framed.extend_from_slice(&[CR, LF]);
    Ok(framed)
}

/// Encodes unterminated E1394 records, splitting records longer than 240 bytes.
pub fn encode_records(records: &[Vec<u8>]) -> Result<Vec<Vec<u8>>, FrameError> {
    let mut frames = Vec::new();
    let mut number = 1_u8;

    for record in records {
        if record.contains(&CR) {
            return Err(FrameError::EmbeddedRecordTerminator);
        }
        validate_allowed_text(record)?;

        let mut terminated = record.clone();
        terminated.push(CR);
        let chunk_count = terminated.len().div_ceil(MAX_TEXT_BYTES);
        for (index, chunk) in terminated.chunks(MAX_TEXT_BYTES).enumerate() {
            let end = if index + 1 == chunk_count {
                FrameEnd::Final
            } else {
                FrameEnd::Intermediate
            };
            frames.push(encode_frame(number, chunk, end)?);
            number = (number + 1) % 8;
        }
    }
    Ok(frames)
}

/// Parses and validates one complete low-level frame.
pub fn parse_frame(raw: &[u8]) -> Result<Frame, FrameError> {
    if raw.len() < FRAME_OVERHEAD_BYTES {
        return Err(FrameError::TooShort);
    }
    if raw.first() != Some(&STX) {
        return Err(FrameError::MissingStart);
    }
    if !raw.ends_with(&[CR, LF]) {
        return Err(FrameError::MissingTerminator);
    }

    let number = parse_number(raw[1])?;
    let marker_index = raw.len() - 5;
    let end = match raw[marker_index] {
        ETB => FrameEnd::Intermediate,
        ETX => FrameEnd::Final,
        _ => return Err(FrameError::MissingEndMarker),
    };
    let text = &raw[2..marker_index];
    validate_text(text)?;
    let transmitted = parse_hex_pair(&raw[marker_index + 1..marker_index + 3])?;
    let expected = checksum(&raw[1..=marker_index]);
    if transmitted != expected {
        return Err(FrameError::BadChecksum);
    }

    Ok(Frame {
        number,
        text: text.to_vec(),
        end,
        raw: raw.to_vec(),
    })
}

fn validate_number(number: u8) -> Result<(), FrameError> {
    if number <= 7 {
        Ok(())
    } else {
        Err(FrameError::BadFrameNumber)
    }
}

fn parse_number(byte: u8) -> Result<u8, FrameError> {
    let number = byte.checked_sub(b'0').ok_or(FrameError::BadFrameNumber)?;
    validate_number(number)?;
    Ok(number)
}

fn validate_text(text: &[u8]) -> Result<(), FrameError> {
    if text.len() > MAX_TEXT_BYTES {
        return Err(FrameError::Oversize);
    }
    validate_allowed_text(text)
}

fn validate_allowed_text(text: &[u8]) -> Result<(), FrameError> {
    if let Some(offset) = text
        .iter()
        .position(|byte| *byte != CR && !(0x20..=0x7e).contains(byte))
    {
        return Err(FrameError::DisallowedTextByte { offset });
    }
    Ok(())
}

fn parse_hex_pair(bytes: &[u8]) -> Result<u8, FrameError> {
    if bytes.len() != 2 || bytes.iter().any(|byte| !byte.is_ascii_hexdigit()) {
        return Err(FrameError::InvalidChecksumDigits);
    }
    if bytes.iter().any(u8::is_ascii_lowercase) {
        return Err(FrameError::InvalidChecksumDigits);
    }
    let high = hex_value(bytes[0]);
    let low = hex_value(bytes[1]);
    Ok((high << 4) | low)
}

fn hex_pair(byte: u8) -> [u8; 2] {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    [HEX[usize::from(byte >> 4)], HEX[usize::from(byte & 0x0f)]]
}

fn hex_value(byte: u8) -> u8 {
    match byte {
        b'0'..=b'9' => byte - b'0',
        b'A'..=b'F' => byte - b'A' + 10,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn frame_round_trip_uses_uppercase_checksum() {
        let raw = encode_frame(1, b"H|\\^&|||SYNTH-ANALYZER^1.0\r", FrameEnd::Final)
            .expect("synthetic frame encodes");
        let parsed = parse_frame(&raw).expect("encoded frame parses");
        assert_eq!(parsed.number(), 1);
        assert_eq!(parsed.end(), FrameEnd::Final);
        assert_eq!(parsed.text(), b"H|\\^&|||SYNTH-ANALYZER^1.0\r");
        assert!(
            raw[raw.len() - 4..raw.len() - 2]
                .iter()
                .all(|byte| !byte.is_ascii_lowercase())
        );
    }

    #[test]
    fn checksum_covers_number_text_and_end_marker() {
        let raw = encode_frame(7, b"R|1|^^^SYN-TEST|1.230|mg/L\r", FrameEnd::Final)
            .expect("synthetic frame encodes");
        let marker = raw.len() - 5;
        let expected = checksum(&raw[1..=marker]);
        assert_eq!(parse_hex_pair(&raw[marker + 1..marker + 3]), Ok(expected));
    }

    #[test]
    fn frame_larger_than_profile_is_rejected() {
        let text = vec![b'A'; 241];
        assert_eq!(
            encode_frame(1, &text, FrameEnd::Final),
            Err(FrameError::Oversize)
        );
    }

    proptest! {
        #[test]
        fn parsing_arbitrary_bytes_never_panics(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
            let _ = parse_frame(&bytes);
        }

        #[test]
        fn encoded_checksum_always_parses(
            number in 0_u8..8,
            text in prop::collection::vec(0x20_u8..0x7f, 0..241),
            final_frame in any::<bool>(),
        ) {
            let end = if final_frame { FrameEnd::Final } else { FrameEnd::Intermediate };
            let raw = encode_frame(number, &text, end).expect("generated frame encodes");
            let parsed = parse_frame(&raw).expect("generated frame parses");
            prop_assert_eq!(parsed.text(), text);
        }
    }
}
