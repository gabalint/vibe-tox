//! Freeze actual selected-measurement run sites. Legacy absence is explicit.
use super::{ComponentLine, LaboratorySection};
use crate::error::ReportingError;
use lis_specimens::SpecimenId;
use rusqlite::Connection;

pub(super) fn populate(
    conn: &Connection,
    specimen_id: SpecimenId,
    lines: &mut [ComponentLine],
) -> Result<(), ReportingError> {
    for result in lines
        .iter_mut()
        .flat_map(|line| &mut line.analytes)
        .filter_map(|a| a.result.as_mut())
    {
        let runs = lis_results::result_revision_run_ids(conn, result.revision_id)?;
        result.performing_site_unknown = runs.is_empty();
        for run_id in runs {
            let profile = lis_analytical::run_laboratory_profile(conn, run_id)?;
            let Some(id) = profile else {
                result.performing_site_unknown = true;
                continue;
            };
            if result
                .performing_sites
                .iter()
                .any(|site| site.profile_id == id)
            {
                continue;
            }
            let site = lis_identity::get_laboratory_profile_by_id(conn, id)?;
            result.performing_sites.push(LaboratorySection {
                profile_id: site.profile_id,
                version: site.version,
                name: site.name,
                address_lines: site.address_lines,
                clia_id: site.clia_id,
                director: site.director,
                phone: site.phone,
                time_zone: site.time_zone,
            });
        }
        result.performing_sites.sort_by_key(|site| site.profile_id);
    }
    reporting_dates(conn, specimen_id, lines)?;
    Ok(())
}

fn reporting_dates(
    conn: &Connection,
    specimen_id: SpecimenId,
    lines: &mut [ComponentLine],
) -> Result<(), ReportingError> {
    let Some(report) = crate::repo::reports::report_for_specimen(conn, specimen_id)? else {
        return Ok(());
    };
    let dates = crate::repo::reports::first_reported_dates(conn, report.id)?;
    for result in lines
        .iter_mut()
        .flat_map(|line| &mut line.analytes)
        .filter_map(|a| a.result.as_mut())
    {
        result.first_reported_at = dates.get(&result.revision_id).copied();
    }
    Ok(())
}
