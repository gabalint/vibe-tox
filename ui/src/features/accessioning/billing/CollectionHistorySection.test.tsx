import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { CollectionHistory } from "@/generated/CollectionHistory";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { renderWithProviders } from "@/lib/test/render";
import { renderRoutes } from "@/lib/test/router";
import { SUMMARY } from "../testData";
import { CollectionHistorySection } from "./CollectionHistorySection";
import { CollectionReviewPage } from "./CollectionReviewPage";

const COLLECTION = {
  collectionId: "synthetic-collection",
  precision: "unknown" as const,
  collectedAt: null,
  collectedDate: null,
  utcOffsetMinutes: null,
  collector: null,
  observations: [],
};
const HISTORY: CollectionHistory = {
  accessionId: "synthetic-accession",
  accessionNumber: "SYN-ACC-1",
  subject: SUMMARY,
  collectionId: COLLECTION.collectionId,
  revision: 1,
  original: COLLECTION,
  current: COLLECTION,
  corrections: [],
  allowedActions: ["correct_collection_time"],
};

describe("Collection correction (BILL-G03)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  it("lets quality staff resolve from the scoped timing page without accession or subject reads", async () => {
    const resolve = vi.fn(() => ({}));
    const correction = {
      correctionId: "synthetic-correction",
      revision: 2,
      precision: "date" as const,
      collectedAt: null,
      utcOffsetMinutes: null,
      collectedDate: "2026-10-12",
      sourceDocumentRef: "Synthetic source",
      reason: "Date clarified",
      recordedBy: "synthetic-user",
      recordedAt: "2026-10-14T12:00:00Z",
      resolvedAt: null,
      resolvedBy: null,
      resolutionReason: null,
      allowedActions: ["resolve_collection_correction"],
    };
    mockCommands({
      get_collection_history: () => ({
        ...HISTORY,
        revision: 2,
        corrections: [correction],
        allowedActions: [],
      }),
      resolve_collection_correction: resolve,
    });
    const { user } = renderRoutes(
      [{ path: "/accessions/:accessionId/collection-review", element: <CollectionReviewPage /> }],
      `/accessions/${HISTORY.accessionId}/collection-review`,
    );
    await user.click(await screen.findByRole("button", { name: "Resolve collection review" }));
    const dialog = await screen.findByRole("dialog", { name: "Resolve collection review" });
    expect(screen.queryByText(/could not be loaded/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Correct collection time" })).toBeNull();
    await user.type(
      within(dialog).getByRole("textbox", { name: /Quality review and resolution reason/ }),
      "Synthetic source reviewed; testing review required",
    );
    await user.click(within(dialog).getByRole("button", { name: "Resolve review" }));
    expect(await screen.findByText("Collection review resolved")).toBeTruthy();
    expect(resolve).toHaveBeenCalledWith({
      input: {
        correctionId: correction.correctionId,
        reason: "Synthetic source reviewed; testing review required",
      },
    });
  });
  it("requires supplied precision and sends an attributable correction with the read revision", async () => {
    const correct = vi.fn(() =>
      Promise.reject(
        engineError("conflict", "Collection history changed. Reload before correcting."),
      ),
    );
    mockCommands({ get_collection_history: () => HISTORY, correct_collection_time: correct });
    const { user } = renderWithProviders(
      <CollectionHistorySection accessionId={HISTORY.accessionId} />,
    );
    await user.click(screen.getByRole("button", { name: "Review collection history" }));
    await user.click(await screen.findByRole("button", { name: "Correct collection time" }));
    const dialog = await screen.findByRole("dialog", { name: "Correct collection time" });
    expect(within(dialog).queryByRole("textbox", { name: "Collected by" })).toBeNull();
    expect(within(dialog).queryByRole("textbox", { name: /Collection date/ })).toBeNull();
    await chooseWithKeyboard(user, /Collection time known/, "Date only; time not recorded");
    await user.type(within(dialog).getByRole("textbox", { name: /Collection date/ }), "2026-10-12");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Source document reference/ }),
      "Synthetic corrected requisition",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Correction reason/ }),
      "Collection date clarified by source",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record correction" }));
    expect(
      await within(dialog).findByText("Collection history changed. Reload before correcting."),
    ).toBeTruthy();
    expect(correct).toHaveBeenCalledWith({
      input: {
        accessionId: HISTORY.accessionId,
        expectedRevision: 1,
        time: { kind: "date", date: "2026-10-12" },
        sourceDocumentRef: "Synthetic corrected requisition",
        reason: "Collection date clarified by source",
      },
    });
  });
});
