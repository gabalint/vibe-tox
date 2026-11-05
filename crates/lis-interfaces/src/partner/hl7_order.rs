//! A narrow HL7 v2.5.1 order reader for incoming orders (ticket #4555 step 3c; ADR 0007
//! §7): it reads OML^O21, or the older ORM^O01, into [`OrderMessage`] and refuses what it
//! does not understand with a PHI-free [`ParseProblem`]. It keeps values as supplied:
//! nothing is defaulted, mapped or inferred (REQ-ORD-005). Pure: no clock, no database.
//! It claims no profile conformance; a partner's contract and acceptance tests decide that
//! (REQ-INT-001).

use super::hl7_read::{Reader, Segment, find, message_text, segments, separators};

/// The message types read.
const ACCEPTED: [&str; 2] = ["OML^O21", "ORM^O01"];

/// Why a message was not read; the codes show in Logs and carry no patient data.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseProblem {
    /// Not UTF-8 text.
    NotText,
    /// No MSH segment first, or its separators are unusable.
    NotHl7,
    /// MSH-9 is not OML^O21 or ORM^O01.
    UnsupportedMessage,
    /// MSH-10, the message control ID, is empty.
    NoControlId,
    /// No PID, or no identifier in PID-3.
    NoPatient,
    /// No ORC with an OBR.
    NoOrder,
    /// An OBR without a test code in OBR-4.
    NoTest,
    /// No placer order number in ORC-2 (or OBR-2).
    NoPlacerOrder,
    /// An order control other than NW (new order): changes and cancellations are not read.
    UnsupportedControl,
}

impl ParseProblem {
    /// The stored and displayed code.
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::NotText => "not_text",
            Self::NotHl7 => "not_hl7",
            Self::UnsupportedMessage => "unsupported_message",
            Self::NoControlId => "no_control_id",
            Self::NoPatient => "no_patient",
            Self::NoOrder => "no_order",
            Self::NoTest => "no_test",
            Self::NoPlacerOrder => "no_placer_order",
            Self::UnsupportedControl => "unsupported_control",
        }
    }
}

/// One order message, as supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderMessage {
    /// MSH-9, such as `OML^O21`.
    pub message_type: String,
    /// MSH-10.
    pub control_id: String,
    /// MSH-4.1: the sending facility.
    pub sending_facility: String,
    pub patient: OrderPatient,
    /// PV1-3.1: the point of care, the client location's code.
    pub location: Option<String>,
    /// The ordering source's order number: ORC-4 (placer group) when given, else the first
    /// ORC-2; with its assigning authority.
    pub placer_order: String,
    pub placer_authority: Option<String>,
    /// ORC-12.1, else OBR-16.1: the ordering provider's ID (an NPI by contract).
    pub provider_id: Option<String>,
    /// OBR-4.1 of every OBR, in order.
    pub tests: Vec<String>,
    /// OBR-7: the observation (collection) time, as supplied.
    pub collected_at: Option<String>,
    /// DG1-3: code and coding system, as supplied.
    pub diagnoses: Vec<(String, Option<String>)>,
}

/// PID, as supplied.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderPatient {
    /// PID-3: value, assigning authority (CX.4.1), identifier type (CX.5).
    pub identifiers: Vec<(String, Option<String>, Option<String>)>,
    /// PID-5.1 and PID-5.2.
    pub family_name: Option<String>,
    pub given_name: Option<String>,
    /// PID-7, as supplied (`YYYYMMDD` or longer).
    pub birth_date: Option<String>,
    /// PID-8, as supplied.
    pub sex: Option<String>,
}

/// Reads one OML^O21 or ORM^O01 message.
pub fn read_order_message(bytes: &[u8]) -> Result<OrderMessage, ParseProblem> {
    // A file may still carry MLLP framing or a byte order mark.
    let text = message_text(bytes).ok_or(ParseProblem::NotText)?;
    let separators = separators(text).ok_or(ParseProblem::NotHl7)?;
    let segments = segments(text, separators);
    let msh = &segments[0];
    let reader = Reader { separators };
    let message_type = format!(
        "{}^{}",
        reader.component(msh.field(9), 1),
        reader.component(msh.field(9), 2)
    );
    if !ACCEPTED.contains(&message_type.as_str()) {
        return Err(ParseProblem::UnsupportedMessage);
    }
    let control_id = reader.text(msh.field(10));
    if control_id.is_empty() {
        return Err(ParseProblem::NoControlId);
    }
    let patient = reader.patient(find(&segments, "PID").ok_or(ParseProblem::NoPatient)?)?;
    let location =
        find(&segments, "PV1").and_then(|pv1| reader.some(&reader.component(pv1.field(3), 1)));
    let order = reader.orders(&segments)?;
    Ok(OrderMessage {
        message_type,
        control_id,
        sending_facility: reader.component(msh.field(4), 1),
        patient,
        location,
        placer_order: order.placer_order,
        placer_authority: order.placer_authority,
        provider_id: order.provider_id,
        tests: order.tests,
        collected_at: order.collected_at,
        diagnoses: reader.diagnoses(&segments),
    })
}

/// What the order segments say, together.
struct Orders {
    placer_order: String,
    placer_authority: Option<String>,
    provider_id: Option<String>,
    tests: Vec<String>,
    collected_at: Option<String>,
}

impl Reader {
    fn patient(&self, pid: &Segment<'_>) -> Result<OrderPatient, ParseProblem> {
        let identifiers: Vec<_> = pid
            .field(3)
            .split(self.separators.repetition)
            .filter_map(|cx| {
                let value = self.component(cx, 1);
                (!value.is_empty()).then(|| {
                    let authority = self.component(cx, 4);
                    let kind = self.component(cx, 5);
                    (value, self.some(&authority), self.some(&kind))
                })
            })
            .collect();
        if identifiers.is_empty() {
            return Err(ParseProblem::NoPatient);
        }
        Ok(OrderPatient {
            identifiers,
            family_name: self.some(&self.component(pid.field(5), 1)),
            given_name: self.some(&self.component(pid.field(5), 2)),
            birth_date: self.some(&self.component(pid.field(7), 1)),
            sex: self.some(&self.component(pid.field(8), 1)),
        })
    }

    /// Every ORC must be a new order; every OBR names a test.
    fn orders(&self, segments: &[Segment<'_>]) -> Result<Orders, ParseProblem> {
        let orcs: Vec<_> = segments.iter().filter(|s| s.name == "ORC").collect();
        let obrs: Vec<_> = segments.iter().filter(|s| s.name == "OBR").collect();
        let (Some(orc), Some(obr)) = (orcs.first(), obrs.first()) else {
            return Err(ParseProblem::NoOrder);
        };
        if orcs
            .iter()
            .any(|orc| self.component(orc.field(1), 1) != "NW")
        {
            return Err(ParseProblem::UnsupportedControl);
        }
        let tests: Vec<String> = obrs
            .iter()
            .map(|obr| self.component(obr.field(4), 1))
            .collect();
        if tests.iter().any(String::is_empty) {
            return Err(ParseProblem::NoTest);
        }
        // The placer group number names the whole requisition; else the first order number.
        let (number, authority) = [orc.field(4), orc.field(2), obr.field(2)]
            .into_iter()
            .map(|field| (self.component(field, 1), self.component(field, 2)))
            .find(|(number, _)| !number.is_empty())
            .ok_or(ParseProblem::NoPlacerOrder)?;
        let provider = self
            .some(&self.component(orc.field(12), 1))
            .or_else(|| self.some(&self.component(obr.field(16), 1)));
        Ok(Orders {
            placer_order: number,
            placer_authority: self.some(&authority),
            provider_id: provider,
            tests,
            collected_at: self.some(&self.component(obr.field(7), 1)),
        })
    }

    fn diagnoses(&self, segments: &[Segment<'_>]) -> Vec<(String, Option<String>)> {
        segments
            .iter()
            .filter(|segment| segment.name == "DG1")
            .filter_map(|dg1| {
                let code = self.component(dg1.field(3), 1);
                let system = self.component(dg1.field(3), 3);
                (!code.is_empty()).then(|| (code, self.some(&system)))
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "hl7_order_tests.rs"]
mod tests;
