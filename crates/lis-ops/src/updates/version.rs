//! Semantic Versioning 2.0.0 text: validation of an approved version, and precedence of two
//! versions for the in-app updater, which installs only newer versions (ADR 0020).

use std::cmp::Ordering;

/// True for `MAJOR.MINOR.PATCH`, optionally with `-pre.release` and `+build`
/// identifiers, as Semantic Versioning 2.0.0 defines them.
pub(super) fn is_semver(text: &str) -> bool {
    let (rest, build) = text
        .split_once('+')
        .map_or((text, None), |(r, b)| (r, Some(b)));
    let (core, pre) = rest
        .split_once('-')
        .map_or((rest, None), |(c, p)| (c, Some(p)));
    let numeric = |part: &str| {
        !part.is_empty()
            && part.bytes().all(|b| b.is_ascii_digit())
            && (part == "0" || !part.starts_with('0'))
    };
    let identifier = |part: &str| {
        !part.is_empty() && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
    };
    let core_parts: Vec<&str> = core.split('.').collect();
    core_parts.len() == 3
        && core_parts.iter().all(|part| numeric(part))
        && pre.is_none_or(|pre| {
            pre.split('.').all(|part| {
                identifier(part) && (!part.bytes().all(|b| b.is_ascii_digit()) || numeric(part))
            })
        })
        && build.is_none_or(|build| build.split('.').all(identifier))
}

/// Semantic Versioning 2.0.0 precedence of two versions; `None` when either is not a
/// semantic version. Build metadata does not count.
#[must_use]
pub fn version_precedence(left: &str, right: &str) -> Option<Ordering> {
    if !is_semver(left) || !is_semver(right) {
        return None;
    }
    let parts = |text: &str| {
        let rest = text.split_once('+').map_or(text, |(rest, _)| rest);
        let (core, pre) = rest
            .split_once('-')
            .map_or((rest, None), |(c, p)| (c, Some(p)));
        let core: Vec<u64> = core
            .split('.')
            .filter_map(|part| part.parse().ok())
            .collect();
        (
            core,
            pre.map(|pre| pre.split('.').map(str::to_owned).collect::<Vec<_>>()),
        )
    };
    let (left_core, left_pre) = parts(left);
    let (right_core, right_pre) = parts(right);
    let order = left_core
        .cmp(&right_core)
        .then_with(|| match (&left_pre, &right_pre) {
            (None, None) => Ordering::Equal,
            // A pre-release precedes its release.
            (None, Some(_)) => Ordering::Greater,
            (Some(_), None) => Ordering::Less,
            (Some(left), Some(right)) => left
                .iter()
                .zip(right)
                .map(|(a, b)| match (a.parse::<u64>(), b.parse::<u64>()) {
                    (Ok(a), Ok(b)) => a.cmp(&b),
                    (Ok(_), Err(_)) => Ordering::Less,
                    (Err(_), Ok(_)) => Ordering::Greater,
                    (Err(_), Err(_)) => a.cmp(b),
                })
                .find(|order| order.is_ne())
                .unwrap_or_else(|| left.len().cmp(&right.len())),
        });
    Some(order)
}
