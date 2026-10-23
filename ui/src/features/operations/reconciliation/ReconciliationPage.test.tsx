import { screen, within } from "@testing-library/react";
import { describe, expect, it, vi } from "vitest";
import type { RecoveryReconciliation } from "@/generated/RecoveryReconciliation";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { renderWithProviders } from "@/lib/test/render";
import { ANCHORS, sessionWith } from "../testData";
import { ReconciliationPage } from "./ReconciliationPage";

const OPEN: RecoveryReconciliation = {
  restoreId: "00000000-0000-7000-8000-0000000000c1",
  setCreatedAt: "2026-10-13T14:00:00Z",
  appliedAt: "2026-10-13T16:30:00Z",
  lostIntervalFrom: "2026-10-13T14:00:00Z",
  items: [
    {
      code: "production_computer",
      label: "This computer replaces production",
      guidance: "A test copy never closes the reconciliation.",
    },
    {
      code: "numbers_checked",
      label: "Number floors checked",
      guidance: "The floors cover every physical label; raise them if needed.",
    },
  ],
  anchors: ANCHORS,
  heldLabelJobs: 4,
  floors: { accession: "A26001000", container: "C00005000" },
  closed: null,
};

function renderPage(
  get: () => unknown,
  commands: Record<string, (args: unknown) => unknown> = {},
  capabilities = ["manage_quality"],
) {
  mockCommands({
    current_session: () => sessionWith(capabilities),
    get_recovery_reconciliation: get,
    get_host_health: () => null,
    list_work_queues: () => [],
    ...commands,
  });
  return renderWithProviders(<ReconciliationPage />);
}

async function openClose(user: ReturnType<typeof renderPage>["user"]) {
  await user.click(await screen.findByRole("button", { name: "Close recovery reconciliation" }));
  return screen.findByRole("alertdialog", { name: "Close recovery reconciliation" });
}

describe("ReconciliationPage (REQ-OPS-002)", () => {
  it("shows the restore facts and the checklist once, and closes it", async () => {
    const closeIt = vi.fn(() => null);
    const { user, container } = renderPage(() => OPEN, {
      close_recovery_reconciliation: closeIt,
    });
    expect(await screen.findByText("A26001000")).toBeTruthy();
    // The frame's banner states the gate; the page does not repeat it (UI review finding 28).
    expect(screen.queryByText(/wait until the laboratory closes/)).toBeNull();
    expect(screen.getByText("Anchors compared")).toBeTruthy();
    expect(screen.getByText("A test copy never closes the reconciliation.")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    const dialog = await openClose(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /replaces production/ }));
    await user.click(within(dialog).getByRole("checkbox", { name: /Number floors checked/ }));
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "All items checked");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Close reconciliation" }));
    expect(await screen.findByText("Recovery reconciliation closed")).toBeTruthy();
    expect(closeIt).toHaveBeenCalledWith({
      input: {
        restoreId: OPEN.restoreId,
        attestedItems: ["production_computer", "numbers_checked"],
        note: "All items checked",
        password: "synthetic-secret",
      },
    });
  });

  it("shows a wrong password on its field and keeps the dialog open", async () => {
    const wrong = engineError("validation", "The password is incorrect.", {
      fieldErrors: [
        { field: "password", code: "incorrect", message: "The password is incorrect." },
      ],
    });
    const { user } = renderPage(() => OPEN, {
      close_recovery_reconciliation: () => Promise.reject(wrong),
    });
    const dialog = await openClose(user);
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Checked");
    await user.type(within(dialog).getByLabelText(/^Your password/), "wrong-password");
    await user.click(within(dialog).getByRole("button", { name: "Close reconciliation" }));
    expect(await within(dialog).findByText("Not closed")).toBeTruthy();
    expect(within(dialog).getByText("corr-test-0001")).toBeTruthy();
    const password = within(dialog).getByLabelText(/^Your password/);
    expect(password.getAttribute("aria-invalid")).toBe("true");
    expect(screen.getByRole("alertdialog", { name: "Close recovery reconciliation" })).toBeTruthy();
  });

  it("shows the engine's refusal of an incomplete checklist on the checklist", async () => {
    const incomplete = engineError("validation", "Check the highlighted fields.", {
      fieldErrors: [
        {
          field: "attestedItems",
          code: "incomplete",
          message: "Attest every checklist item before closing.",
        },
      ],
    });
    const { user, container } = renderPage(() => OPEN, {
      close_recovery_reconciliation: () => Promise.reject(incomplete),
    });
    const dialog = await openClose(user);
    await user.click(within(dialog).getByRole("checkbox", { name: /replaces production/ }));
    await user.type(within(dialog).getByRole("textbox", { name: /^Note/ }), "Partly checked");
    await user.type(within(dialog).getByLabelText(/^Your password/), "synthetic-secret");
    await user.click(within(dialog).getByRole("button", { name: "Close reconciliation" }));
    expect(
      await within(dialog).findByText("Attest every checklist item before closing."),
    ).toBeTruthy();
    expect(
      within(dialog)
        .getByLabelText(/^Your password/)
        .getAttribute("aria-invalid"),
    ).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("raises a number floor and leaves the empty one out", async () => {
    const raise = vi.fn(() => ({
      accession: { highestIssued: "A26000990", floor: "A26001000", next: "A26001000" },
      container: { highestIssued: "C00004990", floor: "C00009000", next: "C00009000" },
    }));
    const { user } = renderPage(() => OPEN, { raise_number_floors: raise });
    await user.click(await screen.findByRole("button", { name: "Raise number floors" }));
    const dialog = await screen.findByRole("dialog", { name: "Raise number floors" });
    await user.type(
      within(dialog).getByRole("textbox", { name: /^New container code floor/ }),
      "C00009000",
    );
    await user.type(within(dialog).getByRole("textbox", { name: /^Reason/ }), "Labels printed");
    await user.click(within(dialog).getByRole("button", { name: "Raise floors" }));
    expect(await screen.findByText("Number floors raised")).toBeTruthy();
    expect(raise).toHaveBeenCalledWith({
      input: { containerFloor: "C00009000", reason: "Labels printed" },
    });
  });

  it("shows a closed reconciliation without actions for others", async () => {
    const other = "00000000-0000-7000-8000-0000000000e2";
    const closed = { ...OPEN, closed: { by: other, at: "2026-10-13T18:00:00Z" } };
    renderPage(() => closed, {}, []);
    expect(await screen.findByText("Recovery reconciliation closed")).toBeTruthy();
    expect(screen.getByText(/by another user/)).toBeTruthy();
    expect(document.body.textContent).not.toContain(other);
    expect(screen.queryByRole("button", { name: "Raise number floors" })).toBeNull();
  });

  it("names the signed-in person who closed it", async () => {
    const closed = { ...OPEN, closed: { by: sessionWith([]).userId, at: "2026-10-13T18:00:00Z" } };
    renderPage(() => closed, {}, []);
    expect(await screen.findByText(/by Test Operator\./)).toBeTruthy();
  });

  it("explains that there is no reconciliation", async () => {
    const { container } = renderPage(() => null);
    expect(await screen.findByText("No recovery reconciliation")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("shows the engine's error with its support reference", async () => {
    renderPage(() => Promise.reject(engineError("internal", "The restore record is unreadable.")));
    expect(await screen.findByText("The restore record is unreadable.")).toBeTruthy();
    expect(screen.getByText("corr-test-0001")).toBeTruthy();
  });
});
