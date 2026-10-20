//! The one encoding every label field value passes through.

/// What a field holds, which decides the characters it accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum FieldKind {
    /// Human-readable text: any characters except control characters.
    Text,
    /// Code 128 barcode data: printable ASCII except `>`.
    Code128,
}

/// Why a field value was refused. Never carries the value itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum FieldError {
    /// Longer than the template allows, counted in characters.
    #[error("longer than {max} characters")]
    TooLong { max: usize },
    /// Contains a control character, which could inject printer commands.
    #[error("contains a control character")]
    ControlCharacter,
    /// A barcode value outside printable ASCII, or containing `>`, which ZPL
    /// reads as a Code 128 subset or function switch.
    #[error("contains a character not allowed in a Code 128 barcode")]
    NotBarcodeCharacter,
}

/// Hex indicator used with `^FH`; the ZPL default.
const HEX_INDICATOR: char = '_';

/// Encodes a value for use inside a `^FH^FD … ^FS` field.
///
/// Printable ASCII passes through, except the ZPL command prefixes `^` and
/// `~` and the hex indicator `_`, which become `_XX` escapes. Every other
/// character becomes the `_XX` escapes of its UTF-8 bytes (the templates
/// select UTF-8 with `^CI28`). The result therefore contains only printable
/// ASCII and cannot start a ZPL command.
pub(crate) fn encode_field(
    value: &str,
    kind: FieldKind,
    max_chars: usize,
) -> Result<String, FieldError> {
    if value.chars().count() > max_chars {
        return Err(FieldError::TooLong { max: max_chars });
    }
    let mut encoded = String::with_capacity(value.len());
    for character in value.chars() {
        if character.is_control() {
            return Err(FieldError::ControlCharacter);
        }
        if kind == FieldKind::Code128 && !is_barcode_character(character) {
            return Err(FieldError::NotBarcodeCharacter);
        }
        if needs_escape(character) {
            let mut utf8 = [0; 4];
            for byte in character.encode_utf8(&mut utf8).bytes() {
                encoded.push(HEX_INDICATOR);
                encoded.push_str(&format!("{byte:02X}"));
            }
        } else {
            encoded.push(character);
        }
    }
    Ok(encoded)
}

fn is_barcode_character(character: char) -> bool {
    matches!(character, ' '..='~') && character != '>'
}

fn needs_escape(character: char) -> bool {
    !matches!(character, ' '..='~') || matches!(character, '^' | '~' | HEX_INDICATOR)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    /// Reverses `^FH` escaping, as the printer does.
    fn decode(encoded: &str) -> Vec<u8> {
        let bytes = encoded.as_bytes();
        let mut decoded = Vec::new();
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] == b'_' {
                let hex = std::str::from_utf8(&bytes[index + 1..index + 3]).expect("ascii");
                decoded.push(u8::from_str_radix(hex, 16).expect("two hex digits"));
                index += 3;
            } else {
                decoded.push(bytes[index]);
                index += 1;
            }
        }
        decoded
    }

    #[test]
    fn plain_text_passes_through() {
        assert_eq!(
            encode_field("SYN-0001 Alpha", FieldKind::Text, 20),
            Ok("SYN-0001 Alpha".to_owned())
        );
    }

    #[test]
    fn zpl_prefixes_and_hex_indicator_are_escaped() {
        assert_eq!(
            encode_field("^XZ~JR_a", FieldKind::Text, 20),
            Ok("_5EXZ_7EJR_5Fa".to_owned())
        );
    }

    #[test]
    fn non_ascii_is_escaped_as_utf8_bytes() {
        assert_eq!(
            encode_field("Zoë", FieldKind::Text, 20),
            Ok("Zo_C3_AB".to_owned())
        );
    }

    #[test]
    fn control_characters_are_refused() {
        for value in ["a\nb", "a\rb", "\u{0}", "a\u{7f}", "a\u{85}b"] {
            assert_eq!(
                encode_field(value, FieldKind::Text, 20),
                Err(FieldError::ControlCharacter),
                "{value:?}"
            );
        }
    }

    #[test]
    fn length_counts_characters_not_bytes() {
        assert!(encode_field("ëëë", FieldKind::Text, 3).is_ok());
        assert_eq!(
            encode_field("abcd", FieldKind::Text, 3),
            Err(FieldError::TooLong { max: 3 })
        );
    }

    #[test]
    fn barcode_refuses_non_ascii_and_subset_switch() {
        assert_eq!(
            encode_field("SYN-ë", FieldKind::Code128, 20),
            Err(FieldError::NotBarcodeCharacter)
        );
        assert_eq!(
            encode_field("SYN>5", FieldKind::Code128, 20),
            Err(FieldError::NotBarcodeCharacter)
        );
        assert_eq!(
            encode_field("SYN-0001", FieldKind::Code128, 20),
            Ok("SYN-0001".to_owned())
        );
    }

    proptest! {
        #[test]
        fn any_input_errors_or_encodes_safely(value in any::<String>(), barcode in any::<bool>()) {
            let kind = if barcode { FieldKind::Code128 } else { FieldKind::Text };
            if let Ok(encoded) = encode_field(&value, kind, usize::MAX) {
                prop_assert!(encoded.chars().all(|c| matches!(c, ' '..='~')));
                prop_assert!(!encoded.contains(['^', '~']));
                prop_assert_eq!(decode(&encoded), value.as_bytes());
            }
        }
    }
}
