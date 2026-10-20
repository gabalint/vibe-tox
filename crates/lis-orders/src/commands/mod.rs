//! Order state-changing commands. Every command authorizes first, rechecks
//! gates inside the transaction and appends its audit event before returning.

mod cancellation;
mod evidence;
mod follow_on;
mod order;
mod order_support;
mod standing;
mod trigger;
pub(crate) mod validation;

pub use cancellation::{decide_cancellation, request_cancellation};
pub use evidence::amend_order_evidence;
pub use follow_on::{decide_follow_on, request_follow_on, request_follow_on_from_result};
pub use order::{
    accept_order, create_billing_review_item, place_clarification_hold, register_order,
    reject_order, resolve_clarification_hold,
};
pub use standing::register_standing_order;
pub use trigger::{record_trigger_not_met, record_trigger_not_met_from_result};
