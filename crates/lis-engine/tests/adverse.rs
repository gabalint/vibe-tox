//! M5 adverse-scenario suite (implementation plan, milestone M5 and scope;
//! validation plan VAL-04, VAL-07, VAL-38, VAL-50, VAL-51,
//! VAL-53, VAL-56): analyzer garbage, interrupted and repeated transfers, damaged
//! storage, a clock stepped back and an expired session around release, and printers
//! that fail. Each test drives the engine's public API on synthetic data; expected
//! outcomes come from the requirements and design briefs named in each test, never from
//! the implementation. A test that finds a defect stays failing until corrected;
//! unresolved release limitations belong in `docs/quality/open-items-g1-g2.md`.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

#[expect(
    dead_code,
    reason = "the scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/fixture.rs"]
mod fixture;
#[path = "scenarios/m2_config.rs"]
mod m2_config;
#[expect(
    dead_code,
    reason = "the scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/m2_lab.rs"]
mod m2_lab;
#[expect(
    dead_code,
    reason = "the scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/m2_steps.rs"]
mod m2_steps;
#[expect(
    dead_code,
    reason = "the scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/m3_fixture.rs"]
mod m3_fixture;
#[expect(
    dead_code,
    reason = "the scenario fixture has helpers this suite does not use"
)]
#[path = "scenarios/m3_report.rs"]
mod m3_report;

#[path = "adverse/analyzer.rs"]
mod analyzer;
#[path = "adverse/printer.rs"]
mod printer;
#[path = "adverse/release.rs"]
mod release;
#[path = "adverse/storage.rs"]
mod storage;
