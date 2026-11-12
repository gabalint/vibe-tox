import { screen, waitFor, within } from "@testing-library/react";
import { createMemoryRouter, RouterProvider } from "react-router";
import { beforeEach, describe, expect, it, vi } from "vitest";
import type { AuditEventView } from "@/generated/AuditEventView";
import type { FoundAuditRecords } from "@/generated/FoundAuditRecords";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { WALKTHROUGH_TIMEOUT_MS } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderWithProviders } from "@/lib/test/render";
import { AuditTrailPage } from "./AuditTrailPage";

const RECORDED_AT = "2026-10-12T14:03:06Z";
const ADMIN = "00000000-0000-7000-8000-000000000101";

/** Synthetic security events with sequence numbers `from` down to `from - count + 1`. */
function events(from: number, count: number): AuditEventView[] {
  return Array.from({ length: count }, (_, index) => ({
    stream: "security",
    seq: from - index,
    occurredAt: "2026-10-12T14:03:05Z",
    recordedAt: RECORDED_AT,
    principalKind: "user",
    principalId: ADMIN,
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
    availableUsers: [[ADMIN, "Test Administrator"]],
  }));
}

/** What an opened sign-in event changed: nothing (ticket #4534). */
const NO_CHANGE = {
  status: "no_change",
  oldRevision: null,
  newRevision: null,
  patient: null,
  fields: [],
  facts: [],
  readCapabilities: [],
};

/** Find's synthetic answer: one user, and more than it lists. */
const FOUND: FoundAuditRecords = {
  records: [
    {
      recordType: "user",
      recordTypeLabel: "User account",
      recordId: "user-7",
      label: "Test Analyst",
      streams: ["security", "access"],
      lastRecordedAt: RECORDED_AT,
    },
  ],
  more: true,
};

function renderPage(commands: Record<string, (args: unknown) => unknown>, path = "/audit") {
  mockCommands({ get_audit_event_changes: () => NO_CHANGE, ...commands });
  const router = createMemoryRouter([{ path: "/audit", element: <AuditTrailPage /> }], {
    initialEntries: [path],
  });
  return { ...renderWithProviders(<RouterProvider router={router} />), router };
}

const PAGE = 100;

describe("AuditTrailPage (REQ-OPS-001)", { timeout: WALKTHROUGH_TIMEOUT_MS }, () => {
  beforeEach(mockGridLayout);

  it("shows the newest events of every stream and loads older pages after the last one", async () => {
    const list = vi.fn().mockReturnValueOnce(events(150, PAGE)).mockReturnValueOnce(events(50, 50));
    const { container, user } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Audit events" })).toBeTruthy();
    expect(screen.getByRole("button", { name: /Stream/ }).textContent).toContain("All streams");
    expect(screen.getByText(/^Security, clinical and access events together\./)).toBeTruthy();
    expect(screen.getAllByRole("gridcell", { name: "Security" }).length).toBe(PAGE);
    expect(screen.getByText("100 events shown")).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "150" })).toBeTruthy();
    expect(screen.getByRole("gridcell", { name: "Denied" })).toBeTruthy();
    // A succeeded action is a success badge, like Failed is a danger one (ticket #4536).
    const succeeded = screen.getAllByRole("gridcell", { name: "Succeeded" })[0];
    expect(succeeded?.querySelector("[data-tone]")?.getAttribute("data-tone")).toBe("success");
    expect(screen.getAllByRole("gridcell", { name: "Sign-in" }).length).toBeGreaterThan(0);
    expect(
      screen.getAllByRole("gridcell", { name: "Test Administrator (user)" }).length,
    ).toBeGreaterThan(0);
    expect(list).toHaveBeenLastCalledWith({ input: { limit: PAGE } });
    await expectNoA11yViolations(container);

    await user.click(screen.getByRole("button", { name: "Load older events" }));
    expect(await screen.findByText("150 events shown")).toBeTruthy();
    expect(list).toHaveBeenLastCalledWith({
      input: { before: { recordedAt: RECORDED_AT, stream: "security", seq: 51 }, limit: PAGE },
    });
    expect(screen.queryByRole("button", { name: "Load older events" })).toBeNull();
  });

  it("names an instrument in By, and shows its ID where the engine has no name (#4743)", async () => {
    const [named, unknown] = events(2, 2).map((event) => ({
      ...event,
      principalKind: "instrument" as const,
      principalId: "00000000-0000-7000-8000-000000000301",
    }));
    const list = vi.fn(() => [
      { ...named, principalDisplay: "Synthetic LC-MS/MS" },
      { ...unknown, principalDisplay: null },
    ]);
    renderPage({ list_audit_events: list });
    expect(
      await screen.findByRole("gridcell", { name: "Synthetic LC-MS/MS (instrument)" }),
    ).toBeTruthy();
    expect(
      screen.getByRole("gridcell", { name: "Instrument 00000000-0000-7000-8000-000000000301" }),
    ).toBeTruthy();
  });

  it("retains the shown count and rows when an older page fails, then handles an empty final page", async () => {
    const list = vi
      .fn()
      .mockReturnValueOnce(events(100, PAGE))
      .mockRejectedValueOnce(engineError("unavailable", "Older events unavailable."))
      .mockReturnValueOnce([]);
    const { user } = renderPage({ list_audit_events: list }, "/audit?stream=security");
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
      input: {
        stream: "security",
        before: { recordedAt: RECORDED_AT, stream: "security", seq: 1 },
        limit: PAGE,
      },
    });
  });

  it("applies each filter when chosen, offering the engine's lists, and keeps them in the address", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage(
      { list_audit_events: list, find_audit_records: () => FOUND },
      "/audit?stream=security",
    );
    expect(await screen.findByRole("grid", { name: "Security audit events" })).toBeTruthy();
    expect(screen.getByText(/^Sign-ins, users, roles and security settings/)).toBeTruthy();
    expect(screen.getAllByRole("gridcell", { name: "User account Test Analyst" })).toHaveLength(1);
    expect(screen.getAllByRole("gridcell", { name: "User account user-2" })).toHaveLength(1);
    await user.click(screen.getByRole("button", { name: /Action/ }));
    await user.click(await screen.findByRole("option", { name: "Session locked" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { stream: "security", action: "auth.lock", limit: PAGE },
      }),
    );
    // A record picked in Find applies with its record type.
    await user.type(screen.getByRole("combobox", { name: "Find a record" }), "analyst");
    await user.click(await screen.findByRole("option", { name: /Test Analyst/ }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          stream: "security",
          action: "auth.lock",
          recordType: "user",
          recordId: "user-7",
          limit: PAGE,
        },
      }),
    );
    expect(router.state.location.search).toBe(
      "?stream=security&action=auth.lock&recordType=user&recordId=user-7",
    );
    // Actions, record types and so records belong to a stream: another stream clears them.
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Access" }));
    await waitFor(() => expect(router.state.location.search).toBe("?stream=access"));
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "All streams" }));
    await waitFor(() => expect(router.state.location.search).toBe(""));
    expect(list).toHaveBeenLastCalledWith({ input: { limit: PAGE } });
  });

  it("finds records by readable values in the chosen record type and stream", async () => {
    const find = vi.fn(() => FOUND);
    const { user } = renderPage(
      { list_audit_events: () => events(3, 3), find_audit_records: find },
      "/audit?stream=security&recordType=instrument",
    );
    const field = await screen.findByRole("combobox", {
      name: "Find: Instrument code or name or record ID",
    });
    expect(field.getAttribute("placeholder")).toBe("Instrument code or name or record ID");
    // One letter is not a search.
    await user.type(field, "A");
    expect(find).not.toHaveBeenCalled();
    await user.type(field, "U-SER");
    await waitFor(() =>
      expect(find).toHaveBeenLastCalledWith({
        input: { text: "AU-SER", recordType: "instrument", stream: "security" },
      }),
    );
    expect(await screen.findByText(/More records match than are listed/)).toBeTruthy();
  });

  it("lists only self-approvals, across streams, and keeps the filter in the address (ADR 0015)", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list }, "/audit?stream=security");
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

  it("filters by who acted and the outcome in the engine's query, and keeps both in the address", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Audit events" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /By/ }));
    await user.click(await screen.findByRole("option", { name: "System only" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({ input: { byKind: "system", limit: PAGE } }),
    );
    await user.click(screen.getByRole("button", { name: /By/ }));
    await user.click(await screen.findByRole("option", { name: "Test Administrator" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({ input: { byUser: ADMIN, limit: PAGE } }),
    );
    await user.click(screen.getByRole("button", { name: /Outcome/ }));
    await user.click(await screen.findByRole("option", { name: "Denied" }));
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { byUser: ADMIN, outcome: "denied", limit: PAGE },
      }),
    );
    expect(router.state.location.search).toBe(`?by=${ADMIN}&outcome=denied`);
    // The stream is independent of who acted and how it ended.
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Clinical" }));
    await waitFor(() =>
      expect(router.state.location.search).toBe(`?stream=clinical&by=${ADMIN}&outcome=denied`),
    );
  });

  it("filters by the days recorded, as the laboratory's days, and refuses an end before the start", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list });
    expect(await screen.findByRole("grid", { name: "Audit events" })).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "More filters" }));
    await user.type(screen.getByRole("textbox", { name: "Recorded from" }), "2026-10-01");
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: { recordedFrom: "2026-10-01T00:00:00.000000000Z", limit: PAGE },
      }),
    );
    const calls = list.mock.calls.length;
    await user.type(screen.getByRole("textbox", { name: "Recorded to" }), "2026-09-30");
    expect(await screen.findByText("Choose a day on or after the start day.")).toBeTruthy();
    expect(list).toHaveBeenCalledTimes(calls);
    await user.clear(screen.getByRole("textbox", { name: "Recorded to" }));
    await user.type(screen.getByRole("textbox", { name: "Recorded to" }), "2026-10-05");
    // The last day counts in full: the range ends where the next day begins.
    await waitFor(() =>
      expect(list).toHaveBeenLastCalledWith({
        input: {
          recordedFrom: "2026-10-01T00:00:00.000000000Z",
          recordedBefore: "2026-10-06T00:00:00.000000000Z",
          limit: PAGE,
        },
      }),
    );
    expect(router.state.location.search).toBe("?from=2026-10-01&to=2026-10-05");
    expect(screen.getByText("Recorded 2026-10-01 to 2026-10-05")).toBeTruthy();
  });

  it("clears every filter at once, keeping the stream", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage(
      { list_audit_events: list },
      "/audit?stream=security&action=auth.lock&from=2026-10-01&by=system&outcome=failed",
    );
    expect(await screen.findByText("Recorded from 2026-10-01")).toBeTruthy();
    expect(screen.getByRole("button", { name: /By/ }).textContent).toContain("System only");
    expect(screen.getByRole("button", { name: /Outcome/ }).textContent).toContain("Failed");
    await user.click(screen.getByRole("button", { name: "Clear filters" }));
    await waitFor(() => expect(router.state.location.search).toBe("?stream=security"));
    expect(list).toHaveBeenLastCalledWith({ input: { stream: "security", limit: PAGE } });
    expect(screen.queryByText("Recorded from 2026-10-01")).toBeNull();
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
    expect(router.state.location.search).toBe("?open=security-3");
    await openRowWithEnter(user, "2");
    expect(screen.queryByRole("region", { name: "Security event 3" })).toBeNull();
    const details = screen.getByRole("region", { name: "Security event 2" });
    expect(details.textContent).toContain("corr-2");
    // The recorded code stays beside the engine's label.
    expect(details.textContent).toContain("Sign-in (auth.sign_in)");
    expect(details.textContent).toContain("user-2");
    expect(details.textContent).toContain("2026-10-12 14:03:06 UTC");
    expect(await within(details).findByText("No data changed.")).toBeTruthy();
    await expectNoA11yViolations(container, { wholePage: true });
  });

  it("opens the event the address names, and Escape returns focus to its row", async () => {
    const list = vi.fn(() => events(3, 3));
    const { user, router } = renderPage({ list_audit_events: list }, "/audit?open=security-2");
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
    const { user } = renderPage(
      { list_audit_events: () => [], verify_audit_chain: verify },
      "/audit?stream=security",
    );
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    const result = (await screen.findByText("Chain intact")).closest("[role=status]");
    expect(result?.textContent).toContain("Chain intact");
    expect(result?.textContent).toContain("120 events in the security stream were checked");
    expect(verify).toHaveBeenCalledWith({ input: { stream: "security" } });
  });

  it("shows an action without a label as recorded, and a check only beside its stream", async () => {
    const { actionLabel: _label, ...unlabelled } = events(1, 1)[0] as AuditEventView;
    const accessEvent = { ...unlabelled, stream: "access", action: "audit.view" } as const;
    const { user } = renderPage(
      {
        list_audit_events: (args) =>
          (args as { input: { stream: string } }).input.stream === "access" ? [accessEvent] : [],
        verify_audit_chain: () => ({
          stream: "security",
          intact: true,
          eventsChecked: 3,
          findings: [],
        }),
      },
      "/audit?stream=security",
    );
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    expect(await screen.findByText("Chain intact")).toBeTruthy();
    await user.click(screen.getByRole("button", { name: /Stream/ }));
    await user.click(await screen.findByRole("option", { name: "Access" }));
    expect(await screen.findByRole("gridcell", { name: "audit.view" })).toBeTruthy();
    expect(screen.queryByText("Chain intact")).toBeNull();
  });

  it("lists what was found when the chain is not intact", async () => {
    const { user, container } = renderPage(
      {
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
      },
      "/audit?stream=security",
    );
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

  it("verifies a chosen stream only: every stream together has no single chain", async () => {
    const verify = vi.fn();
    renderPage({ list_audit_events: () => [], verify_audit_chain: verify });
    const button = await screen.findByRole("button", { name: "Verify chain" });
    expect(button.hasAttribute("disabled")).toBe(true);
    expect(screen.getByText(/Choose a stream to verify its chain/)).toBeTruthy();
    expect(await screen.findByText("Nothing is recorded in any stream yet.")).toBeTruthy();
  });

  it("shows why verification could not run", async () => {
    const { user } = renderPage(
      {
        list_audit_events: () => [],
        verify_audit_chain: () => Promise.reject(engineError("internal", "")),
      },
      "/audit?stream=security",
    );
    await user.click(await screen.findByRole("button", { name: "Verify chain" }));
    expect((await screen.findByRole("alert")).textContent).toContain(
      "The chain could not be verified",
    );
  });
});
