//! The only module that touches the Typst API, which changes between minor
//! versions (ADR 0012). Everything Typst-specific stays behind [`render`].

use std::collections::BTreeSet;
use std::sync::atomic::{AtomicBool, Ordering};

use sha2::{Digest, Sha256};
use typst::diag::{FileError, FileResult, PackageError, Severity, SourceDiagnostic};
use typst::ecow::EcoVec;
use typst::foundations::{Bytes, Datetime, Dict, Duration, IntoValue, Smart};
use typst::layout::{Frame, FrameItem};
use typst::syntax::{FileId, RootedPath, Source, VirtualPath, VirtualRoot};
use typst::text::{Font, FontBook};
use typst::utils::LazyHash;
use typst::{Library, LibraryExt, World, WorldExt};
use typst_pdf::{PdfOptions, PdfStandard, PdfStandards, Timestamp};

use crate::error::RenderError;
use crate::marking::Marking;
use crate::template::{self, TemplateRef};

/// Renderer identity recorded with every artifact (REQ-RPT-007, NFR-07).
pub const RENDERER_VERSION: &str = "typst 0.15.1";

/// A rendered report and the provenance needed to retrieve and explain it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedPdf {
    /// The exact PDF bytes; store these, never re-render to replace them.
    pub bytes: Vec<u8>,
    /// Lowercase hex SHA-256 of `bytes`.
    pub sha256: String,
    /// Always [`RENDERER_VERSION`].
    pub renderer_version: &'static str,
    /// The template version used.
    pub template: TemplateRef,
    /// Lowercase hex SHA-256 of the template source.
    pub template_sha256: String,
    /// PostScript names of the bundled fonts the document uses, sorted.
    pub fonts: Vec<String>,
}

/// Renders `data_json` with the given template to a PDF/A-2a and PDF/UA-1
/// document: a laboratory's output, the template alone. Identical inputs give
/// identical bytes: the PDF date and document identifier derive from the inputs,
/// never from the host (NFR-07, REQ-RPT-010).
pub fn render(
    template: &TemplateRef,
    data_json: &str,
    issued_at: jiff::Timestamp,
) -> Result<RenderedPdf, RenderError> {
    render_marked(template, data_json, issued_at, Marking::Unmarked)
}

/// Renders as [`render`] does, with the installation's [`Marking`]: unmarked output is
/// byte-identical to [`render`]'s; demo output runs the same template inside the pinned
/// demo marking, so every page carries the watermark (proposed REQ-OPS-016). The
/// provenance names the template, whose source is unchanged.
pub fn render_marked(
    template: &TemplateRef,
    data_json: &str,
    issued_at: jiff::Timestamp,
    marking: Marking,
) -> Result<RenderedPdf, RenderError> {
    let source = template::find(template).ok_or_else(|| RenderError::UnknownTemplate {
        id: template.id.clone(),
        version: template.version,
    })?;
    crate::data::check(data_json)?;
    let template_sha256 = sha256_hex(source.source.as_bytes());
    let issued = IssuedAt::new(issued_at).ok_or(RenderError::IssuedAtOutOfRange)?;
    let overlay = marking.overlay();
    let world = ReportWorld::new(source.source, overlay, data_json, &issued);
    let mut ident = vec![
        template_sha256.clone(),
        data_json.to_owned(),
        issued_at.to_string(),
    ];
    ident.extend(overlay.map(|overlay| sha256_hex(overlay.as_bytes())));
    let options = PdfOptions {
        ident: Smart::Custom(document_ident(&ident)),
        timestamp: Some(Timestamp::new_utc(issued.datetime)),
        standards: PdfStandards::new(&[PdfStandard::A_2a, PdfStandard::Ua_1])
            .map_err(|_| RenderError::Export { errors: 1 })?,
        ..PdfOptions::default()
    };
    let typeset = typeset(&world, &options);
    // Typst memoizes globally; drop the cache so a long-running app does not grow.
    typst::comemo::evict(0);
    let typeset = typeset?;
    Ok(RenderedPdf {
        sha256: sha256_hex(&typeset.pdf),
        bytes: typeset.pdf,
        renderer_version: RENDERER_VERSION,
        template: template.clone(),
        template_sha256,
        fonts: used_fonts(&typeset.frames),
    })
}

pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// A stable PDF document identifier: Typst hashes it into the trailer `/ID`. The parts
/// are the template digest, the data, the issued time and, for marked output, the
/// marking's digest.
fn document_ident(parts: &[String]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.len().to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex::encode(hasher.finalize())
}

/// The issued instant in the forms Typst needs, all in UTC.
struct IssuedAt {
    instant: jiff::Timestamp,
    datetime: Datetime,
    date: Datetime,
}

impl IssuedAt {
    fn new(instant: jiff::Timestamp) -> Option<Self> {
        let utc = instant.to_zoned(jiff::tz::TimeZone::UTC).datetime();
        let (year, month, day) = (
            i32::from(utc.year()),
            u8::try_from(utc.month()).ok()?,
            u8::try_from(utc.day()).ok()?,
        );
        let datetime = Datetime::from_ymd_hms(
            year,
            month,
            day,
            u8::try_from(utc.hour()).ok()?,
            u8::try_from(utc.minute()).ok()?,
            u8::try_from(utc.second()).ok()?,
        )?;
        let date = Datetime::from_ymd(year, month, day)?;
        Some(Self {
            instant,
            datetime,
            date,
        })
    }
}

struct Typeset {
    pdf: Vec<u8>,
    frames: Vec<Frame>,
}

fn typeset(world: &ReportWorld, options: &PdfOptions) -> Result<Typeset, RenderError> {
    let document = typst::compile(world)
        .output
        .map_err(|diagnostics| compile_error(world, &diagnostics))?;
    let pdf = typst_pdf::pdf(&document, options).map_err(|diagnostics| RenderError::Export {
        errors: error_count(&diagnostics),
    })?;
    let frames = document
        .pages()
        .iter()
        .map(|page| page.frame.clone())
        .collect();
    Ok(Typeset { pdf, frames })
}

/// Maps Typst diagnostics to a typed error without their messages, which may
/// quote data values; only template line numbers are kept.
fn compile_error(world: &ReportWorld, diagnostics: &EcoVec<SourceDiagnostic>) -> RenderError {
    if world.package_requested.load(Ordering::Relaxed) {
        return RenderError::PackageImport;
    }
    let mut template_lines = BTreeSet::new();
    for diagnostic in diagnostics.iter().filter(|d| d.severity == Severity::Error) {
        if diagnostic.span.id() != Some(world.template.id()) {
            continue;
        }
        let line = world
            .range(diagnostic.span)
            .and_then(|range| world.template.lines().byte_to_line(range.start));
        template_lines.extend(line.map(|line| line + 1));
    }
    RenderError::Compile {
        errors: error_count(diagnostics),
        template_lines: template_lines.into_iter().collect(),
    }
}

fn error_count(diagnostics: &EcoVec<SourceDiagnostic>) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count()
}

fn used_fonts(frames: &[Frame]) -> Vec<String> {
    let mut fonts = BTreeSet::new();
    for frame in frames {
        visit_text(frame, &mut |item| {
            let font = item.font.font();
            fonts.insert(
                font.post_script_name()
                    .unwrap_or_else(|| font.info().family.clone()),
            );
        });
    }
    fonts.into_iter().collect()
}

fn visit_text(frame: &Frame, visit: &mut dyn FnMut(&typst::text::TextItem)) {
    for (_, item) in frame.items() {
        match item {
            FrameItem::Group(group) => visit_text(&group.frame, visit),
            FrameItem::Text(text) => visit(text),
            FrameItem::Shape(..)
            | FrameItem::Image(..)
            | FrameItem::Link(..)
            | FrameItem::Tag(_) => {}
        }
    }
}

/// The sandbox a template compiles in: bundled fonts only, the template as the
/// main file (or, when marked, as `/template.typ` under the marking), the data as
/// `/data.json`, a fixed date, and no other files or packages (no file system or
/// network access).
struct ReportWorld {
    library: LazyHash<Library>,
    book: LazyHash<FontBook>,
    fonts: Vec<Font>,
    main: Source,
    /// The template; the same source as `main` when unmarked.
    template: Source,
    data_id: FileId,
    data: Bytes,
    today: Datetime,
    package_requested: AtomicBool,
}

impl ReportWorld {
    fn new(template: &str, overlay: Option<&str>, data_json: &str, issued: &IssuedAt) -> Self {
        let fonts: Vec<Font> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .collect();
        let issued_text = issued.instant.strftime("%Y-%m-%d %H:%M:%S UTC").to_string();
        let inputs: Dict = [("issued-at".into(), issued_text.into_value())]
            .into_iter()
            .collect();
        let path = if overlay.is_some() {
            "/template.typ"
        } else {
            "/main.typ"
        };
        let template = Source::new(project_file(path), template.to_owned());
        Self {
            library: LazyHash::new(Library::builder().with_inputs(inputs).build()),
            book: LazyHash::new(FontBook::from_fonts(&fonts)),
            fonts,
            main: match overlay {
                Some(overlay) => Source::new(project_file("/main.typ"), overlay.to_owned()),
                None => template.clone(),
            },
            template,
            data_id: project_file("/data.json"),
            data: Bytes::new(data_json.as_bytes().to_vec()),
            today: issued.date,
            package_requested: AtomicBool::new(false),
        }
    }

    fn not_found(&self, id: FileId) -> FileError {
        match id.get().root() {
            VirtualRoot::Package(package) => {
                self.package_requested.store(true, Ordering::Relaxed);
                FileError::Package(PackageError::NotFound(package.clone()))
            }
            VirtualRoot::Project => {
                FileError::NotFound(id.get().vpath().get_without_slash().into())
            }
        }
    }
}

#[expect(
    clippy::expect_used,
    reason = "called only with constant, valid absolute paths"
)]
fn project_file(path: &str) -> FileId {
    let vpath = VirtualPath::new(path).expect("constant virtual path is valid");
    FileId::new(RootedPath::new(VirtualRoot::Project, vpath))
}

impl World for ReportWorld {
    fn library(&self) -> &LazyHash<Library> {
        &self.library
    }

    fn book(&self) -> &LazyHash<FontBook> {
        &self.book
    }

    fn main(&self) -> FileId {
        self.main.id()
    }

    fn source(&self, id: FileId) -> FileResult<Source> {
        if id == self.main.id() {
            return Ok(self.main.clone());
        }
        if id == self.template.id() {
            return Ok(self.template.clone());
        }
        Err(self.not_found(id))
    }

    fn file(&self, id: FileId) -> FileResult<Bytes> {
        if id == self.data_id {
            return Ok(self.data.clone());
        }
        for source in [&self.main, &self.template] {
            if id == source.id() {
                return Ok(Bytes::from_string(source.text().to_owned()));
            }
        }
        Err(self.not_found(id))
    }

    fn font(&self, index: usize) -> Option<Font> {
        self.fonts.get(index).cloned()
    }

    /// Fixed to the issued date so output never depends on the host clock.
    fn today(&self, _offset: Option<Duration>) -> Option<Datetime> {
        Some(self.today)
    }
}

/// The text of every page with whitespace removed, for template tests.
#[cfg(test)]
pub(crate) fn rendered_text(
    template: &TemplateRef,
    data_json: &str,
) -> Result<String, RenderError> {
    let source = template::find(template).expect("registered template");
    tests::typeset_with(source.source, None, data_json).map(|typeset| tests::page_text(&typeset))
}

/// The text of every page of the marked output, for marking tests.
#[cfg(test)]
pub(crate) fn rendered_marked_text(
    template: &TemplateRef,
    data_json: &str,
    marking: Marking,
) -> Result<String, RenderError> {
    let source = template::find(template).expect("registered template");
    tests::typeset_with(source.source, marking.overlay(), data_json)
        .map(|typeset| tests::page_text(&typeset))
}

/// The number of pages, for template tests.
#[cfg(test)]
pub(crate) fn rendered_page_count(
    template: &TemplateRef,
    data_json: &str,
) -> Result<usize, RenderError> {
    let source = template::find(template).expect("registered template");
    tests::typeset_with(source.source, None, data_json).map(|typeset| typeset.frames.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const SYNTHETIC: &str = include_str!("../tests/fixtures/synthetic-report.json");

    pub(super) fn typeset_with(
        template: &str,
        overlay: Option<&str>,
        data_json: &str,
    ) -> Result<Typeset, RenderError> {
        let issued =
            IssuedAt::new("2026-10-12T14:30:00Z".parse().expect("valid")).expect("in range");
        let world = ReportWorld::new(template, overlay, data_json, &issued);
        typeset(&world, &PdfOptions::default())
    }

    pub(super) fn page_text(typeset: &Typeset) -> String {
        let mut text = String::new();
        for frame in &typeset.frames {
            visit_text(frame, &mut |item| text.push_str(&item.text));
        }
        text.split_whitespace().collect()
    }

    #[test]
    fn markup_in_data_values_is_rendered_as_text() {
        let injected = r#"#panic() *bold* _em_ $x$ <label> @ref #import "@preview/x:0.1.0" ]["#;
        let mut data: serde_json::Value = serde_json::from_str(SYNTHETIC).expect("fixture");
        data["subject"]["name"] = injected.into();
        let source = template::TEMPLATES[0].source;
        let typeset = typeset_with(source, None, &data.to_string()).expect("renders");
        let expected: String = injected.split_whitespace().collect();
        assert!(page_text(&typeset).contains(&expected));
    }

    #[test]
    fn package_import_is_refused() {
        let template = "#import \"@preview/example:0.1.0\": *\nText";
        assert_eq!(
            typeset_with(template, None, "{}").err(),
            Some(RenderError::PackageImport)
        );
    }

    #[test]
    fn host_files_are_not_readable() {
        let template = "#read(\"/etc/hosts\")";
        assert!(matches!(
            typeset_with(template, None, "{}"),
            Err(RenderError::Compile { errors: 1, .. })
        ));
    }

    #[test]
    fn compile_error_names_template_lines_only() {
        let template = "Line one\n#json(\"/data.json\").missing";
        let error = typeset_with(template, None, r#"{"present": "SECRET-VALUE"}"#).err();
        assert_eq!(
            error,
            Some(RenderError::Compile {
                errors: 1,
                template_lines: vec![2]
            })
        );
    }

    #[test]
    fn today_is_the_issued_date_not_the_host_date() {
        let template = "#datetime.today().display()";
        let typeset = typeset_with(template, None, "{}").expect("renders");
        assert_eq!(page_text(&typeset), "2026-10-12");
    }

    #[test]
    fn only_bundled_fonts_are_used() {
        let template = "#set text(font: \"Arial\")\nText";
        let typeset = typeset_with(template, None, "{}").expect("renders");
        let bundled: BTreeSet<String> = typst_assets::fonts()
            .flat_map(|data| Font::iter(Bytes::new(data)))
            .filter_map(|font| font.post_script_name())
            .collect();
        let used = used_fonts(&typeset.frames);
        assert!(!used.is_empty());
        assert!(used.iter().all(|font| bundled.contains(font)), "{used:?}");
    }
}
