//! Engine scenario tests on a temporary database (REQ-IDN-003, REQ-IDN-004,
//! REQ-OPS-001, NFR-08, NFR-19). Synthetic data only.

// Marks helpers outside `#[test]` functions as test code for clippy's test allowances.
#![cfg(test)]

mod administration;
mod audit_trail;
mod authorization;
mod fixture;
mod no_secrets;
mod one_person;
mod sessions;
mod sign_in;
// Wave 3+: one scenario module per area. Edit only your block.
// identity: begin
mod identity;
// identity: end
// catalog: begin
mod catalog;
mod starter_catalog;
// catalog: end
// interfaces: begin
mod au_protocol;
mod interfaces;
// interfaces: end
// orders: begin
mod orders;
// orders: end
// specimens: begin
mod specimens;
// specimens: end
// analytical: begin
mod analytical;
// analytical: end
// results: begin
mod reflex;
mod results;
// results: end
// reporting: begin
mod reporting;
// reporting: end
// billing: begin
mod billing;
// billing: end
// work_queues: begin
mod work_queues;
// work_queues: end
// backup: begin
mod m4_backup;
mod m4_journey;
// backup: end
// runtime: begin
mod m4_clock;
mod m4_dialogs;
mod m4_keys;
mod m4_offline_day;
mod m4_runtime;
// runtime: end
// anchors: begin
mod m4_anchors;
mod m4_canary;
// anchors: end
// m2-journey: begin
mod m2_config;
mod m2_imports;
mod m2_journey;
mod m2_lab;
mod m2_orders;
mod m2_review;
mod m2_steps;
// m2-journey: end
// m3-races: begin
mod m3_changes;
mod m3_concurrency;
mod m3_crashes;
mod m3_exit;
mod m3_fixture;
mod m3_release_races;
mod m3_report;
mod m3_validity;
// m3-races: end
// demo: begin
mod demo_lab;
mod demo_setup;
// demo: end
// result_files: begin
mod result_files_intake;
// result_files: end
// file_templates: begin
mod file_templates;
// file_templates: end
// result_files_import: begin
mod rf_import;
mod rf_kept;
mod rf_lab;
// result_files_import: end

mod billing_intake;
mod collection_report_impact;

mod billing_financial;

mod remediation;
