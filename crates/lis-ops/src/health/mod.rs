//! Host health (REQ-OPS-008, NFR-25, NFR-26; M4 design brief D8, §6.1): the check table,
//! the plain-data [`HealthInputs`], the pure [`evaluate`], the state-change history and
//! attestations.
//!
//! - **Evaluation** is pure: the engine gathers the inputs (probes, owners' queries) and
//!   evaluates every 5 minutes. The overall state is the worst check, with `unknown`
//!   counted as a warning ([`overall`]).
//! - **Attestation** replaces `unknown` with `attested` while valid (at most 90 days), for
//!   the attestable checks only; the web engine and Windows 10 Extended Security Updates
//!   use it to clear their warning and alert.
//! - **History** keeps a row when a check's state changes, plus the first actual
//!   observation after legacy migration establishes the current-state pointer.
//! - **Clock jumps** are recorded, so the clock alert survives a restart and a forward
//!   jump while listening is an acknowledged listening gap (M9).

mod attestations;
mod backup_rules;
mod checks;
mod clock;
pub(crate) mod clock_jumps;
mod evaluate;
mod history;
mod host_rules;
mod inputs;
#[cfg(test)]
mod tests;

pub use attestations::{ATTEST_CHECK, AttestCheckInput, attest_check, valid_attestations};
pub use checks::{ALL_CHECKS, AttestationCode, CheckCode, HealthState, overall};
pub use clock::{ClockStep, classify_clock_step};
pub use clock_jumps::{
    CLOCK_CHANGE, ClockJump, ClockJumpId, last_backward_jump, record_clock_jump,
};
pub use evaluate::{CheckResult, evaluate};
pub use history::{
    HealthEventId, HealthEventView, current_health_event, list_health_history,
    list_health_history_page, record_state_changes,
};
pub use inputs::{
    AnchorFacts, Attestation, BackupFacts, ClockFacts, DiskFacts, HealthInputs, KeepAwakeState,
    KitFacts, ListenerFacts, OutboxFacts,
};
