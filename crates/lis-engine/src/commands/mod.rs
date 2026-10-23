//! The engine's IPC commands and queries, one method each, grouped by area.
//! Each method runs through [`Engine::run`](crate::Engine) with its registry entry.

pub(crate) mod audit;
pub(crate) mod listeners;
// Wave 3+: one module per area, each owned by its worker. Edit only your block.
// identity: begin
pub(crate) mod identity;
// identity: end
// catalog: begin
pub(crate) mod catalog;
// catalog: end
// interfaces: begin
pub(crate) mod interfaces;
// interfaces: end
// orders: begin
pub(crate) mod orders;
// orders: end
// specimens: begin
pub(crate) mod order_accessions;
pub(crate) mod specimens;
// specimens: end
// analytical: begin
pub(crate) mod analytical;
// analytical: end
// results: begin
pub(crate) mod results;
// results: end
// reporting: begin
pub(crate) mod reporting;
// reporting: end
// billing: begin
pub(crate) mod billing;
// billing: end
// work_queues: begin
pub(crate) mod work_queues;
// work_queues: end
// backup: begin
pub(crate) mod backup;
// backup: end
// runtime: begin
pub(crate) mod dialogs;
pub(crate) mod operations;
// runtime: end
// anchors: begin
pub(crate) mod anchors;
// anchors: end
// demo: begin
pub(crate) mod demo;
// demo: end
// result_files: begin
pub(crate) mod result_files;
// result_files: end
// file_templates: begin
pub(crate) mod file_templates;
// file_templates: end
pub(crate) mod security_keys;
mod sign_in;
mod unlock_pin;
mod users;

pub(crate) mod billing_intake;
pub(crate) mod collection_corrections;
// people: begin
pub(crate) mod people;
// people: end
