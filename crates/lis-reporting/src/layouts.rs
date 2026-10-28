//! Report layouts (REQ-RPT-007): the ready-made report templates a laboratory
//! may choose from, and the one in force for new candidates. Only the
//! arrangement differs; every layout prints the same content from the same
//! data. A choice is drafted by an administrator and takes effect when a
//! quality manager approves it ([`crate::commands::layouts`]). Reports
//! already issued keep their bytes and their recorded template.

use lis_core::text_enum;
use lis_report::TemplateRef;
use rusqlite::Connection;

use crate::error::ReportingError;
use crate::repo::layouts;

text_enum! {
    /// A ready-made report layout.
    #[ts(export)]
    ReportLayout {
        /// The original single-column layout, in force until a laboratory approves another.
        Classic = "classic",
        /// Header band, three detail cards and a results table.
        Standard = "standard",
        /// Everything on one dense page.
        Compact = "compact",
        /// Results as large cards at the top, details after them.
        ResultsFirst = "results_first",
    }
}

impl ReportLayout {
    /// Every layout, in the order the laboratory sees them.
    pub const ALL: [Self; 4] = [
        Self::Standard,
        Self::Compact,
        Self::ResultsFirst,
        Self::Classic,
    ];

    /// The layout in force while no approved choice exists.
    pub const DEFAULT: Self = Self::Classic;

    /// The pinned template the layout renders with.
    #[must_use]
    pub fn template(self) -> TemplateRef {
        let (id, version) = match self {
            Self::Classic => ("tox-report", 3),
            Self::Standard => ("tox-report-standard", 1),
            Self::Compact => ("tox-report-compact", 1),
            Self::ResultsFirst => ("tox-report-results-first", 1),
        };
        TemplateRef {
            id: id.to_owned(),
            version,
        }
    }

    /// The layout's name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Classic => "Classic",
            Self::Standard => "Standard",
            Self::Compact => "Compact",
            Self::ResultsFirst => "Results first",
        }
    }

    /// One sentence on how the layout arranges the report.
    #[must_use]
    pub const fn description(self) -> &'static str {
        match self {
            Self::Classic => {
                "The original layout: every section as a list of labels, results near the end."
            }
            Self::Standard => {
                "A header band with the laboratory and the report number; Patient, Order and \
                 Specimen in three cards; a results table; a signature area."
            }
            Self::Compact => {
                "The whole report on one page, with a patient strip at the top and a results table."
            }
            Self::ResultsFirst => {
                "The results at the top as large cards, with the order and specimen details \
                 after them."
            }
        }
    }

    /// The layout a recorded template belongs to, if any.
    #[must_use]
    pub fn of_template(id: &str, version: i64) -> Option<Self> {
        Self::ALL.into_iter().find(|layout| {
            let template = layout.template();
            template.id == id && i64::from(template.version) == version
        })
    }
}

/// The layout new candidates render with: the approved choice, or the default.
pub fn in_force(conn: &Connection) -> Result<ReportLayout, ReportingError> {
    Ok(layouts::active(conn)?.map_or(ReportLayout::DEFAULT, |version| version.layout))
}

#[cfg(test)]
mod tests {
    use super::*;

    // Only the original layout's template may serve as the default: issued
    // candidates before this change recorded `tox-report` v3.
    #[test]
    fn the_default_is_the_original_template() {
        let template = ReportLayout::DEFAULT.template();
        assert_eq!((template.id.as_str(), template.version), ("tox-report", 3));
    }

    #[test]
    fn every_layout_maps_back_from_its_template() {
        for layout in ReportLayout::ALL {
            let template = layout.template();
            assert_eq!(
                ReportLayout::of_template(&template.id, i64::from(template.version)),
                Some(layout)
            );
        }
        assert_eq!(ReportLayout::of_template("tox-report", 2), None);
    }
}
