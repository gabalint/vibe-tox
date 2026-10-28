//! Reporting reads. They take a connection (a read connection, or the
//! caller's transaction) and perform no authorization: the engine checks
//! the capability, and audits the read where the result holds PHI.

pub(crate) mod billing;
pub(crate) mod candidate;
pub(crate) mod delivery;
pub(crate) mod destinations;
pub(crate) mod incidents;
pub(crate) mod layouts;
pub(crate) mod queues;
pub(crate) mod report;
pub(crate) mod workspace;

pub(crate) mod obligation;
