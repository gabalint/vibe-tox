//! Line dispositions and completeness, the one home of these rules (brief
//! §3.2, REQ-RPT-005, L3). A held or pending analyte holds its whole
//! component: release is at component level.

use crate::model::{LineDisposition, ReportCompleteness};

/// What decides one analyte's part of a component's disposition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyteState {
    /// No current revision.
    NoResult,
    /// A current revision with no technical review.
    Unreviewed,
    /// A current revision whose review is stale.
    ReviewStale,
    /// A current revision with a current approved review.
    Reviewed,
}

/// The facts behind one component's disposition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComponentFacts {
    /// The component, or its item, is cancelled or declined.
    pub cancelled: bool,
    /// Each expected analyte's state.
    pub analytes: Vec<AnalyteState>,
    /// An unresolved quarantine item touches one of its analytes.
    pub quarantine_open: bool,
    /// The order's testing gate passes for the component.
    pub order_gate_passes: bool,
}

/// First match wins: cancelled; any analyte without a result or review →
/// pending; a stale review, open quarantine or failing order gate → held;
/// otherwise reported.
#[must_use]
pub fn component_disposition(facts: &ComponentFacts) -> LineDisposition {
    let any = |state: AnalyteState| facts.analytes.contains(&state);
    if facts.cancelled {
        LineDisposition::Cancelled
    } else if facts.analytes.is_empty()
        || any(AnalyteState::NoResult)
        || any(AnalyteState::Unreviewed)
    {
        LineDisposition::Pending
    } else if any(AnalyteState::ReviewStale) || facts.quarantine_open || !facts.order_gate_passes {
        LineDisposition::Held
    } else {
        LineDisposition::Reported
    }
}

/// A reflex in scope: unassessed → `trigger_pending`; condition not met →
/// `not_activated`; activated → no trigger line (its item is expected).
#[must_use]
pub fn trigger_disposition(outcome: Option<&str>) -> Option<LineDisposition> {
    match outcome {
        None => Some(LineDisposition::TriggerPending),
        Some("activated") => None,
        Some(_) => Some(LineDisposition::NotActivated),
    }
}

/// `final` exactly when every line is reported, cancelled or not activated.
#[must_use]
pub fn completeness(dispositions: impl IntoIterator<Item = LineDisposition>) -> ReportCompleteness {
    let terminal = |disposition: LineDisposition| match disposition {
        LineDisposition::Reported | LineDisposition::Cancelled | LineDisposition::NotActivated => {
            true
        }
        LineDisposition::Pending | LineDisposition::Held | LineDisposition::TriggerPending => false,
    };
    if dispositions.into_iter().all(terminal) {
        ReportCompleteness::Final
    } else {
        ReportCompleteness::Partial
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn facts(analytes: Vec<AnalyteState>) -> ComponentFacts {
        ComponentFacts {
            cancelled: false,
            analytes,
            quarantine_open: false,
            order_gate_passes: true,
        }
    }

    #[test]
    fn dispositions_follow_the_first_matching_rule() {
        use AnalyteState::{NoResult, ReviewStale, Reviewed, Unreviewed};
        let reported = facts(vec![Reviewed, Reviewed]);
        assert_eq!(component_disposition(&reported), LineDisposition::Reported);
        let cancelled = ComponentFacts {
            cancelled: true,
            ..facts(vec![NoResult])
        };
        assert_eq!(
            component_disposition(&cancelled),
            LineDisposition::Cancelled
        );
        assert_eq!(
            component_disposition(&facts(vec![Reviewed, NoResult])),
            LineDisposition::Pending
        );
        assert_eq!(
            component_disposition(&facts(vec![Reviewed, Unreviewed])),
            LineDisposition::Pending
        );
        // L3: one stale analyte holds the whole component.
        assert_eq!(
            component_disposition(&facts(vec![Reviewed, ReviewStale])),
            LineDisposition::Held
        );
        let quarantined = ComponentFacts {
            quarantine_open: true,
            ..reported.clone()
        };
        assert_eq!(component_disposition(&quarantined), LineDisposition::Held);
        let gated = ComponentFacts {
            order_gate_passes: false,
            ..reported
        };
        assert_eq!(component_disposition(&gated), LineDisposition::Held);
    }

    #[test]
    fn triggers_and_completeness() {
        assert_eq!(
            trigger_disposition(None),
            Some(LineDisposition::TriggerPending)
        );
        assert_eq!(
            trigger_disposition(Some("condition_not_met")),
            Some(LineDisposition::NotActivated)
        );
        assert_eq!(trigger_disposition(Some("activated")), None);
        use LineDisposition::{Cancelled, Held, NotActivated, Reported, TriggerPending};
        assert_eq!(
            completeness([Reported, Cancelled, NotActivated]),
            ReportCompleteness::Final
        );
        assert_eq!(completeness([Reported, Held]), ReportCompleteness::Partial);
        assert_eq!(
            completeness([Reported, TriggerPending]),
            ReportCompleteness::Partial
        );
    }
}
