import type { CommandName } from "@/generated/CommandName";

/** Catalog review is a laboratory workflow, independent of the source of its drafts. */
export const CATALOG_REVIEW_PATH = "/configuration/catalog-review";

/** The catalog directories and version views a load or an approval changes. */
export const CATALOG_QUERIES: readonly CommandName[] = [
  "list_specimen_types",
  "list_analytes",
  "list_methods",
  "list_method_versions",
  "get_method_version",
  "list_orderables",
  "list_orderable_versions",
  "get_orderable_version",
  "list_active_orderables",
  "list_validity_policies",
  "list_validity_policy_versions",
  "get_validity_policy",
];
