//! Delivery labels, in one place (brief §4.4, REQ-RPT-011). Every label
//! names the channel and the evidence; none says "Delivered", because staff
//! attest a hand-off, not receipt. The UI shows only these labels.

use crate::model::{
    DeliveryChannel, DeliveryFailureCode, ObligationClosureCause, ObligationStatus,
};

/// Why an obligation is blocked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockedBy {
    /// Nobody has verified the destination yet.
    Unverified,
    /// The destination was retired.
    Retired,
    /// An attempt found the address wrong; it stays blocked until replaced
    /// or retired, and the obligation redirected (REQ-RPT-012).
    ReportedWrong,
}

/// An obligation's status with the facts its label prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DeliveryStatus {
    /// Closed without a hand-off.
    Closed(ObligationClosureCause),
    /// The destination may not be used.
    Blocked(BlockedBy),
    /// Paused while the revision has open impact.
    Paused,
    /// Staff attested the hand-off.
    HandedOff {
        /// Who attested it.
        attested_by: String,
        /// When it was observed, in the laboratory's zone.
        observed_at: String,
    },
    /// The latest attempt failed.
    Failed(DeliveryFailureCode),
    /// The latest attempt's outcome is unknown.
    OutcomeUnknown,
    /// No attempt yet.
    Awaiting,
}

impl DeliveryStatus {
    /// The status code the queue filters on.
    #[must_use]
    pub fn code(&self) -> ObligationStatus {
        match self {
            Self::Closed(_) => ObligationStatus::Closed,
            Self::Blocked(_) => ObligationStatus::Blocked,
            Self::Paused => ObligationStatus::Paused,
            Self::HandedOff { .. } => ObligationStatus::HandedOff,
            Self::Failed(_) | Self::OutcomeUnknown => ObligationStatus::Attention,
            Self::Awaiting => ObligationStatus::Awaiting,
        }
    }
}

/// The one delivery label: "<channel phrase>: <evidence>".
#[must_use]
pub fn delivery_status_label(channel: DeliveryChannel, status: &DeliveryStatus) -> String {
    let evidence = match status {
        DeliveryStatus::Closed(cause) => format!("closed ({})", closure_text(*cause)),
        DeliveryStatus::Blocked(BlockedBy::Unverified) => {
            "blocked (destination not verified)".to_owned()
        }
        DeliveryStatus::Blocked(BlockedBy::Retired) => "blocked (destination retired)".to_owned(),
        DeliveryStatus::Blocked(BlockedBy::ReportedWrong) => {
            "blocked (destination reported wrong)".to_owned()
        }
        DeliveryStatus::Paused => "paused (impact review open)".to_owned(),
        DeliveryStatus::HandedOff {
            attested_by,
            observed_at,
        } => format!("handed off (attested by {attested_by}, {observed_at})"),
        DeliveryStatus::Failed(code) => format!("failed ({})", failure_text(*code)),
        DeliveryStatus::OutcomeUnknown => "outcome unknown".to_owned(),
        DeliveryStatus::Awaiting => "awaiting delivery".to_owned(),
    };
    format!("{}: {evidence}", channel_phrase(channel))
}

/// "PDF exported by J. Doe (not a delivery)".
#[must_use]
pub fn export_label(exported_by: &str) -> String {
    format!("PDF exported by {exported_by} (not a delivery)")
}

fn channel_phrase(channel: DeliveryChannel) -> &'static str {
    match channel {
        DeliveryChannel::Fax => "Fax by staff",
        DeliveryChannel::Email => "Email by staff",
        DeliveryChannel::Mail => "Mailed by staff",
        DeliveryChannel::Courier => "Handed to courier",
        DeliveryChannel::Pickup => "Collected at the laboratory",
    }
}

fn failure_text(code: DeliveryFailureCode) -> &'static str {
    match code {
        DeliveryFailureCode::NoAnswerOrBusy => "busy or no answer",
        DeliveryFailureCode::WrongDestination => "wrong destination",
        DeliveryFailureCode::Returned => "returned",
        DeliveryFailureCode::Refused => "refused",
        DeliveryFailureCode::Other => "other reason",
    }
}

fn closure_text(cause: ObligationClosureCause) -> &'static str {
    match cause {
        ObligationClosureCause::NewerRevision => "newer revision released",
        ObligationClosureCause::Redirected => "redirected",
        ObligationClosureCause::ReportWithdrawn => "report withdrawn",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const CHANNELS: [DeliveryChannel; 5] = [
        DeliveryChannel::Fax,
        DeliveryChannel::Email,
        DeliveryChannel::Mail,
        DeliveryChannel::Courier,
        DeliveryChannel::Pickup,
    ];

    fn every_status() -> Vec<DeliveryStatus> {
        let mut statuses = vec![
            DeliveryStatus::Blocked(BlockedBy::Unverified),
            DeliveryStatus::Blocked(BlockedBy::Retired),
            DeliveryStatus::Blocked(BlockedBy::ReportedWrong),
            DeliveryStatus::Paused,
            DeliveryStatus::HandedOff {
                attested_by: "J. Doe".to_owned(),
                observed_at: "2026-10-12 14:03 CDT".to_owned(),
            },
            DeliveryStatus::OutcomeUnknown,
            DeliveryStatus::Awaiting,
        ];
        for cause in [
            ObligationClosureCause::NewerRevision,
            ObligationClosureCause::Redirected,
            ObligationClosureCause::ReportWithdrawn,
        ] {
            statuses.push(DeliveryStatus::Closed(cause));
        }
        for code in [
            DeliveryFailureCode::NoAnswerOrBusy,
            DeliveryFailureCode::WrongDestination,
            DeliveryFailureCode::Returned,
            DeliveryFailureCode::Refused,
            DeliveryFailureCode::Other,
        ] {
            statuses.push(DeliveryStatus::Failed(code));
        }
        statuses
    }

    // REQ-RPT-011, brief §4.4: every channel × status names the channel and
    // never claims delivery.
    #[test]
    fn every_label_names_its_channel_and_never_says_delivered() {
        for channel in CHANNELS {
            for status in every_status() {
                let label = delivery_status_label(channel, &status);
                assert!(label.starts_with(channel_phrase(channel)), "{label}");
                assert!(!label.to_lowercase().contains("delivered"), "{label}");
            }
        }
        let export = export_label("J. Doe");
        assert!(export.ends_with("(not a delivery)") && !export.contains("Delivered"));
    }

    #[test]
    fn labels_follow_the_brief() {
        assert_eq!(
            delivery_status_label(DeliveryChannel::Fax, &DeliveryStatus::Awaiting),
            "Fax by staff: awaiting delivery"
        );
        assert_eq!(
            delivery_status_label(
                DeliveryChannel::Fax,
                &DeliveryStatus::Failed(DeliveryFailureCode::NoAnswerOrBusy)
            ),
            "Fax by staff: failed (busy or no answer)"
        );
        assert_eq!(
            delivery_status_label(
                DeliveryChannel::Pickup,
                &DeliveryStatus::Blocked(BlockedBy::Unverified)
            ),
            "Collected at the laboratory: blocked (destination not verified)"
        );
    }
}
