import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard, WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import { expectNextKeepsFocus, renderFromList } from "@/lib/test/sequence";
import { handoffPath } from "../paths";
import { HANDOFF, HANDOFF_ID, sessionWith } from "../testData";
import { HandoffPage } from "./HandoffPage";

const OUTSTANDING_ITEM = "00000000-0000-7000-8000-0000000000f2";
const REJECTED_ITEM = "00000000-0000-7000-8000-0000000000f3";
const SECOND_DERIVATION = "00000000-0000-7000-8000-0000000000d2";

const ROUTE = { path: "/billing/handoffs/:handoffId", element: <HandoffPage /> };

function mockPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["billing"]),
    get_billing_handoff: () => HANDOFF,
    ...commands,
  });
}

function renderPage(commands: Record<string, (args: unknown) => unknown> = {}) {
  mockPage(commands);
  return renderRoutes([ROUTE], `/billing/handoffs/${HANDOFF_ID}`);
}

describe("HandoffPage (REQ-BIL-002)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the file, its items and their outcomes, and opens an item in its panel", async () => {
    const { user, container, router } = renderPage();
    expect(await screen.findByText(HANDOFF.artifactSha256)).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Billing handoff 7" })).toBeTruthy();
    const crumbs = screen.getByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByRole("link", { name: "Billing work" })).toBeTruthy();
    expect(screen.getByText("bil-handoff-v1")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Awaiting outcome" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Rejected" })).toBeTruthy();
    expect(screen.getByText("Synthetic partner rejection,", { exact: false })).toBeTruthy();
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.queryByText(/Select /)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRow(user, REJECTED_ITEM);
    const panel = await screen.findByRole("region", { name: "Derivation 3" });
    expect(router.state.location.search).toBe(`?open=${REJECTED_ITEM}`);
    expect(
      within(panel).getByText(
        "Rejected by the billing partner: Synthetic partner rejection. Recorded",
        { exact: false },
      ),
    ).toBeTruthy();
    // The panel links to the item's derivation.
    await user.click(within(panel).getByRole("link", { name: "Derivation 3" }));
    expect(
      await screen.findByText(`Navigated to /billing/derivations/${SECOND_DERIVATION}`),
    ).toBeTruthy();
  });

  it("opens an item with Enter, steps to the next one, and closes with Escape", async () => {
    const { user } = renderPage();
    await openRowWithEnter(user, "Awaiting outcome");
    const panel = await screen.findByRole("region", { name: "Derivation 2" });
    expect(within(panel).getByText("None recorded yet")).toBeTruthy();
    await user.click(within(panel).getByRole("button", { name: "Next" }));
    expect(await screen.findByRole("region", { name: "Derivation 3" })).toBeTruthy();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Derivation 3" })).toBeNull();
  });

  it("records an outcome from the item's panel", async () => {
    const record = vi.fn(() => ({}));
    const { user } = renderPage({ record_handoff_outcome: record });
    await openRow(user, OUTSTANDING_ITEM);
    const panel = await screen.findByRole("region", { name: "Derivation 2" });
    await user.click(within(panel).getByRole("button", { name: "Record outcome" }));
    const dialog = await screen.findByRole("dialog", { name: "Record outcome" });
    await chooseWithKeyboard(user, /Outcome$/, "Acknowledged by the billing partner");
    await user.click(within(dialog).getByRole("button", { name: "Record outcome" }));
    expect(await screen.findByText("Outcome recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: { itemId: OUTSTANDING_ITEM, outcome: "acknowledged" },
    });
  });

  it("keeps focus on Next while the next handoff loads", async () => {
    const second = "00000000-0000-7000-8000-0000000000f9";
    mockPage({
      get_billing_handoff: (args) =>
        JSON.stringify(args).includes(second)
          ? { ...HANDOFF, handoffId: second, handoffNo: 8 }
          : HANDOFF,
    });
    const { user } = await renderFromList({
      route: ROUTE,
      recordPath: handoffPath,
      ids: [HANDOFF_ID, second, "00000000-0000-7000-8000-0000000000fa"],
    });
    await screen.findByRole("heading", { level: 1, name: "Billing handoff 7" });
    await expectNextKeepsFocus(user, "Billing handoff 8");
  });

  it("exports the file to the Downloads folder", async () => {
    const exportFile = vi.fn(() => ({ fileName: "handoff-0001.json", folder: "downloads" }));
    const { user } = renderPage({ export_billing_handoff: exportFile });
    await user.click(await screen.findByRole("button", { name: "Export file" }));
    expect(
      await screen.findByText("Saved handoff-0001.json to your Downloads folder."),
    ).toBeTruthy();
    expect(exportFile).toHaveBeenCalledWith({ input: { handoffId: HANDOFF_ID } });
  });

  it("shows the engine's refusal to export", async () => {
    const { user } = renderPage({
      export_billing_handoff: () =>
        Promise.reject(engineError("conflict", "The stored file failed verification.")),
    });
    await user.click(await screen.findByRole("button", { name: "Export file" }));
    expect(await screen.findByText("The stored file failed verification.")).toBeTruthy();
  });

  it("records an acknowledgement with the partner's reference", async () => {
    const record = vi.fn(() => ({}));
    const { user } = renderPage({ record_handoff_outcome: record });
    // An item with its outcome recorded has no action.
    await screen.findByRole("gridcell", { name: REJECTED_ITEM });
    expect(screen.queryByRole("button", { name: "Actions for Derivation 3" })).toBeNull();
    const menu = await openRowActions(user, "Derivation 2");
    await user.click(within(menu).getByRole("menuitem", { name: "Record outcome" }));
    const dialog = await screen.findByRole("dialog", { name: "Record outcome" });
    await chooseWithKeyboard(user, /Outcome$/, "Acknowledged by the billing partner");
    await user.type(
      within(dialog).getByRole("textbox", { name: /Partner reference/ }),
      "SYN-REF-1",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record outcome" }));
    expect(await screen.findByText("Outcome recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: { itemId: OUTSTANDING_ITEM, outcome: "acknowledged", externalReference: "SYN-REF-1" },
    });
  });

  it("requires a reason for a rejection and shows the engine's refusal", async () => {
    const record = vi.fn(() =>
      Promise.reject(engineError("conflict", "An outcome is already recorded for this item.")),
    );
    const { user } = renderPage({ record_handoff_outcome: record });
    const menu = await openRowActions(user, "Derivation 2");
    await user.click(within(menu).getByRole("menuitem", { name: "Record outcome" }));
    const dialog = await screen.findByRole("dialog", { name: "Record outcome" });
    await chooseWithKeyboard(user, /Outcome$/, "Rejected by the billing partner");
    await user.click(within(dialog).getByRole("button", { name: "Record outcome" }));
    expect(await within(dialog).findByText("Enter reason.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.type(within(dialog).getByRole("textbox", { name: /Reason/ }), "Missing NPI");
    await user.click(within(dialog).getByRole("button", { name: "Record outcome" }));
    expect(
      await within(dialog).findByText("An outcome is already recorded for this item."),
    ).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: { itemId: OUTSTANDING_ITEM, outcome: "rejected", reason: "Missing NPI" },
    });
  });

  it("shows the engine's error", async () => {
    renderPage({
      get_billing_handoff: () => Promise.reject(engineError("not_found", "No such handoff.")),
    });
    expect(await screen.findByText("No such handoff.")).toBeTruthy();
  });

  it("records a rejected item's attributed follow-up with its outcome revision", async () => {
    const record = vi.fn(() => ({}));
    const { user } = renderPage({
      get_billing_handoff: () => ({
        ...HANDOFF,
        items: HANDOFF.items.map((item) =>
          item.itemId === REJECTED_ITEM
            ? { ...item, outcomeRevision: 2, allowedActions: ["record_handoff_followup"] }
            : item,
        ),
      }),
      record_handoff_followup: record,
    });
    const menu = await openRowActions(user, "Derivation 3");
    await user.click(within(menu).getByRole("menuitem", { name: "Record follow-up outcome" }));
    const dialog = await screen.findByRole("dialog", { name: "Record follow-up outcome" });
    await chooseWithKeyboard(user, /Outcome$/, "Acknowledged by the billing partner");
    await user.click(within(dialog).getByRole("button", { name: "Record follow-up outcome" }));
    expect(await within(dialog).findByText("Enter reason for follow-up.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await user.type(
      within(dialog).getByRole("textbox", { name: /Reason for follow-up/ }),
      "Partner accepted corrected evidence",
    );
    await user.type(
      within(dialog).getByRole("textbox", { name: /Partner reference/ }),
      "SYN-RETRY-2",
    );
    await user.click(within(dialog).getByRole("button", { name: "Record follow-up outcome" }));
    expect(await screen.findByText("Outcome recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: {
        itemId: REJECTED_ITEM,
        expectedRevision: 2,
        outcome: "acknowledged",
        externalReference: "SYN-RETRY-2",
        reason: "Partner accepted corrected evidence",
      },
    });
  });
});
