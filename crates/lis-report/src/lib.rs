//! Report rendering: embedded Typst with bundled fonts and versioned templates,
//! producing PDF bytes plus digest and renderer/template/font versions.
//!
//! [`render`] compiles an embedded template against report data and returns
//! the exact PDF bytes with their SHA-256, the renderer version, the template
//! reference and digest, and the fonts used. The engine stores those bytes as
//! the issued artifact; re-rendering never replaces them (REQ-RPT-001
//! rendering part, REQ-RPT-007, NFR-07).
//!
//! Invariants:
//! - **Sandboxed.** A template sees only itself, the data as `/data.json`,
//!   the issued time as `sys.inputs.issued-at` and the fonts bundled by
//!   `typst-assets`. No system fonts, files, packages or network.
//! - **Deterministic.** The PDF date, `today()` and the document identifier
//!   derive from the inputs, so identical inputs give identical bytes (NFR-07).
//! - **Accessible archive format.** Output conforms to PDF/A-2a and PDF/UA-1,
//!   checked by Typst at export (REQ-RPT-010).
//! - **Data is text.** Every data value is a JSON string or null, inserted as
//!   text; markup characters in values are never evaluated. Numbers are
//!   refused so lexical results keep their exact form.
//! - **Presentational templates.** Templates do no arithmetic, comparison or
//!   interpretation; clinical logic stays in the engine (REQ-RPT-008).
//! - **PHI-free errors.** [`RenderError`] reports counts, codes and template
//!   line numbers only, never data values or Typst messages.
//! - **Pinned Typst.** All Typst API use is confined to one private module
//!   because the API changes between minor versions (ADR 0012).
//!
//! # Templates
//!
//! | Template | Data contract | Typed data |
//! | --- | --- | --- |
//! | `tox-report` v1 | Top of `templates/tox-report/v1.typ` | None; kept pinned, unused from M3 |
//! | `tox-report` v2 | Top of `templates/tox-report/v2.typ` | [`tox_v2::ToxReportData`] |
//! | `tox-report` v3 | Top of `templates/tox-report/v3.typ` | [`tox_v2::ToxReportData`] |
//! | `tox-report-standard` v1, `tox-report-compact` v1, `tox-report-results-first` v1 | Top of `templates/tox-layouts/contract-v1.typ` | [`tox_v2::ToxReportData`] |
//! | `report-notice` v1 | Top of `templates/report-notice/v1.typ` | [`notice_v1::ReportNoticeData`] |
//! | `catalog-preview` v1 | Top of `templates/catalog-preview/v1.typ` | [`catalog_preview_v1::CatalogPreviewDocument`] |
//! | `catalog-preview` v2 | Same data contract; current-value comparison labels | [`catalog_preview_v1::CatalogPreviewDocument`] |
//!
//! `tox-report` v2 shows: the report number and revision on every page; its
//! completeness (partial or final) and its relationship (initial, supplement
//! or correction) separately, with the superseded revision and, for
//! corrections, the reason; the performing laboratory; the patient with
//! identifiers and assigning authority; the order (practitioner, client,
//! location, placer order); the specimen (accession, type, containers,
//! collection and receipt times, condition notes, collection observations);
//! medications as supplied; result groups per ordered component (method,
//! certainty, role; analyte, exact result, units, limits and the engine's
//! comparison wording); tests not reported with their status; specimen
//! validity; the releaser; and the issued time exactly as the engine wrote it
//! in the laboratory's time zone (REQ-RPT-001, REQ-RPT-002, REQ-RPT-005,
//! REQ-RPT-008). The template enforces the relationship, completeness and
//! limits rules and refuses unknown codes.
//!
//! The three report layouts (REQ-RPT-007) print the same content as
//! `tox-report` v3 in other arrangements; each is the shared contract file
//! joined with its layout file at build time, pinned as one source. They
//! colour the engine's comparison wording by its code
//! ([`tox_v2::ToxComparisonCode`]). [`preview`] renders any of them on a fixed
//! synthetic report under the preview marking.
//!
//! `report-notice` v1 is the withdrawal notice: the laboratory, the report
//! number, latest revision and accession, and a fixed statement that the
//! report must not be used. It refuses any key outside its contract, so it
//! cannot carry demographics, results or a reason (REQ-RPT-002).
//!
//! `catalog-preview` v2 lists a laboratory's whole test catalog (specimen
//! types, analytes, method and test versions, validity policies) for a
//! manager to check on paper, with page 1 giving the disclaimer, counts, gaps
//! and drafts awaiting review. Every value shows its source label, and blank
//! cutoffs and starter values are marked in words (proposed REQ-OPS-015).
//!
//! Each typed module offers `to_json`, whose output the engine stores, and
//! `render_json`, which renders stored JSON with the module's template.

pub mod catalog_preview_v1;
mod data;
mod error;
mod laboratory;
mod marking;
pub mod notice_v1;
pub mod preview;
mod renderer;
mod template;
pub mod tox_v2;

pub use error::{DataError, RenderError};
pub use laboratory::ReportLaboratory;
pub use marking::{METADATA_MARK, Marking};
pub use renderer::{RENDERER_VERSION, RenderedPdf, render, render_marked};
pub use template::TemplateRef;
