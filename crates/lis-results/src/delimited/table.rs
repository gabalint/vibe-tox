//! Physical lines, the header search and the data records of decoded text
//! (brief F3, F6, F14).

use csv::StringRecord;
use lis_analytical::{
    EndOfData, FileDelimiter, FileEncoding, FileLayout, MAX_FIELD_BYTES, MAX_FILE_COLUMNS,
};
use serde::Serialize;
use ts_rs::TS;

use super::FileExtractFailure;
use super::decode::decode;

/// The header must start within this many lines (F3); the template page
/// shows as many.
pub const SIGNATURE_LINES: usize = 50;

/// The start offsets of the physical lines of a text. CRLF, LF and a lone
/// CR each end a line, as they end a record for the `csv` reader.
pub(crate) struct Lines<'a> {
    text: &'a str,
    starts: Vec<usize>,
}

impl<'a> Lines<'a> {
    pub(crate) fn new(text: &'a str) -> Self {
        let bytes = text.as_bytes();
        let mut starts = vec![0];
        for (index, &byte) in bytes.iter().enumerate() {
            let ends_line =
                byte == b'\n' || (byte == b'\r' && bytes.get(index + 1) != Some(&b'\n'));
            if ends_line && index + 1 < bytes.len() {
                starts.push(index + 1);
            }
        }
        if text.is_empty() {
            starts.clear();
        }
        Self { text, starts }
    }

    pub(crate) fn count(&self) -> usize {
        self.starts.len()
    }

    /// Line `index` (0-based) without its line end; empty past the end.
    pub(crate) fn line(&self, index: usize) -> &'a str {
        let Some(&start) = self.starts.get(index) else {
            return "";
        };
        let end = self
            .starts
            .get(index + 1)
            .copied()
            .unwrap_or(self.text.len());
        self.text
            .get(start..end)
            .unwrap_or_default()
            .trim_end_matches(['\r', '\n'])
    }

    /// The byte offset where line `index` starts; the text's end past the last line.
    fn start(&self, index: usize) -> usize {
        self.starts.get(index).copied().unwrap_or(self.text.len())
    }

    /// The 1-based number of the line holding byte `offset`.
    fn number_at(&self, offset: usize) -> usize {
        self.starts.partition_point(|&start| start <= offset)
    }
}

fn reader(input: &[u8], delimiter: FileDelimiter) -> csv::Reader<&[u8]> {
    csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .delimiter(delimiter.byte())
        .from_reader(input)
}

/// The cells of one line read on its own: the header search's reading.
/// `None` for a line with more fields than [`MAX_FILE_COLUMNS`], which is
/// refused before it is split (F14).
fn line_cells(line: &str, delimiter: FileDelimiter) -> Option<Vec<String>> {
    if scan(line, delimiter).too_wide() {
        return None;
    }
    let mut record = StringRecord::new();
    Some(
        match reader(line.as_bytes(), delimiter).read_record(&mut record) {
            Ok(true) => record.iter().map(str::to_owned).collect(),
            Ok(false) | Err(_) => Vec::new(),
        },
    )
}

/// One of the first lines of a sample file as cells (brief §7, step 1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct PreviewLine {
    /// The physical line number, from 1.
    pub line: u32,
    /// The cells, exactly as the header search reads this line.
    pub cells: Vec<String>,
}

/// The first [`SIGNATURE_LINES`] lines of `bytes` as cells, each line read
/// on its own as the header search reads it, in the chosen encoding and
/// delimiter.
pub fn preview_lines(
    bytes: &[u8],
    encoding: FileEncoding,
    delimiter: FileDelimiter,
) -> Result<Vec<PreviewLine>, FileExtractFailure> {
    let text = decode(bytes, encoding)?;
    let lines = Lines::new(&text);
    (0..SIGNATURE_LINES.min(lines.count()))
        .map(|index| {
            Ok(PreviewLine {
                line: u32::try_from(index + 1).unwrap_or(u32::MAX),
                cells: line_cells(lines.line(index), delimiter)
                    .ok_or(FileExtractFailure::LimitsExceeded)?,
            })
        })
        .collect()
}

/// The 0-based index of the header's last line (F3): the first line within
/// [`SIGNATURE_LINES`] whose cells equal the first signature row, followed
/// by the second row where there is one.
fn find_header(lines: &Lines<'_>, layout: &FileLayout) -> Result<usize, FileExtractFailure> {
    let matches = |index: usize, row: &[String]| {
        line_cells(lines.line(index), layout.delimiter).is_some_and(|cells| {
            cells.len() == row.len()
                && cells
                    .iter()
                    .zip(row)
                    .all(|(cell, expected)| layout.field(cell) == layout.field(expected))
        })
    };
    (0..SIGNATURE_LINES.min(lines.count()))
        .find(|&start| {
            layout
                .header
                .iter()
                .enumerate()
                .all(|(offset, row)| start + offset < lines.count() && matches(start + offset, row))
        })
        .map(|start| start + layout.header.len().saturating_sub(1))
        .ok_or(FileExtractFailure::LayoutMismatch)
}

/// One data record.
pub(crate) struct DataRecord<'r> {
    /// The physical line where it starts, from 1.
    pub(crate) line: usize,
    /// Its fields as read.
    pub(crate) fields: &'r StringRecord,
    /// False for a final record that ends inside an open quote: it ran to
    /// the end of the file and is never a usable row.
    pub(crate) terminated: bool,
}

/// Calls `visit` with each data record after the header, in order, until
/// the end of data. A record wider than [`MAX_FILE_COLUMNS`] or a field
/// longer than [`MAX_FIELD_BYTES`] fails the file; the width is checked on
/// the text before any record is split, so a hostile line cannot allocate
/// millions of cells.
pub(crate) fn read_data(
    text: &str,
    layout: &FileLayout,
    mut visit: impl FnMut(DataRecord<'_>) -> Result<(), FileExtractFailure>,
) -> Result<(), FileExtractFailure> {
    let lines = Lines::new(text);
    let header = find_header(&lines, layout)?;
    let data_start = lines.start(header + 1);
    let region = text.get(data_start..).unwrap_or_default();
    if scan(region, layout.delimiter).too_wide() {
        return Err(FileExtractFailure::LimitsExceeded);
    }
    let mut reader = reader(region.as_bytes(), layout.delimiter);
    let (mut current, mut next) = (StringRecord::new(), StringRecord::new());
    let mut more = read(&mut reader, &mut current)?;
    // The last line consumed so far, from 1: the header's last line at first.
    let mut last_line = header + 1;
    while more {
        let start = record_start(region, &current);
        let consumed = data_start + offset(reader.position().byte());
        more = read(&mut reader, &mut next)?;
        let line = lines.number_at(data_start + start);
        // The reader skips empty lines; a gap in the line numbers shows one.
        let skipped_blank = line > last_line + 1;
        let blank = is_blank(&current);
        last_line = lines.number_at(consumed.saturating_sub(1)).max(line);
        if (skipped_blank || blank) && layout.end_of_data == EndOfData::FirstBlankLine {
            break;
        }
        if !blank {
            check_limits(&current)?;
            let raw = region.get(start..).unwrap_or_default();
            let terminated = more || !scan(raw, layout.delimiter).open_quote;
            visit(DataRecord {
                line,
                fields: &current,
                terminated,
            })?;
        }
        std::mem::swap(&mut current, &mut next);
    }
    Ok(())
}

fn read(
    reader: &mut csv::Reader<&[u8]>,
    record: &mut StringRecord,
) -> Result<bool, FileExtractFailure> {
    // The input is a `str` read from memory: neither I/O nor UTF-8 can fail.
    reader
        .read_record(record)
        .map_err(|_| FileExtractFailure::UndecodableText)
}

/// Where the record's text starts in `region`. The reader positions a record
/// where it stopped after the previous one, before the line ends it skips.
fn record_start(region: &str, record: &StringRecord) -> usize {
    let from = record
        .position()
        .map_or(0, |position| offset(position.byte()));
    let skipped = region
        .as_bytes()
        .get(from..)
        .unwrap_or_default()
        .iter()
        .take_while(|&&byte| byte == b'\r' || byte == b'\n')
        .count();
    from + skipped
}

fn offset(byte: u64) -> usize {
    usize::try_from(byte).unwrap_or(usize::MAX)
}

/// A record of only empty, space or tab fields reads as a blank line.
fn is_blank(record: &StringRecord) -> bool {
    record
        .iter()
        .all(|cell| cell.trim_matches([' ', '\t']).is_empty())
}

fn check_limits(record: &StringRecord) -> Result<(), FileExtractFailure> {
    if record.len() > MAX_FILE_COLUMNS || record.iter().any(|cell| cell.len() > MAX_FIELD_BYTES) {
        return Err(FileExtractFailure::LimitsExceeded);
    }
    Ok(())
}

/// What a scan of record text finds under RFC 4180 quoting, as the reader
/// reads it: a quote opens a quoted field only at a field's start, and a
/// doubled quote inside one is a quote character.
struct Scan {
    /// The text ends inside an open quote: for a final record, the reader
    /// then took the rest of the file as one field.
    open_quote: bool,
    /// The most delimiters outside quotes in one record.
    widest: usize,
}

impl Scan {
    /// A record has more fields than [`MAX_FILE_COLUMNS`].
    fn too_wide(&self) -> bool {
        self.widest >= MAX_FILE_COLUMNS
    }
}

fn scan(raw: &str, delimiter: FileDelimiter) -> Scan {
    let mut bytes = raw.bytes().peekable();
    let (mut quoted, mut field_start) = (false, true);
    let (mut delimiters, mut widest) = (0, 0);
    while let Some(byte) = bytes.next() {
        if quoted {
            if byte == b'"' && bytes.next_if_eq(&b'"').is_none() {
                quoted = false;
            }
        } else if byte == b'"' && field_start {
            quoted = true;
            field_start = false;
        } else if byte == delimiter.byte() {
            delimiters += 1;
            widest = usize::max(widest, delimiters);
            field_start = true;
        } else {
            field_start = matches!(byte, b'\r' | b'\n');
            if field_start {
                delimiters = 0;
            }
        }
    }
    Scan {
        open_quote: quoted,
        widest,
    }
}
