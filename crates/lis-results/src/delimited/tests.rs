//! Unit tests of the result-file reader: text, header search, records,
//! limits and the value rules (brief §5, §10). Synthetic data only.

use lis_analytical::{
    EndOfData, FileColumns, FileDelimiter, FileEncoding, FileLayout, FileShape, MAX_FIELD_BYTES,
    MAX_FILE_COLUMNS, MAX_FILE_ROWS, ValueColumn, ValueWord, ValueWordMeaning,
};
use lis_catalog::OutputScale;
use lis_core::{Comparator, DecimalText};
use proptest::prelude::*;

use super::decode::decode;
use super::table::read_data;
use super::*;
use crate::value::{MalformedValue, SourceValue};

const HEADER: &str = "Sample,Analyte,Value";

/// A long layout over `Sample,Analyte,Value`.
fn layout() -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![HEADER.split(',').map(str::to_owned).collect()],
        shape: FileShape::Long {
            analyte_column: 1,
            value_column: 2,
        },
        columns: FileColumns {
            sample_label: 0,
            units: None,
            sample_type: None,
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: None,
        },
        end_of_data: EndOfData::EndOfFile,
        sample_types: Vec::new(),
        value_words: Vec::new(),
        not_results: Vec::new(),
        declared_unit: None,
    }
}

fn paths(
    text: &str,
    layout: &FileLayout,
) -> Result<(Vec<String>, Vec<String>), FileExtractFailure> {
    let import = extract_file(text.as_bytes(), layout)?;
    let rows = import.rows.into_iter().map(|row| row.record_path).collect();
    Ok((rows, import.malformed_rows))
}

fn owned(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| (*item).to_owned()).collect()
}

#[test]
fn utf8_is_strict_and_a_utf16_mark_or_nul_is_refused() {
    let refused = [
        (&b"Sample\xff"[..], FileEncoding::Utf8),
        (b"\xfe\xff\x00S", FileEncoding::Utf8),
        (b"\xff\xfeS\x00", FileEncoding::Windows1252),
        (b"Sample\x00,Analyte", FileEncoding::Utf8),
        (b"Sample\x81", FileEncoding::Windows1252),
    ];
    for (bytes, encoding) in refused {
        assert_eq!(
            decode(bytes, encoding),
            Err(FileExtractFailure::UndecodableText),
            "{bytes:?}"
        );
    }
    assert_eq!(
        decode(b"\x80 \x96 \x9f \xb5g/L \xe9", FileEncoding::Windows1252).unwrap(),
        "\u{20AC} \u{2013} \u{178} \u{b5}g/L \u{e9}"
    );
    assert_eq!(
        decode("µg/L".as_bytes(), FileEncoding::Utf8).unwrap(),
        "µg/L"
    );
}

#[test]
fn a_sample_that_is_not_utf8_is_shown_with_replacement_characters() {
    let sample = sample_text(b"Units\r\n\xb5g/L\r\n").unwrap();
    assert!(!sample.valid_utf8);
    assert_eq!(sample.bom_encoding, None);
    assert_eq!(sample.lines, ["Units", "\u{FFFD}g/L"]);
}

#[test]
fn line_numbers_follow_crlf_lf_lone_cr_and_blank_lines() {
    let text = "Sample,Analyte,Value\r\nA,X,1\nB,X,2\rC,X,3\r\n\r\n  \nD,X,4";
    let (rows, malformed) = paths(text, &layout()).unwrap();
    assert_eq!(rows, ["L2", "L3", "L4", "L7"]);
    assert!(malformed.is_empty());
    let mut to_blank = layout();
    to_blank.end_of_data = EndOfData::FirstBlankLine;
    assert_eq!(paths(text, &to_blank).unwrap().0, ["L2", "L3", "L4"]);
    // A line of spaces also ends the data.
    let spaces = "Sample,Analyte,Value\nA,X,1\n \t \nB,X,2\n";
    assert_eq!(paths(spaces, &to_blank).unwrap().0, ["L2"]);
    assert_eq!(paths(spaces, &layout()).unwrap().0, ["L2", "L4"]);
    // A blank line right after the header leaves no data.
    let first = "Sample,Analyte,Value\n\nA,X,1\n";
    assert_eq!(paths(first, &to_blank).unwrap().0, Vec::<String>::new());
}

#[test]
fn the_header_must_start_within_the_first_fifty_lines() {
    let with_preamble =
        |lines: usize| format!("{}{HEADER}\nC1,Morphine,1.5\n", "Title\n".repeat(lines));
    assert_eq!(paths(&with_preamble(49), &layout()).unwrap().0, ["L51"]);
    assert_eq!(
        paths(&with_preamble(50), &layout()),
        Err(FileExtractFailure::LayoutMismatch)
    );
}

#[test]
fn any_header_change_is_a_layout_mismatch() {
    for header in [
        "Sample,Analyte,Result",
        "Sample,Analyte,Value,Units",
        "Sample,Analyte",
        "Analyte,Sample,Value",
        "Sample ,Analyte,Value",
        "",
    ] {
        let text = format!("{header}\nC1,Morphine,1.5\n");
        assert_eq!(
            paths(&text, &layout()),
            Err(FileExtractFailure::LayoutMismatch),
            "{header:?}"
        );
    }
    assert_eq!(
        paths("", &layout()),
        Err(FileExtractFailure::LayoutMismatch)
    );
    assert_eq!(
        paths(&format!("{HEADER}\r\n"), &layout()).unwrap(),
        (Vec::new(), Vec::new())
    );
}

#[test]
fn a_second_header_row_must_follow_the_first_directly() {
    let mut two_rows = layout();
    two_rows
        .header
        .insert(0, owned(&["Sample", "Morphine Results", ""]));
    let text = "Sample,Morphine Results,\nSample,Analyte,Value\nC1,M,1\n";
    assert_eq!(paths(text, &two_rows).unwrap().0, ["L3"]);
    let apart = "Sample,Morphine Results,\n\nSample,Analyte,Value\nC1,M,1\n";
    assert_eq!(
        paths(apart, &two_rows),
        Err(FileExtractFailure::LayoutMismatch)
    );
}

#[test]
fn trimming_removes_surrounding_spaces_only_and_applies_to_the_header() {
    let text = " Sample , Analyte ,Value\n C1 ,\tMorphine , 1.5 \n";
    assert_eq!(
        paths(text, &layout()),
        Err(FileExtractFailure::LayoutMismatch)
    );
    let mut trimmed = layout();
    trimmed.trim_fields = true;
    let row = &extract_file(text.as_bytes(), &trimmed).unwrap().rows[0];
    assert_eq!(row.sample_label.as_deref(), Some("C1"));
    assert_eq!(row.test_code.as_deref(), Some("\tMorphine"));
    assert_eq!(row.value_text.as_deref(), Some("1.5"));
}

#[test]
fn a_cut_last_line_or_an_open_quote_never_passes_as_a_row() {
    let cut = "Sample,Analyte,Value\nC1,Morphine,1.5\nC2,Morph";
    assert_eq!(
        paths(cut, &layout()).unwrap(),
        (owned(&["L2"]), owned(&["L3"]))
    );
    // The open quote takes the rest of the file into the last field, which
    // gives the record the header's width; it is still malformed.
    let open = "Sample,Analyte,Value\nC1,Morphine,1.5\nC2,Codeine,\"2.0\nC3,Fentanyl,3.0\n";
    assert_eq!(
        paths(open, &layout()).unwrap(),
        (owned(&["L2"]), owned(&["L3"]))
    );
    let closed = "Sample,Analyte,Value\nC1,Morphine,\"1.5\"\nC2,Codeine,\"2.0\"";
    assert_eq!(paths(closed, &layout()).unwrap().0, ["L2", "L3"]);
    let swallowed = format!("{HEADER}\nC1,M,\"1.5\n{}", "C2,M,2.0\n".repeat(600));
    assert_eq!(
        paths(&swallowed, &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
}

#[test]
fn row_limit_counts_result_rows() {
    let file = |rows: usize| format!("{HEADER}\n{}", "C1,Morphine,1.5\n".repeat(rows));
    assert_eq!(
        extract_file(file(MAX_FILE_ROWS).as_bytes(), &layout())
            .unwrap()
            .rows
            .len(),
        MAX_FILE_ROWS
    );
    assert_eq!(
        paths(&file(MAX_FILE_ROWS + 1), &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
    let mut wide = layout();
    wide.shape = FileShape::Wide {
        value_columns: vec![
            ValueColumn {
                column: 1,
                analyte_name: "Morphine".to_owned(),
            },
            ValueColumn {
                column: 2,
                analyte_name: "Codeine".to_owned(),
            },
        ],
    };
    let records = |count: usize| format!("{HEADER}\n{}", "C1,1.5,2.5\n".repeat(count));
    assert!(extract_file(records(MAX_FILE_ROWS / 2).as_bytes(), &wide).is_ok());
    assert_eq!(
        extract_file(records(MAX_FILE_ROWS / 2 + 1).as_bytes(), &wide),
        Err(FileExtractFailure::LimitsExceeded)
    );
}

#[test]
fn column_and_field_limits() {
    let wide_record = format!("{HEADER}\n{}\n", vec!["x"; MAX_FILE_COLUMNS + 1].join(","));
    assert_eq!(
        paths(&wide_record, &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
    let at_limit = format!("{HEADER}\n{}\n", vec!["x"; MAX_FILE_COLUMNS].join(","));
    assert_eq!(paths(&at_limit, &layout()).unwrap().1, ["L2"]);
    let field = |bytes: usize| format!("{HEADER}\nC1,Morphine,{}\n", "9".repeat(bytes));
    assert_eq!(paths(&field(MAX_FIELD_BYTES), &layout()).unwrap().0, ["L2"]);
    assert_eq!(
        paths(&field(MAX_FIELD_BYTES + 1), &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
}

// L3 of the wave-10 review: the width is checked on the text before a record is
// split, so a hostile line never becomes millions of cells. Delimiters inside quotes
// separate nothing; a record continued over quoted line breaks counts as one.
#[test]
fn a_too_wide_line_is_refused_before_it_is_split() {
    let hostile = format!("{HEADER}\nC1,Morphine,1.5\n{}\n", ",".repeat(5_000_000));
    assert_eq!(
        paths(&hostile, &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
    let quoted = format!(
        "{HEADER}\nC1,\"{}\",1.5\n",
        ",".repeat(MAX_FILE_COLUMNS * 2)
    );
    assert_eq!(paths(&quoted, &layout()).unwrap().0, ["L2"]);
    let half = vec!["x"; MAX_FILE_COLUMNS / 2 + 1].join(",");
    let continued = format!("{HEADER}\n{half},\"a\nb\",{half}\n");
    assert_eq!(
        paths(&continued, &layout()),
        Err(FileExtractFailure::LimitsExceeded)
    );
    // A wide preamble line can never be the header: it is skipped, not split.
    let preamble = format!(
        "{}\n{HEADER}\nC1,Morphine,1.5\n",
        ",".repeat(MAX_FILE_COLUMNS)
    );
    assert_eq!(paths(&preamble, &layout()).unwrap().0, ["L3"]);
    assert_eq!(
        preview_lines(
            preamble.as_bytes(),
            FileEncoding::Utf8,
            FileDelimiter::Comma
        ),
        Err(FileExtractFailure::LimitsExceeded)
    );
}

fn censored(comparator: Comparator, bound: &str) -> FileValueReading {
    FileValueReading::Value(SourceValue::Censored {
        comparator,
        bound: DecimalText::parse(bound).unwrap(),
    })
}

#[test]
fn value_rules_apply_words_then_empty_then_the_strict_reader_then_the_zero_rule() {
    let words = [
        ValueWord {
            word: "N/F".to_owned(),
            meaning: ValueWordMeaning::BelowLoq,
        },
        ValueWord {
            word: "<0".to_owned(),
            meaning: ValueWordMeaning::BelowLod,
        },
        ValueWord {
            word: "NP".to_owned(),
            meaning: ValueWordMeaning::NotPerformed,
        },
    ];
    let quantity = |text: &str| {
        FileValueReading::Value(SourceValue::Quantity(DecimalText::parse(text).unwrap()))
    };
    let cases = [
        ("N/F", FileValueReading::Word(ValueWordMeaning::BelowLoq)),
        ("<0", FileValueReading::Word(ValueWordMeaning::BelowLod)),
        ("N/A", FileValueReading::UnmappedWord),
        ("degenerate", FileValueReading::UnmappedWord),
        ("n/f", FileValueReading::UnmappedWord),
        ("", FileValueReading::Empty),
        (">0", FileValueReading::ZeroOrNegative),
        ("<=0", FileValueReading::ZeroOrNegative),
        ("<-1", FileValueReading::ZeroOrNegative),
        ("0", FileValueReading::ZeroOrNegative),
        ("0.000000", FileValueReading::ZeroOrNegative),
        ("-0", FileValueReading::ZeroOrNegative),
        ("-1.2", FileValueReading::ZeroOrNegative),
        ("<0.50", censored(Comparator::Lt, "0.50")),
        (">1000", censored(Comparator::Gt, "1000")),
        ("152.40", quantity("152.40")),
        ("0.01", quantity("0.01")),
        (
            "1,234.5",
            FileValueReading::Malformed(MalformedValue::NotADecimal),
        ),
        (
            "1.2E+02",
            FileValueReading::Malformed(MalformedValue::NotADecimal),
        ),
        (
            " 12",
            FileValueReading::Malformed(MalformedValue::NotADecimal),
        ),
        (
            "79228162514264337593543950336",
            FileValueReading::Malformed(MalformedValue::DecimalOutOfRange),
        ),
    ];
    for (text, expected) in cases {
        assert_eq!(
            read_value(text, &words, OutputScale::Quantitative),
            expected,
            "{text:?}"
        );
    }
}

#[test]
fn a_qualitative_method_takes_only_not_performed_and_keeps_other_text_as_its_code() {
    let words = [
        ValueWord {
            word: "N/F".to_owned(),
            meaning: ValueWordMeaning::BelowLoq,
        },
        ValueWord {
            word: "NP".to_owned(),
            meaning: ValueWordMeaning::NotPerformed,
        },
    ];
    let code = |text: &str| FileValueReading::Value(SourceValue::Qualitative(text.to_owned()));
    for (text, expected) in [
        ("NP", FileValueReading::Word(ValueWordMeaning::NotPerformed)),
        ("N/F", code("N/F")),
        ("NEG", code("NEG")),
        ("0", code("0")),
        ("<0", code("<0")),
        ("", FileValueReading::Empty),
    ] {
        assert_eq!(
            read_value(text, &words, OutputScale::Qualitative),
            expected,
            "{text:?}"
        );
    }
}

/// The records of `text` as read: (line, fields, terminated).
fn records(text: &str, layout: &FileLayout) -> Vec<(usize, Vec<String>, bool)> {
    let mut out = Vec::new();
    read_data(text, layout, |record| {
        out.push((
            record.line,
            record.fields.iter().map(str::to_owned).collect(),
            record.terminated,
        ));
        Ok(())
    })
    .unwrap();
    out
}

fn written(
    rows: &[Vec<String>],
    delimiter: FileDelimiter,
    style: csv::QuoteStyle,
    crlf: bool,
) -> String {
    let terminator = if crlf {
        csv::Terminator::CRLF
    } else {
        csv::Terminator::Any(b'\n')
    };
    let mut writer = csv::WriterBuilder::new()
        .delimiter(delimiter.byte())
        .quote_style(style)
        .terminator(terminator)
        .flexible(true)
        .from_writer(Vec::new());
    for row in rows {
        writer.write_record(row).unwrap();
    }
    String::from_utf8(writer.into_inner().unwrap()).unwrap()
}

proptest! {
    // §10: any generated table, written with any supported delimiter and
    // quoting, reads back to the same cells, with rising line numbers.
    #[test]
    fn written_tables_read_back_to_the_same_cells(
        header in prop::collection::vec("[A-Za-z ,;\t\"]{1,8}", 1..6),
        rows in prop::collection::vec(
            ("[A-Z][0-9]{1,5}", prop::collection::vec("[a-z0-9 ,;\t\"\r\n.<>-]{0,10}", 0..7)),
            0..12,
        ),
        delimiter in prop::sample::select(vec![
            FileDelimiter::Comma, FileDelimiter::Tab, FileDelimiter::Semicolon,
        ]),
        style in prop::sample::select(vec![
            csv::QuoteStyle::Necessary, csv::QuoteStyle::Always, csv::QuoteStyle::NonNumeric,
        ]),
        crlf in any::<bool>(),
    ) {
        let rows: Vec<Vec<String>> = rows
            .into_iter()
            .map(|(first, rest)| std::iter::once(first).chain(rest).collect())
            .collect();
        let mut layout = layout();
        layout.delimiter = delimiter;
        layout.header = vec![header.clone()];
        let table: Vec<Vec<String>> = std::iter::once(header).chain(rows.iter().cloned()).collect();
        let read = records(&written(&table, delimiter, style, crlf), &layout);
        let cells: Vec<Vec<String>> = read.iter().map(|(_, cells, _)| cells.clone()).collect();
        prop_assert_eq!(cells, rows);
        prop_assert!(read.iter().all(|(_, _, terminated)| *terminated));
        prop_assert!(read.windows(2).all(|pair| pair[0].0 < pair[1].0));
    }

    #[test]
    fn arbitrary_bytes_never_panic(bytes in prop::collection::vec(any::<u8>(), 0..512)) {
        let mut with_header = HEADER.as_bytes().to_vec();
        with_header.push(b'\n');
        with_header.extend(&bytes);
        for input in [&bytes, &with_header] {
            let _ = extract_file(input, &layout());
            let _ = check_layout(input, &layout(), &[]);
            let _ = preview_lines(input, FileEncoding::Windows1252, FileDelimiter::Comma);
            let _ = sample_text(input);
        }
    }
}
