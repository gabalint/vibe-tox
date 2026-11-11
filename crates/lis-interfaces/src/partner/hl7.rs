//! A narrow HL7 v2.5.1 ORU^R01 encoder for outgoing results (ADR 0007 §7): the
//! engine fills [`Oru`] from a released revision and the connection's approved
//! code mappings, and [`Oru::encode`] makes the bytes. Every text value is
//! escaped, so no value can add a field, component or segment. Pure: no clock,
//! no database. Timestamps arrive formatted (`YYYYMMDDHHMMSS±ZZZZ` or a date).
//! It is not a parser and claims no profile conformance; a partner's contract
//! and acceptance tests decide that (REQ-INT-001).

pub use super::hl7_sex::administrative_sex;

/// The sending application in MSH-3.
pub const SENDING_APPLICATION: &str = "OPENBGA TOX";

/// One ORU^R01 message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Oru {
    /// MSH-10: unique per message; the delivery ID.
    pub control_id: String,
    /// MSH-7.
    pub created_at: String,
    /// MSH-11: `P` for patient data, `T` for a synthetic test message.
    pub processing_id: &'static str,
    /// MSH-4: the laboratory's name and CLIA number.
    pub laboratory: Organization,
    /// MSH-6: the partner.
    pub receiving_facility: String,
    /// MSH-15 and MSH-16 empty: the partner answers with an application ACK on the same
    /// connection (original mode, over MLLP). Otherwise `NE` and `NE`: nothing answers a file.
    pub acknowledged: bool,
    pub patient: Patient,
    /// PV1-3: the client location's code and name, if any.
    pub location: Option<(String, String)>,
    pub order: Order,
    pub groups: Vec<ObservationGroup>,
    /// SPM: the specimen.
    pub specimen: Specimen,
}

/// An organization: its name and identifier (a CLIA number or a client code).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Organization {
    pub name: String,
    pub id: String,
}

/// PID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Patient {
    /// PID-3: value, assigning authority, identifier type.
    pub identifiers: Vec<(String, String, String)>,
    pub family_name: String,
    pub given_names: Option<String>,
    /// `YYYYMMDD`.
    pub birth_date: String,
    /// PID-8: the administrative sex as recorded; sent as its table 0001 code
    /// ([`administrative_sex`]).
    pub sex: Option<String>,
}

/// ORC and the OBR fields every group shares.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Order {
    /// The ordering source's order number, if supplied.
    pub placer_order: Option<String>,
    /// The ordering practitioner: NPI, family name, given names.
    pub provider: (String, String, Option<String>),
    /// ORC-21: the ordering client.
    pub facility: Organization,
    /// OBR-7: collection time, a date, or empty when unknown.
    pub collected_at: String,
    /// OBR-22: when the revision was released.
    pub released_at: String,
}

/// One ordered test (OBR) and its results (OBX).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationGroup {
    /// OBR-3: stable for the test across revisions, so a correction replaces it.
    pub filler_order: String,
    /// OBR-4.
    pub service: Coded,
    /// OBR-25: `F` final, `C` corrected.
    pub status: &'static str,
    pub observations: Vec<Observation>,
}

/// A code with its text, and its LOINC code when known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Coded {
    pub code: String,
    pub text: String,
    pub loinc: Option<String>,
}

/// One OBX.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Observation {
    pub code: Coded,
    /// OBX-4: tells apart two components that report the same analyte.
    pub sub_id: String,
    pub value: ObservationValue,
    /// OBX-6: the UCUM unit.
    pub unit: Option<String>,
    /// OBX-7: the reference interval as configured.
    pub reference_range: Option<String>,
    /// OBX-11: `F`, `C`, `W` (withdrawn: posted in error) or `X` (no result).
    pub status: &'static str,
    /// OBX-23: the performing laboratory.
    pub performer: Organization,
}

/// OBX-2 and OBX-5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationValue {
    /// NM: an exact decimal, as stored.
    Number(String),
    /// SN: a censored value, such as `<` and `0.50`; never sent as an exact number.
    Censored { comparator: String, number: String },
    /// ST: a qualitative code as received.
    Text(String),
    /// No value: the result could not be obtained (status `X`).
    None,
}

/// SPM.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Specimen {
    /// SPM-2: the accession number.
    pub id: String,
    /// SPM-4: the specimen type's code and name.
    pub kind: (String, String),
    /// SPM-17 and SPM-18.
    pub collected_at: String,
    pub received_at: String,
}

impl Oru {
    /// A synthetic message with no patient data, processing ID `T`: the synthetic patient
    /// TEST^SYNTHETIC with sex recorded as Unknown, a test provider, a synthetic specimen and `groups`, all at `at`. For the
    /// test message and the acceptance tests.
    #[must_use]
    pub fn synthetic(
        control_id: String,
        at: &str,
        laboratory: Organization,
        partner: &str,
        groups: Vec<ObservationGroup>,
    ) -> Self {
        Self {
            control_id,
            created_at: at.to_owned(),
            processing_id: "T",
            laboratory,
            receiving_facility: partner.to_owned(),
            acknowledged: false,
            patient: Patient {
                identifiers: vec![(
                    "TEST-00000".to_owned(),
                    "OPENBGA".to_owned(),
                    "MR".to_owned(),
                )],
                family_name: "TEST".to_owned(),
                given_names: Some("SYNTHETIC".to_owned()),
                birth_date: "19000101".to_owned(),
                sex: Some("Unknown".to_owned()),
            },
            location: None,
            order: Order {
                placer_order: None,
                provider: ("0000000000".to_owned(), "TEST".to_owned(), None),
                facility: Organization {
                    name: partner.to_owned(),
                    id: String::new(),
                },
                collected_at: at.to_owned(),
                released_at: at.to_owned(),
            },
            groups,
            specimen: Specimen {
                id: "TEST-1".to_owned(),
                kind: ("TEST".to_owned(), "Synthetic".to_owned()),
                collected_at: at.to_owned(),
                received_at: at.to_owned(),
            },
        }
    }

    /// The message's bytes: segments separated by carriage returns, UTF-8.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut segments = vec![self.msh(), self.pid(), self.pv1()];
        for (index, group) in self.groups.iter().enumerate() {
            segments.push(self.orc(group));
            segments.push(self.obr(index + 1, group));
            for (number, observation) in group.observations.iter().enumerate() {
                segments.push(obx(number + 1, observation));
            }
            segments.push(self.spm());
        }
        let mut text = segments.join("\r");
        text.push('\r');
        text.into_bytes()
    }

    fn msh(&self) -> String {
        let ack = if self.acknowledged { "" } else { "NE" };
        let fields = [
            esc(SENDING_APPLICATION),
            comp(&[&self.laboratory.name, &self.laboratory.id, "CLIA"]),
            String::new(),
            esc(&self.receiving_facility),
            esc(&self.created_at),
            String::new(),
            "ORU^R01^ORU_R01".to_owned(),
            esc(&self.control_id),
            esc(self.processing_id),
            "2.5.1".to_owned(),
            String::new(),
            String::new(),
            ack.to_owned(),
            ack.to_owned(),
            String::new(),
            "UNICODE UTF-8".to_owned(),
        ];
        format!("MSH|^~\\&|{}", fields.join("|"))
    }

    fn pid(&self) -> String {
        let patient = &self.patient;
        let identifiers: Vec<String> = patient
            .identifiers
            .iter()
            .map(|(value, authority, kind)| comp(&[value, "", "", authority, kind]))
            .collect();
        let given = patient.given_names.as_deref().unwrap_or_default();
        segment(
            "PID",
            &[
                "1".to_owned(),
                String::new(),
                identifiers.join("~"),
                String::new(),
                comp(&[&patient.family_name, given]),
                String::new(),
                esc(&patient.birth_date),
                administrative_sex(patient.sex.as_deref()).to_owned(),
            ],
        )
    }

    fn pv1(&self) -> String {
        let location = self
            .location
            .as_ref()
            .map_or_else(String::new, |(code, name)| {
                comp(&[code, "", "", "", "", "", "", "", name])
            });
        segment("PV1", &["1".to_owned(), "U".to_owned(), location])
    }

    fn orc(&self, group: &ObservationGroup) -> String {
        let mut fields = vec![String::new(); 21];
        fields[0] = "RE".to_owned();
        fields[1] = esc(self.order.placer_order.as_deref().unwrap_or_default());
        fields[2] = filler(&group.filler_order);
        fields[11] = self.provider();
        fields[20] = organization(&self.order.facility);
        segment("ORC", &fields)
    }

    fn obr(&self, set_id: usize, group: &ObservationGroup) -> String {
        let mut fields = vec![String::new(); 25];
        fields[0] = set_id.to_string();
        fields[1] = esc(self.order.placer_order.as_deref().unwrap_or_default());
        fields[2] = filler(&group.filler_order);
        fields[3] = coded(&group.service);
        fields[6] = esc(&self.order.collected_at);
        fields[15] = self.provider();
        fields[21] = esc(&self.order.released_at);
        fields[24] = group.status.to_owned();
        segment("OBR", &fields)
    }

    fn spm(&self) -> String {
        let specimen = &self.specimen;
        let mut fields = vec![String::new(); 18];
        fields[0] = "1".to_owned();
        // EIP: the accession number is the filler's identifier (component 2).
        fields[1] = comp(&["", &specimen.id]);
        fields[3] = comp(&[&specimen.kind.0, &specimen.kind.1, "L"]);
        fields[16] = esc(&specimen.collected_at);
        fields[17] = esc(&specimen.received_at);
        segment("SPM", &fields)
    }

    /// XCN: NPI, family and given names, identifier type NPI (component 13).
    fn provider(&self) -> String {
        let (npi, family, given) = &self.order.provider;
        let mut components = vec![""; 13];
        components[0] = npi;
        components[1] = family;
        components[2] = given.as_deref().unwrap_or_default();
        components[12] = "NPI";
        comp(&components)
    }
}

fn obx(set_id: usize, observation: &Observation) -> String {
    let (kind, value) = match &observation.value {
        ObservationValue::Number(number) => ("NM", esc(number)),
        ObservationValue::Censored { comparator, number } => ("SN", comp(&[comparator, number])),
        ObservationValue::Text(text) => ("ST", esc(text)),
        ObservationValue::None => ("", String::new()),
    };
    let mut fields = vec![String::new(); 23];
    fields[0] = set_id.to_string();
    fields[1] = kind.to_owned();
    fields[2] = coded(&observation.code);
    fields[3] = esc(&observation.sub_id);
    fields[4] = value;
    fields[5] = observation
        .unit
        .as_deref()
        .map_or_else(String::new, |unit| comp(&[unit, "", "UCUM"]));
    fields[6] = esc(observation.reference_range.as_deref().unwrap_or_default());
    fields[10] = observation.status.to_owned();
    fields[22] = organization(&observation.performer);
    segment("OBX", &fields)
}

/// CWE: the partner's code and text (system `L`), then the LOINC code (system `LN`).
fn coded(coded: &Coded) -> String {
    match &coded.loinc {
        Some(loinc) => comp(&[&coded.code, &coded.text, "L", loinc, "", "LN"]),
        None => comp(&[&coded.code, &coded.text, "L"]),
    }
}

/// EI: the filler order number in the LIS's namespace.
fn filler(number: &str) -> String {
    comp(&[number, SENDING_APPLICATION])
}

/// XON: the name, and the identifier in component 10.
fn organization(organization: &Organization) -> String {
    let mut components = vec![""; 10];
    components[0] = &organization.name;
    components[9] = &organization.id;
    comp(&components)
}

fn segment(name: &str, fields: &[String]) -> String {
    let mut fields = fields.to_vec();
    while fields.last().is_some_and(String::is_empty) {
        fields.pop();
    }
    format!("{name}|{}", fields.join("|"))
}

/// Components joined by `^`, each escaped; trailing empty components dropped.
fn comp(components: &[&str]) -> String {
    let mut escaped: Vec<String> = components.iter().map(|text| esc(text)).collect();
    while escaped.last().is_some_and(String::is_empty) {
        escaped.pop();
    }
    escaped.join("^")
}

/// HL7 escapes for the delimiters, and line breaks as `\.br\`.
#[must_use]
pub fn esc(text: &str) -> String {
    let mut escaped = String::with_capacity(text.len());
    for character in text.chars() {
        match character {
            '\\' => escaped.push_str("\\E\\"),
            '|' => escaped.push_str("\\F\\"),
            '^' => escaped.push_str("\\S\\"),
            '&' => escaped.push_str("\\T\\"),
            '~' => escaped.push_str("\\R\\"),
            '\r' => {}
            '\n' => escaped.push_str("\\.br\\"),
            other => escaped.push(other),
        }
    }
    escaped
}

#[cfg(test)]
#[path = "hl7_tests.rs"]
mod tests;
