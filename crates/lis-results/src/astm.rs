//! Extraction of result rows from an ASTM result message (REQ-ANA-003, NFR-10).
//!
//! Every text stays exactly as received: nothing is trimmed, parsed or
//! converted here. An absent or empty field is `None`. Patient-record fields
//! are not extracted: the LIS matches material only by the sample label, and
//! the run position's role (patient, control, calibrator or blank) decides
//! where a row goes, never the payload (brief §3.3–3.4).

use lis_astm::{
    Component, HeaderRecord, Message, MessageError, OrderRecord, Record, RecordError, ResultRecord,
    assemble_message, parse_message_records,
};
use thiserror::Error;

/// The part of an approved source registration that extraction needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceProfile {
    /// The order field that carries the sample label.
    pub sample_id_field: SampleIdField,
}

/// Which order field carries the label the LIS matches to a run position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SampleIdField {
    /// O.3, the specimen ID.
    SpecimenId,
    /// O.4, the instrument specimen ID.
    InstrumentSpecimenId,
}

/// The sender and result rows of one message or result file, in source order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedImport {
    /// H.5 sender name, checked against the registration by the importer.
    pub sender_name: Option<String>,
    /// H.5 sender software version.
    pub sender_version: Option<String>,
    /// H.14 message time, uninterpreted.
    pub message_time_text: Option<String>,
    /// One row per result record; none is dropped.
    pub rows: Vec<ExtractedRow>,
    /// Record paths of result-file records whose field count differs from
    /// the header (quarantined as `malformed_row`); always empty for ASTM.
    pub malformed_rows: Vec<String>,
}

/// One result record with its sample label, as received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExtractedRow {
    /// Position in the message, `P<n>/O<n>/R<n>` (1-based), or in a result
    /// file, `L<line>` or `L<line>/<analyte>`.
    pub record_path: String,
    /// The label from the order field the profile names.
    pub sample_label: Option<String>,
    /// R.3 manufacturer's local test code (fourth component).
    pub test_code: Option<String>,
    /// R.4 value text.
    pub value_text: Option<String>,
    /// R.5 units text.
    pub units_text: Option<String>,
    /// R.7 abnormal flags.
    pub flags_text: Option<String>,
    /// R.9 result status, such as `F` or `C`.
    pub status_text: Option<String>,
    /// R.11 operator asserted by the source.
    pub operator_text: Option<String>,
    /// R.13 completion time, uninterpreted.
    pub completed_text: Option<String>,
    /// A result file's sample-type word; always absent for ASTM.
    pub sample_type_text: Option<String>,
    /// A result file's dilution factor text; always absent for ASTM.
    pub dilution_text: Option<String>,
}

/// Why a receipt artifact yields no rows; the importer records the attempt
/// as `malformed_message`. Display text never echoes payload data.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ExtractFailure {
    /// The bytes are not well-formed ASTM records.
    #[error(transparent)]
    Records(#[from] RecordError),
    /// The records do not form a valid message hierarchy.
    #[error(transparent)]
    Message(#[from] MessageError),
    /// The message carries queries or manufacturer records, not results.
    #[error("ASTM message carries no results")]
    NotAResultMessage,
}

/// Parses a receipt artifact (records each ended by CR) and extracts it.
pub fn extract_artifact(
    bytes: &[u8],
    profile: &SourceProfile,
) -> Result<ExtractedImport, ExtractFailure> {
    let message = assemble_message(parse_message_records(bytes)?)?;
    extract(&message, profile)
}

/// Extracts the sender and every result row of an assembled message.
pub fn extract(
    message: &Message,
    profile: &SourceProfile,
) -> Result<ExtractedImport, ExtractFailure> {
    if message.patients().is_empty() {
        return Err(ExtractFailure::NotAResultMessage);
    }
    let header = message.header().as_header();
    let mut rows = Vec::new();
    for (p, patient) in message.patients().iter().enumerate() {
        for (o, order) in patient.orders().iter().enumerate() {
            let label = sample_label(order.record().as_order(), profile.sample_id_field);
            for (r, result) in order.results().iter().enumerate() {
                let path = format!("P{}/O{}/R{}", p + 1, o + 1, r + 1);
                rows.push(row(path, label.clone(), result.record()));
            }
        }
    }
    Ok(ExtractedImport {
        sender_name: text(header.as_ref().and_then(HeaderRecord::sender_name)),
        sender_version: text(header.as_ref().and_then(HeaderRecord::sender_version)),
        message_time_text: text(header.as_ref().and_then(HeaderRecord::timestamp_text)),
        rows,
        malformed_rows: Vec::new(),
    })
}

fn sample_label(order: Option<OrderRecord<'_>>, field: SampleIdField) -> Option<String> {
    let get = match field {
        SampleIdField::SpecimenId => OrderRecord::specimen_id,
        SampleIdField::InstrumentSpecimenId => OrderRecord::instrument_specimen_id,
    };
    text(order.as_ref().and_then(get))
}

fn row(record_path: String, sample_label: Option<String>, record: &Record) -> ExtractedRow {
    let result = record.as_result();
    let test_code = result
        .as_ref()
        .and_then(ResultRecord::universal_test_id_components)
        .and_then(|components| components.get(3))
        .map(Component::decoded);
    ExtractedRow {
        record_path,
        sample_label,
        test_code: text(test_code),
        value_text: text(result.as_ref().and_then(ResultRecord::value_text)),
        units_text: text(result.as_ref().and_then(ResultRecord::units)),
        flags_text: text(result.as_ref().and_then(ResultRecord::abnormal_flags)),
        status_text: text(result.as_ref().and_then(ResultRecord::result_status)),
        operator_text: text(result.as_ref().and_then(ResultRecord::operator)),
        completed_text: text(
            result
                .as_ref()
                .and_then(ResultRecord::completion_timestamp_text),
        ),
        sample_type_text: None,
        dilution_text: None,
    }
}

/// An empty ASTM field means "no value"; any other text is kept exactly.
fn text(field: Option<&str>) -> Option<String> {
    field.filter(|t| !t.is_empty()).map(str::to_owned)
}

#[cfg(test)]
mod tests;
