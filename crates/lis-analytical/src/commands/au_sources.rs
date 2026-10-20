//! AU registration gates, rechecked at approval (REQ-INT-001/004).

use lis_catalog::AnalyteId;
use lis_core::InstrumentId;
use lis_interfaces::InstrumentTransport;
use lis_store::CommandContext;

use crate::error::{AnalyticalError, invalid};
use crate::{AuSourceProfile, RevisionIndicator, SampleIdField};

pub(super) fn validate_profile(
    profile: Option<&AuSourceProfile>,
    codes: &[(String, AnalyteId)],
    id_field: SampleIdField,
    revision: RevisionIndicator,
) -> Result<(), AnalyticalError> {
    let Some(profile) = profile else {
        return Ok(());
    };
    profile.validate()?;
    if id_field != SampleIdField::SpecimenId || revision != RevisionIndicator::None {
        return Err(invalid(
            "auProfile",
            "unsupported_semantics",
            "AU sources require specimen barcode matching and no automatic correction marker.",
        ));
    }
    if profile.tests.len() != codes.len()
        || codes
            .iter()
            .any(|(code, _)| !profile.tests.iter().any(|test| test.test_code == *code))
    {
        return Err(invalid(
            "auProfile.tests",
            "mapping_mismatch",
            "Declare exactly the numeric tests in the source analyte map; exclude LIH tests.",
        ));
    }
    Ok(())
}

pub(super) fn validate_connection(
    ctx: &CommandContext<'_>,
    instrument_id: InstrumentId,
    profile: Option<&AuSourceProfile>,
    file_source: bool,
) -> Result<(), AnalyticalError> {
    let wire = lis_interfaces::instrument_au_profile(ctx.tx(), instrument_id)?;
    if wire.as_ref() != profile.map(|profile| &profile.protocol)
        || file_source
            != (lis_interfaces::instrument_transport(ctx.tx(), instrument_id)?
                == InstrumentTransport::ResultFile)
    {
        return Err(invalid(
            "instrumentId",
            "source_format_mismatch",
            "The source format must match the instrument connection and its exact AU wire settings.",
        ));
    }
    Ok(())
}
