//! An HL7 Results delivery's ORU^R01, made from the released revision's frozen content
//! and the connection's approved code mappings (REQ-INT-004). A test or analyte without
//! an approved mapping, or a quantity in another unit than its mapping's, holds the
//! message: it is never sent with a guessed code or unit. Values are sent exactly as
//! stored; a censored value is SN, never an exact number; no abnormal flag is invented
//! (REQ-RPT-008). A correction is sent with status C, a withdrawal with every result
//! posted in error (W).

use std::collections::HashMap;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use lis_core::{Comparator, UtcTimestamp};
use lis_interfaces::partner::hl7::{
    Coded, Observation, ObservationGroup, ObservationValue, Order, Organization, Oru, Patient,
    Specimen,
};
use lis_interfaces::partner::{ApprovedMappings, LisRecordId, MappingKind, MappingValues};
use lis_reporting::snapshot::{AnalyteLine, ComponentLine, LaboratorySection, ReportSnapshot};
use lis_reporting::{LineDisposition, ReportRelationship, RevisionOutbound};
use lis_results::ResultOutcome;
use lis_specimens::CollectionPrecision;

/// A test or analyte the message needs a mapping for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Missing {
    pub(crate) kind: MappingKind,
    pub(crate) lis_id: LisRecordId,
}

/// What the message header names: its delivery, when it was made, and the partner.
pub(crate) struct Header<'a> {
    pub(crate) control_id: String,
    pub(crate) created_at: UtcTimestamp,
    pub(crate) partner: &'a str,
    /// The report was withdrawn: every result is posted in error.
    pub(crate) withdrawal: bool,
    /// Sent over MLLP: the partner answers with an application ACK.
    pub(crate) acknowledged: bool,
}

/// The ORU, or every test and analyte it misses a mapping for.
pub(crate) fn oru(
    revision: &RevisionOutbound,
    mappings: Option<&ApprovedMappings>,
    catalog_loinc: &HashMap<LisRecordId, String>,
    header: &Header<'_>,
) -> Result<Oru, Vec<Missing>> {
    let snapshot = &revision.snapshot;
    let zone = TimeZone::get(&snapshot.laboratory.time_zone).unwrap_or(TimeZone::UTC);
    let context = Context {
        mappings,
        catalog_loinc,
        status: status(revision.relationship, header.withdrawal),
        laboratory: organization(&snapshot.laboratory),
    };
    let mut missing = Vec::new();
    let mut groups = Vec::new();
    for (index, item_id) in snapshot.specimen.order_item_ids.iter().enumerate() {
        let lines: Vec<&ComponentLine> = snapshot
            .lines
            .iter()
            .filter(|line| {
                line.item_id == *item_id && line.disposition == LineDisposition::Reported
            })
            .collect();
        if lines.is_empty() {
            continue;
        }
        let filler = format!("{}-{}", revision.report_number, index + 1);
        if let Some(group) = context.group(filler, &lines, &mut missing) {
            groups.push(group);
        }
    }
    if !missing.is_empty() || groups.is_empty() {
        let mut unique = Vec::with_capacity(missing.len());
        for gap in missing {
            if !unique.contains(&gap) {
                unique.push(gap);
            }
        }
        return Err(unique);
    }
    Ok(Oru {
        control_id: header.control_id.clone(),
        created_at: dtm(header.created_at, &zone),
        processing_id: "P",
        laboratory: context.laboratory.clone(),
        receiving_facility: header.partner.to_owned(),
        acknowledged: header.acknowledged,
        patient: patient(snapshot),
        location: revision.parties.locations.first().cloned(),
        order: order(revision, &zone),
        groups,
        specimen: specimen(snapshot, &zone),
    })
}

/// OBR-25 and OBX-11 for a revision: `F` final, `C` corrected, `W` withdrawn.
const fn status(
    relationship: ReportRelationship,
    withdrawal: bool,
) -> (&'static str, &'static str) {
    if withdrawal {
        return ("C", "W");
    }
    match relationship {
        ReportRelationship::Correction => ("C", "C"),
        ReportRelationship::Initial | ReportRelationship::Supplement => ("F", "F"),
    }
}

struct Context<'a> {
    mappings: Option<&'a ApprovedMappings>,
    catalog_loinc: &'a HashMap<LisRecordId, String>,
    /// OBR-25 and OBX-11.
    status: (&'static str, &'static str),
    laboratory: Organization,
}

impl Context<'_> {
    fn mapping(&self, kind: MappingKind, lis_id: LisRecordId) -> Option<&MappingValues> {
        self.mappings?.get(kind, lis_id)
    }

    /// One ordered test's OBR with its reported results; `None` when it misses a mapping.
    fn group(
        &self,
        filler_order: String,
        lines: &[&ComponentLine],
        missing: &mut Vec<Missing>,
    ) -> Option<ObservationGroup> {
        let first = lines.first()?;
        let test = first.orderable_id.cast();
        let service = self.mapping(MappingKind::Test, test).map(|mapping| Coded {
            code: mapping.partner_code.clone(),
            text: first.orderable_name.clone(),
            loinc: mapping.loinc.clone(),
        });
        if service.is_none() {
            missing.push(Missing {
                kind: MappingKind::Test,
                lis_id: test,
            });
        }
        let results = lines.iter().enumerate().flat_map(|(index, line)| {
            line.analytes
                .iter()
                .map(move |analyte| self.observation(index + 1, analyte))
        });
        let mut observations = Vec::new();
        for result in results {
            match result {
                Ok(observation) => observations.extend(observation),
                Err(gap) => missing.push(gap),
            }
        }
        Some(ObservationGroup {
            filler_order,
            service: service?,
            status: self.status.0,
            observations,
        })
    }

    /// One analyte's OBX; `Ok(None)` without a result.
    fn observation(
        &self,
        component: usize,
        analyte: &AnalyteLine,
    ) -> Result<Option<Observation>, Missing> {
        let Some(result) = &analyte.result else {
            return Ok(None);
        };
        let lis_id = analyte.analyte_id.cast();
        let gap = Missing {
            kind: MappingKind::Analyte,
            lis_id,
        };
        let mapping = self.mapping(MappingKind::Analyte, lis_id).ok_or(gap)?;
        let magnitude = result
            .magnitude
            .as_ref()
            .map(|value| value.as_str().to_owned());
        let (value, measured) = match (result.outcome, magnitude) {
            (ResultOutcome::Quantity, Some(number)) => (ObservationValue::Number(number), true),
            (ResultOutcome::Censored, Some(number)) => (
                ObservationValue::Censored {
                    comparator: result.comparator.map_or("", Comparator::as_str).to_owned(),
                    number,
                },
                true,
            ),
            (ResultOutcome::Qualitative, _) => (
                ObservationValue::Text(result.code_text.clone().unwrap_or_default()),
                false,
            ),
            _ => (ObservationValue::None, false),
        };
        let unit = result.units_text.as_ref().or(analyte.unit.as_ref());
        if measured && unit != mapping.ucum_unit.as_ref() {
            return Err(gap);
        }
        let status = match (&value, self.status.1) {
            (ObservationValue::None, "F" | "C") => "X",
            (_, status) => status,
        };
        Ok(Some(Observation {
            code: Coded {
                code: mapping.partner_code.clone(),
                text: analyte.name.clone(),
                loinc: mapping
                    .loinc
                    .clone()
                    .or_else(|| self.catalog_loinc.get(&lis_id).cloned()),
            },
            sub_id: component.to_string(),
            unit: measured.then(|| mapping.ucum_unit.clone()).flatten(),
            value,
            reference_range: analyte.reference_interval.clone(),
            status,
            performer: result
                .performing_sites
                .first()
                .map_or_else(|| self.laboratory.clone(), organization),
        }))
    }
}

fn organization(laboratory: &LaboratorySection) -> Organization {
    Organization {
        name: laboratory.name.clone(),
        id: laboratory.clia_id.clone(),
    }
}

fn patient(snapshot: &ReportSnapshot) -> Patient {
    let subject = &snapshot.subject;
    Patient {
        identifiers: subject
            .identifiers
            .iter()
            .map(|id| {
                (
                    id.value.clone(),
                    id.authority.clone(),
                    id.identifier_type.clone(),
                )
            })
            .collect(),
        family_name: subject.family_name.clone(),
        given_names: subject.given_names.clone(),
        birth_date: subject.birth_date.replace('-', ""),
        sex: subject.sex.clone(),
    }
}

fn order(revision: &RevisionOutbound, zone: &TimeZone) -> Order {
    let order = &revision.snapshot.order;
    // The snapshot prints the practitioner as "Family, Given".
    let (family, given) = match order.practitioner_name.split_once(", ") {
        Some((family, given)) => (family.to_owned(), Some(given.to_owned())),
        None => (order.practitioner_name.clone(), None),
    };
    let facility = revision.parties.clients.first().map_or_else(
        || Organization {
            name: order.client_name.clone(),
            id: String::new(),
        },
        |(code, name)| Organization {
            name: name.clone(),
            id: code.clone(),
        },
    );
    Order {
        placer_order: order.placer_order_id.clone(),
        provider: (order.practitioner_npi.clone(), family, given),
        facility,
        collected_at: collected(&revision.snapshot),
        released_at: dtm(revision.released_at, zone),
    }
}

fn specimen(snapshot: &ReportSnapshot, zone: &TimeZone) -> Specimen {
    let specimen = &snapshot.specimen;
    Specimen {
        id: specimen.accession_number.clone(),
        kind: (
            specimen.specimen_type_code.clone(),
            specimen.specimen_type_name.clone(),
        ),
        collected_at: collected(snapshot),
        received_at: dtm(specimen.received_at, zone),
    }
}

/// The collection time as recorded: at the source's offset, a date only, or empty.
fn collected(snapshot: &ReportSnapshot) -> String {
    let specimen = &snapshot.specimen;
    match (
        specimen.collection_precision,
        specimen.collected_at,
        specimen.utc_offset_minutes,
    ) {
        (CollectionPrecision::Datetime, Some(at), Some(minutes)) => {
            jiff::tz::Offset::from_seconds(minutes.saturating_mul(60))
                .map(|offset| dtm(at, &TimeZone::fixed(offset)))
                .unwrap_or_default()
        }
        (CollectionPrecision::Date, _, _) => specimen
            .collected_date
            .as_deref()
            .map(|date| date.replace('-', ""))
            .unwrap_or_default(),
        (CollectionPrecision::Datetime | CollectionPrecision::Unknown, _, _) => String::new(),
    }
}

/// HL7 DTM: `YYYYMMDDHHMMSS±ZZZZ` in `zone`.
pub(crate) fn dtm(at: UtcTimestamp, zone: &TimeZone) -> String {
    Timestamp::from_second(at.unix_seconds())
        .unwrap_or(Timestamp::UNIX_EPOCH)
        .to_zoned(zone.clone())
        .strftime("%Y%m%d%H%M%S%z")
        .to_string()
}
