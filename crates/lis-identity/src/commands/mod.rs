//! Use cases that change state. Each checks its capability first, validates,
//! rechecks state inside the transaction, writes and audits.

pub(crate) mod holds;
pub(crate) mod laboratory;
pub(crate) mod organizations;
pub(crate) mod subjects;
