import { screen } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { renderRoutes } from "@/lib/test/router";
import { ACCESSION } from "../testData";
import { ReceiptChronologyDialog } from "./ReceiptChronologyDialog";

describe("Receipt chronology source review", () => {
  it("does not assume an order or submit missing source evidence", async () => {
    const specimen = ACCESSION.specimens[0];
    if (!specimen) throw new Error("Synthetic specimen missing");
    const resolve = vi.fn();
    mockCommands({ resolve_receipt_chronology: resolve });
    const onClose = vi.fn();
    const { user } = renderRoutes(
      [
        {
          path: "/",
          element: (
            <ReceiptChronologyDialog
              specimen={{
                ...specimen,
                decisions: [
                  {
                    decisionId: "synthetic-held",
                    decision: "held",
                    decidedBy: "synthetic-user",
                    decidedAt: "2026-10-14T12:00:00Z",
                    causalSequence: null,
                    discrepancies: [],
                    policyReference: null,
                    scope: null,
                    reason: "Source requires review",
                  },
                ],
              }}
              onClose={onClose}
            />
          ),
        },
      ],
      "/",
    );
    expect(screen.getByText(/No order is assumed/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Record reviewed chronology" }));
    expect(resolve).not.toHaveBeenCalled();
    await user.click(screen.getByRole("button", { name: "Cancel" }));
    expect(onClose).toHaveBeenCalledOnce();
  });
});
