//! Layout previews (REQ-RPT-007): a report template rendered on one fixed,
//! synthetic report under the preview marking, so a laboratory can see a
//! layout before approving it. No laboratory data ever enters a preview.

use crate::error::RenderError;
use crate::marking::Marking;
use crate::renderer::{RenderedPdf, render_marked};
use crate::template::TemplateRef;

/// The synthetic sample report every preview shows (`tox-report` v3 contract
/// with comparison codes); every name and number in it is fictional.
pub const SAMPLE_REPORT_JSON: &str = include_str!("../preview/sample-report-v1.json");

/// Renders `template` on [`SAMPLE_REPORT_JSON`], marked "PREVIEW – NOT A REPORT"
/// on every page.
pub fn render_preview(
    template: &TemplateRef,
    issued_at: jiff::Timestamp,
) -> Result<RenderedPdf, RenderError> {
    render_marked(template, SAMPLE_REPORT_JSON, issued_at, Marking::Preview)
}
