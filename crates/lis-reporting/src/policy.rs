//! The fixed M3 release policy, in one place (brief §0). Lab configuration
//! comes later as a catalog reporting-policy version (Q-08). Whether one
//! person may review and release is already the laboratory's
//! separation-of-duties policy (ADR 0015), read where it applies
//! ([`crate::authority`]).

use std::time::Duration;

use lis_core::UserId;
use lis_report::TemplateRef;

/// L5: a candidate may be released within this time after preparation.
pub const CANDIDATE_MAX_AGE: Duration = Duration::from_secs(60 * 60);

/// L13: no attempt is recorded while the revision has open impact.
pub const DELIVERY_PAUSED_WHILE_IMPACT_OPEN: bool = true;

/// The withdrawal-notice template.
#[must_use]
pub fn notice_template() -> TemplateRef {
    TemplateRef {
        id: "report-notice".to_owned(),
        version: 1,
    }
}

/// L3: a partial report is released only with explicit acknowledgement.
pub const PARTIAL_RELEASE_NEEDS_ACKNOWLEDGEMENT: bool = true;

/// L4, REQ-RES-005: a release is a self-release when `releaser` is the
/// technical reviewer of any reported line (`reviewers`). The laboratory's
/// separation-of-duties policy decides whether one may still release it
/// (ADR 0015).
pub(crate) fn is_self_release(releaser: UserId, reviewers: &[UserId]) -> bool {
    reviewers.contains(&releaser)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn releasing_lines_one_reviewed_is_a_self_release() {
        let (releaser, other) = (UserId::generate(), UserId::generate());
        assert!(!is_self_release(releaser, &[other]));
        assert!(is_self_release(releaser, &[other, releaser]));
        assert!(!is_self_release(releaser, &[]));
    }
}
