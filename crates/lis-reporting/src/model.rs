//! Identifiers and closed code sets of the reporting module. Each code set
//! matches a `CHECK` in `migrations/0001_reporting.sql`.

use lis_core::{Id, text_enum};

/// Marker for [`ReportId`].
pub enum Report {}
/// One report per specimen (L1).
pub type ReportId = Id<Report>;
/// Marker for [`CandidateId`].
pub enum Candidate {}
/// A prepared, rendered and not yet released revision.
pub type CandidateId = Id<Candidate>;
/// Marker for [`ReportRevisionId`].
pub enum ReportRevision {}
/// A released, immutable revision of a report.
pub type ReportRevisionId = Id<ReportRevision>;
/// Marker for [`DestinationId`].
pub enum Destination {}
/// A verified-by-a-second-person place a report goes to (REQ-RPT-012).
pub type DestinationId = Id<Destination>;
/// Marker for [`ObligationId`].
pub enum Obligation {}
/// The duty to deliver one revision, or a withdrawal notice, to one destination.
pub type ObligationId = Id<Obligation>;
/// Marker for [`DeliveryAttemptId`].
pub enum DeliveryAttempt {}
/// One attested delivery attempt (REQ-RPT-011).
pub type DeliveryAttemptId = Id<DeliveryAttempt>;
/// Marker for [`ExportId`].
pub enum Export {}
/// One PDF export of an issued file.
pub type ExportId = Id<Export>;
/// Marker for [`ImpactAssessmentId`].
pub enum ImpactAssessment {}
/// A quality decision on the impact of changes after release.
pub type ImpactAssessmentId = Id<ImpactAssessment>;
/// Marker for [`PreservationIncidentId`].
pub enum PreservationIncident {}
/// An issued file found missing or damaged (NFR-07).
pub type PreservationIncidentId = Id<PreservationIncident>;

text_enum! {
    /// How a revision relates to the previous one (REQ-RPT-002).
    #[ts(export)]
    ReportRelationship {
        /// The first revision.
        Initial = "initial",
        /// Only lines not reported before become reported.
        Supplement = "supplement",
        /// Reported content changed; needs a reason.
        Correction = "correction",
    }
}

text_enum! {
    /// Whether every expected line is resolved (REQ-RPT-005).
    #[ts(export)]
    ReportCompleteness {
        /// Some line is pending, held or awaiting a trigger decision.
        Partial = "partial",
        /// Every line is reported, cancelled or not activated.
        Final = "final",
    }
}

text_enum! {
    /// The stored state of a candidate.
    #[ts(export)]
    CandidateState {
        /// Prepared; its PDF is being rendered.
        Rendering = "rendering",
        /// Rendered and stored; may be released.
        Ready = "ready",
        /// Released as a revision.
        Released = "released",
        /// Replaced by a newer candidate.
        Superseded = "superseded",
        /// Rendering or storing the PDF failed.
        Failed = "failed",
    }
}

text_enum! {
    /// Why a line is or is not reported (REQ-RPT-005).
    #[ts(export)]
    LineDisposition {
        /// Current, reviewed and released on this revision.
        Reported = "reported",
        /// A result or its review is missing.
        Pending = "pending",
        /// A stale review, open quarantine or order gate holds it.
        Held = "held",
        /// The component or its item was cancelled.
        Cancelled = "cancelled",
        /// A reflex whose condition was not met.
        NotActivated = "not_activated",
        /// A reflex awaiting its trigger assessment.
        TriggerPending = "trigger_pending",
    }
}

text_enum! {
    /// What a candidate line stands for.
    LineKind {
        /// One analyte of an expected component.
        Result = "result",
        /// A reflex orderable in scope of a parent item.
        Trigger = "trigger",
    }
}

text_enum! {
    /// Staff-operated delivery channels (REQ-RPT-003 local).
    #[ts(export)]
    DeliveryChannel {
        /// Fax sent by staff.
        Fax = "fax",
        /// Email sent by staff.
        Email = "email",
        /// Mail sent by staff.
        Mail = "mail",
        /// Handed to a courier.
        Courier = "courier",
        /// Collected at the laboratory.
        Pickup = "pickup",
    }
}

text_enum! {
    /// What a delivery attempt shows (REQ-RPT-011). Never "delivered".
    #[ts(export)]
    DeliveryEvidence {
        /// Staff attest it left the laboratory by the channel.
        HandedOff = "handed_off",
        /// It did not go out.
        Failed = "failed",
        /// Nobody knows whether it went out.
        OutcomeUnknown = "outcome_unknown",
    }
}

text_enum! {
    /// Why an attempt failed.
    #[ts(export)]
    DeliveryFailureCode {
        /// Busy or no answer.
        NoAnswerOrBusy = "no_answer_or_busy",
        /// The address reached someone else.
        WrongDestination = "wrong_destination",
        /// Returned to the laboratory.
        Returned = "returned",
        /// The recipient refused it.
        Refused = "refused",
        /// Another reason, in the note.
        Other = "other",
    }
}

text_enum! {
    /// What an obligation delivers.
    #[ts(export)]
    ObligationKind {
        /// The revision's PDF.
        Report = "report",
        /// The withdrawal notice.
        WithdrawalNotice = "withdrawal_notice",
    }
}

text_enum! {
    /// Why an obligation exists.
    #[ts(export)]
    ObligationOrigin {
        /// Routing at release (L11).
        Release = "release",
        /// A destination of an earlier revision (REQ-RPT-013).
        CorrectionFollowUp = "correction_follow_up",
        /// A withdrawal notice.
        Withdrawal = "withdrawal",
        /// Added by staff, with a reason.
        Added = "added",
        /// Moved from another obligation, with a reason.
        Redirected = "redirected",
    }
}

text_enum! {
    /// Why an obligation was closed without a hand-off.
    ObligationClosureCause {
        /// A newer revision was released.
        NewerRevision = "newer_revision",
        /// Redirected to another destination.
        Redirected = "redirected",
        /// The report was withdrawn.
        ReportWithdrawn = "report_withdrawn",
    }
}

text_enum! {
    /// Why a PDF was exported.
    #[ts(export)]
    ExportPurpose {
        /// To deliver it by a staff-operated channel.
        Delivery = "delivery",
        /// For records.
        Records = "records",
        /// Internal use.
        Internal = "internal",
    }
}

text_enum! {
    /// A quality decision on impact after release.
    #[ts(export)]
    ImpactDecision {
        /// The released content stands.
        NoImpact = "no_impact",
        /// The signatory must issue a correction.
        CorrectionRequired = "correction_required",
    }
}

text_enum! {
    /// How an issued file failed verification.
    #[ts(export)]
    PreservationIncidentKind {
        /// The file is gone.
        Missing = "missing",
        /// The file's digest no longer matches.
        Corrupt = "corrupt",
    }
}

text_enum! {
    /// How a preservation incident was closed (NFR-07).
    #[ts(export)]
    IncidentResolution {
        /// The file or record was restored intact from a backup.
        ArtifactRestoredFromBackup = "artifact_restored_from_backup",
        /// A new revision was issued in place of the damaged one.
        RevisionReissued = "revision_reissued",
        /// The loss is accepted; later detections of it add no incident.
        AcceptedAsLost = "accepted_as_lost",
    }
}

text_enum! {
    /// Which stored record of an issued report failed its digest.
    #[ts(export)]
    StoredRecord {
        /// The PDF bytes.
        Pdf = "pdf",
        /// The candidate's canonical content (`content_json`).
        ContentJson = "content_json",
        /// The candidate's printed data (`data_json`).
        DataJson = "data_json",
    }
}

text_enum! {
    /// A revision's computed availability.
    #[ts(export)]
    RevisionAvailability {
        /// The latest revision of a report that is not withdrawn.
        Current = "current",
        /// A later revision exists.
        Superseded = "superseded",
        /// The report was withdrawn.
        Withdrawn = "withdrawn",
    }
}

text_enum! {
    /// An obligation's computed status; the first match wins (brief §2.1).
    #[ts(export)]
    ObligationStatus {
        /// Closed by a newer revision, a redirect or a withdrawal.
        Closed = "closed",
        /// The destination is unverified or retired.
        Blocked = "blocked",
        /// The revision has open impact (L13).
        Paused = "paused",
        /// An attempt was handed off.
        HandedOff = "handed_off",
        /// The latest attempt failed or has an unknown outcome.
        Attention = "attention",
        /// No attempt yet.
        Awaiting = "awaiting",
    }
}
