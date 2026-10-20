// Accessioning, receipt, aliquots and labels (REQ-SPC-001 to REQ-SPC-003; design brief §4).
// Every command needs `accession`; changes carry the specimen's or container's `revision`
// and fail with `conflict` when stale, and answer with the updated `AccessionDetail`. Types
// are generated from `lis-specimens` into `@/generated`; views carry the patient by ID,
// except the container lookup, which returns the patient for checking the label.
import type { CommandName } from "@/generated/CommandName";
import { ORDER_ACCESSION_CONTEXT } from "../useOrderAccessionContext";

/** `list_specimen_types() → SpecimenTypeView[]`. */
export const LIST_SPECIMEN_TYPES = "list_specimen_types";

/** `create_accession({input: CreateAccessionInput}) → AccessionDetail`. */
export const CREATE_ACCESSION = "create_accession";

/** `get_accession({input: GetAccessionInput}) → AccessionDetail` (audited read). */
export const GET_ACCESSION = "get_accession";

/** `find_container({input: FindContainerInput}) → ContainerLookup` (audited read). */
export const FIND_CONTAINER = "find_container";

/** The queries every accessioning command makes stale. */
export const ACCESSION_QUERIES: readonly CommandName[] = [
  GET_ACCESSION,
  FIND_CONTAINER,
  "get_container_history",
  "list_excursions",
  "get_excursion",
  ORDER_ACCESSION_CONTEXT,
];
