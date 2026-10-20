// Configuration commands (REQ-ANA-002, REQ-ANA-009, REQ-SPC-008, REQ-INT-005). Types are
// generated from `lis-catalog`, `lis-identity` and `lis-interfaces` into `@/generated`.
// Catalog reads need only a signed-in session; drafting and every client, practitioner and
// instrument change need `manage_configuration`; approving and retiring catalog versions need
// `approve_configuration`, and the engine refuses an approval by the draft's author.
import type { CommandName } from "@/generated/CommandName";

export const MANAGE_CONFIGURATION = "manage_configuration";
export const APPROVE_CONFIGURATION = "approve_configuration";

export const LIST_SPECIMEN_TYPES = "list_specimen_types";
export const LIST_ANALYTES = "list_analytes";
export const LIST_METHODS = "list_methods";
export const LIST_ORDERABLES = "list_orderables";
/** `list_active_orderables() → OrderableVersionView[]`: what order entry offers. */
const LIST_ACTIVE_ORDERABLES = "list_active_orderables";

export const LIST_METHOD_VERSIONS = "list_method_versions";
export const GET_METHOD_VERSION = "get_method_version";
export const LIST_ORDERABLE_VERSIONS = "list_orderable_versions";
export const GET_ORDERABLE_VERSION = "get_orderable_version";
export const LIST_VALIDITY_POLICY_VERSIONS = "list_validity_policy_versions";
export const GET_VALIDITY_POLICY = "get_validity_policy";

/**
 * What a method version change makes stale: the directory's version numbers too; orderable
 * versions name their methods.
 */
export const METHOD_QUERIES: readonly CommandName[] = [
  LIST_METHODS,
  LIST_METHOD_VERSIONS,
  GET_METHOD_VERSION,
  GET_ORDERABLE_VERSION,
  LIST_ACTIVE_ORDERABLES,
];
/** What an orderable version change makes stale, including the tests order entry offers. */
export const ORDERABLE_QUERIES: readonly CommandName[] = [
  LIST_ORDERABLES,
  LIST_ORDERABLE_VERSIONS,
  GET_ORDERABLE_VERSION,
  LIST_ACTIVE_ORDERABLES,
];
export const VALIDITY_POLICY_QUERIES: readonly CommandName[] = [
  LIST_SPECIMEN_TYPES,
  LIST_VALIDITY_POLICY_VERSIONS,
  GET_VALIDITY_POLICY,
];

export const LIST_CLIENTS = "list_clients";
export const LIST_PRACTITIONERS = "list_practitioners";

export const LIST_INSTRUMENTS = "list_instruments";
/** Each instrument change restarts the listeners, so their status changes too. */
export const INSTRUMENT_QUERIES: readonly CommandName[] = [
  LIST_INSTRUMENTS,
  "list_listener_status",
];
