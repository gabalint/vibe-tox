//! Current account display names for the user IDs that views keep as text.

use std::collections::{BTreeSet, HashMap};

use lis_auth::AuthError;
use lis_core::UserId;
use lis_store::StoreError;

/// The current display name of each listed user ID, keyed by the ID text. Views keep only the
/// immutable ID and never cache a mutable name; an ID that is not a user (a workstation or
/// system principal, a removed account) is simply absent.
pub(crate) fn display_names<'a>(
    conn: &rusqlite::Connection,
    ids: impl IntoIterator<Item = &'a str>,
) -> Result<HashMap<String, String>, StoreError> {
    let users: BTreeSet<UserId> = ids.into_iter().filter_map(|id| id.parse().ok()).collect();
    let names = lis_auth::display_names(conn, &users).map_err(|error| match error {
        AuthError::Store(error) => error,
        // Unreachable today: the lookup only runs SQL, and `AuthError` wraps every SQL failure
        // as `Store`. Kept so a future variant still surfaces as a store failure, not a panic.
        other => StoreError::Sql(rusqlite::Error::ToSqlConversionFailure(Box::new(other))),
    })?;
    Ok(names
        .into_iter()
        .map(|(id, name)| (id.to_string(), name))
        .collect())
}

/// The display name for `id`, if it names a current account.
pub(crate) fn name_of(names: &HashMap<String, String>, id: &str) -> Option<String> {
    names.get(id).cloned()
}

/// Names who recorded each row: `recorded_by` reads a row's user ID text and `set` stores that
/// account's current display name, looked up once for all rows.
pub(crate) fn name_recorders<T>(
    conn: &rusqlite::Connection,
    rows: &mut [T],
    recorded_by: impl Fn(&T) -> Option<&str>,
    set: impl Fn(&mut T, Option<String>),
) -> Result<(), StoreError> {
    let names = display_names(conn, rows.iter().filter_map(&recorded_by))?;
    for row in rows.iter_mut() {
        let name = recorded_by(row).and_then(|id| name_of(&names, id));
        set(row, name);
    }
    Ok(())
}
