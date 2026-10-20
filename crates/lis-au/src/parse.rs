use crate::fields::validate_header;
use crate::{
    AuConfig, AuError, BlockId, Demographics, Frame, FrameEnd, Message, Query, ResultBlock,
    ResultKind, SampleHeader, SessionKind, TriBlock, TriTest,
};
use lis_core::Phi;

/// Parses fixed-width text with the caller's instrument-matched settings (pp8–14).
pub fn parse_message(config: &AuConfig, frame: &Frame) -> Result<Message, AuError> {
    config.validate()?;
    let bytes = frame.text.expose();
    if bytes.len() + config.overhead() > usize::from(config.framing.max_wire_len) {
        return Err(AuError::FrameTooLong);
    }
    if !bytes.iter().all(|b| (32..=126).contains(b)) {
        return Err(AuError::Character);
    }
    let mut cursor = Cursor { bytes, offset: 0 };
    let code = cursor.take(2)?;
    let unit_no = if config.format.unit_no && code[0] != b'S' {
        let unit = cursor.take(2)?;
        if !unit.iter().all(u8::is_ascii_digit) {
            return Err(AuError::Field);
        }
        Some(Phi::new(unit.to_vec()))
    } else {
        None
    };
    if let Some(kind) = session_kind(code) {
        if !cursor.remaining().is_empty() {
            return Err(AuError::Length);
        }
        if frame.end != FrameEnd::Text {
            return Err(AuError::BlockTerminator);
        }
        return Ok(Message::Session { kind, unit_no });
    }
    match code {
        b"R " | b"RH" => parse_query(config, cursor, unit_no, code == b"RH", frame.end),
        b"S " | b"SH" => parse_tri(config, cursor, code == b"SH", frame.end),
        _ => parse_result(config, cursor, unit_no, result_kind(code)?, frame.end),
    }
}

fn session_kind(code: &[u8]) -> Option<SessionKind> {
    match code {
        b"RB" => Some(SessionKind::QueryStart),
        b"RE" => Some(SessionKind::QueryEnd),
        b"DB" => Some(SessionKind::ResultsStart),
        b"DE" => Some(SessionKind::ResultsEnd),
        b"SE" => Some(SessionKind::TriStop),
        _ => None,
    }
}
fn result_kind(code: &[u8]) -> Result<ResultKind, AuError> {
    match code {
        b"D " => Ok(ResultKind::Normal),
        b"DH" => Ok(ResultKind::Rerun),
        b"d " => Ok(ResultKind::Stat),
        b"dH" => Ok(ResultKind::StatRerun),
        b"DR" => Ok(ResultKind::ReagentBlank),
        b"DA" => Ok(ResultKind::Calibration),
        b"DQ" => Ok(ResultKind::QualityControl),
        _ => Err(AuError::Classification),
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, width: usize) -> Result<&'a [u8], AuError> {
        let end = self.offset.checked_add(width).ok_or(AuError::Length)?;
        let field = self.bytes.get(self.offset..end).ok_or(AuError::Length)?;
        self.offset = end;
        Ok(field)
    }
    fn raw(&mut self, width: usize) -> Result<Phi<Vec<u8>>, AuError> {
        Ok(Phi::new(self.take(width)?.to_vec()))
    }
    fn remaining(&self) -> &'a [u8] {
        &self.bytes[self.offset..]
    }
}

fn header(
    config: &AuConfig,
    cursor: &mut Cursor<'_>,
    unit_no: Option<Phi<Vec<u8>>>,
) -> Result<SampleHeader, AuError> {
    let rack = cursor.raw(usize::from(config.format.rack_digits))?;
    let cup = cursor.raw(if config.format.rack_digits == 0 { 0 } else { 2 })?;
    Ok(SampleHeader {
        unit_no,
        rack,
        cup,
        sample_type: cursor.raw(1)?,
        sample_no: cursor.raw(4)?,
        sample_id: cursor.raw(usize::from(config.format.sample_id_digits))?,
        reserved: Phi::new(Vec::new()),
        demographics: None,
    })
}

fn demographics(config: &AuConfig, cursor: &mut Cursor<'_>) -> Result<Demographics, AuError> {
    let sex = cursor.raw(usize::from(config.format.sex))?;
    let age = cursor.raw(if config.format.age { 3 } else { 0 })?;
    let month = cursor.raw(if config.format.month { 2 } else { 0 })?;
    let mut information = std::array::from_fn(|_| Phi::new(Vec::new()));
    for (field, width) in information
        .iter_mut()
        .zip(config.format.patient_info_digits)
    {
        *field = cursor.raw(usize::from(width))?;
    }
    Ok(Demographics {
        sex,
        age,
        month,
        information,
    })
}

fn parse_query(
    config: &AuConfig,
    mut cursor: Cursor<'_>,
    unit: Option<Phi<Vec<u8>>>,
    rerun: bool,
    end: FrameEnd,
) -> Result<Message, AuError> {
    let header = header(config, &mut cursor, unit)?;
    if !cursor.remaining().is_empty() {
        return Err(AuError::Length);
    }
    if end != FrameEnd::Text {
        return Err(AuError::BlockTerminator);
    }
    validate_header(&header, None)?;
    Ok(Message::Query(Query { rerun, header }))
}

fn blocked_header(
    config: &AuConfig,
    cursor: &mut Cursor<'_>,
    unit: Option<Phi<Vec<u8>>>,
    normal: bool,
    end: FrameEnd,
) -> Result<(SampleHeader, BlockId), AuError> {
    let mut header = header(config, cursor, unit)?;
    header.reserved = cursor.raw(4)?;
    let block = match cursor.take(1)? {
        [b'E'] => BlockId::Final,
        [digit @ b'0'..=b'9'] => BlockId::Intermediate(digit - b'0'),
        _ => return Err(AuError::BlockId),
    };
    if config.etb_enabled() {
        let expected = if block == BlockId::Final {
            FrameEnd::Text
        } else {
            FrameEnd::Block
        };
        if expected != end {
            return Err(AuError::BlockTerminator);
        }
    } else if end == FrameEnd::Block {
        return Err(AuError::BlockTerminator);
    }
    if normal {
        header.demographics = Some(demographics(config, cursor)?);
    }
    Ok((header, block))
}

fn parse_result(
    config: &AuConfig,
    mut cursor: Cursor<'_>,
    unit: Option<Phi<Vec<u8>>>,
    kind: ResultKind,
    end: FrameEnd,
) -> Result<Message, AuError> {
    if config.format.rack_digits == 0
        && matches!(
            kind,
            ResultKind::ReagentBlank | ResultKind::Calibration | ResultKind::QualityControl
        )
    {
        return Err(AuError::Configuration);
    }
    let (header, block) = blocked_header(config, &mut cursor, unit, kind.normal(), end)?;
    validate_header(&header, Some(kind))?;
    let width = usize::from(config.format.result_digits) + 4;
    if !cursor.remaining().len().is_multiple_of(width) {
        return Err(AuError::Length);
    }
    let mut tests = Vec::new();
    while !cursor.remaining().is_empty() {
        let code = test_code(&mut cursor)?;
        tests.push(crate::ResultTest {
            code,
            value: cursor.raw(usize::from(config.format.result_digits))?,
            flags: cursor.raw(2)?,
        });
    }
    if tests.is_empty() {
        return Err(AuError::Length);
    }
    Ok(Message::ResultBlock(ResultBlock {
        kind,
        header,
        block,
        tests,
    }))
}

fn parse_tri(
    config: &AuConfig,
    mut cursor: Cursor<'_>,
    rerun: bool,
    end: FrameEnd,
) -> Result<Message, AuError> {
    let (header, block) = blocked_header(config, &mut cursor, None, !rerun, end)?;
    validate_header(
        &header,
        Some(if rerun {
            ResultKind::Rerun
        } else {
            ResultKind::Normal
        }),
    )?;
    let width = if rerun { 3 } else { 2 };
    if !cursor.remaining().len().is_multiple_of(width) {
        return Err(AuError::Length);
    }
    let mut tests = Vec::new();
    while !cursor.remaining().is_empty() {
        let code = test_code(&mut cursor)?;
        let method = if rerun {
            let raw = cursor.take(1)?[0];
            if !matches!(raw, b'0'..=b'2') {
                return Err(AuError::Field);
            }
            Some(raw)
        } else {
            None
        };
        tests.push(TriTest { code, method });
    }
    Ok(Message::TriBlock(TriBlock {
        rerun,
        header,
        block,
        tests,
    }))
}
fn test_code(cursor: &mut Cursor<'_>) -> Result<Phi<Vec<u8>>, AuError> {
    let code = cursor.take(2)?;
    if !code.iter().all(u8::is_ascii_digit) || code == b"00" {
        return Err(AuError::Field);
    }
    Ok(Phi::new(code.to_vec()))
}
