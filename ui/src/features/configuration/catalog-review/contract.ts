// REQ-OPS-015: review covers all catalog drafts, regardless of how they were entered.
// Preview commands need manage_configuration or approve_configuration. Approval needs
// approve_configuration, fresh authentication and the laboratory's separation-of-duties policy.
import type { CommandName } from "@/generated/CommandName";
import { CATALOG_QUERIES } from "@/lib/catalogQueries";

/** `generate_catalog_preview() → string`: the new report's ID. */
export const GENERATE_CATALOG_PREVIEW = "generate_catalog_preview" satisfies CommandName;
/** `list_catalog_previews() → CatalogPreviewSummary[]`, newest first. */
export const LIST_CATALOG_PREVIEWS = "list_catalog_previews" satisfies CommandName;
/** `view_catalog_preview_pdf({input: {previewId}}) → null`: opens a report window. */
export const VIEW_CATALOG_PREVIEW_PDF = "view_catalog_preview_pdf" satisfies CommandName;
/** `export_catalog_preview_pdf({input: {previewId}}) → ExportedReportFile`, to Downloads. */
export const EXPORT_CATALOG_PREVIEW_PDF = "export_catalog_preview_pdf" satisfies CommandName;
/** `approve_reviewed_catalog({input: ApproveReviewedCatalogInput}) → CatalogReviewApproved`. */
export const APPROVE_REVIEWED_CATALOG = "approve_reviewed_catalog" satisfies CommandName;

export const REVIEW_INVALIDATES: readonly CommandName[] = [
  LIST_CATALOG_PREVIEWS,
  "standard_catalog_status",
  ...CATALOG_QUERIES,
];
