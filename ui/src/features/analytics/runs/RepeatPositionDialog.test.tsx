import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { INSTRUMENT, RUN, sessionWith } from "../testData";
import { RepeatPositionDialog } from "./RepeatPositionDialog";

const target = {
  runId: "target-run",
  instrumentId: INSTRUMENT.instrumentId,
  methodVersionId: RUN.methodVersionId,
  state: "open",
  createdAt: RUN.createdAt,
  positionCount: 0,
};
const prep = {
  prepBatchId: "batch-complete",
  methodVersionId: RUN.methodVersionId,
  state: "completed",
  createdAt: RUN.createdAt,
  createdBy: "Synthetic analyst",
  completedAt: RUN.createdAt,
  revision: 3,
  stepCount: 1,
};

function renderRepeat() {
  const add = vi.fn(() => RUN);
  mockCommands({
    current_session: () => sessionWith(["perform_analysis"]),
    list_runs: () => [target],
    list_instrument_readiness: () => [INSTRUMENT],
    list_prep_batches: () => [prep],
    add_patient_position: add,
  });
  const position = RUN.positions[0];
  if (!position?.execution) throw new Error("patient fixture required");
  const rendered = renderRoutes(
    [
      {
        path: "/",
        element: <RepeatPositionDialog run={RUN} position={position} onClose={() => undefined} />,
      },
    ],
    "/",
  );
  return { ...rendered, add, position };
}

describe("Repeat testing (UI-02)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);
  it("records dilution with exact factor, original parent and completed preparation", async () => {
    const { user, add, position } = renderRepeat();
    const dialog = await screen.findByRole("dialog", { name: "Repeat test" });
    await screen.findByRole("button", { name: /Target run$/ });
    await chooseWithKeyboard(
      user,
      /Target run$/,
      "2026-10-12 10:00 UTC · Synthetic analyzer 1 · 0 positions",
    );
    await user.type(within(dialog).getByRole("textbox", { name: /^Position/ }), "P03");
    await chooseWithKeyboard(user, /Attempt kind$/, "Dilution");
    await user.type(within(dialog).getByRole("textbox", { name: /^Dilution factor/ }), "10.00");
    await chooseWithKeyboard(
      user,
      /Completed preparation batch$/,
      "2026-10-12 10:00 UTC · 1 steps · batch-complete",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /^Reason/ }),
      "Synthetic repeat evidence",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record repeat" }));
    expect(await screen.findByText("Repeat recorded")).toBeTruthy();
    expect(add).toHaveBeenCalledWith({
      input: {
        runId: "target-run",
        positionLabel: "P03",
        containerLabel: position.sampleLabel,
        attemptKind: "dilution",
        relatedExecutionId: position.execution?.executionId,
        dilutionFactor: "10.00",
        prepBatchId: "batch-complete",
        reason: "Synthetic repeat evidence",
      },
    });
  });
  it("requires a reason and explicit repeat kind without submitting partial work", async () => {
    const { user, add } = renderRepeat();
    const dialog = await screen.findByRole("dialog", { name: "Repeat test" });
    await user.click(within(dialog).getByRole("button", { name: "Record repeat" }));
    expect(await within(dialog).findByText("Choose attempt kind.")).toBeTruthy();
    expect(within(dialog).getByText("Enter reason.")).toBeTruthy();
    expect(add).not.toHaveBeenCalled();
  });
});
