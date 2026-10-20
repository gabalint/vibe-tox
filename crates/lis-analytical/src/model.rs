//! Identifiers, coded enums and the pure rules over them. No database.

use lis_core::{Id, UtcTimestamp, text_enum};

use crate::error::AnalyticalError;

/// Marker for [`StatusEventId`].
pub(crate) enum StatusEvent {}
/// One instrument status event.
pub(crate) type StatusEventId = Id<StatusEvent>;
/// Marker for [`SourceRegistrationId`].
pub enum SourceRegistration {}
/// An ASTM source registration of one instrument.
pub type SourceRegistrationId = Id<SourceRegistration>;
/// Marker for [`LotId`].
pub enum Lot {}
/// A reagent, calibrator, control or internal-standard lot.
pub type LotId = Id<Lot>;
/// Marker for [`PrepBatchId`].
pub enum PrepBatch {}
/// A preparation batch.
pub type PrepBatchId = Id<PrepBatch>;
/// Marker for [`PrepStepId`].
pub enum PrepStep {}
/// One step of a preparation batch.
pub type PrepStepId = Id<PrepStep>;
/// Marker for [`RunId`].
pub enum Run {}
/// An instrument run.
pub type RunId = Id<Run>;
/// Marker for [`ExecutionId`].
pub enum Execution {}
/// One test execution of patient material.
pub type ExecutionId = Id<Execution>;
/// Marker for [`PositionId`].
pub enum Position {}
/// A run position.
pub type PositionId = Id<Position>;
/// Marker for [`QcObservationId`].
pub enum QcObservation {}
/// One QC observation as received.
pub type QcObservationId = Id<QcObservation>;
/// Marker for [`QcAssessmentId`].
pub enum QcAssessment {}
/// One QC assessment.
pub type QcAssessmentId = Id<QcAssessment>;
/// Marker for [`QcConflictId`].
pub enum QcConflict {}
/// A changed QC row awaiting a person's decision.
pub type QcConflictId = Id<QcConflict>;
/// Marker for [`DeviationId`].
pub enum Deviation {}
/// A quality deviation.
pub type DeviationId = Id<Deviation>;

text_enum! {
    /// Whether an instrument may be used (REQ-ANA-007).
    InstrumentStatus {
        /// Ready for runs.
        InService = "in_service",
        /// Not to be used.
        OutOfService = "out_of_service",
    }
}

text_enum! {
    /// The ASTM order field that carries the sample label (O.3 or O.4).
    SampleIdField {
        /// O.3, the specimen ID.
        SpecimenId = "specimen_id",
        /// O.4, the instrument specimen ID.
        InstrumentSpecimenId = "instrument_specimen_id",
    }
}

text_enum! {
    /// How a source marks a corrected re-export (brief §3.5, R5).
    RevisionIndicator {
        /// The source has no correction marker: every changed row is a conflict.
        None = "none",
        /// A changed row with result status `C` is a correction.
        ResultStatusCorrection = "result_status_correction",
    }
}

text_enum! {
    /// Lifecycle of a source registration.
    RegistrationState {
        /// Recorded, not yet usable.
        Draft = "draft",
        /// The instrument's one usable registration.
        Active = "active",
        /// Replaced by a newer approval.
        Retired = "retired",
    }
}

text_enum! {
    /// What a lot contains.
    MaterialKind {
        /// A reagent.
        Reagent = "reagent",
        /// A calibrator.
        Calibrator = "calibrator",
        /// A control material.
        Control = "control",
        /// An internal standard.
        InternalStandard = "internal_standard",
    }
}

text_enum! {
    /// Whether a lot may be used for new work.
    LotState {
        /// Usable until its expiry date.
        Active = "active",
        /// Taken out of use; past work keeps its link.
        Withdrawn = "withdrawn",
    }
}

text_enum! {
    /// Whether a preparation batch still takes steps (REQ-ANA-001).
    PrepBatchState {
        /// Steps are being recorded.
        Open = "open",
        /// Frozen: no more steps; it can be assessed.
        Completed = "completed",
    }
}

text_enum! {
    /// Whether a run accepts positions and imports.
    RunState {
        /// Accepts positions and imports.
        Open = "open",
        /// Closed.
        Closed = "closed",
    }
}

text_enum! {
    /// Why an execution exists (REQ-ANA-006).
    AttemptKind {
        /// The first execution for its components.
        Initial = "initial",
        /// The same preparation measured again.
        Rerun = "rerun",
        /// The material prepared again.
        Reextraction = "reextraction",
        /// A diluted measurement; the source must report final concentration.
        Dilution = "dilution",
    }
}

text_enum! {
    /// Where an execution stands.
    ExecutionState {
        /// In progress or completed; acquired data is derived from results.
        Active = "active",
        /// Failed, with a reason.
        Failed = "failed",
    }
}

text_enum! {
    /// What a run position holds.
    PositionRole {
        /// Patient material, through an execution.
        Patient = "patient",
        /// A control lot.
        Control = "control",
        /// A calibrator lot.
        Calibrator = "calibrator",
        /// A blank.
        Blank = "blank",
    }
}

text_enum! {
    /// The non-patient roles of [`add_qc_position`](crate::add_qc_position).
    QcRole {
        /// A control lot.
        Control = "control",
        /// A calibrator lot.
        Calibrator = "calibrator",
        /// A blank, without a lot.
        Blank = "blank",
    }
}

text_enum! {
    /// What a QC assessment covers (REQ-ANA-004).
    QcScope {
        /// One analyte of one run.
        RunAnalyte = "run_analyte",
        /// A whole run.
        Run = "run",
        /// A preparation batch.
        PrepBatch = "prep_batch",
    }
}

text_enum! {
    /// The decision of a QC assessment. There is no waiver (REQ-ANA-005).
    QcOutcome {
        /// Acceptable.
        Accepted = "accepted",
        /// Not acceptable; never relabeled as accepted.
        Rejected = "rejected",
    }
}

text_enum! {
    /// Where the QC of one run × analyte stands for result review
    /// (REQ-ANA-004, REQ-ANA-005); see `dependency::qc_status`.
    QcStatus {
        /// No assessment yet.
        Pending = "pending",
        /// Accepted, but the current observations or an open QC conflict
        /// differ from what the assessment cited.
        Stale = "stale",
        /// The current assessment rejects it.
        Rejected = "rejected",
        /// The current assessment accepts every current observation.
        Accepted = "accepted",
    }
}

text_enum! {
    /// Where a changed QC row stands (brief §3.5, R5).
    QcConflictState {
        /// Awaiting a decision; blocks dependent results.
        Open = "open",
        /// Applied as a correction that supersedes the contradicted observation.
        Accepted = "accepted",
        /// Not applied; the contradicted observation stays current.
        Rejected = "rejected",
    }
}

text_enum! {
    /// A person's decision on a changed QC row.
    QcConflictResolution {
        /// Store the row as a correction of the observation it contradicts.
        AcceptAsCorrection = "accept_as_correction",
        /// Keep the stored observation; the row is not applied.
        Reject = "reject",
    }
}

text_enum! {
    /// What a deviation applies to.
    DeviationScope {
        /// An instrument.
        Instrument = "instrument",
        /// A lot.
        Lot = "lot",
        /// A preparation batch.
        PrepBatch = "prep_batch",
        /// A run.
        Run = "run",
        /// One run position.
        Position = "position",
    }
}

text_enum! {
    /// Whether a deviation still blocks dependent results.
    DeviationState {
        /// Blocks dependent results.
        Open = "open",
        /// Closed with a disposition.
        Closed = "closed",
    }
}

impl QcRole {
    /// The stored position role.
    #[must_use]
    pub(crate) fn position_role(self) -> PositionRole {
        match self {
            Self::Control => PositionRole::Control,
            Self::Calibrator => PositionRole::Calibrator,
            Self::Blank => PositionRole::Blank,
        }
    }

    /// The lot kind this role needs, or `None` for a blank, which has no lot.
    pub(crate) fn lot_kind(self) -> Option<MaterialKind> {
        match self {
            Self::Control => Some(MaterialKind::Control),
            Self::Calibrator => Some(MaterialKind::Calibrator),
            Self::Blank => None,
        }
    }
}

/// A calendar date `YYYY-MM-DD`, such as a lot's expiry date.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CivilDate(jiff::civil::Date);

impl CivilDate {
    /// Parses `YYYY-MM-DD`; the error names `field`.
    pub(crate) fn parse(field: &str, text: &str) -> Result<Self, AnalyticalError> {
        let valid = text.len() == 10 && text.as_bytes().get(4) == Some(&b'-');
        match text.parse::<jiff::civil::Date>() {
            Ok(date) if valid => Ok(Self(date)),
            Ok(_) | Err(_) => Err(crate::error::invalid(
                field,
                "invalid_date",
                "Enter a date as YYYY-MM-DD.",
            )),
        }
    }

    /// The UTC calendar date of `at`. Until the laboratory time zone exists
    /// (M3), dates are UTC: for US laboratories the UTC date is never behind
    /// the local date, so a lot expires early rather than late.
    pub(crate) fn utc_date_of(at: UtcTimestamp) -> Self {
        // Every `UtcTimestamp` is in jiff's range; the fallback (the latest
        // date) would make every lot count as expired, never the reverse.
        Self(
            jiff::Timestamp::from_second(at.unix_seconds())
                .map_or(jiff::civil::Date::MAX, |instant| {
                    instant.to_zoned(jiff::tz::TimeZone::UTC).date()
                }),
        )
    }

    /// The stored text.
    pub(crate) fn to_text(self) -> String {
        self.0.to_string()
    }
}

/// A lot is usable through its expiry date and expired after it (REQ-ANA-007).
pub(crate) fn is_expired(expires_on: CivilDate, at: UtcTimestamp) -> bool {
    CivilDate::utc_date_of(at) > expires_on
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(text: &str) -> UtcTimestamp {
        UtcTimestamp::parse(text).unwrap()
    }

    #[test]
    fn a_lot_is_usable_through_its_expiry_date() {
        let expiry = CivilDate::parse("expiresOn", "2026-03-31").unwrap();
        assert!(!is_expired(expiry, at("2026-03-31T23:59:59.000000000Z")));
        assert!(is_expired(expiry, at("2026-04-01T00:00:00.000000000Z")));
        assert!(!is_expired(expiry, at("2025-12-31T12:00:00.000000000Z")));
    }

    #[test]
    fn dates_must_be_complete_iso_dates() {
        for bad in [
            "2026-3-31",
            "2026-02-30",
            "31/03/2026",
            "",
            "2026-03-31T00:00",
        ] {
            assert!(CivilDate::parse("expiresOn", bad).is_err(), "{bad}");
        }
        assert_eq!(
            CivilDate::parse("expiresOn", "2026-03-31")
                .unwrap()
                .to_text(),
            "2026-03-31"
        );
    }

    #[test]
    fn qc_roles_name_their_lot_kind() {
        assert_eq!(QcRole::Control.lot_kind(), Some(MaterialKind::Control));
        assert_eq!(
            QcRole::Calibrator.lot_kind(),
            Some(MaterialKind::Calibrator)
        );
        assert_eq!(QcRole::Blank.lot_kind(), None);
        assert_eq!(QcRole::Blank.position_role(), PositionRole::Blank);
    }
}
