//! The demo data file shipped with the app (`demo/demo-v1.json`; its README is the format
//! contract). Records map one-to-one onto the identity and reporting command inputs; the
//! loader resolves the file-local keys and codes.

use std::collections::BTreeSet;

use lis_catalog::DemoValues;
use lis_core::DecimalText;
use lis_identity::{
    CreateClient, DemographicsInput, IdentifierInput, RegisterPractitioner, SetLaboratoryProfile,
};
use lis_reporting::DeliveryChannel;
use lis_store::ArtifactDigest;
use serde::Deserialize;

/// The file this build ships.
const EMBEDDED: &str = include_str!("../../demo/demo-v1.json");
const FORMAT: &str = "vibe-tox/demo-data";
const FORMAT_VERSION: u32 = 1;
/// The label every demo cutoff carries, so none can pass for a recommendation (decision 3).
pub(crate) const DEMO_CUTOFF_LABEL: &str = "Demo value, not a recommended cutoff";

/// A parsed demo data file.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoFile {
    format: String,
    format_version: u32,
    pub(crate) data_version: String,
    pub(crate) notice: String,
    pub(crate) laboratory_profile: SetLaboratoryProfile,
    pub(crate) clients: Vec<DemoClient>,
    pub(crate) practitioners: Vec<RegisterPractitioner>,
    pub(crate) patients: Vec<DemoPatient>,
    #[serde(default)]
    pub(crate) demo_cutoffs: Vec<DemoCutoff>,
}

/// A client with its locations and delivery destinations.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoClient {
    pub(crate) code: String,
    pub(crate) name: String,
    pub(crate) locations: Vec<DemoLocation>,
    pub(crate) destinations: Vec<DemoDestination>,
}

/// A client location; its code is unique within the client.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoLocation {
    pub(crate) code: String,
    pub(crate) name: String,
}

/// A delivery destination; `location_code` names a location of the same client.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoDestination {
    pub(crate) channel: DeliveryChannel,
    pub(crate) label: String,
    #[serde(default)]
    pub(crate) address: Option<String>,
    #[serde(default)]
    pub(crate) location_code: Option<String>,
}

/// A patient; `key` is file-local, and `acknowledge_candidates` names earlier patients.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoPatient {
    pub(crate) key: String,
    pub(crate) demographics: DemographicsInput,
    pub(crate) source: String,
    pub(crate) identifiers: Vec<IdentifierInput>,
    #[serde(default)]
    pub(crate) acknowledge_candidates: Vec<String>,
}

/// A demo value for a cutoff the Standard Catalog leaves blank.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct DemoCutoff {
    pub(crate) method: String,
    pub(crate) analyte: String,
    pub(crate) value: DecimalText,
    label: String,
}

/// Why the shipped file cannot be used: a build defect, which a unit test prevents.
#[derive(Debug, thiserror::Error)]
#[error("the demo data file is invalid: {0}")]
pub(crate) struct DemoFileError(pub(crate) &'static str);

impl DemoFile {
    /// The file embedded in this build, with the SHA-256 of its bytes.
    pub(crate) fn embedded() -> Result<(Self, String), DemoFileError> {
        let file: Self =
            serde_json::from_str(EMBEDDED).map_err(|_| DemoFileError("not the expected JSON"))?;
        file.check()?;
        let (digest, _) =
            ArtifactDigest::of_reader(EMBEDDED.as_bytes()).map_err(|_| DemoFileError("digest"))?;
        Ok((file, digest.to_string()))
    }

    /// The structural rules the loader relies on; content rules are tested.
    fn check(&self) -> Result<(), DemoFileError> {
        if self.format != FORMAT || self.format_version != FORMAT_VERSION {
            return Err(DemoFileError("unknown format or version"));
        }
        let mut keys = BTreeSet::new();
        for patient in &self.patients {
            let known = patient
                .acknowledge_candidates
                .iter()
                .all(|key| keys.contains(key.as_str()));
            if !known || !keys.insert(patient.key.as_str()) {
                return Err(DemoFileError("patient keys"));
            }
        }
        if !self.clients.iter().all(DemoClient::locations_resolve) {
            return Err(DemoFileError("destination location codes"));
        }
        if self
            .demo_cutoffs
            .iter()
            .any(|c| c.label != DEMO_CUTOFF_LABEL)
        {
            return Err(DemoFileError("demo cutoff label"));
        }
        Ok(())
    }

    /// The demo values by (method code, analyte code).
    pub(crate) fn demo_values(&self) -> DemoValues {
        self.demo_cutoffs
            .iter()
            .map(|c| ((c.method.clone(), c.analyte.clone()), c.value.clone()))
            .collect()
    }
}

impl DemoClient {
    /// Every destination's `location_code` names one of this client's locations.
    fn locations_resolve(&self) -> bool {
        let codes: BTreeSet<&str> = self.locations.iter().map(|l| l.code.as_str()).collect();
        self.destinations
            .iter()
            .filter_map(|destination| destination.location_code.as_deref())
            .all(|code| codes.contains(code))
    }

    /// The engine input that creates this client.
    pub(crate) fn input(&self) -> CreateClient {
        CreateClient {
            code: self.code.clone(),
            name: self.name.clone(),
        }
    }
}
