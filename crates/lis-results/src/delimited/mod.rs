//! Reading result files exported by LC-MS/MS quantitation software (result
//! files brief F3–F7, F14, §5, §13; REQ-INT-005, REQ-RES-001). Pure: no
//! database, no file system.
//!
//! - **Text** ([`decode`]). A UTF-8 byte-order mark is stripped and decides
//!   UTF-8; a UTF-16 one is refused. Otherwise the layout's encoding is read
//!   strictly: UTF-8, or Windows-1252 only when the template says so. Invalid
//!   text is `undecodable_text`.
//! - **Header** (F3). The first line within the first [`SIGNATURE_LINES`]
//!   whose cells equal the layout's first signature row, followed by the
//!   second row where there is one, is the header; without one the file is
//!   `layout_mismatch`. Each line is parsed on its own, so a preamble cannot
//!   hide or shift the header.
//! - **Records** follow RFC 4180 quoting through the `csv` crate, with
//!   flexible field counts. A record whose field count differs from the
//!   header, or a final record left inside an open quote, is a malformed row
//!   ([`ExtractedImport::malformed_rows`](crate::astm::ExtractedImport));
//!   a row is never dropped silently.
//!   Data ends at the end of the file or at the first blank line, as the
//!   layout says; a blank line is empty or holds only spaces, tabs and
//!   delimiters.
//! - **Limits** (F14): [`lis_analytical::MAX_FILE_ROWS`] result rows,
//!   [`lis_analytical::MAX_FILE_COLUMNS`] fields per record and
//!   [`lis_analytical::MAX_FIELD_BYTES`] per field, else `limits_exceeded`. The byte limit is checked by intake before reading
//!   (`lis_interfaces::MAX_RESULT_FILE_BYTES`).
//! - **Rows** keep every text as read; an empty field is absent. A row's
//!   reference is `L<line>` (long) or `L<line>/<analyte>` (wide), where the
//!   line is the physical line of the decoded text where the record starts.
//! - **Values** ([`read_value`]) is the one home of the F7 rules, used by the
//!   template check and the importer.

mod check;
mod decode;
mod extract;
mod table;
mod value;

#[cfg(test)]
mod tests;

pub use check::{LayoutCheck, LayoutCheckCounts, LayoutCheckRow, ReadingKind, check_layout};
pub use decode::{SampleText, sample_text};
pub use extract::extract_file;
pub use table::{PreviewLine, SIGNATURE_LINES, preview_lines};
pub use value::{FileValueReading, read_value};

use serde::Serialize;
use ts_rs::TS;

/// Why a whole file yields no rows (brief §5). The import attempt records
/// the code; `Display` tells the user what to do and never echoes content.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS, thiserror::Error)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum FileExtractFailure {
    /// Not valid text in the declared encoding, or a UTF-16 byte-order mark.
    #[error("The file is not readable text in the template's encoding. Save the export as UTF-8.")]
    UndecodableText,
    /// No line in the first 50 matches the template's header.
    #[error("The file's header does not match the approved template.")]
    LayoutMismatch,
    /// Over the row, column or field limit.
    #[error("The file has more rows, columns or text in a field than the LIS reads.")]
    LimitsExceeded,
}

impl FileExtractFailure {
    /// The stable failure code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UndecodableText => "undecodable_text",
            Self::LayoutMismatch => "layout_mismatch",
            Self::LimitsExceeded => "limits_exceeded",
        }
    }
}
