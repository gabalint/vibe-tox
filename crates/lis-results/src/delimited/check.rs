//! The template page's check (brief §7, step 6): the reading of a sample
//! file under a layout, row by row, with counts. It writes nothing, and
//! placement on runs is known only at a real import.

use lis_analytical::{FileLayout, FileShape, PositionRole, SourceTestCodeInput, ValueWordMeaning};
use lis_catalog::{AnalyteId, OutputScale};
use serde::Serialize;
use ts_rs::TS;

use super::FileExtractFailure;
use super::extract::extract_file;
use super::value::{FileValueReading, read_value};
use crate::astm::ExtractedRow;
use crate::value::SourceValue;

/// How a row's value text reads, as the template page shows it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "snake_case")]
#[ts(export)]
pub enum ReadingKind {
    /// A positive quantity.
    Quantity,
    /// A censored value with a positive bound, such as `<0.50`.
    Censored,
    /// An approved word; see the row's meaning.
    Word,
    /// Text without a digit that no approved word matches.
    UnmappedWord,
    /// An empty field: always quarantined.
    Empty,
    /// Zero or below, or a comparator bound of zero or below: quarantined unless mapped.
    ZeroOrNegative,
    /// Neither a value nor a word.
    Malformed,
}

/// One row of the check. It holds sample labels and values from the file,
/// shown only to the person checking and never logged.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutCheckRow {
    /// `L<line>` or `L<line>/<analyte>`.
    pub record_path: String,
    /// The sample label as read.
    pub sample_label: Option<String>,
    /// The analyte name as read (long) or listed (wide).
    pub analyte_name: Option<String>,
    /// For a wide file, the file's own text above the analyte's column: its
    /// header cell, or its group label under a two-row header. The approver
    /// compares it with the listed name. None for a long file, whose rows
    /// carry the name themselves.
    pub header_text: Option<String>,
    /// The catalog analyte the name maps to.
    pub analyte_id: Option<AnalyteId>,
    /// The name is on the not-a-result list.
    pub not_result: bool,
    /// The sample-type word as read.
    pub type_word: Option<String>,
    /// The role the word maps to; none when unmapped.
    pub role: Option<PositionRole>,
    /// The value text as read.
    pub value_text: Option<String>,
    /// How the value reads for a quantitative method.
    pub reading: ReadingKind,
    /// The approved word's meaning, for a word.
    pub word_meaning: Option<ValueWordMeaning>,
}

/// Row counts of the check.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutCheckCounts {
    /// Result rows read.
    pub rows: u32,
    /// Records whose field count differs from the header (quarantined at import).
    pub malformed_rows: u32,
    /// Rows whose analyte is not a result.
    pub not_results: u32,
    /// Rows whose analyte is neither mapped nor a not-a-result name.
    pub unmapped_analytes: u32,
    /// Rows without a mapped sample type, when the layout has a type column.
    pub untyped: u32,
    /// Positive quantities.
    pub quantities: u32,
    /// Censored values.
    pub censored: u32,
    /// Approved words.
    pub words: u32,
    /// Unmapped words.
    pub unmapped_words: u32,
    /// Empty values.
    pub empty: u32,
    /// Zero or negative values.
    pub zero_or_negative: u32,
    /// Malformed values.
    pub malformed_values: u32,
}

/// The check of a sample file: every row's reading and the counts, or the
/// whole-file failure with no rows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct LayoutCheck {
    /// Why the file yields no rows, if it does not.
    pub failure: Option<FileExtractFailure>,
    /// Each result row in file order.
    pub rows: Vec<LayoutCheckRow>,
    /// The references of malformed records.
    pub malformed_rows: Vec<String>,
    /// The counts.
    pub counts: LayoutCheckCounts,
}

/// Reads `bytes` under `layout` as an import would, mapping analyte names
/// through `test_codes` and values through [`read_value`] for a
/// quantitative method. The layout must already be valid.
#[must_use]
pub fn check_layout(
    bytes: &[u8],
    layout: &FileLayout,
    test_codes: &[SourceTestCodeInput],
) -> LayoutCheck {
    let import = match extract_file(bytes, layout) {
        Ok(import) => import,
        Err(failure) => {
            return LayoutCheck {
                failure: Some(failure),
                rows: Vec::new(),
                malformed_rows: Vec::new(),
                counts: LayoutCheckCounts::default(),
            };
        }
    };
    let rows: Vec<LayoutCheckRow> = import
        .rows
        .into_iter()
        .map(|row| check_row(row, layout, test_codes))
        .collect();
    let counts = count(
        &rows,
        &import.malformed_rows,
        layout.columns.sample_type.is_some(),
    );
    LayoutCheck {
        failure: None,
        rows,
        malformed_rows: import.malformed_rows,
        counts,
    }
}

fn check_row(
    row: ExtractedRow,
    layout: &FileLayout,
    test_codes: &[SourceTestCodeInput],
) -> LayoutCheckRow {
    let name = row.test_code.as_deref();
    let analyte_id = test_codes
        .iter()
        .find(|code| Some(code.source_test_code.as_str()) == name)
        .map(|code| code.analyte_id);
    let not_result = name.is_some_and(|name| layout.not_results.iter().any(|n| n == name));
    let header_text = match &layout.shape {
        FileShape::Wide { value_columns } => value_columns
            .iter()
            .find(|value| Some(value.analyte_name.as_str()) == name)
            .and_then(|value| layout.analyte_label(value.column))
            .map(str::to_owned),
        FileShape::Long { .. } => None,
    };
    let role = layout
        .sample_types
        .iter()
        .find(|t| Some(t.word.as_str()) == row.sample_type_text.as_deref())
        .map(|t| t.role);
    let reading = read_value(
        row.value_text.as_deref().unwrap_or_default(),
        &layout.value_words,
        OutputScale::Quantitative,
    );
    let (reading, word_meaning) = match reading {
        FileValueReading::Value(SourceValue::Quantity(_)) => (ReadingKind::Quantity, None),
        FileValueReading::Value(SourceValue::Censored { .. }) => (ReadingKind::Censored, None),
        // Only a qualitative reading gives a code; this check reads quantitatively.
        FileValueReading::Value(SourceValue::Qualitative(_)) | FileValueReading::UnmappedWord => {
            (ReadingKind::UnmappedWord, None)
        }
        FileValueReading::Word(meaning) => (ReadingKind::Word, Some(meaning)),
        FileValueReading::Empty => (ReadingKind::Empty, None),
        FileValueReading::ZeroOrNegative => (ReadingKind::ZeroOrNegative, None),
        FileValueReading::Malformed(_) => (ReadingKind::Malformed, None),
    };
    LayoutCheckRow {
        record_path: row.record_path,
        sample_label: row.sample_label,
        analyte_name: row.test_code,
        header_text,
        analyte_id,
        not_result,
        type_word: row.sample_type_text,
        role,
        value_text: row.value_text,
        reading,
        word_meaning,
    }
}

fn count(rows: &[LayoutCheckRow], malformed: &[String], typed: bool) -> LayoutCheckCounts {
    let mut counts = LayoutCheckCounts {
        rows: u32::try_from(rows.len()).unwrap_or(u32::MAX),
        malformed_rows: u32::try_from(malformed.len()).unwrap_or(u32::MAX),
        ..LayoutCheckCounts::default()
    };
    for row in rows {
        if row.not_result {
            counts.not_results += 1;
        } else if row.analyte_id.is_none() {
            counts.unmapped_analytes += 1;
        }
        if typed && row.role.is_none() {
            counts.untyped += 1;
        }
        let reading = match row.reading {
            ReadingKind::Quantity => &mut counts.quantities,
            ReadingKind::Censored => &mut counts.censored,
            ReadingKind::Word => &mut counts.words,
            ReadingKind::UnmappedWord => &mut counts.unmapped_words,
            ReadingKind::Empty => &mut counts.empty,
            ReadingKind::ZeroOrNegative => &mut counts.zero_or_negative,
            ReadingKind::Malformed => &mut counts.malformed_values,
        };
        *reading += 1;
    }
    counts
}
