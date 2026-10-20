//! Audit action names of the reporting module; the engine's command
//! registry uses the same names.

/// T1 of preparation: the candidate and its lines are stored.
pub const PREPARE_CANDIDATE: &str = "report.prepare_candidate";
/// T2 of preparation: the PDF is stored and the candidate is ready.
pub const RENDER_CANDIDATE: &str = "report.render_candidate";
/// T2 of preparation when rendering or storing failed.
pub const RENDER_FAILED: &str = "report.render_failed";
/// Release: the release manifest in the audit chain (`rpt_revisions` trigger).
pub const RELEASE: &str = "report.release";
/// Report-level, terminal withdrawal.
pub const WITHDRAW: &str = "report.withdraw";
/// A rendered withdrawal notice made durable before the withdrawal transaction.
pub const STAGE_WITHDRAWAL_NOTICE: &str = "report.stage_withdrawal_notice";
/// A quality decision on impact after release.
pub const ASSESS_IMPACT: &str = "report.assess_impact";
/// An issued file found missing or damaged.
pub const PRESERVATION_INCIDENT: &str = "report.preservation_incident";
/// A preservation incident closed with a resolution.
pub const CLOSE_PRESERVATION_INCIDENT: &str = "report.close_preservation_incident";
/// Destination lifecycle.
pub const CREATE_DESTINATION: &str = "report.create_destination";
/// A destination verified: by a second person, or by its creator where the
/// separation-of-duties policy allows it (marked as a self-approval, ADR 0015).
pub const VERIFY_DESTINATION: &str = "report.verify_destination";
/// A destination replaced by a new, unverified one.
pub const REPLACE_DESTINATION: &str = "report.replace_destination";
/// A destination retired.
pub const RETIRE_DESTINATION: &str = "report.retire_destination";
/// An obligation added by staff.
pub const ADD_OBLIGATION: &str = "report.add_obligation";
/// An open obligation moved to another destination.
pub const REDIRECT_OBLIGATION: &str = "report.redirect_obligation";
/// An attested delivery attempt.
pub const RECORD_DELIVERY: &str = "report.record_delivery";
/// A PDF export (security stream).
pub const EXPORT_PDF: &str = "report.export_pdf";
