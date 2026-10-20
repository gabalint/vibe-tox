//! AU400/AU640 result extraction (REQ-ANA-003, REQ-INT-005, REQ-RES-001).
//!
//! The receipt's frozen, explicit online profile reads exact wire bytes.
//! Source rows keep fixed-width padding and both flag characters. AU has no
//! reported unit, software header, completion time or correction status: those
//! fields stay absent. Approved source registration names are setup metadata.
//! Only an approved per-code AU quantitative declaration supplies a numeric
//! interpretation and declared unit. Packed LIH codes must not appear in that
//! whitelist. Flags and the all-9 overflow marker require human reconciliation.

use lis_analytical::{AuSourceProfile, PositionRole};
use lis_au::{
    AssemblyOutcome, AuConfig, AuError, Message, ResultAssembler, ResultKind, SampleResults,
    StreamDecoder, WireEvent,
};
use lis_catalog::{MethodAnalyte, OutputScale};
use thiserror::Error;

use crate::astm::{ExtractedImport, ExtractedRow};
use crate::model::QuarantineReason;
use crate::result_value::ResultValue;

/// PHI-free reason that an AU artifact yields no complete result rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum AuExtractFailure {
    #[error(transparent)]
    Protocol(#[from] AuError),
    #[error("AU artifact carries no complete results")]
    NotAResultMessage,
    #[error("AU result artifact contains an unexpected message")]
    UnexpectedMessage,
    #[error("AU artifact exceeds the result import limit")]
    LimitsExceeded,
}

/// Reads one or more complete sample texts from a receipt's exact wire bytes.
/// No manufactured ASTM records or clinical interpretation are involved.
pub fn extract_artifact(
    bytes: &[u8],
    profile: &AuConfig,
) -> Result<ExtractedImport, AuExtractFailure> {
    if bytes.len() > 1024 * 1024 {
        return Err(AuExtractFailure::LimitsExceeded);
    }
    let mut decoder = StreamDecoder::new(profile.clone())?;
    let mut assembler = ResultAssembler::default();
    let mut rows = Vec::new();
    let mut samples = 0;
    for chunk in bytes.chunks(4096) {
        for event in decoder.push(chunk) {
            let WireEvent::Frame(frame) = event? else {
                return Err(AuExtractFailure::UnexpectedMessage);
            };
            let Message::ResultBlock(block) = lis_au::parse_message(profile, &frame)? else {
                return Err(AuExtractFailure::UnexpectedMessage);
            };
            if let AssemblyOutcome::Complete(sample) = assembler.push(block)? {
                samples += 1;
                append_sample(&mut rows, &sample, samples)?;
            }
        }
    }
    decoder.finish()?;
    if assembler.is_pending() {
        return Err(AuError::BlockSequence.into());
    }
    if rows.is_empty() {
        return Err(AuExtractFailure::NotAResultMessage);
    }
    Ok(ExtractedImport {
        sender_name: None,
        sender_version: None,
        message_time_text: None,
        rows,
        malformed_rows: Vec::new(),
    })
}

fn append_sample(
    rows: &mut Vec<ExtractedRow>,
    sample: &SampleResults,
    ordinal: usize,
) -> Result<(), AuExtractFailure> {
    let label = ascii(sample.header.sample_id.expose())?;
    for (index, test) in sample.tests.iter().enumerate() {
        rows.push(ExtractedRow {
            record_path: format!("AU/S{ordinal}/R{}", index + 1),
            sample_label: Some(label.clone()),
            test_code: Some(ascii(test.code.expose())?),
            value_text: Some(ascii(test.value.expose())?),
            units_text: None,
            flags_text: Some(ascii(test.flags.expose())?),
            status_text: None,
            operator_text: None,
            completed_text: None,
            sample_type_text: Some(kind_text(sample.kind).to_owned()),
            dilution_text: None,
        });
    }
    Ok(())
}

fn ascii(bytes: &[u8]) -> Result<String, AuExtractFailure> {
    std::str::from_utf8(bytes)
        .map(str::to_owned)
        .map_err(|_| AuError::Character.into())
}

fn kind_text(kind: ResultKind) -> &'static str {
    match kind {
        ResultKind::Normal => "D ",
        ResultKind::Rerun => "DH",
        ResultKind::Stat => "d ",
        ResultKind::StatRerun => "dH",
        ResultKind::ReagentBlank => "DR",
        ResultKind::Calibration => "DA",
        ResultKind::QualityControl => "DQ",
    }
}

pub(crate) fn role_mismatch(row: &ExtractedRow, role: PositionRole) -> bool {
    let expected = match row.sample_type_text.as_deref() {
        Some("D " | "DH" | "d " | "dH") => PositionRole::Patient,
        Some("DQ") => PositionRole::Control,
        Some("DA") => PositionRole::Calibrator,
        Some("DR") => PositionRole::Blank,
        Some(_) | None => return true,
    };
    expected != role
}

/// Parses a numeric view of padding while leaving the source evidence intact.
pub(crate) fn patient_value(
    row: &ExtractedRow,
    scale: OutputScale,
    analyte: &MethodAnalyte,
    profile: &AuSourceProfile,
) -> Result<ResultValue, QuarantineReason> {
    let declared = profile
        .tests
        .iter()
        .find(|test| Some(test.test_code.as_str()) == row.test_code.as_deref())
        .ok_or(QuarantineReason::UnknownAnalyte)?;
    if analyte.unit != declared.unit || row.units_text.is_some() {
        return Err(QuarantineReason::IncompatibleUnit);
    }
    if scale != OutputScale::Quantitative {
        return Err(QuarantineReason::ScaleMismatch);
    }
    let raw = row
        .value_text
        .as_deref()
        .ok_or(QuarantineReason::MalformedValue)?;
    if raw.trim_matches(' ').bytes().all(|byte| byte == b'9')
        || row.flags_text.as_deref() != Some("  ")
        || row.status_text.is_some()
    {
        return Err(QuarantineReason::MalformedValue);
    }
    let view = raw.trim_matches(' ');
    let negative;
    let numeric = if let Some(magnitude) = view.strip_prefix('-') {
        negative = format!("-{}", magnitude.trim_start_matches(' '));
        negative.as_str()
    } else {
        view
    };
    let crate::SourceValue::Quantity(value) =
        crate::parse_value(numeric, OutputScale::Quantitative)
            .map_err(|_| QuarantineReason::MalformedValue)?
    else {
        return Err(QuarantineReason::MalformedValue);
    };
    Ok(ResultValue {
        value: Some(crate::SourceValue::Quantity(value)),
        units: declared.unit.clone(),
        rule: None,
    })
}

#[cfg(test)]
mod tests;
