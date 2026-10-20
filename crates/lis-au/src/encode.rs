use crate::{
    AuConfig, AuError, FrameEnd, Message, SampleHeader, TriBlock, encode_frame, parse_message,
};
use lis_core::Phi;

/// Explicit test code and rerun method supplied by an authorized order provider.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TriTest {
    /// Two ASCII digits 01–99, preserved as bytes.
    pub code: Phi<Vec<u8>>,
    /// Rerun ASCII method byte 0/1/2; absent for a normal requisition.
    pub method: Option<u8>,
}

/// Caller-provided requisition; the codec never infers sample identity or tests.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tri {
    pub rerun: bool,
    pub header: SampleHeader,
    pub tests: Vec<TriTest>,
}

/// Encodes SE (session stop), which is not a per-sample no-order response (p10).
pub fn encode_stop(config: &AuConfig) -> Result<Vec<u8>, AuError> {
    encode_frame(config, b"SE", FrameEnd::Text)
}

/// Splits TRI only between complete test items, repeating all header data (pp7,9).
/// S messages intentionally omit optional incoming Unit No (manual pp9–10).
pub fn encode_tri(config: &AuConfig, tri: &Tri) -> Result<Vec<Vec<u8>>, AuError> {
    config.validate()?;
    // The supplied manual does not qualify an empty per-sample reply.
    if tri.tests.is_empty() {
        return Err(AuError::Length);
    }
    let prefix = tri_prefix(config, tri)?;
    let item_width = if tri.rerun { 3 } else { 2 };
    let capacity = usize::from(config.framing.max_wire_len)
        .checked_sub(config.overhead() + prefix.len())
        .ok_or(AuError::FrameTooLong)?;
    let per_block = capacity / item_width;
    if per_block == 0 {
        return Err(AuError::FrameTooLong);
    }
    let count = tri.tests.len().div_ceil(per_block);
    if count > 11 {
        return Err(AuError::TooManyBlocks);
    }
    let mut frames = Vec::new();
    for index in 0..count {
        let final_block = index + 1 == count;
        let mut text = prefix.clone();
        text[block_offset(config)] = if final_block {
            b'E'
        } else {
            b'0' + index as u8
        };
        for test in tri.tests.iter().skip(index * per_block).take(per_block) {
            append_test(&mut text, test, tri.rerun)?;
        }
        let end = if final_block {
            FrameEnd::Text
        } else {
            FrameEnd::Block
        };
        let wire = encode_frame(config, &text, end)?;
        // Reuse the single fixed-field validator at this outward trust boundary.
        let check = crate::Frame {
            wire: Phi::new(wire.clone()),
            text: Phi::new(text),
            end: if config.etb_enabled() {
                end
            } else {
                FrameEnd::Text
            },
        };
        let Message::TriBlock(TriBlock { .. }) = parse_message(config, &check)? else {
            return Err(AuError::Classification);
        };
        frames.push(wire);
    }
    Ok(frames)
}

fn block_offset(config: &AuConfig) -> usize {
    2 + usize::from(config.format.rack_digits)
        + if config.format.rack_digits == 0 { 0 } else { 2 }
        + 1
        + 4
        + usize::from(config.format.sample_id_digits)
        + 4
}
fn append_field(text: &mut Vec<u8>, value: &[u8], width: usize) -> Result<(), AuError> {
    if value.len() != width {
        return Err(AuError::Length);
    }
    text.extend_from_slice(value);
    Ok(())
}
fn tri_prefix(config: &AuConfig, tri: &Tri) -> Result<Vec<u8>, AuError> {
    let mut text = if tri.rerun {
        b"SH".to_vec()
    } else {
        b"S ".to_vec()
    };
    let header = &tri.header;
    append_field(
        &mut text,
        header.rack.expose(),
        usize::from(config.format.rack_digits),
    )?;
    append_field(
        &mut text,
        header.cup.expose(),
        if config.format.rack_digits == 0 { 0 } else { 2 },
    )?;
    append_field(&mut text, header.sample_type.expose(), 1)?;
    append_field(&mut text, header.sample_no.expose(), 4)?;
    append_field(
        &mut text,
        header.sample_id.expose(),
        usize::from(config.format.sample_id_digits),
    )?;
    append_field(&mut text, header.reserved.expose(), 4)?;
    text.push(b'E');
    if !tri.rerun {
        let demographics = header.demographics.as_ref().ok_or(AuError::Field)?;
        append_field(
            &mut text,
            demographics.sex.expose(),
            usize::from(config.format.sex),
        )?;
        append_field(
            &mut text,
            demographics.age.expose(),
            if config.format.age { 3 } else { 0 },
        )?;
        append_field(
            &mut text,
            demographics.month.expose(),
            if config.format.month { 2 } else { 0 },
        )?;
        for (field, width) in demographics
            .information
            .iter()
            .zip(config.format.patient_info_digits)
        {
            append_field(&mut text, field.expose(), usize::from(width))?;
        }
    } else if header.demographics.is_some() {
        return Err(AuError::Field);
    }
    Ok(text)
}
fn append_test(text: &mut Vec<u8>, test: &TriTest, rerun: bool) -> Result<(), AuError> {
    append_field(text, test.code.expose(), 2)?;
    match (rerun, test.method) {
        (true, Some(method @ b'0'..=b'2')) => text.push(method),
        (false, None) => {}
        _ => return Err(AuError::Field),
    }
    Ok(())
}
