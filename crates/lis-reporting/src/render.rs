//! Rendering with the pinned templates of [`policy`](crate::policy). Called
//! by the engine outside any transaction (brief §3.1); the bytes it returns
//! are stored once and never regenerated (NFR-07).

use lis_core::UtcTimestamp;
use lis_report::{Marking, RenderedPdf};

use crate::error::ReportingError;
use crate::policy;

/// Renders a candidate's stored `data_json` with `tox-report` v2, issued at
/// its preparation time, with the installation's marking (a demo laboratory's
/// reports are watermarked; proposed REQ-OPS-016).
pub fn render_report(
    data_json: &str,
    prepared_at: UtcTimestamp,
    marking: Marking,
) -> Result<RenderedPdf, ReportingError> {
    Ok(lis_report::render_marked(
        &policy::report_template(),
        data_json,
        crate::display::instant(prepared_at)?,
        marking,
    )?)
}

/// Renders a withdrawal notice's JSON with `report-notice` v1 and the
/// installation's marking.
pub fn render_notice(
    data_json: &str,
    issued_at: UtcTimestamp,
    marking: Marking,
) -> Result<RenderedPdf, ReportingError> {
    Ok(lis_report::render_marked(
        &policy::notice_template(),
        data_json,
        crate::display::instant(issued_at)?,
        marking,
    )?)
}
