//! Rows of a result file under its layout (brief F4, F5, §13).

use lis_analytical::{FileLayout, FileShape, MAX_FILE_ROWS};

use super::FileExtractFailure;
use super::decode::decode;
use super::table::{DataRecord, read_data};
use crate::astm::{ExtractedImport, ExtractedRow};

/// Reads a result file's rows under `layout`, or the whole-file failure.
/// Every text stays as read (after the layout's trimming); an empty field
/// is absent. Each long record gives one row; each wide record gives one row
/// per listed analyte column. A record whose field count differs from the
/// header, or that ends inside an open quote, is listed in
/// `malformed_rows` instead. Files carry no sender or message time.
pub fn extract_file(
    bytes: &[u8],
    layout: &FileLayout,
) -> Result<ExtractedImport, FileExtractFailure> {
    let text = decode(bytes, layout.encoding)?;
    let width = layout.width();
    let mut rows = Vec::new();
    let mut malformed_rows = Vec::new();
    read_data(&text, layout, |record| {
        if record.terminated && record.fields.len() == width {
            push_rows(layout, &record, &mut rows);
        } else {
            malformed_rows.push(format!("L{}", record.line));
        }
        if rows.len() + malformed_rows.len() > MAX_FILE_ROWS {
            return Err(FileExtractFailure::LimitsExceeded);
        }
        Ok(())
    })?;
    Ok(ExtractedImport {
        sender_name: None,
        sender_version: None,
        message_time_text: None,
        rows,
        malformed_rows,
    })
}

fn push_rows(layout: &FileLayout, record: &DataRecord<'_>, rows: &mut Vec<ExtractedRow>) {
    let cell = |column: Option<u16>| {
        let text = layout.field(record.fields.get(usize::from(column?))?);
        (!text.is_empty()).then(|| text.to_owned())
    };
    let columns = &layout.columns;
    let row = |record_path: String, test_code: Option<String>, value: u16| ExtractedRow {
        record_path,
        sample_label: cell(Some(columns.sample_label)),
        test_code,
        value_text: cell(Some(value)),
        units_text: cell(columns.units),
        flags_text: cell(columns.flags),
        status_text: None,
        operator_text: cell(columns.operator),
        completed_text: cell(columns.acquired_at),
        sample_type_text: cell(columns.sample_type),
        dilution_text: cell(columns.dilution),
    };
    match &layout.shape {
        FileShape::Long {
            analyte_column,
            value_column,
        } => rows.push(row(
            format!("L{}", record.line),
            cell(Some(*analyte_column)),
            *value_column,
        )),
        FileShape::Wide { value_columns } => {
            for value in value_columns {
                rows.push(row(
                    format!("L{}/{}", record.line, value.analyte_name),
                    Some(value.analyte_name.clone()),
                    value.column,
                ));
            }
        }
    }
}
