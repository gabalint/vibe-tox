//! Shared kernel for the LIS engine. Pure types only: no I/O beyond the
//! `rusqlite` value conversions.
//!
//! - [`Id`]: typed UUIDv7 identifiers, one distinct type per entity.
//! - [`DecimalText`]: exact lexical decimals that keep the received text,
//!   never rounded and never floats (NFR-10, ADR 0012).
//! - [`UtcTimestamp`] and [`Clock`]: UTC instants whose canonical text sorts
//!   chronologically, and an injectable clock.
//! - [`Phi`]: redaction of protected health information in `Debug`/`Display`.
//! - [`Principal`] and [`Capability`]: who acts and the deny-by-default check
//!   every command performs (NFR-01).
//! - [`CommandError`]: the only error type that crosses into the UI.
//! - [`Migration`] and [`ModuleSchema`]: per-module schema descriptors.
//!
//! Invariants: every value type validates on construction and has a single
//! text form shared by Display, serde, TypeScript bindings (`string`) and
//! SQLite (`TEXT`). Error messages never echo input.
//!
//! See `docs/planning/implementation-plan.md` (crate map) and
//! `docs/governance/coding-standards.md`.

mod comparator;
mod decimal;
mod error;
mod id;
mod migration;
mod phi;
mod principal;
mod text;
mod text_enum;
mod time;

pub use comparator::Comparator;
pub use decimal::DecimalText;
pub use error::{CommandError, ErrorCode, FieldError, GateReason, ParseError};
pub use id::{
    CorrelationId, Id, InstallationId, InstrumentId, SessionId, UserId, WorkstationId, kinds,
};
pub use migration::{Migration, ModuleSchema};
pub use phi::Phi;
pub use principal::{Actor, Capability, Denied, Principal};
pub use text::{optional_text, required_text};
pub use time::{Clock, FixedClock, SystemClock, UtcTimestamp};

/// Re-exported so dependent crates name the same decimal type.
pub use rust_decimal::Decimal;
