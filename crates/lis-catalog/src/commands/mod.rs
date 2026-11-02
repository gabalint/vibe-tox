//! Use cases that change state. Each checks its capability first, validates,
//! rechecks state inside the transaction, writes and audits.

pub(crate) mod analytes;
pub(crate) mod lifecycle;
pub(crate) mod methods;
pub(crate) mod orderables;
pub(crate) mod reference;
pub(crate) mod review;
pub(crate) mod standard;
pub(crate) mod validity;
