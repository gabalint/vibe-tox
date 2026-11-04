//! Routing rules against a message's values, and schedules against the clock.
//! Pure.
//!
//! A rule holds when every value of its field in the message meets it, and
//! fails when the message has no value for the field. A billing handoff that
//! covers two clients therefore goes only to a connection whose rules accept
//! both, never to one client's billing company with the other's patients.

use jiff::Timestamp;
use jiff::tz::TimeZone;
use lis_core::UtcTimestamp;

use super::model::{RoutingFacts, RoutingRule, RoutingValue, RuleCondition, RuleField, Schedule};
use super::settings::clock_time;

/// Whether every rule holds; no rules accept every message.
#[must_use]
pub fn rules_match(rules: &[RoutingRule], facts: &RoutingFacts) -> bool {
    unmet(rules, facts).next().is_none()
}

/// The rules that do not hold, in order.
pub(crate) fn unmet<'a>(
    rules: &'a [RoutingRule],
    facts: &'a RoutingFacts,
) -> impl Iterator<Item = &'a RoutingRule> {
    rules.iter().filter(|rule| !holds(rule, facts))
}

fn holds(rule: &RoutingRule, facts: &RoutingFacts) -> bool {
    let values = values_of(facts, rule.field);
    !values.is_empty()
        && values.iter().all(|value| match rule.condition {
            RuleCondition::Equals => value.value == rule.value,
            RuleCondition::OneOf => rule
                .value
                .split(',')
                .any(|accepted| accepted.trim() == value.value),
            RuleCondition::StartsWith => value.value.starts_with(&rule.value),
        })
}

fn values_of(facts: &RoutingFacts, field: RuleField) -> &[RoutingValue] {
    match field {
        RuleField::Client => &facts.clients,
        RuleField::Practitioner => &facts.practitioners,
        RuleField::Location => &facts.locations,
        RuleField::Test => &facts.tests,
    }
}

/// Whether a message made at `created_at` may be written at `now` on a
/// connection with `schedule`: at once, or once a send time has come since it
/// was made. Daily times are in the laboratory's `zone`; `None` (no time zone
/// recorded) holds daily messages back.
#[must_use]
pub fn schedule_due(
    schedule: &Schedule,
    created_at: UtcTimestamp,
    now: UtcTimestamp,
    zone: Option<&TimeZone>,
) -> bool {
    match schedule {
        Schedule::Immediately => true,
        Schedule::EveryMinutes { minutes } => {
            let period = i64::from(*minutes) * 60;
            let window = now.unix_seconds() - now.unix_seconds().rem_euclid(period);
            created_at.unix_seconds() <= window
        }
        Schedule::DailyAt { time } => zone
            .and_then(|zone| latest_daily(time, now, zone))
            .is_some_and(|window| created_at.unix_seconds() <= window),
    }
}

/// The latest daily send time at or before `now`, in Unix seconds.
fn latest_daily(time: &str, now: UtcTimestamp, zone: &TimeZone) -> Option<i64> {
    let (hours, minutes) = clock_time(time)?;
    let today = Timestamp::from_second(now.unix_seconds())
        .ok()?
        .to_zoned(zone.clone());
    let at = today
        .with()
        .hour(hours)
        .minute(minutes)
        .second(0)
        .build()
        .ok()?;
    let at = if at.timestamp().as_second() > now.unix_seconds() {
        at.yesterday().ok()?
    } else {
        at
    };
    Some(at.timestamp().as_second())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn value(value: &str) -> RoutingValue {
        RoutingValue {
            value: value.to_owned(),
            label: value.to_owned(),
        }
    }

    fn facts(clients: &[&str]) -> RoutingFacts {
        RoutingFacts {
            clients: clients.iter().copied().map(value).collect(),
            practitioners: vec![value("1234567893")],
            ..RoutingFacts::default()
        }
    }

    fn rule(field: RuleField, condition: RuleCondition, value: &str) -> RoutingRule {
        RoutingRule {
            field,
            condition,
            value: value.to_owned(),
        }
    }

    fn at(text: &str) -> UtcTimestamp {
        UtcTimestamp::parse(text).unwrap()
    }

    #[test]
    fn no_rules_accept_every_message() {
        assert!(rules_match(&[], &facts(&["CLINIC-A"])));
    }

    #[test]
    fn every_rule_must_hold() {
        let rules = [
            rule(RuleField::Client, RuleCondition::Equals, "CLINIC-A"),
            rule(RuleField::Practitioner, RuleCondition::StartsWith, "12"),
        ];
        assert!(rules_match(&rules, &facts(&["CLINIC-A"])));
        assert!(!rules_match(&rules, &facts(&["CLINIC-B"])));
        assert_eq!(unmet(&rules, &facts(&["CLINIC-B"])).count(), 1);
    }

    // A file with two clients' patients never goes to one client's partner.
    #[test]
    fn a_rule_holds_only_for_all_values_of_its_field() {
        let one = [rule(RuleField::Client, RuleCondition::Equals, "CLINIC-A")];
        assert!(!rules_match(&one, &facts(&["CLINIC-A", "CLINIC-B"])));
        let both = [rule(
            RuleField::Client,
            RuleCondition::OneOf,
            "CLINIC-A, CLINIC-B",
        )];
        assert!(rules_match(&both, &facts(&["CLINIC-A", "CLINIC-B"])));
    }

    #[test]
    fn a_rule_on_a_missing_value_does_not_hold() {
        let rules = [rule(RuleField::Location, RuleCondition::Equals, "MAIN")];
        assert!(!rules_match(&rules, &facts(&["CLINIC-A"])));
    }

    #[test]
    fn every_minutes_waits_for_the_next_boundary() {
        let schedule = Schedule::EveryMinutes { minutes: 15 };
        let made = at("2026-11-03T10:07:00.000000000Z");
        assert!(!schedule_due(
            &schedule,
            made,
            at("2026-11-03T10:14:59.000000000Z"),
            None
        ));
        assert!(schedule_due(
            &schedule,
            made,
            at("2026-11-03T10:15:00.000000000Z"),
            None
        ));
    }

    #[test]
    fn daily_waits_for_the_lab_time() {
        let zone = TimeZone::get("America/Chicago").unwrap();
        let schedule = Schedule::DailyAt {
            time: "18:00".to_owned(),
        };
        // 10:00 CST is 16:00 UTC; 18:00 CST is 00:00 UTC the next day.
        let made = at("2026-11-03T16:00:00.000000000Z");
        let before = at("2026-11-03T23:59:00.000000000Z");
        let after = at("2026-11-04T00:00:00.000000000Z");
        assert!(!schedule_due(&schedule, made, before, Some(&zone)));
        assert!(schedule_due(&schedule, made, after, Some(&zone)));
        assert!(!schedule_due(&schedule, made, after, None));
    }
}
