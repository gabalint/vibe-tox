//! The installation's marking of its documents (proposed REQ-OPS-016): a laboratory's
//! output is the template alone; the demo laboratory's output runs the same template inside
//! the pinned demo marking, so no demo report can pass for a laboratory's.

/// The demo marking: wraps a template and watermarks every page.
const DEMO_V1: &str = include_str!("../templates/demo-marking/v1.typ");

/// What an installation's documents carry besides their template.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Marking {
    /// A laboratory's output, byte for byte as the template renders it.
    Unmarked,
    /// The demo laboratory's output: "DEMO" behind the content of every page.
    Demo,
}

/// The keyword the demo marking writes into a PDF's plain XMP metadata.
pub const METADATA_MARK: &str = "Demo laboratory: not for patient testing";

impl Marking {
    /// The Typst source that wraps the template, if any.
    pub(crate) fn overlay(self) -> Option<&'static str> {
        match self {
            Self::Unmarked => None,
            Self::Demo => Some(DEMO_V1),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::renderer::{rendered_marked_text, sha256_hex};
    use crate::template::TemplateRef;

    const REPORT: &str = include_str!("../tests/fixtures/synthetic-report-v2.json");
    const NOTICE: &str = include_str!("../tests/fixtures/synthetic-notice.json");
    const PREVIEW: &str = include_str!("../tests/fixtures/synthetic-catalog-preview.json");
    /// The watermark's text with whitespace removed, as the page text shows it.
    const WATERMARK: &str = "DEMODemolaboratory:notforpatienttesting";

    fn reference(id: &str, version: u32) -> TemplateRef {
        TemplateRef {
            id: id.to_owned(),
            version,
        }
    }

    /// The marking is issued like a template: a change is a new version.
    #[test]
    fn the_demo_marking_is_pinned() {
        assert_eq!(
            sha256_hex(DEMO_V1.as_bytes()),
            "2dc627e8dbb99d91b54eb20e6a20f74757903d2f034870cd2c0aee641d24f0aa"
        );
    }

    // Proposed REQ-OPS-016: every page of demo output carries the watermark; unmarked
    // output carries none.
    #[test]
    fn demo_output_is_watermarked_and_laboratory_output_is_not() {
        for (template, data) in [
            (reference("tox-report", 2), REPORT),
            (reference("report-notice", 1), NOTICE),
            (reference("catalog-preview", 1), PREVIEW),
            (reference("catalog-preview", 2), PREVIEW),
        ] {
            let demo = rendered_marked_text(&template, data, Marking::Demo).expect("renders");
            let lab = rendered_marked_text(&template, data, Marking::Unmarked).expect("renders");
            assert!(demo.starts_with(WATERMARK), "{demo}");
            assert!(!lab.contains(WATERMARK), "{lab}");
            assert_eq!(
                demo.replace(WATERMARK, ""),
                lab,
                "the same content under the mark"
            );
        }
    }

    // The marking is also in the PDF's uncompressed metadata, where a test or tool finds it.
    #[test]
    fn demo_pdf_metadata_names_the_marking() {
        let template = reference("report-notice", 1);
        let issued = jiff::Timestamp::constant(1_791_903_900, 0);
        let render = |marking| {
            crate::renderer::render_marked(&template, NOTICE, issued, marking).expect("renders")
        };
        assert!(contains(&render(Marking::Demo).bytes, METADATA_MARK));
        assert!(!contains(&render(Marking::Unmarked).bytes, METADATA_MARK));
    }

    fn contains(bytes: &[u8], text: &str) -> bool {
        bytes
            .windows(text.len())
            .any(|window| window == text.as_bytes())
    }
}
