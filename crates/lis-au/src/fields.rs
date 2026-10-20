use crate::{AuError, ResultKind, SampleHeader};

pub(crate) fn validate_header(
    header: &SampleHeader,
    kind: Option<ResultKind>,
) -> Result<(), AuError> {
    let nonpatient = matches!(
        kind,
        Some(ResultKind::ReagentBlank | ResultKind::Calibration | ResultKind::QualityControl)
    );
    if !matches!(kind, Some(ResultKind::QualityControl))
        && !matches!(header.sample_type.expose().as_slice(), b" " | b"U" | b"X")
    {
        return Err(AuError::Field);
    }
    let sample = header.sample_no.expose();
    if sample.len() != 4
        || !sample[1..].iter().all(u8::is_ascii_digit)
        || !matches!(sample[0], b'0'..=b'9' | b'E' | b'P' | b'R' | b'A' | b'Q')
        || sample.iter().all(|byte| *byte == b'0')
        || (!sample[0].is_ascii_digit() && sample[1..] == *b"000")
    {
        return Err(AuError::Field);
    }
    if nonpatient {
        return validate_nonpatient(header, kind);
    }
    if !matches!(sample[0], b'0'..=b'9' | b'E' | b'P') {
        return Err(AuError::Field);
    }
    if let Some(demographics) = &header.demographics {
        validate_demographics(demographics)?;
    }
    for field in [&header.rack, &header.cup] {
        if !field.expose().iter().all(u8::is_ascii_digit)
            && !field.expose().iter().all(|b| *b == b' ')
        {
            return Err(AuError::Field);
        }
    }
    if let Some(kind) = kind {
        match kind {
            ResultKind::Normal | ResultKind::Stat => {
                if header.reserved.expose() != b"    " {
                    return Err(AuError::Field);
                }
            }
            ResultKind::Rerun | ResultKind::StatRerun => {
                let original = header.reserved.expose();
                if original.len() != 4
                    || !original[1..].iter().all(u8::is_ascii_digit)
                    || !matches!(original[0], b'0'..=b'9' | b'E' | b'P')
                {
                    return Err(AuError::Field);
                }
            }
            ResultKind::ReagentBlank | ResultKind::Calibration | ResultKind::QualityControl => {}
        }
    }
    Ok(())
}

fn validate_nonpatient(header: &SampleHeader, kind: Option<ResultKind>) -> Result<(), AuError> {
    for field in [&header.rack, &header.cup] {
        if !field.expose().iter().all(|b| *b == b' ')
            && !field.expose().iter().all(u8::is_ascii_digit)
        {
            return Err(AuError::Field);
        }
    }
    let reserved = header.reserved.expose();
    match kind {
        Some(ResultKind::ReagentBlank) => {
            if reserved != b"    "
                || !header.sample_id.expose().iter().all(|b| *b == b' ')
                || header.sample_no.expose()[0] != b'R'
            {
                return Err(AuError::Field);
            }
        }
        Some(ResultKind::Calibration | ResultKind::QualityControl) => {
            if reserved.len() != 4
                || reserved[..2] != *b"  "
                || !reserved[2..].iter().all(u8::is_ascii_digit)
            {
                return Err(AuError::Field);
            }
            let expected = if kind == Some(ResultKind::Calibration) {
                b'A'
            } else {
                b'Q'
            };
            if header.sample_no.expose()[0] != expected {
                return Err(AuError::Field);
            }
            if kind == Some(ResultKind::Calibration) {
                let number = (reserved[2] - b'0') * 10 + reserved[3] - b'0';
                if !(1..=80).contains(&number) {
                    return Err(AuError::Field);
                }
            }
        }
        Some(ResultKind::Normal | ResultKind::Rerun | ResultKind::Stat | ResultKind::StatRerun)
        | None => return Err(AuError::Field),
    }
    Ok(())
}

fn validate_demographics(fields: &crate::Demographics) -> Result<(), AuError> {
    let sex = fields.sex.expose().as_slice();
    if !sex.is_empty() && !matches!(sex, b"M" | b"F" | b" " | b"0") {
        return Err(AuError::Field);
    }
    for (field, maximum) in [(&fields.age, 150), (&fields.month, 11)] {
        let bytes = field.expose();
        if bytes.is_empty() || bytes.iter().all(|b| *b == b' ') {
            continue;
        }
        if !bytes.iter().all(u8::is_ascii_digit) {
            return Err(AuError::Field);
        }
        let value = bytes
            .iter()
            .fold(0u16, |value, byte| value * 10 + u16::from(byte - b'0'));
        if value > maximum {
            return Err(AuError::Field);
        }
    }
    Ok(())
}
