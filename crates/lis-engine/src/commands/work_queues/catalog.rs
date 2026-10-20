//! Static work-queue catalog. Workflow selection remains with each owner.

use lis_core::Capability;

#[derive(Clone, Copy)]
pub(super) struct QueueDefinition {
    pub(super) code: &'static str,
    pub(super) label: &'static str,
    pub(super) capability: Capability,
    pub(super) next_action: &'static str,
}

macro_rules! queue {
    ($code:literal, $label:literal, $capability:ident, $next:literal) => {
        QueueDefinition {
            code: $code,
            label: $label,
            capability: Capability::$capability,
            next_action: $next,
        }
    };
}

pub(super) const QUEUES: &[QueueDefinition] = &[
    queue!(
        "orders_needing_decision",
        "Orders needing a decision",
        EnterOrders,
        "Accept, hold or reject"
    ),
    queue!(
        "orders_on_hold",
        "Orders on hold",
        EnterOrders,
        "Resolve the hold"
    ),
    queue!("identity_holds", "Identity holds", ManageQuality, "Resolve"),
    queue!(
        "specimen_decisions",
        "Specimens awaiting a decision",
        Accession,
        "Record a receipt decision"
    ),
    queue!(
        "label_failures",
        "Label print failures",
        Accession,
        "Reprint"
    ),
    queue!(
        "collection_time_review",
        "Collection timing review",
        ManageQuality,
        "Assess corrected collection timing"
    ),
    queue!("qc_review", "QC requiring review", ManageQuality, "Assess"),
    queue!("open_deviations", "Open deviations", ManageQuality, "Close"),
    queue!(
        "review_ready",
        "Results ready for review",
        ReviewResults,
        "Review"
    ),
    queue!(
        "review_stale",
        "Stale result reviews",
        ReviewResults,
        "Review again"
    ),
    queue!(
        "review_blocked",
        "Result review blocked",
        ReviewResults,
        "Resolve gates"
    ),
    queue!(
        "import_quarantine",
        "Open import quarantine",
        ReviewResults,
        "Resolve"
    ),
    queue!(
        "performed_not_ordered",
        "Performed but not ordered",
        ReviewResults,
        "Resolve"
    ),
    queue!(
        "release_ready",
        "Reports ready for release",
        ReleaseReports,
        "Prepare and release"
    ),
    queue!(
        "release_blocked",
        "Report release blocked",
        ReleaseReports,
        "Resolve gates"
    ),
    queue!(
        "partial_reports",
        "Partial reports outstanding",
        ReleaseReports,
        "Complete"
    ),
    queue!(
        "impact_review",
        "Report impact review",
        ManageQuality,
        "Assess"
    ),
    queue!(
        "correction_due",
        "Report corrections due",
        ReleaseReports,
        "Issue a correction"
    ),
    queue!(
        "delivery_work",
        "Report delivery work",
        DeliverReports,
        "Reconcile delivery"
    ),
    queue!(
        "reports_without_destination",
        "Reports without a destination",
        DeliverReports,
        "Add a destination"
    ),
    queue!(
        "unverified_destinations",
        "Delivery destinations to verify or replace",
        ManageConfiguration,
        "Verify"
    ),
    queue!(
        "preservation_incidents",
        "Report preservation incidents",
        ManageQuality,
        "Investigate"
    ),
    queue!(
        "billing_work",
        "Billing work",
        Billing,
        "Derive, review or reconcile"
    ),
    queue!(
        "outbox_dead_letters",
        "Outbox dead letters",
        ManageConfiguration,
        "Investigate"
    ),
    queue!(
        "ops_alerts",
        "Health checks needing attention",
        ManageConfiguration,
        "Open the health page"
    ),
    queue!(
        "listening_gaps",
        "Listening gaps to acknowledge",
        PerformAnalysis,
        "Check for missing results and acknowledge"
    ),
    queue!(
        "recovery_reconciliation",
        "Recovery reconciliation after a restore",
        ManageQuality,
        "Reconcile and close"
    ),
    queue!(
        "version_acceptance",
        "Version awaiting acceptance",
        ApproveConfiguration,
        "Accept for clinical use"
    ),
    queue!(
        "results_expected",
        "Results expected from analyzers",
        PerformAnalysis,
        "Check the analyzer and resend"
    ),
];

pub(super) fn queue_definition(code: &str) -> Option<QueueDefinition> {
    QUEUES.iter().find(|queue| queue.code == code).copied()
}

/// Plain laboratory wording for every work-queue state. Unknown gate codes
/// still become readable words instead of leaking a machine token to the UI.
pub(super) fn state_label(code: &str) -> String {
    let known = match code {
        "received" => "Received, awaiting a decision",
        "held" => "Held",
        "possible_duplicate" => "Possible duplicate",
        "requestor_missing" => "Requestor missing",
        "test_selection_missing" => "Test selection missing",
        "signature_or_intent_required" => "Signature or intent to order required",
        "standing_order_expired" => "Standing order expired",
        "standing_order_exhausted" => "Standing order used up",
        "standing_order_out_of_scope" => "Test outside the standing order",
        "reflex_authority_missing" => "Reflex authority missing",
        "other_authority" => "Other authority question",
        "printer_inactive" => "Printer not in service",
        "printer_misconfigured" => "Printer set up incorrectly",
        "printer_unavailable" => "Printer unavailable",
        "render_failed" => "Label could not be drawn",
        "invalid_queue" => "Print queue not found",
        "connection_refused" => "Printer refused the connection",
        "connect_timeout" => "Printer did not answer",
        "write_timeout" => "Printer stopped answering",
        "destination_refused" => "Printer refused the label",
        "network_error" => "Network error",
        "spooler_unavailable" => "Print service unavailable",
        "spooler_rejected" => "Print service refused the label",
        "spooler_timeout" => "Print service did not answer",
        "pending" => "Not yet assessed",
        "stale" => "Out of date",
        "rejected" => "Rejected",
        "open" => "Open",
        "ready" => "Ready",
        "blocked" => "Blocked",
        "awaiting_result" => "Awaiting result",
        "unknown_sample" => "Unknown sample",
        "ambiguous_sample" => "More than one sample matches",
        "run_closed" => "Run closed before the result arrived",
        "execution_failed" => "Analysis attempt failed",
        "unknown_analyte" => "Test code not mapped",
        "incompatible_unit" => "Units differ from the method",
        "malformed_value" => "Value could not be read",
        "scale_mismatch" => "Value form does not match the method",
        "conflicting_duplicate" => "A different value arrived for the same row",
        "dilution_not_supported" => "Dilution not supported by the source",
        "malformed_row" => "Row does not match the file's columns",
        "role_mismatch" => "Sample type differs from the run position",
        "dilution_mismatch" => "Dilution differs from the analysis attempt",
        "not_in_ordered_scope" => "Not ordered",
        "partial_reports" => "Partial report",
        "impact_review" => "Impact review open",
        "correction_due" => "Correction due",
        "reports_without_destination" => "No delivery destination",
        "awaiting" => "Awaiting delivery",
        "attention" => "Failed or outcome unknown",
        "paused" => "Paused for impact review",
        "handed_off" => "Handed off",
        "unverified" => "Not verified",
        "reported_wrong" => "Reported wrong",
        "missing" => "File missing",
        "corrupt" => "File damaged",
        "needs_derivation" => "Needs derivation",
        "candidate" => "Ready for approval",
        "approved" => "Approved",
        "outstanding" => "Awaiting the billing owner's outcome",
        "dead" => "Stopped after retries",
        "warning" => "Warning",
        "alert" => "Alert",
        "unacknowledged" => "Not acknowledged",
        "unapproved" => "Not approved",
        "awaiting_acceptance" => "Awaiting acceptance",
        _ => return machine_code_label(code),
    };
    known.to_owned()
}

fn machine_code_label(code: &str) -> String {
    let mut label = code.replace('_', " ");
    if let Some(first) = label.get_mut(0..1) {
        first.make_ascii_uppercase();
    }
    label
}

#[cfg(test)]
mod tests {
    use super::state_label;

    #[test]
    fn states_and_gate_codes_have_plain_labels() {
        assert_eq!(state_label("awaiting_result"), "Awaiting result");
        assert_eq!(state_label("identity_hold_open"), "Identity hold open");
        assert!(!state_label("identity_hold_open").contains('_'));
    }
}
