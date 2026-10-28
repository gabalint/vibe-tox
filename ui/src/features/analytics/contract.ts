// Bench work and result review (REQ-ANA-001, REQ-ANA-004, REQ-ANA-005, REQ-ANA-007,
// REQ-RES-001, REQ-RES-005; design brief §4). Types are generated from `lis-analytical`,
// `lis-results` and `lis-catalog` into `@/generated`. Analytical views carry no
// demographics; result views are audited reads. Readiness, lots and source registrations
// need only a signed-in session to read.
import type { CommandName } from "@/generated/CommandName";

export const PERFORM_ANALYSIS = "perform_analysis";
export const MANAGE_QUALITY = "manage_quality";
export const MANAGE_CONFIGURATION = "manage_configuration";
export const APPROVE_CONFIGURATION = "approve_configuration";

/** `list_instrument_readiness() → InstrumentReadinessView[]`. */
export const LIST_INSTRUMENT_READINESS = "list_instrument_readiness";
/** `list_source_registrations({input:{instrumentId}}) → SourceRegistrationView[]`. */
export const LIST_SOURCE_REGISTRATIONS = "list_source_registrations";
/** `list_lots({input:{activeOnly}}) → LotView[]`, with expiry derived by the engine. */
export const LIST_LOTS = "list_lots";
/**
 * `list_instruments() → InstrumentConfiguration[]` (`manage_configuration`, as for
 * `register_source`): an AU source restates its analyzer's exact wire settings.
 */
export const LIST_INSTRUMENTS = "list_instruments";

/** `list_worklist({input:{methodVersionId}}) → WorklistEntry[]` (`perform_analysis`). */
export const LIST_WORKLIST = "list_worklist";
/** `list_runs({input:{state?}}) → RunSummary[]` (`perform_analysis`). */
export const LIST_RUNS = "list_runs";
/** `get_run({input:{runId}}) → RunDetail` (`perform_analysis`). */
export const GET_RUN = "get_run";
export const LIST_PREP_BATCHES = "list_prep_batches";
export const GET_PREP_BATCH = "get_prep_batch";
export const PREP_QUERIES: readonly CommandName[] = [LIST_PREP_BATCHES, GET_PREP_BATCH];
/** `get_run_qc({input:{runId}}) → RunQcView` (`manage_quality`). */
export const GET_RUN_QC = "get_run_qc";

/** `list_review_queue({input:{filter, limit}}) → ReviewQueuePage` (`review_results`). */
export const LIST_REVIEW_QUEUE = "list_review_queue";
/** `get_specimen_results({input:{specimenId}}) → SpecimenResults` (audited read). */
export const GET_SPECIMEN_RESULTS = "get_specimen_results";
/** `list_import_attempts({input:{instrumentId, limit, before}}) → ImportAttemptSummary[]`. */
export const LIST_IMPORT_ATTEMPTS = "list_import_attempts";
/** `get_import_attempt({input:{importAttemptId}}) → ImportAttemptDetail` (audited read). */
export const GET_IMPORT_ATTEMPT = "get_import_attempt";
/** `list_validity_policies({input:{specimenTypeCode}}) → CatalogVersionSummary[]`. */
export const LIST_VALIDITY_POLICIES = "list_validity_policies";

/** The file template page (result files brief §7, §9); none of these writes a record. */
export const OPEN_LAYOUT_SAMPLE = "open_layout_sample";
export const PREVIEW_LAYOUT_SAMPLE = "preview_layout_sample";
export const CHECK_FILE_LAYOUT = "check_file_layout";

/** Catalog reads (any signed-in session). */
export const LIST_METHODS = "list_methods";
export const LIST_METHOD_VERSIONS = "list_method_versions";
export const LIST_ANALYTES = "list_analytes";
export const GET_VALIDITY_POLICY = "get_validity_policy";

/** What each change makes stale. */
export const READINESS_QUERIES: readonly CommandName[] = [LIST_INSTRUMENT_READINESS];
export const SOURCE_QUERIES: readonly CommandName[] = [
  LIST_SOURCE_REGISTRATIONS,
  LIST_INSTRUMENT_READINESS,
];
export const LOT_QUERIES: readonly CommandName[] = [LIST_LOTS, GET_RUN];
export const RUN_QUERIES: readonly CommandName[] = [GET_RUN, LIST_RUNS, LIST_WORKLIST];
export const QC_QUERIES: readonly CommandName[] = [GET_RUN_QC];
export const RESULT_QUERIES: readonly CommandName[] = [GET_SPECIMEN_RESULTS, LIST_REVIEW_QUEUE];
/** A reflex request changes the order, which the order screens show. */
export const ORDER_QUERIES: readonly CommandName[] = ["get_order", "list_orders"];
export const IMPORT_QUERIES: readonly CommandName[] = [
  "get_receipt_import_attempt",
  GET_IMPORT_ATTEMPT,
  LIST_IMPORT_ATTEMPTS,
  GET_SPECIMEN_RESULTS,
  LIST_REVIEW_QUEUE,
];

/** Records displayed per interactive page; continuation retains older matching work. */
export const LIST_LIMIT = 200;
