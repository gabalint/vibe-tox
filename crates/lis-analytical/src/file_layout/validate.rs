//! [`FileLayout::validate`]: the one check of a layout's structure (brief
//! F3–F10, §13). Field names in its errors are relative to the command input
//! (`layout.…`).

use std::collections::BTreeSet;

use lis_core::{Comparator, Decimal, DecimalText, FieldError};

use super::{FileColumns, FileLayout, FileShape, MAX_FIELD_BYTES, MAX_FILE_COLUMNS, ValueColumn};

/// The longest declared unit.
const UNIT_MAX: usize = 64;

impl FileLayout {
    /// Checks the layout's structure: a header of one or two rows of equal
    /// width, column indexes in range and distinct, a complete shape whose
    /// wide analyte names are the file's own header text, and non-empty,
    /// unique words that the reader can match. Field names are relative to
    /// `layout`.
    pub fn validate(&self) -> Result<(), FieldError> {
        let width = header_width(&self.header)?;
        let mut used = BTreeSet::new();
        let mut take = |field: String, column: u16| -> Result<(), FieldError> {
            if usize::from(column) >= width {
                return Err(invalid(
                    &field,
                    "out_of_range",
                    "Choose a column of the header.",
                ));
            }
            if !used.insert(column) {
                return Err(invalid(&field, "duplicate", "Each column has one role."));
            }
            Ok(())
        };
        for (name, column) in self.columns.roles() {
            if let Some(column) = column {
                take(format!("layout.columns.{name}"), column)?;
            }
        }
        match &self.shape {
            FileShape::Long {
                analyte_column,
                value_column,
            } => {
                take("layout.shape.analyteColumn".to_owned(), *analyte_column)?;
                take("layout.shape.valueColumn".to_owned(), *value_column)?;
            }
            FileShape::Wide { value_columns } => {
                if value_columns.is_empty() {
                    return Err(invalid(
                        "layout.shape.valueColumns",
                        "required",
                        "Choose at least one analyte column.",
                    ));
                }
                for (index, value) in value_columns.iter().enumerate() {
                    take(
                        format!("layout.shape.valueColumns[{index}].column"),
                        value.column,
                    )?;
                }
                self.validate_wide(value_columns)?;
            }
        }
        self.validate_words()
    }

    /// A wide file's analyte names are unique and each is the file's own
    /// text for its column (M2 of the wave-10 review): the header cell of a
    /// one-row header, or the group label of a two-row header, whose value
    /// columns all read the same value field. Only the listed columns are
    /// read, so there is no not-a-result list.
    fn validate_wide(&self, value_columns: &[ValueColumn]) -> Result<(), FieldError> {
        self.unique_words(
            "layout.shape.valueColumns",
            value_columns
                .iter()
                .map(|value| value.analyte_name.as_str()),
        )?;
        let field_row = self.header.get(1);
        let value_field = |value: &ValueColumn| {
            field_row
                .and_then(|row| row.get(usize::from(value.column)))
                .map(|cell| self.field(cell))
        };
        let first_field = value_columns.first().and_then(value_field);
        for (index, value) in value_columns.iter().enumerate() {
            if self.analyte_label(value.column) != Some(value.analyte_name.as_str()) {
                return Err(invalid(
                    &format!("layout.shape.valueColumns[{index}].analyteName"),
                    "not_header_text",
                    "Name each analyte column with its header text, or its group label.",
                ));
            }
            if value_field(value) != first_field {
                return Err(invalid(
                    &format!("layout.shape.valueColumns[{index}].column"),
                    "other_value_field",
                    "Every analyte column must read the same value field.",
                ));
            }
        }
        if !self.not_results.is_empty() {
            return Err(invalid(
                "layout.notResults",
                "not_applicable",
                "A wide file lists only the analyte columns it reads.",
            ));
        }
        Ok(())
    }

    /// Words, names and the unit: non-empty and unique; with trimming, as the
    /// reader trims them; a value word is never text the strict reader reads
    /// as a positive value (F7); a declared unit only without a units column.
    fn validate_words(&self) -> Result<(), FieldError> {
        self.unique_words(
            "layout.sampleTypes",
            self.sample_types.iter().map(|t| t.word.as_str()),
        )?;
        self.unique_words(
            "layout.valueWords",
            self.value_words.iter().map(|w| w.word.as_str()),
        )?;
        if self
            .value_words
            .iter()
            .any(|w| reads_as_positive_value(&w.word))
        {
            return Err(invalid(
                "layout.valueWords",
                "reads_as_value",
                "A positive number or a comparator with a positive bound is a value, not a word.",
            ));
        }
        self.unique_words(
            "layout.notResults",
            self.not_results.iter().map(String::as_str),
        )?;
        if !self.sample_types.is_empty() && self.columns.sample_type.is_none() {
            return Err(invalid(
                "layout.sampleTypes",
                "not_applicable",
                "Choose the sample type column to map its words.",
            ));
        }
        let Some(unit) = &self.declared_unit else {
            return Ok(());
        };
        if self.columns.units.is_some() {
            return Err(invalid(
                "layout.declaredUnit",
                "exclusive",
                "Use a units column or a declared unit, not both.",
            ));
        }
        let trimmed = lis_core::required_text("layout.declaredUnit", unit, UNIT_MAX)?;
        if trimmed != *unit {
            return Err(untrimmed("layout.declaredUnit"));
        }
        Ok(())
    }

    /// Every word non-empty, at most a field long, listed once and as the
    /// layout reads a field: a word with spaces that trimming removes could
    /// never match.
    fn unique_words<'a>(
        &self,
        field: &str,
        words: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), FieldError> {
        let mut seen = BTreeSet::new();
        for word in words {
            if word.is_empty() {
                return Err(invalid(field, "required", "A word or name is empty."));
            }
            if word.len() > MAX_FIELD_BYTES {
                return Err(invalid(field, "too_long", "A word or name is too long."));
            }
            if self.field(word) != word {
                return Err(untrimmed(field));
            }
            if !seen.insert(word) {
                return Err(invalid(
                    field,
                    "duplicate",
                    "Each word or name appears once.",
                ));
            }
        }
        Ok(())
    }
}

impl FileColumns {
    /// Each role's field name and column.
    fn roles(&self) -> [(&'static str, Option<u16>); 7] {
        [
            ("sampleLabel", Some(self.sample_label)),
            ("units", self.units),
            ("sampleType", self.sample_type),
            ("flags", self.flags),
            ("operator", self.operator),
            ("acquiredAt", self.acquired_at),
            ("dilution", self.dilution),
        ]
    }
}

/// Whether the strict reader of `lis-results` reads `text` as a positive
/// quantity, or a comparator immediately followed by a positive bound: an
/// optional `<=`, `>=`, `<` or `>`, then a plain decimal (F7). Such text is a
/// value, so mapping it as a word would hide a measured result.
fn reads_as_positive_value(text: &str) -> bool {
    let comparator_len = [2, 1]
        .into_iter()
        .find(|&len| {
            text.get(..len)
                .is_some_and(|prefix| Comparator::parse(prefix).is_ok())
        })
        .unwrap_or(0);
    text.get(comparator_len..)
        .and_then(|bound| DecimalText::parse(bound).ok())
        .is_some_and(|bound| bound.to_decimal() > Decimal::ZERO)
}

/// One or two rows of equal width, at most [`MAX_FILE_COLUMNS`], with cells
/// that fit a field and hold no line break (the signature is matched per line).
fn header_width(header: &[Vec<String>]) -> Result<usize, FieldError> {
    let field = "layout.header";
    let (Some(first), 1..=2) = (header.first(), header.len()) else {
        return Err(invalid(field, "rows", "Choose one or two header rows."));
    };
    let width = first.len();
    if width == 0 || width > MAX_FILE_COLUMNS {
        return Err(invalid(
            field,
            "width",
            "The header has too few or too many columns.",
        ));
    }
    if header.iter().any(|row| row.len() != width) {
        return Err(invalid(
            field,
            "unequal_rows",
            "Both header rows need the same columns.",
        ));
    }
    let cells = header.iter().flatten();
    if cells.clone().any(|cell| cell.len() > MAX_FIELD_BYTES) {
        return Err(invalid(field, "too_long", "A header cell is too long."));
    }
    if cells.clone().any(|cell| cell.contains(['\r', '\n'])) {
        return Err(invalid(
            field,
            "line_break",
            "A header cell holds a line break.",
        ));
    }
    Ok(width)
}

fn untrimmed(field: &str) -> FieldError {
    invalid(field, "untrimmed", "Remove surrounding spaces.")
}

fn invalid(field: &str, code: &str, message: &str) -> FieldError {
    FieldError {
        field: field.to_owned(),
        code: code.to_owned(),
        message: message.to_owned(),
    }
}
