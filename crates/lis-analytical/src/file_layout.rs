//! The file template of a result-file source (result files brief F2–F10,
//! F14, §13): how the text is read, the exact header signature, the shape,
//! the column roles and the words a person mapped. Pure: no database.
//!
//! A layout is laboratory configuration stored with its registration and
//! approved with it (ADR 0015). [`FileLayout::validate`] is the one check of
//! its structure; the reader in `lis-results` relies on it.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

use crate::model::PositionRole;

/// F14: a file with more result rows than this fails with `limits_exceeded`.
pub const MAX_FILE_ROWS: usize = 50_000;
/// F14: the widest header and record the reader accepts.
pub const MAX_FILE_COLUMNS: usize = 512;
/// F14: the longest field, in bytes of decoded text.
pub const MAX_FIELD_BYTES: usize = 4096;

lis_core::text_enum! {
    /// The text encoding when the file has no byte-order mark (F6).
    FileEncoding {
        /// UTF-8, read strictly.
        Utf8 = "utf8",
        /// Windows-1252, only when the template says so.
        Windows1252 = "windows1252",
    }
}

lis_core::text_enum! {
    /// The field delimiter, chosen by the person who writes the template (F6).
    FileDelimiter {
        /// `,`
        Comma = "comma",
        /// A tab character.
        Tab = "tab",
        /// `;`
        Semicolon = "semicolon",
    }
}

impl FileDelimiter {
    /// The delimiter byte.
    #[must_use]
    pub const fn byte(self) -> u8 {
        match self {
            Self::Comma => b',',
            Self::Tab => b'\t',
            Self::Semicolon => b';',
        }
    }
}

lis_core::text_enum! {
    /// Where the data rows end (§4).
    EndOfData {
        /// At the end of the file.
        EndOfFile = "end_of_file",
        /// At the first blank line after the header, for exports with a footer.
        FirstBlankLine = "first_blank_line",
    }
}

lis_core::text_enum! {
    /// What an approved value word means (F7, Q-40).
    ValueWordMeaning {
        /// Below the method version's limit of quantitation.
        BelowLoq = "below_loq",
        /// Below the method version's limit of detection (quantitative methods only).
        BelowLod = "below_lod",
        /// The test was not performed.
        NotPerformed = "not_performed",
    }
}

/// The file template of a result-file source (brief §4, §13).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct FileLayout {
    /// The encoding when there is no byte-order mark.
    pub encoding: FileEncoding,
    /// The field delimiter.
    pub delimiter: FileDelimiter,
    /// Whether surrounding spaces are removed from every field (F6).
    pub trim_fields: bool,
    /// The exact header cells, one or two rows of equal width: the signature (F3).
    pub header: Vec<Vec<String>>,
    /// Long or wide.
    pub shape: FileShape,
    /// The column roles, by 0-based index into the last header row (F5).
    pub columns: FileColumns,
    /// Where the data rows end.
    pub end_of_data: EndOfData,
    /// Sample-type words and the run-position role each means (F9).
    pub sample_types: Vec<SampleTypeWord>,
    /// Exact non-empty value texts and what each means (F7).
    pub value_words: Vec<ValueWord>,
    /// Analyte names counted as not results, such as internal standards (F10; long only).
    pub not_results: Vec<String>,
    /// The unit of every value when the file has no units column (F8).
    pub declared_unit: Option<String>,
}

/// How a file lays out samples and analytes (F4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
#[serde(deny_unknown_fields)]
#[ts(export)]
pub enum FileShape {
    /// One row per sample and analyte.
    Long {
        /// The column holding the analyte name.
        analyte_column: u16,
        /// The column holding the value.
        value_column: u16,
    },
    /// One row per sample; each listed column holds one analyte's value.
    Wide {
        /// One entry per analyte (per analyte group for a two-row header).
        value_columns: Vec<ValueColumn>,
    },
}

/// One analyte's value column in a wide file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ValueColumn {
    /// The column, 0-based.
    pub column: u16,
    /// The analyte name the rows of this column carry (the group label of a
    /// two-row header); it must have a test code.
    pub analyte_name: String,
}

/// The column roles (F5). Indexes are 0-based into the last header row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct FileColumns {
    /// The sample label: a container label or a lot's sample label.
    pub sample_label: u16,
    /// The units text.
    pub units: Option<u16>,
    /// The sample-type word.
    pub sample_type: Option<u16>,
    /// Flags, kept uninterpreted.
    pub flags: Option<u16>,
    /// The operator, kept uninterpreted.
    pub operator: Option<u16>,
    /// The acquisition time, kept uninterpreted.
    pub acquired_at: Option<u16>,
    /// The dilution factor text (F11).
    pub dilution: Option<u16>,
}

/// A sample-type word and the role it means (F9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct SampleTypeWord {
    /// The exact word.
    pub word: String,
    /// Patient, control, calibrator or blank.
    pub role: PositionRole,
}

/// An exact, non-empty value text and its meaning (F7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct ValueWord {
    /// The exact text, never empty.
    pub word: String,
    /// What it means.
    pub meaning: ValueWordMeaning,
}

impl FileLayout {
    /// The width of the header: the number of cells in each row.
    #[must_use]
    pub fn width(&self) -> usize {
        self.header.last().map_or(0, Vec::len)
    }

    /// A field as this layout reads it: without surrounding spaces when it
    /// trims (F6). The one home of the trimming rule.
    #[must_use]
    pub fn field<'a>(&self, text: &'a str) -> &'a str {
        if self.trim_fields {
            text.trim_matches(' ')
        } else {
            text
        }
    }

    /// The file's own text naming the analyte of a wide file's `column`: its
    /// header cell under a one-row header, or under a two-row header the group
    /// label, written only at a group's first column and carried forward to
    /// the empty cells after it (F4). `None` when no label precedes it.
    #[must_use]
    pub fn analyte_label(&self, column: u16) -> Option<&str> {
        let first = self.header.first()?;
        let column = usize::from(column);
        if self.header.len() == 1 {
            return first.get(column).map(|cell| self.field(cell));
        }
        first
            .get(..=column)?
            .iter()
            .rev()
            .map(|cell| self.field(cell))
            .find(|cell| !cell.is_empty())
    }
}

mod validate;

#[cfg(test)]
mod tests;
