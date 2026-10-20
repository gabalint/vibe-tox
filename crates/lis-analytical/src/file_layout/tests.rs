//! Table tests of [`FileLayout::validate`] (brief §13) and of the stored
//! JSON shape.

use super::*;

/// A valid long layout: `Sample Name,Sample Type,Component,Final Conc,Units,Dilution`.
fn long() -> FileLayout {
    FileLayout {
        encoding: FileEncoding::Utf8,
        delimiter: FileDelimiter::Comma,
        trim_fields: false,
        header: vec![
            [
                "Sample Name",
                "Sample Type",
                "Component",
                "Final Conc",
                "Units",
                "Dilution",
            ]
            .map(str::to_owned)
            .to_vec(),
        ],
        shape: FileShape::Long {
            analyte_column: 2,
            value_column: 3,
        },
        columns: FileColumns {
            sample_label: 0,
            units: Some(4),
            sample_type: Some(1),
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: Some(5),
        },
        end_of_data: EndOfData::EndOfFile,
        sample_types: vec![SampleTypeWord {
            word: "Standard".to_owned(),
            role: PositionRole::Calibrator,
        }],
        value_words: vec![ValueWord {
            word: "N/F".to_owned(),
            meaning: ValueWordMeaning::BelowLoq,
        }],
        not_results: vec!["Morphine-D6".to_owned()],
        declared_unit: None,
    }
}

/// A valid two-row wide layout (MassHunter-like).
fn wide() -> FileLayout {
    let row = |cells: &[&str]| cells.iter().map(|c| (*c).to_owned()).collect::<Vec<_>>();
    FileLayout {
        header: vec![
            row(&[
                "Sample",
                "",
                "",
                "Morphine Results",
                "",
                "Fentanyl Results",
                "",
            ]),
            row(&[
                "Name",
                "Type",
                "Dil.",
                "RT",
                "Final Conc.",
                "RT",
                "Final Conc.",
            ]),
        ],
        shape: FileShape::Wide {
            value_columns: vec![
                ValueColumn {
                    column: 4,
                    analyte_name: "Morphine Results".to_owned(),
                },
                ValueColumn {
                    column: 6,
                    analyte_name: "Fentanyl Results".to_owned(),
                },
            ],
        },
        columns: FileColumns {
            sample_label: 0,
            units: None,
            sample_type: Some(1),
            flags: None,
            operator: None,
            acquired_at: None,
            dilution: Some(2),
        },
        not_results: Vec::new(),
        declared_unit: Some("ng/mL".to_owned()),
        ..long()
    }
}

fn code(layout: &FileLayout) -> Option<(String, String)> {
    layout
        .validate()
        .err()
        .map(|error| (error.field, error.code))
}

#[test]
fn valid_long_and_wide_layouts_pass() {
    assert_eq!(code(&long()), None);
    assert_eq!(code(&wide()), None);
}

/// A change to a valid layout, and the field and code it must fail with.
type Case = (
    &'static str,
    fn(&mut FileLayout),
    &'static str,
    &'static str,
);

fn assert_cases(base: fn() -> FileLayout, cases: Vec<Case>) {
    for (case, change, field, expected) in cases {
        let mut layout = base();
        change(&mut layout);
        assert_eq!(
            code(&layout),
            Some((field.to_owned(), expected.to_owned())),
            "{case}"
        );
    }
}

fn header_cases() -> Vec<Case> {
    vec![
        ("no header", |l| l.header.clear(), "layout.header", "rows"),
        (
            "three rows",
            |l| l.header = vec![vec!["a".into()]; 3],
            "layout.header",
            "rows",
        ),
        (
            "empty row",
            |l| l.header = vec![Vec::new()],
            "layout.header",
            "width",
        ),
        (
            "513 columns",
            |l| l.header = vec![vec![String::new(); MAX_FILE_COLUMNS + 1]],
            "layout.header",
            "width",
        ),
        (
            "unequal rows",
            |l| l.header.push(vec!["x".into()]),
            "layout.header",
            "unequal_rows",
        ),
        (
            "line break in a cell",
            |l| l.header[0][0] = "Sample\nName".into(),
            "layout.header",
            "line_break",
        ),
        (
            "cell over a field",
            |l| l.header[0][0] = "x".repeat(MAX_FIELD_BYTES + 1),
            "layout.header",
            "too_long",
        ),
    ]
}

fn column_cases() -> Vec<Case> {
    vec![
        (
            "label out of range",
            |l| l.columns.sample_label = 6,
            "layout.columns.sampleLabel",
            "out_of_range",
        ),
        (
            "units on the label",
            |l| l.columns.units = Some(0),
            "layout.columns.units",
            "duplicate",
        ),
        (
            "analyte column is the value column",
            |l| {
                l.shape = FileShape::Long {
                    analyte_column: 3,
                    value_column: 3,
                }
            },
            "layout.shape.valueColumn",
            "duplicate",
        ),
        (
            "analyte column on a role",
            |l| {
                l.shape = FileShape::Long {
                    analyte_column: 1,
                    value_column: 3,
                }
            },
            "layout.shape.analyteColumn",
            "duplicate",
        ),
    ]
}

fn word_cases() -> Vec<Case> {
    vec![
        (
            "empty value word",
            |l| l.value_words[0].word = String::new(),
            "layout.valueWords",
            "required",
        ),
        (
            "repeated value word",
            |l| l.value_words.push(l.value_words[0].clone()),
            "layout.valueWords",
            "duplicate",
        ),
        (
            "repeated type word",
            |l| l.sample_types.push(l.sample_types[0].clone()),
            "layout.sampleTypes",
            "duplicate",
        ),
        (
            "type words without their column",
            |l| l.columns.sample_type = None,
            "layout.sampleTypes",
            "not_applicable",
        ),
        (
            "empty not-a-result",
            |l| l.not_results.push(String::new()),
            "layout.notResults",
            "required",
        ),
        (
            "repeated not-a-result",
            |l| l.not_results.push("Morphine-D6".into()),
            "layout.notResults",
            "duplicate",
        ),
        (
            "value word that reads as a positive quantity",
            |l| l.value_words[0].word = "5".into(),
            "layout.valueWords",
            "reads_as_value",
        ),
        (
            "value word that reads as a censored value",
            |l| l.value_words[0].word = "<10".into(),
            "layout.valueWords",
            "reads_as_value",
        ),
        (
            "value word that reads as a censored value with a two-character comparator",
            |l| l.value_words[0].word = ">=0.5".into(),
            "layout.valueWords",
            "reads_as_value",
        ),
        (
            "untrimmed value word while trimming",
            |l| {
                l.trim_fields = true;
                l.value_words[0].word = " N/F".into();
            },
            "layout.valueWords",
            "untrimmed",
        ),
        (
            "untrimmed type word while trimming",
            |l| {
                l.trim_fields = true;
                l.sample_types[0].word = "Standard ".into();
            },
            "layout.sampleTypes",
            "untrimmed",
        ),
        (
            "declared unit beside a units column",
            |l| l.declared_unit = Some("ng/mL".into()),
            "layout.declaredUnit",
            "exclusive",
        ),
    ]
}

fn wide_columns(layout: &mut FileLayout) -> &mut Vec<ValueColumn> {
    match &mut layout.shape {
        FileShape::Wide { value_columns } => value_columns,
        FileShape::Long { .. } => unreachable!("a wide layout"),
    }
}

fn wide_cases() -> Vec<Case> {
    vec![
        (
            "no analyte column",
            |l| wide_columns(l).clear(),
            "layout.shape.valueColumns",
            "required",
        ),
        (
            "repeated column",
            |l| wide_columns(l)[1].column = 4,
            "layout.shape.valueColumns[1].column",
            "duplicate",
        ),
        (
            "column out of range",
            |l| wide_columns(l)[1].column = 7,
            "layout.shape.valueColumns[1].column",
            "out_of_range",
        ),
        (
            "value column on the label",
            |l| wide_columns(l)[0].column = 0,
            "layout.shape.valueColumns[0].column",
            "duplicate",
        ),
        (
            "repeated analyte name",
            |l| wide_columns(l)[1].analyte_name = "Morphine Results".into(),
            "layout.shape.valueColumns",
            "duplicate",
        ),
        (
            "empty analyte name",
            |l| wide_columns(l)[0].analyte_name = String::new(),
            "layout.shape.valueColumns",
            "required",
        ),
        (
            "analyte name other than the group label",
            |l| wide_columns(l)[0].analyte_name = "Morphine".into(),
            "layout.shape.valueColumns[0].analyteName",
            "not_header_text",
        ),
        (
            "analyte name of another group",
            |l| {
                wide_columns(l)[0].analyte_name = "Fentanyl Results".into();
                wide_columns(l)[1].analyte_name = "Morphine Results".into();
            },
            "layout.shape.valueColumns[0].analyteName",
            "not_header_text",
        ),
        (
            "a group read at another field",
            |l| wide_columns(l)[1].column = 5,
            "layout.shape.valueColumns[1].column",
            "other_value_field",
        ),
        (
            "not-a-result list on a wide file",
            |l| l.not_results = vec!["Morphine-D3".into()],
            "layout.notResults",
            "not_applicable",
        ),
        (
            "blank unit",
            |l| l.declared_unit = Some("  ".into()),
            "layout.declaredUnit",
            "required",
        ),
        (
            "untrimmed unit",
            |l| l.declared_unit = Some(" ng/mL".into()),
            "layout.declaredUnit",
            "untrimmed",
        ),
    ]
}

#[test]
fn each_structural_error_names_its_field_and_code() {
    assert_cases(long, header_cases());
    assert_cases(long, column_cases());
    assert_cases(long, word_cases());
    assert_cases(wide, wide_cases());
}

// F7, L2 of the wave-10 review: zero, negative and non-numeric words stay
// mappable (`<0`, `>0`, `0`, `-1.2`, `N/F`); text the strict reader reads
// differently (`1,234.5`, `1E2`, ` 5` untrimmed) is not a positive value.
#[test]
fn words_the_reader_does_not_read_as_positive_values_can_be_mapped() {
    for word in [
        "<0", ">0", "0", "0.000", "-1.2", "<-1", "N/F", "1,234.5", "1E2", " 5", "<",
    ] {
        let mut layout = long();
        layout.value_words[0].word = word.to_owned();
        assert_eq!(code(&layout), None, "{word}");
    }
    let mut trimmed = long();
    trimmed.value_words[0].word = " N/F".to_owned();
    assert_eq!(code(&trimmed), None, "spaces are kept without trimming");
}

// M2 of the wave-10 review: a one-row wide header names each analyte column
// with its own cell; trimming applies to the comparison.
#[test]
fn a_one_row_wide_header_names_each_analyte_with_its_cell() {
    let mut layout = wide();
    layout.header = vec![
        ["Name", "Type", "Dil.", "RT", " Morphine ", "RT", "Fentanyl"]
            .map(str::to_owned)
            .to_vec(),
    ];
    wide_columns(&mut layout)[0].analyte_name = " Morphine ".into();
    wide_columns(&mut layout)[1].analyte_name = "Fentanyl".into();
    assert_eq!(code(&layout), None);
    assert_eq!(layout.analyte_label(4), Some(" Morphine "));
    layout.trim_fields = true;
    assert_eq!(
        code(&layout),
        Some((
            "layout.shape.valueColumns".to_owned(),
            "untrimmed".to_owned()
        ))
    );
    wide_columns(&mut layout)[0].analyte_name = "Morphine".into();
    assert_eq!(code(&layout), None);
    wide_columns(&mut layout)[1].analyte_name = "RT".into();
    assert_eq!(
        code(&layout),
        Some((
            "layout.shape.valueColumns[1].analyteName".to_owned(),
            "not_header_text".to_owned()
        ))
    );
}

#[test]
fn a_group_label_is_carried_forward_to_the_empty_cells_after_it() {
    let layout = wide();
    let labels: Vec<_> = (0..7).map(|column| layout.analyte_label(column)).collect();
    assert_eq!(
        labels,
        [
            Some("Sample"),
            Some("Sample"),
            Some("Sample"),
            Some("Morphine Results"),
            Some("Morphine Results"),
            Some("Fentanyl Results"),
            Some("Fentanyl Results"),
        ]
    );
    let mut unlabelled = wide();
    unlabelled.header[0][0] = String::new();
    assert_eq!(unlabelled.analyte_label(1), None);
}

#[test]
fn empty_header_cells_are_part_of_the_signature() {
    let mut layout = long();
    layout.header[0][5] = String::new();
    assert_eq!(code(&layout), None);
}

#[test]
fn layout_json_is_camel_case_with_snake_case_values_and_refuses_unknown_fields() {
    let json = serde_json::to_value(wide()).unwrap();
    assert_eq!(json["shape"]["kind"], "wide");
    assert_eq!(
        json["shape"]["valueColumns"][0]["analyteName"],
        "Morphine Results"
    );
    assert_eq!(json["endOfData"], "end_of_file");
    assert_eq!(json["valueWords"][0]["meaning"], "below_loq");
    assert_eq!(json["sampleTypes"][0]["role"], "calibrator");
    assert_eq!(json["encoding"], "utf8");
    let back: FileLayout = serde_json::from_value(json.clone()).unwrap();
    assert_eq!(back, wide());

    let mut extra = json.clone();
    extra["columns"]["reviewer"] = 3.into();
    assert!(serde_json::from_value::<FileLayout>(extra).is_err());
    let mut extra = json;
    extra["shape"]["groupRow"] = 0.into();
    assert!(serde_json::from_value::<FileLayout>(extra).is_err());
}
