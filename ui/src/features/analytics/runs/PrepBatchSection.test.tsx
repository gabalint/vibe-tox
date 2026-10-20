import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { PrepBatchView } from "@/generated/PrepBatchView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { METHOD_VERSION_ID, REAGENT_LOT, sessionWith } from "../testData";
import { WorklistPage } from "./WorklistPage";

const BATCH_ID = "00000000-0000-7000-8000-000000000e01";

const BATCH: PrepBatchView = {
  prepBatchId: BATCH_ID,
  methodVersionId: METHOD_VERSION_ID,
  state: "open",
  createdBy: "analyst-1",
  createdAt: "2026-10-12T09:00:00Z",
  completedBy: null,
  completedAt: null,
  revision: 1,
  steps: [],
};

const STEPPED: PrepBatchView = {
  ...BATCH,
  revision: 2,
  steps: [
    {
      stepId: "step-1",
      seq: 1,
      stepCode: "EXTRACT",
      lotId: REAGENT_LOT.lotId,
      note: "Synthetic note",
      performedBy: "analyst-1",
      performedAt: "2026-10-12T09:10:00Z",
    },
  ],
};

function renderPage(commands: Record<string, (args: unknown) => unknown>, batchId?: string) {
  let saved: PrepBatchView | null = null;
  const record = (command: (args: unknown) => unknown) => async (args: unknown) => {
    const result = await command(args);
    saved = result as PrepBatchView;
    return result;
  };
  const mutations = Object.fromEntries(
    Object.entries(commands).map(([name, command]) => [
      name,
      ["create_prep_batch", "record_prep_step", "complete_prep_batch"].includes(name)
        ? record(command)
        : command,
    ]),
  );
  mockCommands({
    current_session: () => sessionWith(["perform_analysis"]),
    list_methods: () => [],
    list_method_versions: () => [],
    list_worklist: () => [],
    list_lots: () => [REAGENT_LOT],
    list_prep_batches: () => (saved ? [{ ...saved, stepCount: saved.steps.length }] : []),
    get_prep_batch: () => saved ?? BATCH,
    ...mutations,
  });
  return renderRoutes(
    [{ path: "/bench/worklist", element: <WorklistPage /> }],
    `/bench/worklist?method=method-1&version=${METHOD_VERSION_ID}${batchId ? `&batch=${batchId}` : ""}`,
  );
}

async function section() {
  const heading = await screen.findByRole("heading", { name: "Preparation batch" });
  const found = heading.closest("section");
  if (!found) throw new Error("no section");
  return found;
}

describe("PrepBatchSection (REQ-ANA-001, REQ-ANA-004)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("starts a batch, records a step and completes it with the revision shown", async () => {
    const create = vi.fn(() => BATCH);
    const step = vi.fn(() => STEPPED);
    const complete = vi.fn(() => ({
      ...STEPPED,
      state: "completed",
      completedBy: "analyst-1",
      completedAt: "2026-10-12T09:20:00Z",
      revision: 3,
    }));
    const { user, container } = renderPage({
      create_prep_batch: create,
      record_prep_step: step,
      complete_prep_batch: complete,
    });
    const batch = await section();
    expect(batch.textContent).toContain("QC can accept a batch only once it is completed.");
    await user.click(within(batch).getByRole("button", { name: "Start preparation batch" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Start batch" }),
    );
    expect(await screen.findByText("Preparation batch started")).toBeTruthy();
    expect(create).toHaveBeenCalledWith({ input: { methodVersionId: METHOD_VERSION_ID } });
    expect(within(batch).getAllByText(BATCH_ID).length).toBeGreaterThan(0);

    await user.click(await within(batch).findByRole("button", { name: "Record step" }));
    const dialog = await screen.findByRole("dialog", { name: "Record preparation step" });
    await user.type(within(dialog).getByRole("textbox", { name: /Step code/ }), "EXTRACT");
    await chooseWithKeyboard(
      user,
      /Lot used$/,
      `${REAGENT_LOT.productName}, lot ${REAGENT_LOT.lotNumber}`,
    );
    await user.type(within(dialog).getByRole("textbox", { name: /Note/ }), "Synthetic note");
    await user.click(within(dialog).getByRole("button", { name: "Record step" }));
    expect(await screen.findByText("Preparation step recorded")).toBeTruthy();
    expect(step).toHaveBeenCalledWith({
      input: {
        prepBatchId: BATCH_ID,
        stepCode: "EXTRACT",
        lotId: REAGENT_LOT.lotId,
        note: "Synthetic note",
      },
    });
    const steps = await within(batch).findByRole("grid", { name: "Preparation steps" });
    expect(
      within(steps).getByRole("gridcell", {
        name: `${REAGENT_LOT.productName}, lot ${REAGENT_LOT.lotNumber}`,
      }),
    ).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });

    await user.click(within(batch).getByRole("button", { name: "Complete batch" }));
    const confirm = await screen.findByRole("alertdialog", {
      name: "Complete the preparation batch?",
    });
    expect(confirm.textContent).toContain("with the 1 step shown");
    await user.click(within(confirm).getByRole("button", { name: "Complete batch" }));
    expect(await screen.findByText("Preparation batch completed")).toBeTruthy();
    expect(complete).toHaveBeenCalledWith({
      input: { prepBatchId: BATCH_ID, expectedRevision: 2 },
    });
    // A completed batch takes no more steps.
    expect(within(batch).getAllByText("Completed").length).toBeGreaterThanOrEqual(2);
    expect(within(batch).queryByRole("button", { name: "Record step" })).toBeNull();
    expect(within(batch).queryByRole("button", { name: "Complete batch" })).toBeNull();
  });

  it("shows why the engine refuses to complete a batch without steps", async () => {
    const { user } = renderPage({
      create_prep_batch: () => BATCH,
      complete_prep_batch: () =>
        Promise.reject(
          engineError("gate_blocked", "This action is blocked.", {
            gateReasons: [
              {
                code: "prep_no_steps",
                message: "Record the preparation steps before completing the batch.",
              },
            ],
          }),
        ),
    });
    const batch = await section();
    await user.click(within(batch).getByRole("button", { name: "Start preparation batch" }));
    await user.click(
      within(await screen.findByRole("alertdialog")).getByRole("button", { name: "Start batch" }),
    );
    await user.click(await within(batch).findByRole("button", { name: "Complete batch" }));
    const confirm = await screen.findByRole("alertdialog");
    await user.click(within(confirm).getByRole("button", { name: "Complete batch" }));
    const gates = await within(confirm).findByRole("list", { name: "Blocked by" });
    expect(gates.textContent).toContain(
      "No preparation steps recorded: Record the preparation steps before completing the batch.",
    );
    expect(within(confirm).getByText("corr-test-0001")).toBeTruthy();
  });
  it("reopens an interrupted batch from the directory after returning to the page", async () => {
    const get = vi.fn(() => STEPPED);
    const { user } = renderPage({
      list_prep_batches: () => [{ ...STEPPED, stepCount: 1 }],
      get_prep_batch: get,
    });
    await user.click(await screen.findByText(BATCH_ID));
    expect(await screen.findByRole("grid", { name: "Preparation steps" })).toBeTruthy();
    expect(screen.getByText("EXTRACT")).toBeTruthy();
    expect(screen.getByRole("button", { name: "Record step" })).toBeTruthy();
    expect(get).toHaveBeenCalledWith({ input: { prepBatchId: BATCH_ID } });
  });
  it("does not expose another method version's preparation actions through a mismatched URL", async () => {
    renderPage(
      { get_prep_batch: () => ({ ...STEPPED, methodVersionId: "another-version" }) },
      BATCH_ID,
    );
    expect(await screen.findByText("Batch belongs to another method version")).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Record step" })).toBeNull();
    expect(screen.queryByRole("button", { name: "Complete batch" })).toBeNull();
    expect(screen.queryByRole("grid", { name: "Preparation steps" })).toBeNull();
  });
});
