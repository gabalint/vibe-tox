import { screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AuditEventView } from "@/generated/AuditEventView";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { AuditTrailPage } from "./AuditTrailPage";

/** Synthetic security events with sequence numbers `from` down to `from - count + 1`. */
function events(from: number, count: number): AuditEventView[] {
  return Array.from({ length: count }, (_, index) => ({
    stream: "security",
    seq: from - index,
    occurredAt: "2026-10-12T14:03:05Z",
    recordedAt: "2026-10-12T14:03:06Z",
    principalKind: "user",
    principalId: "00000000-0000-7000-8000-000000000101",
    principalDisplay: "Test Administrator",
    action: "auth.sign_in",
    actionLabel: "Sign-in",
    outcome: from - index === 150 ? "denied" : "succeeded",
    recordType: "user",
    recordTypeLabel: "User account",
    recordId: `user-${from - index}`,
    recordLabel: from - index === 3 ? "Test Analyst" : undefined,
    reason: null,
    authority: null,
    correlationId: `corr-${from - index}`,
    availableActions: [
      ["auth.sign_in", "Sign-in"],
      ["auth.lock", "Session locked"],
    ],
    availableRecordTypes: [["user", "User account"]],
  }));
}

function renderPage(commands: Record<string, (args: unknown) => unknown>, path = "/audit") {
  mockCommands(commands);
  const router = createMemoryRouter([{ path: "/audit", element: <AuditTrailPage /> }], {
    initialEntries: [path],
  });
  return { ...renderWithProviders(<RouterProvider router={router} />), router };
}

const PAGE = 100;

describe("AuditTrailPage (REQ-OPS-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the newest security events and loads older pages before the last one", async () => {
    const list = vi.fn().mockReturnValueOnce(events(150, PAGE)).mockReturnValueOnce(events(50, 50));
    const { container, user } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Security audit events" })).toBeTruthy();
    expect(screen.getByText("100 events shown")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "150" })).toBeTruthy();
    expect(screen.getByText("Denied")).toBeTruthy();
    expect(screen.getAllByRole("gridcell", { name: "Sign-in" }).length).toBeGreaterThan(0);
    expect(
      screen.getAllByRole("gridcell", { name: "Test Administrator (user)" }).length,
    ).toBeGreaterThan(0);
    expect(list).toHaveBeenLastCalledWith({ input: { stream: "security", limit: PAGE } });
    await expectNoA11yViolations(container);

    await user.click(screen.getByRole("button", { name: "Load older events" }));
    expect(await screen.findByText("150 events shown")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: { stream: "security", beforeSeq: 51, limit: PAGE },
    });
    expect(screen.queryByRole("button", { name: "Load older events" })).toBeNull();
  });

  it("retains the shown count and rows when an older page fails, then handles an empty final page", async () => {
    const list = vi
      .fn()
      .mockReturnValueOnce(events(100, PAGE))
      .mockRejectedValueOnce(engineError("unavailable", "Older events unavailable."))
      .mockReturnValueOnce([]);
    const { user } = renderPage({ list_audit_events: list });
    expect(await screen.findByText("100 events shown")).toBeTruthy();
    // A full page permits another read, but does not prove that more events exist.
    expect(screen.queryByText(/more available/)).toBeNull();
    await user.click(screen.getByRole("button", { name: "Load older events" }));
    expect(await screen.findByText("Older events unavailable.")).toBeTruthy();
    expect(screen.getByRole("grid", { name: "Security audit events" })).toBeTruthy();
    expect(screen.getByText("100 events shown")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Load older events" }));
    await waitFor(() =>
      expect(screen.queryByRole("button", { name: "Load older events" })).toBeNull(),
    );
    expect(screen.getByText("100 events shown")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: { stream: "security", beforeSeq: 1, limit: PAGE },
    });
  });

  it("applies each filter when chosen, offering the engine's lists, and keeps them in the address", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Security audit events" })).toBeTruthy();
    expect(screen.getAllByRole("gridcell", { name: "User account Test Analyst" })).toHaveLength(1);
    expect(screen.getAllByRole("gridcell", { name: "User account user-2" })).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: /Action/ }));
    await user.click(await screen.findByRole("option", { name: "Session locked" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { stream: "security", action: "auth.lock", limit: PAGE },
      }),
    );
    // The record ID is a find: no "Press Enter" hint, and Enter applies it at once.
    expect(screen.queryByText(/Press Enter/)).toBeNull();
    await user.type(
      screen.getByRole("searchbox", { name: "Find by record ID" }),
      " user-7 {Enter}",
    );
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { stream: "security", action: "auth.lock", recordId: "user-7", limit: PAGE },
      }),
    );
    expect(router.state.location.search).toBe("?stream=security&action=auth.lock&recordId=user-7");
    // Actions belong to a stream: another stream keeps only the record ID.
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Access" }));
    await waitFor(() =>
      expect(router.state.location.search).toBe("?stream=access&recordId=user-7"),
    );
  });

  it("lists only self-approvals, across streams, and keeps the filter in the address (ADR 0015)", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Security audit events" })).toBeTruthy();
    await user.click(screen.getByRole("checkbox", { name: "Self-approvals only" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { stream: "security", selfApprovalsOnly: true, limit: PAGE },
      }),
    );
    expect(router.state.location.search).toBe("?stream=security&selfApprovalsOnly=true");
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Access" }));
    await waitFor(() =>
      expect(router.state.location.search).toBe("?stream=access&selfApprovalsOnly=true"),
    );
    expect(screen.getByRole("checkbox", { name: "Self-approvals only" })).toHaveProperty(
      "checked",
      true,
    );
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(router.state.location.search).toBe("?stream=access"));
    expect(list).toHaveBeenLastCalledWith({ input: { stream: "access", limit: PAGE } });
  });

  it("clamps a long reason to two lines in the grid and shows all of it in the panel", async () => {
    const reason = "Synthetic correction reason. ".repeat(20).trim();
    const [first, ...rest] = events(3, 3);
    if (first === undefined) throw new Error("no synthetic event");
    const { user } = renderPage({ list_audit_events: () => [{ ...first, reason }, ...rest] });
    const clamped = await screen.findByTitle(reason);
    expect(clamped.className).toMatch(/reason/);
    expect(clamped.closest("[role=gridcell]")).toBeTruthy();
    await openRow(user, "3");
    await waitFor(() => expect(screen.getAllByText(reason)).toHaveLength(2));
    const inPanel = screen.getAllByText(reason).find((each) => !each.closest("[role=grid]"));
    expect(inPanel?.className ?? "").not.toMatch(/reason/);
  });

  it("reads the filters from the address, and offers to clear them when nothing matches", async () => {
    const list = vi.fn(() => []);
    const { user, router } = renderPage(
      { list_audit_events: list },
      "/audit?stream=clinical&recordType=report",
    );
    expect(await screen.findByText("No event matches")).toBeTruthy();
    expect(list).toHaveBeenCalledWith({
      input: { stream: "clinical", recordType: "report", limit: PAGE },
    });
    expect(screen.getByText("Record type: report")).toBeTruthy();
    expect(screen.queryByRole("button", { name: /Record type/ })).toBeNull();
    await user.click(screen.getByRole("button", { name: "More filters" }));
    expect(screen.getByRole("button", { name: /Record type/ }).textContent).toContain("report");
    await user.click(screen.getByRole("button", { name: "Show all events" }));
    await waitFor(() => expect(router.state.location.search).toBe("?stream=clinical"));
    expect(await screen.findByText("No audit events")).toBeTruthy();
  });

  it("opens an event in the side panel with every field, by click or Enter", async () => {
    const { user, router, container } = renderPage({ list_audit_events: () => events(3, 3) });
    await openRow(user, "3");
    expect(screen.getByRole("region", { name: "Security event 3" })).toBeTruthy();
    expect(router.state.location.search).toBe("?open=3");
    await openRowWithEnter(user, "2");
    expect(screen.queryByRole("region", { name: "Security event 3" })).toBeNull();
    const details = screen.getByRole("region", { name: "Security event 2" });
    expect(details.textContent).toContain("corr-2");
    // The recorded code stays beside the engine's label.
    expect(details.textContent).toContain("Sign-in (auth.sign_in)");
    expect(details.textContent).toContain("user-2");
    expect(details.textContent).toContain("2026-10-12 14:03:06 UTC");
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens the event the address names, and Escape returns focus to its row", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list }, "/audit?open=2");
    const details = await screen.findByRole("region", { name: "Security event 2" });
    // Opening an event reads nothing more than the list.
    expect(list).toHaveBeenCalledTimes(1);
    within(details).getByRole("button", { name: "Close" }).focus();
    await user.keyboard("{Escape}");
    expect(screen.queryByRole("region", { name: "Security event 2" })).toBeNull();
    expect(router.state.location.search).toBe("");
    const row = screen.getByRole("gridcell", { name: "2" }).closest("[role=row]");
    expect(row?.contains(document.activeElement)).toBe(true);
  });

  it("shows the engine error with its support reference", async () => {
    renderPage({
      list_audit_events: () =>
        Promise.reject(engineError("forbidden", "You cannot view the audit trail.")),
    });
    const alert = await screen.findByRole("alert");
    expect(alert.textContent).toContain("You cannot view the audit trail.");
    expect(alert.textContent).toContain("Support reference: corr-test-0001");
  });
});

describe("Audit chain verification (REQ-OPS-001)", () => {
  beforeEach(mockGridLayout);

  it("reports an intact chain", async () => {
    const verify = vi.fn(() => ({
      stream: "security",
      intact: true,
      eventsChecked: 120,
      findings: [],
    }));
    const { user } = renderPage({ list_audit_events: () => [], verify_audit_chain: verify });
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    const result = (await screen.findByText("Chain intact")).closest("[role=status]");
    expect(result?.textContent).toContain("Chain intact");
    expect(result?.textContent).toContain("120 events in the security stream were checked");
    expect(verify).toHaveBeenCalledWith({ input: { stream: "security" } });
  });

  it("shows an action without a label as recorded, and a check only beside its stream", async () => {
    const { actionLabel: _label, ...unlabelled } = events(1, 1)[0] as AuditEventView;
    const accessEvent = { ...unlabelled, stream: "access", action: "audit.view" } as const;
    const { user } = renderPage({
      list_audit_events: (args) =>
        (args as { input: { stream: string } }).input.stream === "access" ? [accessEvent] : [],
      verify_audit_chain: () => ({
        stream: "security",
        intact: true,
        eventsChecked: 3,
        findings: [],
      }),
    });
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    expect(await screen.findByText("Chain intact")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Access" }));
    expect(await screen.findByRole("gridcell", { name: "audit.view" })).toBeTruthy();
    expect(screen.queryByText("Chain intact")).toBeNull();
  });

  it("lists what was found when the chain is not intact", async () => {
    const { user, container } = renderPage({
      list_audit_events: () => [],
      verify_audit_chain: () => ({
        stream: "security",
        intact: false,
        eventsChecked: 120,
        findings: [
          { kind: "missing_events", seq: 41 },
          { kind: "broken_link", seq: 43 },
          { kind: "new_kind", seq: null },
        ],
      }),
    });
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    const result = await screen.findByRole("alert");
    expect(result.textContent).toContain("The chain is not intact");
    const findings = within(result)
      .getAllByRole("listitem")
      .map((item) => item.textContent);
    expect(findings).toEqual([
      "Events are missing: they were deleted (event 41).",
      "An event does not link to the event before it: an earlier event was removed, inserted or moved (event 43).",
      "new_kind.",
    ]);
    await expectNoA11yViolations(container);
  });

  it("shows why verification could not run", async () => {
    const { user } = renderPage({
      list_audit_events: () => [],
      verify_audit_chain: () => Promise.reject(engineError("internal", "")),
    });
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The chain could not be verified",
    );
  });
});
