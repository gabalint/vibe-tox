//! The finding codes of a restore test and a staged restore (M4 design brief §3.6): their
//! one home. A set passes when no finding other than `lost_interval` remains.

lis_core::text_enum! {
    /// What a restore test or staging found wrong with a set.
    FindingCode {
        /// The manifest does not decrypt or parse.
        ManifestUnreadable = "manifest_unreadable",
        /// The set was written under another installation key.
        KeyMismatch = "key_mismatch",
        /// The database file does not decrypt to the manifest's digest and length.
        DatabaseDigestMismatch = "database_digest_mismatch",
        /// The decrypted database fails SQLite's integrity check.
        IntegrityFailed = "integrity_failed",
        /// The set's migration ledger is not a subset of this build's migrations.
        SchemaNewerThanApp = "schema_newer_than_app",
        /// The database does not open with this build's migrations.
        OpenFailed = "open_failed",
        /// An audit chain fails verification (each finding counted).
        ChainBroken = "chain_broken",
        /// A chain head differs from the manifest's.
        HeadMismatch = "head_mismatch",
        /// A seal disagrees with its chain or with the manifest.
        SealInconsistent = "seal_inconsistent",
        /// An anchor the restored chain does not reproduce, or an unreadable anchor line.
        AnchorMismatch = "anchor_mismatch",
        /// An anchor older than the set names an event the set lacks: truncation.
        LocalEventsMissing = "local_events_missing",
        /// An anchor newer than the set or an earlier restore: the expected lost interval.
        LostInterval = "lost_interval",
        /// A seal number below the newest anchor has no anchor in the lineage.
        AnchorGap = "anchor_gap",
        /// A manifest artifact has no file on the target.
        ArtifactMissing = "artifact_missing",
        /// A manifest artifact does not decrypt to its digest and length.
        ArtifactCorrupt = "artifact_corrupt",
        /// The restored database references an artifact the manifest lacks.
        ReferencedArtifactMissing = "referenced_artifact_missing",
    }
}

/// One finding code and how often it was found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Finding {
    /// The code.
    pub code: FindingCode,
    /// How often, at least 1.
    pub count: u64,
}

/// The findings of one check, in the order first found.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Findings(Vec<Finding>);

impl Findings {
    /// Counts `count` more of `code`; zero adds nothing.
    pub(crate) fn add(&mut self, code: FindingCode, count: u64) {
        if count == 0 {
            return;
        }
        match self.0.iter_mut().find(|finding| finding.code == code) {
            Some(finding) => finding.count += count,
            None => self.0.push(Finding { code, count }),
        }
    }

    /// True when nothing but the expected lost interval was found. Unanchored lag is
    /// informational and is not added here; an internal anchor gap is a failure.
    #[must_use]
    pub fn passed(&self) -> bool {
        self.0
            .iter()
            .all(|finding| finding.code == FindingCode::LostInterval)
    }

    /// Every finding.
    #[must_use]
    pub fn list(&self) -> &[Finding] {
        &self.0
    }

    /// How often `code` was found.
    #[must_use]
    pub fn count(&self, code: FindingCode) -> u64 {
        self.0
            .iter()
            .find(|finding| finding.code == code)
            .map_or(0, |finding| finding.count)
    }
}
