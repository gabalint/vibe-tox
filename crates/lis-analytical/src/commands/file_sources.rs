//! Result-file source registrations (result files brief F2, §13;
//! REQ-ANA-003, REQ-INT-005): an ordinary draft registration with a file
//! layout, approved with [`approve_source`](super::readiness::approve_source)
//! and bound to each import like any registration.

use lis_audit::AuditDetail;
use lis_core::{Capability, InstrumentId};
use lis_interfaces::InstrumentTransport;
use lis_store::CommandContext;
use serde::Deserialize;
use ts_rs::TS;

use super::NAME_MAX;
use super::readiness::{
    Draft, SourceTestCodeInput, insert_draft, instrument_transport, test_codes,
};
use crate::error::{AnalyticalError, invalid};
use crate::file_layout::{FileLayout, FileShape};
use crate::model::{RevisionIndicator, SampleIdField};
use crate::queries;
use crate::repo::readiness as repo;
use crate::views::SourceRegistrationView;

/// The audit `format` detail of a file registration.
const FORMAT: &str = "delimited";

/// Input of [`register_file_source`].
#[derive(Debug, Clone, Deserialize, TS)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
#[ts(export)]
pub struct RegisterFileSourceInput {
    /// The instrument; its connection type must be result files.
    pub instrument_id: InstrumentId,
    /// The declared software name, shown on every import; not checked against the file.
    pub software_name: String,
    /// The declared software version.
    pub software_version: String,
    /// Whether the source reports final (dilution-corrected) concentrations.
    pub reports_final_concentration: bool,
    /// The file template.
    pub layout: FileLayout,
    /// Analyte names, exactly as the file writes them, mapped to catalog analytes.
    pub test_codes: Vec<SourceTestCodeInput>,
}

/// Records a draft result-file source registration with its file layout
/// and analyte map (brief F2). The file's sample label is the LIS-assigned
/// container label (`specimen_id`); files carry no correction marker, so
/// the revision indicator is `none`. It takes effect only when approved
/// ([`approve_source`](super::readiness::approve_source)).
pub fn register_file_source(
    ctx: &mut CommandContext<'_>,
    input: RegisterFileSourceInput,
) -> Result<SourceRegistrationView, AnalyticalError> {
    ctx.require(Capability::ManageConfiguration)?;
    let name = lis_core::required_text("softwareName", &input.software_name, NAME_MAX)?;
    let version = lis_core::required_text("softwareVersion", &input.software_version, NAME_MAX)?;
    input.layout.validate()?;
    let codes = test_codes(ctx, &input.test_codes)?;
    check_names(&input.layout, &codes)?;
    if instrument_transport(ctx, input.instrument_id)? != InstrumentTransport::ResultFile {
        return Err(invalid(
            "instrumentId",
            "not_result_file",
            "Choose an instrument whose connection type is Result files.",
        ));
    }
    let (id, event) = insert_draft(
        ctx,
        &Draft {
            instrument_id: input.instrument_id,
            sender_name: &name,
            sender_version: &version,
            sample_id_field: SampleIdField::SpecimenId,
            revision_indicator: RevisionIndicator::None,
            reports_final_concentration: input.reports_final_concentration,
            codes: &codes,
        },
    )?;
    repo::insert_file_layout(ctx.tx(), id, &input.layout)?;
    ctx.audit(event.detail("format", AuditDetail::code(FORMAT)))?;
    queries::source_registration(ctx.tx(), id)
}

/// Every wide analyte name has a test code, and no not-a-result name is
/// also mapped (F10): each name has exactly one meaning.
fn check_names(
    layout: &FileLayout,
    codes: &[(String, lis_catalog::AnalyteId)],
) -> Result<(), AnalyticalError> {
    let mapped = |name: &str| codes.iter().any(|(code, _)| code == name);
    if let FileShape::Wide { value_columns } = &layout.shape
        && let Some(index) = value_columns.iter().position(|v| !mapped(&v.analyte_name))
    {
        return Err(invalid(
            &format!("layout.shape.valueColumns[{index}].analyteName"),
            "unmapped",
            "Map every analyte column to a catalog analyte.",
        ));
    }
    if let Some(index) = layout.not_results.iter().position(|name| mapped(name)) {
        return Err(invalid(
            &format!("layout.notResults[{index}]"),
            "mapped",
            "A name is either mapped to an analyte or not a result, not both.",
        ));
    }
    Ok(())
}
