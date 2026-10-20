//! The synthetic result files in fixtures/files/ read to the rows written
//! here by hand from the files (result files brief §5, §10; REQ-INT-005,
//! REQ-RES-001). Synthetic data only.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

use lis_analytical::{
    EndOfData, FileColumns, FileDelimiter, FileEncoding, FileLayout, FileShape, PositionRole,
    SampleTypeWord, SourceTestCodeInput, ValueColumn, ValueWord, ValueWordMeaning,
};
use lis_catalog::AnalyteId;
use lis_results::astm::ExtractedImport;
use lis_results::delimited::{
    FileExtractFailure, ReadingKind, check_layout, extract_file, preview_lines, sample_text,
};

macro_rules! fixture {
    ($name:literal) => {
        include_bytes!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../fixtures/files/",
            $name
        ))
    };
}

fn cells(row: &[&str]) -> Vec<String> {
    row.iter().map(|cell| (*cell).to_owned()).collect()
}

/// A long layout over `header`, with the label, analyte and value columns.
fn long(header: &[&str], label: u16, analyte: u16, value: u16) -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![cells(header)],
        shape: FileShape::Long {
            analyte_column: analyte,
            value_column: value,
        },
        columns: FileColumns {
            sample_label: label,
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

fn extract(bytes: &[u8], layout: &FileLayout) -> ExtractedImport {
    layout.validate().unwrap();
    let import = extract_file(bytes, layout).unwrap();
    assert_eq!(import.sender_name, None);
    assert_eq!(import.message_time_text, None);
    import
}

/// (reference, label, analyte, value) per row; absent texts are `-`.
fn rows(import: &ExtractedImport) -> Vec<[&str; 4]> {
    fn text(value: &Option<String>) -> &str {
        value.as_deref().unwrap_or("-")
    }
    import
        .rows
        .iter()
        .map(|row| {
            [
                row.record_path.as_str(),
                text(&row.sample_label),
                text(&row.test_code),
                text(&row.value_text),
            ]
        })
        .collect()
}

const SCIEX_HEADER: [&str; 8] = [
    "Sample Name",
    "Sample ID",
    "Sample Type",
    "Component Name",
    "Calculated Concentration",
    "Conc. Units",
    "Dilution Factor",
    "Ion Ratio",
];

fn sciex() -> FileLayout {
    let mut layout = long(&SCIEX_HEADER, 0, 3, 4);
    layout.columns.units = Some(5);
    layout.columns.sample_type = Some(2);
    layout.columns.dilution = Some(6);
    layout
}

#[test]
fn sciex_long_file_keeps_words_and_comparators_as_received() {
    let import = extract(fixture!("sciex_long.csv"), &sciex());
    assert_eq!(
        rows(&import),
        [
            ["L2", "C00000123", "Morphine", "152.4"],
            ["L3", "C00000123", "Morphine-D3", "N/A"],
            ["L4", "C00000123", "Fentanyl", "N/A"],
            ["L5", "C00000124", "Morphine", "<0"],
            ["L6", "C00000124", "Fentanyl", ">0"],
            ["L7", "QC-LOW-L1234", "Morphine", "48.9"],
            ["L8", "CAL-1", "Morphine", "10.2"],
            ["L9", "CAL-2", "Fentanyl", "degenerate"],
        ]
    );
    let first = &import.rows[0];
    assert_eq!(first.units_text.as_deref(), Some("ng/mL"));
    assert_eq!(first.sample_type_text.as_deref(), Some("Unknown"));
    assert_eq!(first.dilution_text.as_deref(), Some("1"));
    assert_eq!(first.status_text, None);
    assert_eq!(import.rows[4].dilution_text.as_deref(), Some("10"));
    assert!(import.malformed_rows.is_empty());
}

#[test]
fn tracefinder_long_file_reads_its_type_column_and_nf_words() {
    let mut layout = long(
        &[
            "Filename",
            "Sample ID",
            "Sample Type",
            "Compound",
            "Calculated Amt",
            "Theoretical Amt",
            "Response Ratio",
            "Excluded",
        ],
        1,
        3,
        4,
    );
    layout.columns.sample_type = Some(2);
    layout.declared_unit = Some("ng/mL".to_owned());
    let import = extract(fixture!("tracefinder_long.csv"), &layout);
    assert_eq!(
        rows(&import),
        [
            ["L2", "C00000125", "Oxycodone", "85.20"],
            ["L3", "C00000125", "Oxymorphone", "N/F"],
            ["L4", "C00000125", "Oxycodone-D6", "N/F"],
            ["L5", "QC-HIGH-L5678", "Oxycodone", "412.0"],
            ["L6", "CAL-3", "Oxycodone", "49.6"],
        ]
    );
    let types: Vec<_> = import
        .rows
        .iter()
        .map(|row| row.sample_type_text.as_deref().unwrap())
        .collect();
    assert_eq!(
        types,
        ["Unknown", "Unknown", "Unknown", "QC Std", "Cal Std"]
    );
    // A declared unit is the importer's to apply; the row keeps the file's (none).
    assert_eq!(import.rows[0].units_text, None);
}

fn masshunter() -> FileLayout {
    let mut layout = long(&[], 2, 0, 0);
    layout.header = vec![
        cells(&[
            "",
            "",
            "Sample",
            "",
            "",
            "",
            "",
            "Morphine Results",
            "",
            "",
            "Qualifier (286.1 -> 165.1) Results",
            "Morphine-D6 (ISTD) Results",
            "",
            "Fentanyl Results",
            "",
        ]),
        cells(&[
            "",
            "",
            "Name",
            "Data File",
            "Type",
            "Dil.",
            "Acq. Date-Time",
            "RT",
            "Final Conc.",
            "MI",
            "Ratio",
            "RT",
            "Area",
            "RT",
            "Final Conc.",
        ]),
    ];
    layout.shape = FileShape::Wide {
        value_columns: vec![
            ValueColumn {
                column: 8,
                analyte_name: "Morphine Results".to_owned(),
            },
            ValueColumn {
                column: 14,
                analyte_name: "Fentanyl Results".to_owned(),
            },
        ],
    };
    layout.columns.sample_type = Some(4);
    layout.columns.dilution = Some(5);
    layout.columns.acquired_at = Some(6);
    layout
}

#[test]
fn masshunter_wide_file_gives_one_row_per_sample_and_value_group() {
    let import = extract(fixture!("masshunter_wide.csv"), &masshunter());
    assert_eq!(
        rows(&import),
        [
            [
                "L3/Morphine Results",
                "C00000123",
                "Morphine Results",
                "152.4"
            ],
            ["L3/Fentanyl Results", "C00000123", "Fentanyl Results", "-"],
            [
                "L4/Morphine Results",
                "C00000126",
                "Morphine Results",
                "1520.5"
            ],
            [
                "L4/Fentanyl Results",
                "C00000126",
                "Fentanyl Results",
                "2.07"
            ],
            [
                "L5/Morphine Results",
                "QC-LOW-L1234",
                "Morphine Results",
                "48.9"
            ],
            [
                "L5/Fentanyl Results",
                "QC-LOW-L1234",
                "Fentanyl Results",
                "1.98"
            ],
            ["L6/Morphine Results", "CAL-1", "Morphine Results", "10.2"],
            ["L6/Fentanyl Results", "CAL-1", "Fentanyl Results", "0.51"],
            ["L7/Morphine Results", "BLANK-1", "Morphine Results", "-"],
            ["L7/Fentanyl Results", "BLANK-1", "Fentanyl Results", "-"],
        ]
    );
    let row = &import.rows[2];
    assert_eq!(row.dilution_text.as_deref(), Some("10"));
    assert_eq!(row.sample_type_text.as_deref(), Some("Sample"));
    assert_eq!(row.completed_text.as_deref(), Some("10/12/2026 9:22 AM"));
}

#[test]
fn changed_group_label_or_field_fails_the_masshunter_file() {
    for (row, column, text) in [
        (0, 7, "Morphine Result"),
        (1, 8, "Calc. Conc."),
        (0, 0, "!"),
    ] {
        let mut layout = masshunter();
        layout.header[row][column] = text.to_owned();
        assert_eq!(
            extract_file(fixture!("masshunter_wide.csv"), &layout),
            Err(FileExtractFailure::LayoutMismatch),
            "{text}"
        );
    }
}

#[test]
fn one_row_wide_tab_file() {
    let mut layout = long(
        &[
            "Sample Name",
            "Sample Type",
            "Morphine",
            "Codeine",
            "Fentanyl",
        ],
        0,
        0,
        0,
    );
    layout.delimiter = FileDelimiter::Tab;
    layout.shape = FileShape::Wide {
        value_columns: ["Morphine", "Codeine", "Fentanyl"]
            .iter()
            .zip(2..)
            .map(|(name, column)| ValueColumn {
                column,
                analyte_name: (*name).to_owned(),
            })
            .collect(),
    };
    let import = extract(fixture!("wide_one_row.txt"), &layout);
    assert_eq!(
        rows(&import),
        [
            ["L2/Morphine", "C00000127", "Morphine", "12.5"],
            ["L2/Codeine", "C00000127", "Codeine", "-"],
            ["L2/Fentanyl", "C00000127", "Fentanyl", "0.82"],
            ["L3/Morphine", "QC-LOW-L1234", "Morphine", "49.1"],
            ["L3/Codeine", "QC-LOW-L1234", "Codeine", "51.3"],
            ["L3/Fentanyl", "QC-LOW-L1234", "Fentanyl", "2.01"],
        ]
    );
}

#[test]
fn windows_1252_is_read_only_when_the_template_says_so() {
    let bytes = fixture!("windows1252_ugl.csv");
    let mut layout = long(&["Sample", "Analyte", "Result", "Units", "Flags"], 0, 1, 2);
    layout.columns.units = Some(3);
    layout.columns.flags = Some(4);
    assert_eq!(
        extract_file(bytes, &layout),
        Err(FileExtractFailure::UndecodableText)
    );
    layout.encoding = FileEncoding::Windows1252;
    let import = extract(bytes, &layout);
    assert_eq!(import.rows[0].units_text.as_deref(), Some("µg/L"));
    assert_eq!(import.rows[1].value_text.as_deref(), Some("<5.0"));
    assert_eq!(
        import.rows[1].flags_text.as_deref(),
        Some("Ion ratio \u{2013} low")
    );
    assert_eq!(import.rows[0].flags_text, None);
}

#[test]
fn a_utf8_byte_order_mark_is_stripped_and_decides_utf8() {
    let mut layout = long(
        &[
            "Sample Name",
            "Component Name",
            "Calculated Concentration",
            "Conc. Units",
        ],
        0,
        1,
        2,
    );
    // The mark decides, whatever the template says.
    layout.encoding = FileEncoding::Windows1252;
    let import = extract(fixture!("utf8_bom.csv"), &layout);
    assert_eq!(
        rows(&import),
        [["L2", "C00000129", "Methamphetamine", "203.7"]]
    );
    let sample = sample_text(fixture!("utf8_bom.csv")).unwrap();
    assert_eq!(sample.bom_encoding, Some(FileEncoding::Utf8));
    assert!(sample.valid_utf8);
    assert_eq!(
        sample.lines[0],
        "Sample Name,Component Name,Calculated Concentration,Conc. Units"
    );
}

#[test]
fn a_utf16_file_is_refused_everywhere() {
    let bytes = fixture!("utf16le_bom.txt");
    let mut layout = long(
        &["Sample Name", "Component Name", "Calculated Concentration"],
        0,
        1,
        2,
    );
    layout.delimiter = FileDelimiter::Tab;
    assert_eq!(
        extract_file(bytes, &layout),
        Err(FileExtractFailure::UndecodableText)
    );
    assert_eq!(sample_text(bytes), Err(FileExtractFailure::UndecodableText));
    assert_eq!(
        preview_lines(bytes, FileEncoding::Utf8, FileDelimiter::Tab),
        Err(FileExtractFailure::UndecodableText)
    );
    assert_eq!(
        check_layout(bytes, &layout, &[]).failure,
        Some(FileExtractFailure::UndecodableText)
    );
}

fn four_columns() -> FileLayout {
    let mut layout = long(
        &[
            "Sample Name",
            "Component Name",
            "Calculated Concentration",
            "Conc. Units",
        ],
        0,
        1,
        2,
    );
    layout.columns.units = Some(3);
    layout
}

#[test]
fn ragged_rows_are_listed_as_malformed_and_the_rest_read() {
    let import = extract(fixture!("ragged_row.csv"), &four_columns());
    assert_eq!(
        rows(&import),
        [
            ["L2", "C00000131", "Morphine", "22.1"],
            ["L5", "C00000131", "Hydromorphone", "3.2"],
        ]
    );
    assert_eq!(import.malformed_rows, ["L3", "L4"]);
}

#[test]
fn the_header_is_found_after_preambles_of_any_length() {
    let mut layout = long(&["Sample", "Peak Name", "Amount", "Unit"], 0, 1, 2);
    layout.delimiter = FileDelimiter::Semicolon;
    layout.columns.units = Some(3);
    let short = extract(fixture!("preamble_short.csv"), &layout);
    assert_eq!(
        rows(&short),
        [
            ["L4", "C00000132", "Cotinine", "310.5"],
            ["L5", "C00000132", "Nicotine", "n.a."],
        ]
    );
    let long_preamble = extract(fixture!("preamble_long.csv"), &layout);
    assert_eq!(
        rows(&long_preamble),
        [
            ["L14", "C00000132", "Cotinine", "310.5"],
            ["L15", "C00000132", "Nicotine", "n.a."],
        ]
    );
    let preview = preview_lines(
        fixture!("preamble_long.csv"),
        FileEncoding::Utf8,
        FileDelimiter::Semicolon,
    )
    .unwrap();
    assert_eq!(preview.len(), 15);
    assert_eq!(preview[12].line, 13);
    assert_eq!(preview[12].cells, ["Sample", "Peak Name", "Amount", "Unit"]);
    assert_eq!(preview[1].cells, Vec::<String>::new());
    assert_eq!(
        preview[5].cells,
        ["Comment", "operator note without a closing quote"]
    );
}

#[test]
fn a_footer_after_a_blank_line_ends_the_data_when_the_template_says_so() {
    let bytes = fixture!("footer_after_blank.csv");
    let mut layout = four_columns();
    let to_end = extract(bytes, &layout);
    assert_eq!(to_end.rows.len(), 2);
    assert_eq!(to_end.malformed_rows, ["L5", "L6"]);
    layout.end_of_data = EndOfData::FirstBlankLine;
    let to_blank = extract(bytes, &layout);
    assert_eq!(
        rows(&to_blank),
        [
            ["L2", "C00000133", "Morphine", "61.0"],
            ["L3", "C00000133", "Codeine", "14.2"],
        ]
    );
    assert!(to_blank.malformed_rows.is_empty());
}

#[test]
fn quoted_fields_keep_commas_doubled_quotes_and_line_breaks() {
    let mut layout = four_columns();
    layout.header[0].push("Flags".to_owned());
    layout.columns.flags = Some(4);
    let import = extract(fixture!("quoted_fields.csv"), &layout);
    assert_eq!(
        rows(&import),
        [
            ["L2", "C00000134", "Morphine, total", "88.0"],
            ["L3", "C00000134", "Codeine", "12.0"],
            ["L4", "C00000134", "Hydrocodone", "4.4"],
            ["L6", "C00000134", "Oxycodone", "9.1"],
        ]
    );
    let flags: Vec<_> = import
        .rows
        .iter()
        .map(|row| row.flags_text.as_deref())
        .collect();
    assert_eq!(
        flags,
        [
            None,
            Some("Ion ratio \"high\""),
            Some("Peak split\r\nmanual integration"),
            None
        ]
    );
}

#[test]
fn the_check_maps_names_types_and_words_and_counts_each_reading() {
    let mut layout = sciex();
    layout.not_results = vec!["Morphine-D3".to_owned()];
    layout.value_words = vec![ValueWord {
        word: "N/A".to_owned(),
        meaning: ValueWordMeaning::BelowLoq,
    }];
    layout.sample_types = vec![
        SampleTypeWord {
            word: "Unknown".to_owned(),
            role: PositionRole::Patient,
        },
        SampleTypeWord {
            word: "Quality Control".to_owned(),
            role: PositionRole::Control,
        },
    ];
    let morphine = AnalyteId::generate();
    let codes = [SourceTestCodeInput {
        source_test_code: "Morphine".to_owned(),
        analyte_id: morphine,
    }];
    let check = check_layout(fixture!("sciex_long.csv"), &layout, &codes);
    assert_eq!(check.failure, None);
    let readings: Vec<_> = check.rows.iter().map(|row| row.reading).collect();
    assert_eq!(
        readings,
        [
            ReadingKind::Quantity,
            ReadingKind::Word,
            ReadingKind::Word,
            ReadingKind::ZeroOrNegative,
            ReadingKind::ZeroOrNegative,
            ReadingKind::Quantity,
            ReadingKind::Quantity,
            ReadingKind::UnmappedWord,
        ]
    );
    assert_eq!(check.rows[0].analyte_id, Some(morphine));
    assert_eq!(check.rows[0].role, Some(PositionRole::Patient));
    assert!(check.rows[1].not_result);
    assert_eq!(check.rows[2].word_meaning, Some(ValueWordMeaning::BelowLoq));
    assert_eq!(check.rows[6].role, None);
    let counts = &check.counts;
    assert_eq!(
        (
            counts.rows,
            counts.not_results,
            counts.unmapped_analytes,
            counts.untyped
        ),
        (8, 1, 3, 2)
    );
    assert_eq!(
        (
            counts.quantities,
            counts.words,
            counts.zero_or_negative,
            counts.unmapped_words
        ),
        (3, 2, 2, 1)
    );
}

// M2 of the wave-10 review: each check row of a wide file shows the file's own text
// above its column (the group label here) next to the listed analyte name; a long
// file's rows carry the name themselves.
#[test]
fn the_check_shows_the_header_text_of_each_wide_analyte_column() {
    let check = check_layout(fixture!("masshunter_wide.csv"), &masshunter(), &[]);
    let shown: Vec<_> = check
        .rows
        .iter()
        .take(2)
        .map(|row| (row.analyte_name.as_deref(), row.header_text.as_deref()))
        .collect();
    assert_eq!(
        shown,
        [
            (Some("Morphine Results"), Some("Morphine Results")),
            (Some("Fentanyl Results"), Some("Fentanyl Results")),
        ]
    );
    let long = check_layout(fixture!("sciex_long.csv"), &sciex(), &[]);
    assert!(!long.rows.is_empty());
    assert!(long.rows.iter().all(|row| row.header_text.is_none()));
}
