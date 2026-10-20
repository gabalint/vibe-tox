//! Audit action names (`analytical.<verb>`), shared by the commands and the
//! engine's command registry.

/// `set_instrument_status`.
pub const SET_INSTRUMENT_STATUS: &str = "analytical.set_instrument_status";
/// `register_source`.
pub const REGISTER_SOURCE: &str = "analytical.register_source";
/// `approve_source`.
pub const APPROVE_SOURCE: &str = "analytical.approve_source";
/// `register_lot`.
pub const REGISTER_LOT: &str = "analytical.register_lot";
/// `withdraw_lot`.
pub const WITHDRAW_LOT: &str = "analytical.withdraw_lot";
/// `create_prep_batch`.
pub const CREATE_PREP_BATCH: &str = "analytical.create_prep_batch";
/// `record_prep_step`.
pub const RECORD_PREP_STEP: &str = "analytical.record_prep_step";
/// `complete_prep_batch`.
pub const COMPLETE_PREP_BATCH: &str = "analytical.complete_prep_batch";
/// `create_run`.
pub const CREATE_RUN: &str = "analytical.create_run";
/// `add_patient_position`.
pub const ADD_PATIENT_POSITION: &str = "analytical.add_patient_position";
/// `add_qc_position`.
pub const ADD_QC_POSITION: &str = "analytical.add_qc_position";
/// `fail_execution`.
pub const FAIL_EXECUTION: &str = "analytical.fail_execution";
/// `close_run`.
pub const CLOSE_RUN: &str = "analytical.close_run";
/// `record_qc_observations`, called by the result importer.
pub const RECORD_QC_OBSERVATIONS: &str = "analytical.record_qc_observations";
/// `assess_qc`.
pub const ASSESS_QC: &str = "analytical.assess_qc";
/// `resolve_qc_conflict`.
pub const RESOLVE_QC_CONFLICT: &str = "analytical.resolve_qc_conflict";
/// `open_deviation`.
pub const OPEN_DEVIATION: &str = "analytical.open_deviation";
/// `close_deviation`.
pub const CLOSE_DEVIATION: &str = "analytical.close_deviation";

/// Factual resolution of an unknown performing site from retained evidence.
pub const RECORD_RUN_PERFORMING_SITE: &str = "analytical.record_run_performing_site";
