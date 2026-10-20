import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowActions, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import type { ObligationView } from "../contract";
import { DESTINATION, OBLIGATION, REPORT_ID, REVISION_ID, sessionWith } from "../testData";
import { DeliveryQueuePage } from "./DeliveryQueuePage";

const STATUS = "Fax by staff: awaiting delivery";
const PAUSED = "Delivery is paused until the impact on this revision is assessed.";
const NAME = "A26000001-URN to Synthetic Clinic fax";

function renderPage(path: string, commands: Record<string, (args: unknown) => unknown> = {}) {
  mockCommands({
    current_session: () => sessionWith(["deliver_reports"]),
    list_delivery_obligations: () => ({ items: [OBLIGATION], nextAfter: null }),
    list_delivery_destinations: () => [],
    get_delivery_obligation: () =>
      Promise.reject(engineError("not_found", "The linked delivery obligation was not found.")),
    ...commands,
  });
  return renderRoutes([{ path: "/delivery", element: <DeliveryQueuePage /> }], path);
}

const panel = () => screen.getByRole("region", { name: NAME });

describe("DeliveryQueuePage (REQ-RPT-003, REQ-RPT-011)", () => {
  beforeEach(mockGridLayout);

  it("lists obligations by the engine's channel and evidence label and opens one in the panel", async () => {
    const list = vi.fn(() => ({ items: [OBLIGATION], nextAfter: null }));
    const { user, container, router } = renderPage("/delivery", {
      list_delivery_obligations: list,
    });
    expect(await screen.findByRole("gridcell", { name: STATUS })).toBeTruthy();
    expect(list).toHaveBeenCalledWith({ input: { filter: "awaiting", limit: 200 } });
    expect(screen.queryByText(/double-click|Select an/)).toBeNull();
    expect(screen.queryByRole("button", { name: "Load more obligations" })).toBeNull();
    await openRow(user, "A26000001-URN");
    expect(router.state.location.search).toBe(`?open=${OBLIGATION.obligationId}`);
    expect(within(panel()).getByText(STATUS)).toBeTruthy();
    expect(within(panel()).getByText("None recorded")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
    await user.click(within(panel()).getByRole("link", { name: "A26000001-URN" }));
    expect(await screen.findByText(`Navigated to /reports/${REPORT_ID}`)).toBeTruthy();
  });

  it("opens with Enter, and Escape closes the panel with focus back on the row", async () => {
    const { user, router } = renderPage("/delivery");
    await openRowWithEnter(user, STATUS);
    const recordAttempt = within(panel()).getByRole("button", { name: "Record attempt" });
    recordAttempt.focus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: NAME })).toBeNull();
    expect(router.state.location.search).toBe("");
    const row = screen.getByRole("gridcell", { name: STATUS }).closest("[role=row]");
    expect(row?.contains(document.activeElement)).toBe(true);
  });

  it("opens the exact obligation a link names, with its attempts", async () => {
    const attempted: ObligationView = {
      ...OBLIGATION,
      status: { code: "attention", label: "Fax by staff: failed, no answer or busy" },
      attempts: [
        {
          evidence: "failed",
          attemptId: "synthetic-attempt-1",
          observedAt: "2026-10-12T15:00:00Z",
          recordedBy: { userId: "00000000-0000-7000-8000-000000000e02", displayName: "Test Clerk" },
          failureCode: "no_answer_or_busy",
          note: "Line busy",
        },
      ],
    };
    renderPage(`/delivery?filter=all&open=${OBLIGATION.obligationId}`, {
      list_delivery_obligations: () => ({ items: [attempted], nextAfter: null }),
    });
    const attempts = await within(await screen.findByRole("region", { name: NAME })).findByRole(
      "list",
      { name: "Attempts" },
    );
    expect(attempts.textContent).toContain("Failed: No answer or busy");
    expect(attempts.textContent).toContain("recorded by Test Clerk. Line busy");
    expect(screen.getByRole("row", { current: true })).toBeTruthy();
  });

  it("opens an exact retained obligation outside loaded pages without changing filters", async () => {
    const exact = vi.fn(() => ({
      ...OBLIGATION,
      obligationId: "outside-page",
      status: { code: "closed", label: "Closed delivery evidence" },
      allowedActions: [],
    }));
    const { router } = renderPage("/delivery?open=outside-page", {
      get_delivery_obligation: exact,
    });
    expect(await screen.findByText("Closed delivery evidence")).toBeTruthy();
    expect(exact).toHaveBeenCalledWith({ input: { obligationId: "outside-page" } });
    expect(router.state.location.search).toBe("?open=outside-page");
    expect(within(panel()).queryByRole("button", { name: "Record attempt" })).toBeNull();
  });
  it("reports an exact missing obligation without selecting another", async () => {
    renderPage("/delivery?open=unknown-obligation");
    expect(await screen.findByText("The linked delivery obligation was not found.")).toBeTruthy();
    expect(screen.queryByRole("region", { name: NAME })).toBeNull();
  });

  it("refreshes on request and says when the queue was read", async () => {
    const list = vi.fn(() => ({ items: [OBLIGATION], nextAfter: null }));
    const { user } = renderPage("/delivery", { list_delivery_obligations: list });
    await screen.findByRole("gridcell", { name: STATUS });
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
  });

  it("keeps the filter in the address", async () => {
    const list = vi.fn(() => ({ items: [OBLIGATION], nextAfter: null }));
    const { user, router } = renderPage("/delivery", { list_delivery_obligations: list });
    await screen.findByRole("gridcell", { name: STATUS });
    await chooseWithKeyboard(user, /Show$/, "Failed or outcome unknown");
    expect(router.state.location.search).toBe("?filter=attention");
    expect(list).toHaveBeenLastCalledWith({ input: { filter: "attention", limit: 200 } });
  });

  it("records an attested hand-off from the panel", async () => {
    const record = vi.fn(() => OBLIGATION);
    const { user } = renderPage(`/delivery?open=${OBLIGATION.obligationId}`, {
      record_delivery_attempt: record,
    });
    await user.click(
      await within(await screen.findByRole("region", { name: NAME })).findByRole("button", {
        name: "Record attempt",
      }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Record delivery attempt" });
    await chooseWithKeyboard(
      user,
      /What happened/,
      "Handed off (I attest it was sent or handed over)",
    );
    expect(within(dialog).queryByRole("button", { name: /Why it failed/ })).toBeNull();
    await user.click(within(dialog).getByRole("button", { name: "Record attempt" }));
    expect(await screen.findByText("Delivery attempt recorded")).toBeTruthy();
    expect(record).toHaveBeenCalledWith({
      input: { obligationId: OBLIGATION.obligationId, evidence: "handed_off" },
    });
  });

  it("keeps the obligation open with its new status once the attempt moves it out of the filter", async () => {
    const SECOND: ObligationView = {
      ...OBLIGATION,
      obligationId: "00000000-0000-7000-8000-00000000b002",
      reportNumber: "A26000002-URN",
    };
    const HANDED: ObligationView = {
      ...OBLIGATION,
      status: { code: "handed_off", label: "Fax by staff: handed off, attested by Test Releaser" },
      allowedActions: ["export_report_pdf"],
    };
    // After the attempt the engine no longer lists it under "Awaiting delivery".
    const list = vi
      .fn()
      .mockReturnValueOnce({ items: [OBLIGATION, SECOND], nextAfter: null })
      .mockReturnValue({ items: [SECOND], nextAfter: null });
    let current = HANDED;
    const exact = vi.fn(() => current);
    const { user, router } = renderPage("/delivery", {
      list_delivery_obligations: list,
      record_delivery_attempt: () => HANDED,
      get_delivery_obligation: exact,
    });
    await openRow(user, "A26000001-URN");
    await user.click(within(panel()).getByRole("button", { name: "Record attempt" }));
    await chooseWithKeyboard(
      user,
      /What happened/,
      "Handed off (I attest it was sent or handed over)",
    );
    const dialog = screen.getByRole("dialog", { name: "Record delivery attempt" });
    await user.click(within(dialog).getByRole("button", { name: "Record attempt" }));
    expect(await screen.findByText("Delivery attempt recorded")).toBeTruthy();
    await vi.waitFor(() => expect(list).toHaveBeenCalledTimes(2));
    await vi.waitFor(() =>
      expect(screen.queryByRole("gridcell", { name: "A26000001-URN" })).toBeNull(),
    );
    // Still open, with the status the engine returned and the actions it now allows.
    expect(within(panel()).getByText(HANDED.status.label)).toBeTruthy();
    expect(within(panel()).getByText(/No longer in “Awaiting delivery”/)).toBeTruthy();
    expect(within(panel()).queryByRole("button", { name: "Record attempt" })).toBeNull();
    expect(screen.queryByText(/not in “Awaiting delivery”/)).toBeNull();
    current = {
      ...HANDED,
      status: { code: "paused", label: "Paused after external impact review" },
      allowedActions: [],
    };
    await user.click(within(panel()).getByRole("button", { name: "Refresh" }));
    expect(await within(panel()).findByText(current.status.label)).toBeTruthy();
    expect(exact).toHaveBeenCalledTimes(2);
    // The rest of the list stays as it was, and the next obligation opens from its row.
    await openRow(user, "A26000002-URN");
    expect(router.state.location.search).toBe(`?open=${SECOND.obligationId}`);
    expect(
      screen.getByRole("region", { name: "A26000002-URN to Synthetic Clinic fax" }),
    ).toBeTruthy();
  });

  it("needs the cause of a failed attempt and shows the engine's refusal", async () => {
    const record = vi.fn(() =>
      Promise.reject(
        engineError("conflict", PAUSED, {
          gateReasons: [{ code: "revision_impact_open", message: PAUSED }],
        }),
      ),
    );
    const { user } = renderPage("/delivery", { record_delivery_attempt: record });
    const menu = await openRowActions(user, NAME);
    await user.click(within(menu).getByRole("menuitem", { name: "Record attempt" }));
    const dialog = await screen.findByRole("dialog");
    await chooseWithKeyboard(user, /What happened/, "Failed");
    await user.click(within(dialog).getByRole("button", { name: "Record attempt" }));
    expect(await within(dialog).findByText("Choose why it failed.")).toBeTruthy();
    expect(record).not.toHaveBeenCalled();
    await chooseWithKeyboard(user, /Why it failed/, "No answer or busy");
    await user.type(within(dialog).getByLabelText(/^Note/), "Line busy twice");
    await user.click(within(dialog).getByRole("button", { name: "Record attempt" }));
    expect(record).toHaveBeenCalledWith({
      input: {
        obligationId: OBLIGATION.obligationId,
        evidence: "failed",
        failureCode: "no_answer_or_busy",
        note: "Line busy twice",
      },
    });
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("Impact review open:");
  });

  it("exports the PDF to deliver this obligation, from the row menu", async () => {
    const exportPdf = vi.fn(() => ({ fileName: "A26000001-URN-r1.pdf", folder: "downloads" }));
    const { user } = renderPage("/delivery", { export_report_pdf: exportPdf });
    const menu = await openRowActions(user, NAME);
    await user.click(within(menu).getByRole("menuitem", { name: "Export PDF" }));
    const dialog = await screen.findByRole("dialog", { name: "Export PDF of A26000001-URN" });
    await user.click(within(dialog).getByRole("button", { name: "Export PDF" }));
    expect(
      await screen.findByText("PDF saved to your Downloads folder. An export is not a delivery."),
    ).toBeTruthy();
    expect(exportPdf).toHaveBeenCalledWith({
      input: {
        revisionId: REVISION_ID,
        purpose: "delivery",
        obligationId: OBLIGATION.obligationId,
      },
    });
  });

  it("redirects an obligation and shows the refusal of another client's destination", async () => {
    const redirect = vi.fn(() =>
      Promise.reject(
        engineError("gate_blocked", "This action is blocked by one or more gates.", {
          gateReasons: [
            {
              code: "destination_not_of_client",
              message: "The destination belongs to another client.",
            },
          ],
        }),
      ),
    );
    const destinations = vi.fn(() => [{ ...DESTINATION, clientId: "unknown-client" }]);
    const { user } = renderPage(`/delivery?open=${OBLIGATION.obligationId}`, {
      redirect_delivery_obligation: redirect,
      list_delivery_destinations: destinations,
    });
    await user.click(
      await within(await screen.findByRole("region", { name: NAME })).findByRole("button", {
        name: "Redirect",
      }),
    );
    const dialog = await screen.findByRole("dialog", { name: "Redirect delivery" });
    expect(destinations).toHaveBeenCalledWith({
      input: { clientId: OBLIGATION.clientId, state: "verified" },
    });
    await chooseWithKeyboard(user, /Destination/, "Results email (results@clinic.example)");
    await user.type(within(dialog).getByLabelText(/^Reason/), "Fax number disconnected");
    await user.click(within(dialog).getByRole("button", { name: "Redirect" }));
    const blocked = await within(dialog).findByRole("list", { name: "Blocked by" });
    expect(blocked.textContent).toContain("The destination belongs to another client.");
    expect(redirect).toHaveBeenCalledWith({
      input: {
        obligationId: OBLIGATION.obligationId,
        destinationId: DESTINATION.destinationId,
        reason: "Fax number disconnected",
      },
    });
  });

  it("offers only the actions the engine allows, in the panel and the row menu", async () => {
    const blocked: ObligationView = {
      ...OBLIGATION,
      status: { code: "blocked", label: "Fax by staff: blocked (destination not verified)" },
      allowedActions: ["redirect_delivery_obligation"],
    };
    const closed: ObligationView = {
      ...OBLIGATION,
      obligationId: "00000000-0000-7000-8000-00000000b002",
      reportNumber: "A26000002-URN",
      allowedActions: [],
    };
    const { user } = renderPage(`/delivery?filter=blocked&open=${OBLIGATION.obligationId}`, {
      list_delivery_obligations: () => ({ items: [blocked, closed], nextAfter: null }),
    });
    const shown = await screen.findByRole("region", { name: NAME });
    expect(within(shown).queryByRole("button", { name: "Record attempt" })).toBeNull();
    expect(within(shown).getByRole("button", { name: "Redirect" })).toBeTruthy();
    const menu = await openRowActions(user, NAME);
    expect(
      within(menu)
        .getAllByRole("menuitem")
        .map((item) => item.textContent),
    ).toEqual(["Redirect"]);
    await user.keyboard("{Escape}");
    expect(
      screen.queryByRole("button", { name: "Actions for A26000002-URN to Synthetic Clinic fax" }),
    ).toBeNull();
  });

  it("tells a filter that excludes everything from nothing open, and offers every open one", async () => {
    const list = vi.fn(() => ({ items: [], nextAfter: null }));
    const { user } = renderPage("/delivery?filter=paused", { list_delivery_obligations: list });
    expect(await screen.findByText("Nothing in “Paused for impact review”")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Show all open" }));
    expect(await screen.findByText("Nothing to deliver")).toBeTruthy();
    expect(screen.getByText("No delivery obligation is open.")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({ input: { filter: "all", limit: 200 } });
    expect(screen.queryByRole("button", { name: "Show all open" })).toBeNull();
  });

  it("shows loading, then the engine's error", async () => {
    let fail: (error: unknown) => void = () => undefined;
    renderPage("/delivery", {
      list_delivery_obligations: () =>
        new Promise((_, reject) => {
          fail = reject;
        }),
    });
    expect(await screen.findByText("Loading delivery obligations…")).toBeTruthy();
    fail(engineError("internal", "Queue failed."));
    expect(await screen.findByText("Queue failed.")).toBeTruthy();
    expect(screen.getByRole("heading", { level: 1, name: "Delivery" })).toBeTruthy();
  });

  it("continues a bounded scan with the returned cursor, keeping the rows already shown", async () => {
    const second: ObligationView = {
      ...OBLIGATION,
      obligationId: "00000000-0000-7000-8000-0000000000f2",
      reportNumber: "A26000002-URN",
    };
    const list = vi
      .fn()
      .mockReturnValueOnce({ items: [], nextAfter: "obligation-cursor-1" })
      .mockReturnValueOnce({ items: [OBLIGATION], nextAfter: "obligation-cursor-2" })
      .mockReturnValueOnce({ items: [second], nextAfter: null });
    const { user } = renderPage("/delivery", { list_delivery_obligations: list });
    expect(await screen.findByText(/None of the obligations checked so far match/)).toBeTruthy();
    expect(screen.queryByText("Nothing to deliver")).toBeNull();
    expect(screen.getByText("0 obligations shown · more available")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load more obligations" }));
    expect(await screen.findByRole("gridcell", { name: "A26000001-URN" })).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: { filter: "awaiting", limit: 200, after: "obligation-cursor-1" },
    });
    expect(screen.getByText("1 obligation shown · more available")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load more obligations" }));
    expect(await screen.findByRole("gridcell", { name: "A26000002-URN" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "A26000001-URN" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "Load more obligations" })).toBeNull();
  });

  it("keeps the rows shown when the next page fails", async () => {
    const list = vi
      .fn()
      .mockReturnValueOnce({ items: [OBLIGATION], nextAfter: "obligation-cursor" })
      .mockImplementationOnce(() => Promise.reject(engineError("internal", "Scan failed.")));
    const { user } = renderPage("/delivery", { list_delivery_obligations: list });
    await screen.findByRole("gridcell", { name: STATUS });
    await user.click(screen.getByRole("button", { name: "Load more obligations" }));
    expect(await screen.findByText("More obligations could not be loaded")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: STATUS })).toBeTruthy();
  });
});
