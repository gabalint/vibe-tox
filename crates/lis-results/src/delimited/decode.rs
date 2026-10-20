//! Decoding file bytes into text (brief F6).

use lis_analytical::FileEncoding;

use super::FileExtractFailure;
use super::table::{Lines, SIGNATURE_LINES};

const UTF8_BOM: &[u8] = &[0xEF, 0xBB, 0xBF];
const UTF16_LE_BOM: &[u8] = &[0xFF, 0xFE];
const UTF16_BE_BOM: &[u8] = &[0xFE, 0xFF];

/// Windows-1252 bytes 0x80–0x9F; the rest of the code page is Latin-1. The
/// five bytes the code page leaves undefined are not text.
const WINDOWS_1252_80_9F: [Option<char>; 32] = [
    Some('\u{20AC}'),
    None,
    Some('\u{201A}'),
    Some('\u{0192}'),
    Some('\u{201E}'),
    Some('\u{2026}'),
    Some('\u{2020}'),
    Some('\u{2021}'),
    Some('\u{02C6}'),
    Some('\u{2030}'),
    Some('\u{0160}'),
    Some('\u{2039}'),
    Some('\u{0152}'),
    None,
    Some('\u{017D}'),
    None,
    None,
    Some('\u{2018}'),
    Some('\u{2019}'),
    Some('\u{201C}'),
    Some('\u{201D}'),
    Some('\u{2022}'),
    Some('\u{2013}'),
    Some('\u{2014}'),
    Some('\u{02DC}'),
    Some('\u{2122}'),
    Some('\u{0161}'),
    Some('\u{203A}'),
    Some('\u{0153}'),
    None,
    Some('\u{017E}'),
    Some('\u{0178}'),
];

/// The encoding a byte-order mark decides: UTF-8 for a UTF-8 mark, `None`
/// without one; a UTF-16 mark is refused (UTF-16 is not read, F6).
fn bom_encoding(bytes: &[u8]) -> Result<Option<FileEncoding>, FileExtractFailure> {
    if bytes.starts_with(UTF16_LE_BOM) || bytes.starts_with(UTF16_BE_BOM) {
        return Err(FileExtractFailure::UndecodableText);
    }
    Ok(bytes.starts_with(UTF8_BOM).then_some(FileEncoding::Utf8))
}

/// The bytes as text: a byte-order mark decides the encoding, otherwise
/// `encoding` does. Strict: an invalid sequence, an undefined Windows-1252
/// byte or a NUL character fails the file.
pub(crate) fn decode(bytes: &[u8], encoding: FileEncoding) -> Result<String, FileExtractFailure> {
    let (encoding, body) = match bom_encoding(bytes)? {
        Some(found) => (found, &bytes[UTF8_BOM.len()..]),
        None => (encoding, bytes),
    };
    let text = match encoding {
        FileEncoding::Utf8 => {
            String::from_utf8(body.to_vec()).map_err(|_| FileExtractFailure::UndecodableText)?
        }
        FileEncoding::Windows1252 => windows_1252(body)?,
    };
    if text.contains('\0') {
        return Err(FileExtractFailure::UndecodableText);
    }
    Ok(text)
}

fn windows_1252(bytes: &[u8]) -> Result<String, FileExtractFailure> {
    bytes
        .iter()
        .map(|&byte| match byte {
            0x80..=0x9F => WINDOWS_1252_80_9F[usize::from(byte - 0x80)],
            _ => Some(char::from(byte)),
        })
        .collect::<Option<String>>()
        .ok_or(FileExtractFailure::UndecodableText)
}

/// What the template page shows of a sample file before the person chooses
/// the encoding and delimiter (brief §7, step 1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SampleText {
    /// The encoding a byte-order mark decides; it cannot be changed.
    pub bom_encoding: Option<FileEncoding>,
    /// Whether the text is valid UTF-8. When it is not, the lines show
    /// replacement characters and the file needs another encoding.
    pub valid_utf8: bool,
    /// The first [`SIGNATURE_LINES`] physical lines, without line ends.
    pub lines: Vec<String>,
}

/// The first lines of a sample file for display. A UTF-16 file is refused.
pub fn sample_text(bytes: &[u8]) -> Result<SampleText, FileExtractFailure> {
    let bom = bom_encoding(bytes)?;
    let body = if bom.is_some() {
        &bytes[UTF8_BOM.len()..]
    } else {
        bytes
    };
    let text = String::from_utf8_lossy(body);
    let lines = Lines::new(&text);
    Ok(SampleText {
        bom_encoding: bom,
        valid_utf8: std::str::from_utf8(body).is_ok(),
        lines: (0..SIGNATURE_LINES.min(lines.count()))
            .map(|index| lines.line(index).to_owned())
            .collect(),
    })
}
