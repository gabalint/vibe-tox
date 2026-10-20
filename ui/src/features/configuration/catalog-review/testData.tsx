// Synthetic reports from laboratory-entered catalog drafts; no starter load is needed.
import type { CatalogPreviewSummary } from "@/generated/CatalogPreviewSummary";
import { CATALOG_REVIEW_PATH } from "@/lib/catalogQueries";
import { mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { session } from "@/lib/test/session";
import { CatalogReviewPage } from "./CatalogReviewPage";

export const PREVIEW_ID = "00000000-0000-7000-8000-00000000d001";

export function preview(overrides: Partial<CatalogPreviewSummary> = {}): CatalogPreviewSummary {
  return {
    id: PREVIEW_ID,
    generatedAt: "2026-10-13T09:00:00Z",
    generatedBy: "00000000-0000-7000-8000-000000000002",
    generatedByDisplay: "Casey Author",
    contentDigest: "cd34",
    drafts: 5,
    reviewedAt: null,
    reviewedBy: null,
    blankCutoffs: [
      {
        methodVersionId: "00000000-0000-7000-8000-00000000e001",
        analyteId: "00000000-0000-7000-8000-00000000e101",
        methodCode: "UR-IA",
        methodVersion: 1,
        analyteCode: "UR-AMP-IA",
      },
      {
        methodVersionId: "00000000-0000-7000-8000-00000000e002",
        analyteId: "00000000-0000-7000-8000-00000000e102",
        methodCode: "UR-OPI-MS",
        methodVersion: 1,
        analyteCode: "UR-CODE",
      },
    ],
    ...overrides,
  };
}

export const REVIEWED = preview({
  id: "00000000-0000-7000-8000-00000000d000",
  generatedAt: "2026-10-12T09:00:00Z",
  reviewedAt: "2026-10-12T10:00:00Z",
  reviewedBy: "00000000-0000-7000-8000-000000000003",
  reviewedByDisplay: "Robin Reviewer",
  blankCutoffs: [],
});

export function renderCatalogReview(
  capabilities: readonly string[],
  commands: Record<string, (args: unknown) => unknown>,
) {
  mockCommands({ current_session: () => session(capabilities), ...commands });
  return renderRoutes(
    [{ path: CATALOG_REVIEW_PATH, element: <CatalogReviewPage /> }],
    CATALOG_REVIEW_PATH,
  );
}
