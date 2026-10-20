//! Retention of a target's sets (NFR-11 retention part; M4 design brief §3.4).

use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

use jiff::Timestamp;
use jiff::tz::TimeZone;
use lis_core::{InstallationId, UtcTimestamp};

use super::manifest::{SETS_FOLDER, list_sets};
use super::{BackupError, RunKind};
use crate::format::BackupKey;
use crate::policy::OperationsSettings;
use crate::targets::TargetId;

/// One of this installation's sets on a target.
#[derive(Debug, Clone, PartialEq, Eq)]
struct SetInfo {
    set_id: String,
    created_at: UtcTimestamp,
    kind: RunKind,
}

/// What a pruning pass did on one target.
#[derive(Debug, Default)]
pub(super) struct Pruned {
    /// Sets deleted.
    pub(super) set_ids: Vec<String>,
    /// Sets retention released that could not be deleted (for example on a write-once
    /// share); a warning, never a run failure.
    pub(super) failures: usize,
}

/// Deletes the sets of `installation` on the target at `root` that retention does not
/// keep. Sets of other installations and sets whose manifest cannot be read are never
/// touched.
pub(super) fn prune(
    root: &Path,
    target_id: TargetId,
    key: &BackupKey,
    installation: InstallationId,
    settings: &OperationsSettings,
) -> Result<Pruned, BackupError> {
    let ours: Vec<SetInfo> = list_sets(root, key)?
        .into_iter()
        .filter_map(|listing| match listing.manifest {
            Ok(manifest) if manifest.installation_id == installation => Some(SetInfo {
                set_id: listing.set_id,
                created_at: manifest.created_at,
                kind: manifest.kind,
            }),
            Ok(_) | Err(_) => None,
        })
        .collect();
    let keep = kept(&ours, settings);
    let mut pruned = Pruned::default();
    for set in ours.iter().filter(|set| !keep.contains(&set.set_id)) {
        match fs::remove_dir_all(root.join(SETS_FOLDER).join(&set.set_id)) {
            Ok(()) => pruned.set_ids.push(set.set_id.clone()),
            Err(error) => {
                tracing::warn!(%target_id, error_kind = ?error.kind(), "backup set not pruned");
                pruned.failures += 1;
            }
        }
    }
    Ok(pruned)
}

/// The set IDs retention keeps: the newest `keep_recent`; the newest of each of the
/// newest `keep_daily` UTC days and `keep_weekly` ISO weeks that have sets; every manual
/// and pre-update set; always the newest set.
fn kept(sets: &[SetInfo], settings: &OperationsSettings) -> BTreeSet<String> {
    let mut newest_first: Vec<&SetInfo> = sets.iter().collect();
    newest_first.sort_by(|a, b| (b.created_at, &b.set_id).cmp(&(a.created_at, &a.set_id)));
    let mut keep = BTreeSet::new();
    let mut days = Vec::new();
    let mut weeks = Vec::new();
    for (index, set) in newest_first.iter().enumerate() {
        let protected = index == 0
            || set.kind != RunKind::Scheduled
            || index < usize::try_from(settings.keep_recent).unwrap_or(usize::MAX);
        let Some((day, week)) = calendar(set.created_at) else {
            keep.insert(set.set_id.clone());
            continue;
        };
        let new_day = first_in_period(&mut days, day, settings.keep_daily);
        let new_week = first_in_period(&mut weeks, week, settings.keep_weekly);
        if protected || new_day || new_week {
            keep.insert(set.set_id.clone());
        }
    }
    keep
}

/// Records `period` when it is new and fewer than `limit` periods are recorded; true
/// when it was recorded now, so the set is the newest of a kept period.
fn first_in_period(seen: &mut Vec<(i16, i16)>, period: (i16, i16), limit: u32) -> bool {
    let fits = u32::try_from(seen.len()).is_ok_and(|count| count < limit);
    if seen.contains(&period) || !fits {
        return false;
    }
    seen.push(period);
    true
}

/// The UTC day `(year, day of year)` and ISO week `(ISO year, week)` of `at`.
fn calendar(at: UtcTimestamp) -> Option<((i16, i16), (i16, i16))> {
    let date = at
        .to_string()
        .parse::<Timestamp>()
        .ok()?
        .to_zoned(TimeZone::UTC)
        .date();
    let week = date.iso_week_date();
    Some((
        (date.year(), date.day_of_year()),
        (week.year(), i16::from(week.week())),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy::DEFAULT_SETTINGS;

    fn set(id: &str, at: &str, kind: RunKind) -> SetInfo {
        SetInfo {
            set_id: id.to_owned(),
            created_at: UtcTimestamp::parse(at).unwrap(),
            kind,
        }
    }

    fn settings(keep_recent: u32, keep_daily: u32, keep_weekly: u32) -> OperationsSettings {
        OperationsSettings {
            keep_recent,
            keep_daily,
            keep_weekly,
            ..DEFAULT_SETTINGS
        }
    }

    fn ids(keep: &BTreeSet<String>) -> Vec<&str> {
        keep.iter().map(String::as_str).collect()
    }

    #[test]
    fn keeps_the_recent_sets_and_the_newest_of_each_kept_day_and_week() {
        // 2026-01-05 is a Monday (ISO week 2); 2026-01-04 is a Sunday (ISO week 1).
        let sets = [
            set("a", "2026-01-03T10:00:00.000000000Z", RunKind::Scheduled),
            set("b", "2026-01-04T09:00:00.000000000Z", RunKind::Scheduled),
            set("c", "2026-01-04T10:00:00.000000000Z", RunKind::Scheduled),
            set("d", "2026-01-05T09:00:00.000000000Z", RunKind::Scheduled),
            set("e", "2026-01-05T10:00:00.000000000Z", RunKind::Scheduled),
            set("f", "2026-01-05T11:00:00.000000000Z", RunKind::Scheduled),
        ];
        assert_eq!(ids(&kept(&sets, &settings(2, 0, 0))), ["e", "f"]);
        assert_eq!(ids(&kept(&sets, &settings(2, 2, 0))), ["c", "e", "f"]);
        assert_eq!(ids(&kept(&sets, &settings(2, 0, 2))), ["c", "e", "f"]);
        assert_eq!(ids(&kept(&sets, &settings(2, 3, 0))), ["a", "c", "e", "f"]);
        assert_eq!(ids(&kept(&sets, &settings(0, 0, 0))), ["f"]);
    }

    #[test]
    fn the_defaults_keep_fifteen_of_two_months_of_hourly_sets() {
        // Hourly sets from 07:00 to 18:00 every day for 60 days; the newest set counts as
        // recent, daily and weekly at once (the operations guide's figure).
        let start = jiff::civil::date(2026, 1, 1);
        let mut sets = Vec::new();
        for day in 0..60 {
            let date = start.checked_add(jiff::Span::new().days(day)).unwrap();
            for hour in 7..=18 {
                let id = format!("{date}T{hour:02}");
                sets.push(set(
                    &id,
                    &format!("{id}:00:00.000000000Z"),
                    RunKind::Scheduled,
                ));
            }
        }
        assert_eq!(kept(&sets, &DEFAULT_SETTINGS).len(), 6 + 6 + 3);
    }

    #[test]
    fn manual_and_pre_update_sets_are_always_kept() {
        let sets = [
            set("a", "2025-01-01T00:00:00.000000000Z", RunKind::PreUpdate),
            set("b", "2025-06-01T00:00:00.000000000Z", RunKind::Manual),
            set("c", "2025-07-01T00:00:00.000000000Z", RunKind::Scheduled),
            set("d", "2026-01-01T00:00:00.000000000Z", RunKind::Scheduled),
        ];
        assert_eq!(ids(&kept(&sets, &settings(0, 0, 0))), ["a", "b", "d"]);
    }
}
