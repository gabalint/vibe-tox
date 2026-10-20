// Billing (REQ-BIL-002, REQ-BIL-005; M3 design brief §5 and §7 "Billing"). Every command
// needs `billing`; confirming a class mapping needs `approve_configuration`, checked by the
// engine. Derivation and handoff reads are audited, since they carry patient identity. The UI
// shows what the engine derived and never suggests a class, a service or a code.
import type { CommandName } from "@/generated/CommandName";

export const BILLING = "billing";

/** `list_class_mapping_versions() → BillingClassMappingVersionSummary[]`. */
export const LIST_CLASS_MAPPING_VERSIONS = "list_class_mapping_versions";
/** `get_class_mapping_version({input:{versionId}}) → BillingClassMappingVersionView`. */
export const GET_CLASS_MAPPING_VERSION = "get_class_mapping_version";
/** `list_billing_work({input:{filter, limit}}) → BillingWorkItem[]`. */
export const LIST_BILLING_WORK = "list_billing_work";
/** `get_service_derivation({input:{derivationId}}) → DerivationView` (audited read). */
export const GET_SERVICE_DERIVATION = "get_service_derivation";
/** `get_billing_handoff({input:{handoffId}}) → HandoffView` (audited read). */
export const GET_BILLING_HANDOFF = "get_billing_handoff";
/** Catalog read (any signed-in session): analyte names for the mapping entries. */
export const LIST_ANALYTES = "list_analytes";

export const LIST_LIMIT = 200;

/** What each change makes stale. */
export const MAPPING_QUERIES: readonly CommandName[] = [
  LIST_CLASS_MAPPING_VERSIONS,
  GET_CLASS_MAPPING_VERSION,
];
export const DERIVATION_QUERIES: readonly CommandName[] = [
  LIST_BILLING_WORK,
  GET_SERVICE_DERIVATION,
];
export const HANDOFF_QUERIES: readonly CommandName[] = [
  LIST_BILLING_WORK,
  GET_SERVICE_DERIVATION,
  GET_BILLING_HANDOFF,
];
