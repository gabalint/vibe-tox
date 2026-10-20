//! Render errors. `Display` never contains data values or Typst messages,
//! which can quote the data; only counts, codes and template positions.

/// Why a report could not be rendered.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum RenderError {
    /// No embedded template has this id and version.
    #[error("unknown report template {id} version {version}")]
    UnknownTemplate { id: String, version: u32 },
    /// The data is not acceptable report input.
    #[error("invalid report data: {0}")]
    InvalidData(#[from] DataError),
    /// The template tried to import a Typst package; templates are self-contained.
    #[error("report templates may not import packages")]
    PackageImport,
    /// The template failed against the data, for example a missing contract field.
    #[error("report template failed with {errors} error(s) at template lines {template_lines:?}")]
    Compile {
        errors: usize,
        /// One-based lines of the template where errors arose; empty if none
        /// could be located in the template itself.
        template_lines: Vec<usize>,
    },
    /// The issued time cannot be expressed as a PDF date.
    #[error("issued time is outside the supported date range")]
    IssuedAtOutOfRange,
    /// PDF export failed, including PDF/A or PDF/UA conformance checks.
    #[error("PDF export failed with {errors} error(s)")]
    Export { errors: usize },
}

/// Why the report data was refused before rendering.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DataError {
    /// Not valid JSON; the position is in the data text.
    #[error("malformed JSON at line {line}, column {column}")]
    Syntax { line: usize, column: usize },
    /// Valid JSON, but not an object at the top level.
    #[error("the data must be a JSON object")]
    NotAnObject,
    /// A JSON number or boolean; every value must be a string or null.
    #[error("every value must be a string or null")]
    NonStringScalar,
    /// Typed report data could not be written as JSON.
    #[error("the report data could not be serialized")]
    Unserializable,
}
