//! Laboratory profile reads, naming who set each version. The profile keeps only the user ID;
//! the engine looks up the current display name on every read.

use lis_identity::{IdentityError, LaboratoryProfileView};

use crate::commands::people::{display_names, name_of};

/// The current laboratory profile, or `None` before one is set.
pub(super) fn current(
    conn: &rusqlite::Connection,
) -> Result<Option<LaboratoryProfileView>, IdentityError> {
    let mut profile = lis_identity::current_laboratory_profile(conn)?;
    name_profiles(conn, profile.iter_mut())?;
    Ok(profile)
}

/// Every retained laboratory profile version.
pub(super) fn list(
    conn: &rusqlite::Connection,
) -> Result<Vec<LaboratoryProfileView>, IdentityError> {
    let mut profiles = lis_identity::list_laboratory_profiles(conn)?;
    name_profiles(conn, profiles.iter_mut())?;
    Ok(profiles)
}

fn name_profiles<'a>(
    conn: &rusqlite::Connection,
    profiles: impl Iterator<Item = &'a mut LaboratoryProfileView>,
) -> Result<(), IdentityError> {
    let mut profiles: Vec<_> = profiles.collect();
    let names = display_names(conn, profiles.iter().map(|p| p.set_by.as_str()))?;
    for profile in &mut profiles {
        profile.set_by_display = name_of(&names, &profile.set_by);
    }
    Ok(())
}
