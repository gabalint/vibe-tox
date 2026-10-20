// Starter loading (REQ-OPS-014, REQ-OPS-016). The engine checks every command:
// the tree and load need manage_configuration; status queries need any session.
import type { CommandName } from "@/generated/CommandName";
import { CATALOG_QUERIES } from "@/lib/catalogQueries";

export const MANAGE_CONFIGURATION = "manage_configuration";
export const APPROVE_CONFIGURATION = "approve_configuration";

export const STARTER_DATA_PATH = "/configuration/starter-data";
export const LOAD_CATALOG_PATH = `${STARTER_DATA_PATH}/load`;

/** `standard_catalog_tree() → StandardCatalogTree`: every item of the file, with its state here. */
export const STANDARD_CATALOG_TREE = "standard_catalog_tree" satisfies CommandName;
/** `standard_catalog_status() → StandardCatalogStatus`: the loads and the drafts still unreviewed. */
export const STANDARD_CATALOG_STATUS = "standard_catalog_status" satisfies CommandName;
/** `load_standard_catalog({input: LoadStandardCatalog}) → StandardCatalogLoaded`. */
export const LOAD_STANDARD_CATALOG = "load_standard_catalog" satisfies CommandName;
/** `demo_status() → DemoStatus`. */
export const DEMO_STATUS = "demo_status" satisfies CommandName;
/** `load_demo_data() → DemoDataLoaded`: refused outside the demo laboratory. */
export const LOAD_DEMO_DATA = "load_demo_data" satisfies CommandName;

export const LOAD_INVALIDATES: readonly CommandName[] = [
  STANDARD_CATALOG_TREE,
  STANDARD_CATALOG_STATUS,
  ...CATALOG_QUERIES,
];

/** Demo data adds parties and patients, sets a missing profile and fills demo cutoffs. */
export const DEMO_INVALIDATES: readonly CommandName[] = [
  DEMO_STATUS,
  "get_laboratory_profile",
  "list_clients",
  "list_practitioners",
  "list_delivery_destinations",
  "list_identity_holds",
  "list_work_queues",
  "list_method_versions",
  "get_method_version",
];
