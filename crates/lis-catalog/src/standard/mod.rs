//! The Standard Catalog shipped with the app (REQ-OPS-014, proposed): a
//! versioned JSON file embedded in the build, holding published reference
//! content and published cutoffs with their sources. It is loaded only as
//! drafts, through the ordinary catalog commands; nothing in it is approved
//! until the laboratory reviews it (REQ-OPS-015, proposed).

pub(crate) mod format;
pub(crate) mod state;
pub(crate) mod tree;
mod validate;

use lis_core::{FieldError, text_enum};
use lis_store::{ArtifactDigest, StoreError};
use serde::Serialize;
use ts_rs::TS;

use crate::error::CatalogError;
use crate::standard::format::StandardCatalog;

/// The file this build ships.
const EMBEDDED: &str = include_str!("../../standard/catalog-v1.json");

text_enum! {
    /// Whether a named clinical reviewer has reviewed the file's content version (Q-02).
    StandardReviewStatus {
        /// Not yet reviewed.
        Unreviewed = "unreviewed",
        /// Reviewed by a named clinical reviewer.
        Reviewed = "reviewed",
    }
}

/// A parsed and validated Standard Catalog file with the digest of its bytes.
#[derive(Debug, Clone)]
pub struct StandardFile {
    catalog: StandardCatalog,
    digest: String,
}

impl StandardFile {
    /// The file embedded in this build. A unit test guarantees it is valid,
    /// so an error here is a build defect.
    pub fn embedded() -> Result<Self, CatalogError> {
        Self::parse(EMBEDDED)
    }

    /// Parses and validates a file in format version 1. The engine uses
    /// [`embedded`](Self::embedded); tests use synthetic files.
    pub fn parse(json: &str) -> Result<Self, CatalogError> {
        let catalog: StandardCatalog = serde_json::from_str(json).map_err(|error| {
            CatalogError::StandardFile(FieldError {
                field: "file".to_owned(),
                code: "syntax".to_owned(),
                message: error.to_string(),
            })
        })?;
        validate::validate(&catalog).map_err(|error| {
            if let CatalogError::Invalid(field) = error {
                CatalogError::StandardFile(field)
            } else {
                error
            }
        })?;
        let (digest, _) = ArtifactDigest::of_reader(json.as_bytes()).map_err(StoreError::from)?;
        Ok(Self {
            catalog,
            digest: digest.to_string(),
        })
    }

    /// The file's identity, disclaimer and LOINC notice.
    #[must_use]
    pub fn info(&self) -> StandardCatalogInfo {
        let c = &self.catalog;
        StandardCatalogInfo {
            catalog_version: c.catalog_version.clone(),
            file_digest: self.digest.clone(),
            review_status: c.review_status,
            disclaimer: c.disclaimer.clone(),
            loinc_version: c.loinc.version.clone(),
            loinc_notice: c.loinc.notice.clone(),
        }
    }

    pub(crate) fn catalog(&self) -> &StandardCatalog {
        &self.catalog
    }

    pub(crate) fn digest(&self) -> &str {
        &self.digest
    }
}

/// What the load dialog and the Catalog Preview show about the file. The
/// disclaimer is shown until All Data Reviewed succeeds; the LOINC notice
/// accompanies every copy of LOINC content (DOM-017, DOM-106).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct StandardCatalogInfo {
    /// The content version, such as `1.0.0`.
    pub catalog_version: String,
    /// Lowercase hex SHA-256 of the embedded file's bytes.
    pub file_digest: String,
    /// Whether a named clinical reviewer has reviewed this content version.
    pub review_status: StandardReviewStatus,
    /// The not-for-production disclaimer, verbatim.
    pub disclaimer: String,
    /// The pinned LOINC release.
    pub loinc_version: String,
    /// Regenstrief's notice, verbatim.
    pub loinc_notice: String,
}
