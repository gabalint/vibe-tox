import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter, selectRow } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import {
  DERIVATION,
  DERIVATION_ID,
  HANDOFF,
  HANDOFF_ID,
  SUBJECT,
  sessionWith,
  workEntry,
} from "../testData";
import { BillingWorkPage } from "./BillingWorkPage";

const UNKNOWN_DATE = workEntry({
  subjectId: "00000000-0000-7000-8000-0000000000c9",
  dateOfService: null,
  accessionNumbers: ["A26000009", "A26000010"],
});
const SECOND_ID = "00000000-0000-7000-8000-0000000000d3";
const ITEM_ID = "00000000-0000-7000-8000-0000000000f2";
const HANDS_OFF = ["create_billing_handoff"];

function renderPage(path: string, commands: Record<string, (args: unknown) => unknown>) {
  mockCommands({ current_session: () => sessionWith(["billing"]), ...commands });
  return renderRoutes([{ path: "/billing/work", element: <BillingWorkPage /> }], path);
}

const approved = (subjectId: string, accession: string, derivationId: string) =>
  workEntry({
    subjectId,
    accessionNumbers: [accession],
    derivationId,
    state: "approved",
    allowedActions: HANDS_OFF,
  });

describe("BillingWorkPage (REQ-BIL-002, REQ-BIL-005)", () => {
  beforeEach(mockGridLayout);

  it("lists work needing derivation and derives services from a row's menu", async () => {
    const list = vi.fn(() => [workEntry(), UNKNOWN_DATE]);
    const derive = vi.fn(() => DERIVATION);
    const { user, container } = renderPage("/billing/work", {
      list_billing_work: list,
      derive_services: derive,
    });
    expect(await screen.findByRole("gridcell", { name: "Unknown" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "A26000009, A26000010" })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: { filter: "needs_derivation", limit: 200, target: null, after: null },
    });
    // Every row needs derivation, so the state is plain text; nothing is selectable.
    expect(screen.getAllByRole("gridcell", { name: "Needs derivation" })).toHaveLength(2);
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.queryByText(/Select /)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    const menu = await openRowActions(user, "A26000001");
    await user.click(within(menu).getByRole("menuitem", { name: "Derive services" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Derive billable services?" });
    await user.click(within(dialog).getByRole("button", { name: "Derive services" }));
    expect(
      await screen.findByText(`Navigated to /billing/derivations/${DERIVATION_ID}`),
    ).toBeTruthy();
    expect(derive).toHaveBeenCalledWith({
      input: { subjectId: SUBJECT.subjectId, dateOfService: "2026-10-10" },
    });
  });

  it("shows the engine's refusal to derive", async () => {
    const { user } = renderPage("/billing/work", {
      list_billing_work: () => [UNKNOWN_DATE],
      derive_services: () =>
        Promise.reject(engineError("forbidden", "Deriving services needs the billing capability.")),
    });
    const menu = await openRowActions(user, "A26000009, A26000010");
    await user.click(within(menu).getByRole("menuitem", { name: "Derive services" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Derive services" }));
    expect(
      await within(dialog).findByText("Deriving services needs the billing capability."),
    ).toBeTruthy();
  });

  it("keeps the filter in the address", async () => {
    const list = vi.fn(() => [workEntry()]);
    const { user, router } = renderPage("/billing/work", { list_billing_work: list });
    await screen.findByRole("gridcell", { name: "A26000001" });
    await chooseWithKeyboard(user, /Show$/, "Held");
    expect(router.state.location.search).toBe("?filter=held");
    expect(list).toHaveBeenLastCalledWith({
      input: { filter: "held", limit: 200, target: null, after: null },
    });
  });

  it("hands off two approved derivations in one file through the selection bar", async () => {
    const create = vi.fn(() => HANDOFF);
    const { user } = renderPage("/billing/work?filter=ready", {
      list_billing_work: () => [
        approved(SUBJECT.subjectId, "A26000001", DERIVATION_ID),
        approved("00000000-0000-7000-8000-0000000000c3", "A26000002", SECOND_ID),
        workEntry({
          subjectId: "00000000-0000-7000-8000-0000000000c4",
          accessionNumbers: ["A26000003"],
          derivationId: "00000000-0000-7000-8000-0000000000d4",
          state: "candidate",
          allowedActions: ["approve_service_derivation"],
        }),
      ],
      create_billing_handoff: create,
    });
    // Mixed states keep their badges.
    expect(await screen.findByText("Ready for approval")).toBeTruthy();
    await selectRow(user, "A26000003");
    expect(screen.getByText("None of these is approved for handoff.")).toBeTruthy();
    await selectRow(user, "A26000001");
    await selectRow(user, "A26000002");
    expect(screen.getByText("3 selected.")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Create handoff" }));
    const dialog = await screen.findByRole("alertdialog", { name: "Create a handoff file?" });
    expect(dialog.textContent).toContain("2 approved derivations go into one handoff file");
    await user.click(within(dialog).getByRole("button", { name: "Create handoff" }));
    expect(await screen.findByText(`Navigated to /billing/handoffs/${HANDOFF_ID}`)).toBeTruthy();
    expect(create).toHaveBeenCalledWith({ input: { derivationIds: [DERIVATION_ID, SECOND_ID] } });
  });

  it("shows the engine's refusal to hand off", async () => {
    const { user } = renderPage("/billing/work?filter=ready", {
      list_billing_work: () => [approved(SUBJECT.subjectId, "A26000001", DERIVATION_ID)],
      create_billing_handoff: () =>
        Promise.reject(engineError("conflict", "A derivation is no longer current.")),
    });
    await selectRow(user, "A26000001");
    await user.click(screen.getByRole("button", { name: "Create handoff" }));
    const dialog = await screen.findByRole("alertdialog");
    await user.click(within(dialog).getByRole("button", { name: "Create handoff" }));
    expect(await within(dialog).findByText("A derivation is no longer current.")).toBeTruthy();
  });

  it("opens a rejected item's handoff with Enter, with billing work in its breadcrumb", async () => {
    const { user, router } = renderPage("/billing/work?filter=rejected", {
      list_billing_work: () => [
        workEntry({ derivationId: DERIVATION_ID, handoffId: HANDOFF_ID, state: "rejected" }),
      ],
    });
    expect(await screen.findByRole("gridcell", { name: "Rejected" })).toBeTruthy();
    expect(screen.queryByRole("checkbox")).toBeNull();
    await openRowWithEnter(user, "Rejected");
    expect(await screen.findByText(`Navigated to /billing/handoffs/${HANDOFF_ID}`)).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Billing work", href: "/billing/work?filter=rejected" },
        ids: [HANDOFF_ID],
      },
    });
  });

  it("opens Derive services from a row still to derive, by click or Enter", async () => {
    const derive = vi.fn(() => DERIVATION);
    const { user } = renderPage("/billing/work", {
      list_billing_work: () => [workEntry(), UNKNOWN_DATE],
      derive_services: derive,
    });
    await openRow(user, "A26000009, A26000010");
    const dialog = await screen.findByRole("alertdialog", { name: "Derive billable services?" });
    await user.click(within(dialog).getByRole("button", { name: "Cancel" }));
    await openRowWithEnter(user, "A26000001");
    const again = await screen.findByRole("alertdialog", { name: "Derive billable services?" });
    await user.click(within(again).getByRole("button", { name: "Derive services" }));
    expect(
      await screen.findByText(`Navigated to /billing/derivations/${DERIVATION_ID}`),
    ).toBeTruthy();
    expect(derive).toHaveBeenCalledWith({
      input: { subjectId: SUBJECT.subjectId, dateOfService: "2026-10-10" },
    });
  });

  it("opens a derivation with a click; work the engine allows nothing on stays", async () => {
    const { user, router } = renderPage("/billing/work?filter=held", {
      list_billing_work: () => [
        workEntry({ derivationId: DERIVATION_ID, state: "held" }),
        { ...UNKNOWN_DATE, allowedActions: [] },
      ],
    });
    await openRow(user, "A26000009, A26000010");
    expect(screen.queryByRole("alertdialog")).toBeNull();
    expect(screen.getByRole("heading", { level: 1, name: "Billing work" })).toBeTruthy();
    await openRow(user, "A26000001");
    expect(
      await screen.findByText(`Navigated to /billing/derivations/${DERIVATION_ID}`),
    ).toBeTruthy();
    expect(router.state.location.state).toEqual({
      recordSequence: {
        list: { label: "Billing work", href: "/billing/work?filter=held" },
        ids: [DERIVATION_ID],
      },
    });
  });

  it("marks the entry a work-queue item names, and records its outcome from the row menu", async () => {
    const record = vi.fn(() => HANDOFF);
    const outstanding = workEntry({
      derivationId: DERIVATION_ID,
      handoffId: HANDOFF_ID,
      handoffNo: 7,
      itemId: ITEM_ID,
      state: "outstanding",
      allowedActions: ["record_handoff_outcome"],
    });
    const { user } = renderPage(`/billing/work?filter=outstanding&open=${ITEM_ID}`, {
      list_billing_work: () => [outstanding],
      record_handoff_outcome: record,
    });
    expect(await screen.findByRole("row", { current: true })).toBeTruthy();
    const menu = await openRowActions(user, "A26000001");
    await user.click(within(menu).getByRole("menuitem", { name: "Record outcome" }));
    const dialog = await screen.findByRole("dialog", { name: "Record outcome" });
    await chooseWithKeyboard(user, /Outcome/, "Acknowledged by the billing partner");
    await user.click(within(dialog).getByRole("button", { name: "Record outcome" }));
    expect(await screen.findByText("Outcome recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({ input: { itemId: ITEM_ID, outcome: "acknowledged" } });
  });

  it("says when the list stops at the limit, and refreshes on request", async () => {
    // The engine answers at most the limit and marks every row when more exist.
    const answer = Array.from({ length: 200 }, (_, index) =>
      workEntry({
        subjectId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`,
        accessionNumbers: [`A2600${String(index).padStart(4, "0")}`],
        hasMore: true,
      }),
    );
    const list = vi.fn(() => answer);
    const { user } = renderPage("/billing/work", { list_billing_work: list });
    expect(await screen.findByText(/^Showing the oldest results; sorting/)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("shows loading, and says when nothing matches", async () => {
    let answer: (rows: unknown) => void = () => undefined;
    renderPage("/billing/work?filter=outstanding", {
      list_billing_work: () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    });
    expect(await screen.findByText("Loading billing work…")).toBeTruthy();
    answer([]);
    expect(await screen.findByText("Nothing in “Handed off, awaiting outcome”")).toBeTruthy();
    expect(screen.queryByRole("grid")).toBeNull();
  });

  it("shows the engine's error", async () => {
    renderPage("/billing/work", {
      list_billing_work: () =>
        Promise.reject(engineError("forbidden", "Billing work needs the billing capability.")),
    });
    expect(await screen.findByText("Billing work needs the billing capability.")).toBeTruthy();
  });
});
