import { screen } from "@testing-library/react";
import { describe, expect, it } from "vitest";
import type { MaterialHistoryPage } from "@/generated/MaterialHistoryPage";
import { mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { sessionWith } from "../testData";
import { MaterialHistory } from "./MaterialHistory";

const HOLDER = "00000000-0000-7000-8000-000000000e11";

const PAGE: MaterialHistoryPage = {
  snapshot: { maxMoveRowId: "1", maxHoldRowId: "1" },
  moves: [],
  holds: [
    {
      id: "1",
      kind: "hold",
      actor: HOLDER,
      actorDisplay: "Test Quality Holder",
      recordedAt: "2026-10-12T14:03:00Z",
      description: "Quality hold placed",
      reason: "Synthetic leaking container",
      causalSequence: 1,
    },
  ],
  disposal: null,
  depletion: null,
  hasMore: false,
};

describe("MaterialHistory", () => {
  it("names who recorded each event instead of the account ID", async () => {
    mockCommands({
      current_session: () => sessionWith(["accession"]),
      get_container_history: () => PAGE,
    });
    renderWithProviders(<MaterialHistory containerId="00000000-0000-7000-8000-000000000c01" />);
    expect(await screen.findByText(/Recorded by Test Quality Holder/)).toBeTruthy();
    expect(screen.queryByText(new RegExp(HOLDER))).toBeNull();
  });
});
