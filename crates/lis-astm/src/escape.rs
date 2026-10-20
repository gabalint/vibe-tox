use crate::record_model::{Delimiters, RecordError};

pub(crate) fn decode_escapes(raw: &[u8], delimiters: Delimiters) -> Result<String, RecordError> {
    let mut decoded = Vec::with_capacity(raw.len());
    let mut index = 0;
    while index < raw.len() {
        if raw[index] != delimiters.escape() {
            decoded.push(raw[index]);
            index += 1;
            continue;
        }
        let tail = &raw[index + 1..];
        let Some(end) = tail.iter().position(|byte| *byte == delimiters.escape()) else {
            return Err(RecordError::MalformedEscape);
        };
        decode_escape_code(&tail[..end], delimiters, &mut decoded)?;
        index += end + 2;
    }
    String::from_utf8(decoded).map_err(|_| RecordError::InvalidHexText)
}

fn decode_escape_code(
    code: &[u8],
    delimiters: Delimiters,
    output: &mut Vec<u8>,
) -> Result<(), RecordError> {
    match code {
        b"F" => output.push(delimiters.field()),
        b"S" => output.push(delimiters.component()),
        b"R" => output.push(delimiters.repeat()),
        b"E" => output.push(delimiters.escape()),
        [b'X', hex @ ..] if !hex.is_empty() && hex.len() % 2 == 0 => {
            for pair in hex.chunks_exact(2) {
                output.push(decode_hex_pair(pair)?);
            }
        }
        _ => return Err(RecordError::MalformedEscape),
    }
    Ok(())
}

fn decode_hex_pair(pair: &[u8]) -> Result<u8, RecordError> {
    let high = hex_nibble(pair[0]).ok_or(RecordError::MalformedEscape)?;
    let low = hex_nibble(pair[1]).ok_or(RecordError::MalformedEscape)?;
    Ok((high << 4) | low)
}

fn hex_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        _ => None,
    }
}
