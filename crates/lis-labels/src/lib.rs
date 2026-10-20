//! Specimen labels: versioned ZPL templates with safe field encoding, and raw
//! transports (TCP 9100, CUPS raw queue).
//!
//! [`render`] fills an embedded template's `{{name}}` placeholders and returns
//! the exact ZPL bytes; [`Transport::print`] hands those bytes to a printer.
//! Container labels carry the laboratory's container identifier, so a reprint
//! of the same container produces the same identifier (REQ-SPC-001, ADR 0012).
//!
//! Invariants:
//! - **One encoding.** Every value passes through a single encoder: control
//!   characters are refused, ZPL prefixes (`^`, `~`), the `^FH` hex indicator
//!   (`_`) and all non-ASCII characters become `_XX` hex escapes of their
//!   UTF-8 bytes, and each field has a character limit. Barcode (Code 128)
//!   fields accept printable ASCII only, without `>`.
//! - **Checked templates.** A template's placeholders must equal its declared
//!   fields, and each placeholder must sit inside a `^FH^FD … ^FS` field so
//!   escaping always applies. Missing, unknown or duplicate fields are refused.
//! - **Demo marking.** The demo laboratory's labels carry a fixed "DEMO" block in space
//!   the template leaves free ([`render_marked`]); a laboratory's labels are unchanged.
//! - **No patient data in barcodes.** `specimen` v1 encodes only the container
//!   identifier in its barcode; patient name and birth date are printed as text.
//! - **No shell.** CUPS printing runs `lp -d <queue> -o raw` directly with the
//!   label on stdin; queue names are validated so they cannot become options.
//! - **PHI-free errors.** Errors name template fields and I/O error kinds,
//!   never field values or label content.
//!
//! # Templates
//!
//! | Template | Fields (max characters) |
//! | --- | --- |
//! | `specimen` v1, 2 x 1 in at 203 dpi | `container_id` (Code 128, 14), `subject_name` (28), `birth_date` (10), `accession` (16), `matrix` (12) |

mod encode;
mod template;
mod transport;

pub use encode::FieldError;
pub use template::{LabelError, LabelTemplateRef, Marking, render, render_marked};
pub use transport::{
    DEFAULT_TCP_PORT, PrintError, PrinterDestinations, QueueName, Transport, check_destination,
};
