//! Wall-clock jump detection (NFR-26, M4 design brief §6.1): every light-loop tick
//! compares how far the wall clock moved with how far the monotonic clock moved. Pure,
//! so the engine passes both readings.

use std::time::Duration;

use lis_core::UtcTimestamp;

/// What one tick says about the wall clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClockStep {
    /// The two clocks agree within the threshold.
    Steady,
    /// The wall clock moved back against the monotonic clock: an alert for 24 hours.
    Backward {
        /// By how many seconds.
        seconds: u64,
    },
    /// The wall clock moved ahead: sleep or a clock change, recorded as a gap.
    Forward {
        /// By how many seconds.
        seconds: u64,
    },
}

/// Classifies the step between two wall-clock readings `monotonic_elapsed` apart. A
/// discrepancy larger than `threshold_seconds` (the `clock_jump_seconds` setting) is a
/// jump; smaller ones are ordinary drift and synchronization.
#[must_use]
pub fn classify_clock_step(
    wall_before: UtcTimestamp,
    wall_now: UtcTimestamp,
    monotonic_elapsed: Duration,
    threshold_seconds: u32,
) -> ClockStep {
    let wall = i128::from(wall_now.unix_seconds()) - i128::from(wall_before.unix_seconds());
    let monotonic = i128::from(monotonic_elapsed.as_secs());
    let discrepancy = wall - monotonic;
    let seconds = u64::try_from(discrepancy.unsigned_abs()).unwrap_or(u64::MAX);
    if seconds <= u64::from(threshold_seconds) {
        ClockStep::Steady
    } else if discrepancy < 0 {
        ClockStep::Backward { seconds }
    } else {
        ClockStep::Forward { seconds }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(seconds: u64) -> UtcTimestamp {
        lis_store::testing::test_start()
            .checked_add(Duration::from_secs(seconds))
            .unwrap()
    }

    #[test]
    fn drift_up_to_the_threshold_is_steady() {
        let tick = Duration::from_secs(15);
        assert_eq!(
            classify_clock_step(at(100), at(115), tick, 60),
            ClockStep::Steady
        );
        assert_eq!(
            classify_clock_step(at(100), at(175), tick, 60),
            ClockStep::Steady
        );
        assert_eq!(
            classify_clock_step(at(100), at(55), tick, 60),
            ClockStep::Steady
        );
    }

    #[test]
    fn larger_discrepancies_are_backward_or_forward_jumps() {
        let tick = Duration::from_secs(15);
        assert_eq!(
            classify_clock_step(at(100), at(176), tick, 60),
            ClockStep::Forward { seconds: 61 }
        );
        assert_eq!(
            classify_clock_step(at(100), at(54), tick, 60),
            ClockStep::Backward { seconds: 61 }
        );
        assert_eq!(
            classify_clock_step(at(3700), at(100), tick, 60),
            ClockStep::Backward { seconds: 3615 }
        );
    }
}
