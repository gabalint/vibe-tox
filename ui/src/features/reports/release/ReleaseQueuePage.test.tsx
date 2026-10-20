import { screen, within } from "@testing-library/react";
import { beforeEach, describe, expect, it, vi } from "vitest";
import { expectNoA11yViolations } from "@/lib/test/a11y";
import { openRow, openRowWithEnter } from "@/lib/test/grid";
import { engineError, mockCommands } from "@/lib/test/ipc";
import { chooseWithKeyboard } from "@/lib/test/keyboard";
import { mockGridLayout } from "@/lib/test/layout";
import { renderRoutes } from "@/lib/test/router";
import type { WorkQueueItem, WorkQueueSummary } from "../contract";
import { REVISION_ID, SPECIMEN_ID, sessionWith, WORKSPACE } from "../testData";
import { ReleaseQueuePage } from "./ReleaseQueuePage";
import { ReportWorkspacePage } from "./ReportWorkspacePage";

const QUEUES: WorkQueueSummary[] = [
  {
    queue: "release_ready",
    label: "Reports ready for release",
    ownerCapability: "release_reports",
    count: 1,
    countCapped: false,
    oldestSince: "2026-10-12T13:00:00Z",
    nextAction: "Prepare and release",
  },
  {
    queue: "release_blocked",
    label: "Report release blocked",
    ownerCapability: "release_reports",
    count: 0,
    countCapped: false,
    oldestSince: null,
    nextAction: "Resolve gates",
  },
  {
    queue: "review_ready",
    label: "Awaiting review",
    ownerCapability: "review_results",
    count: 500,
    countCapped: true,
    oldestSince: "2026-10-12T08:00:00Z",
    nextAction: "Review",
  },
];

const ITEM: WorkQueueItem = {
  queue: "release_ready",
  itemKind: "specimen_report",
  itemId: SPECIMEN_ID,
  title: "A26000001 urine",
  state: "ready",
  stateLabel: "Ready",
  since: "2026-10-12T13:00:00Z",
  nextAction: "Prepare and release",
};

function renderPage(
  path: string,
  listItems: (args: unknown) => unknown,
  listQueues: () => unknown = () => QUEUES,
) {
  mockCommands({
    current_session: () => sessionWith(["release_reports"]),
    list_work_queues: listQueues,
    list_work_queue_items: async (args) => {
      const answer = await listItems(args);
      return Array.isArray(answer) ? { items: answer, next: null, scanned: 0 } : answer;
    },
  });
  return renderRoutes([{ path: "/reports/release", element: <ReleaseQueuePage /> }], path);
}

describe("ReleaseQueuePage (REQ-OPS-004)", () => {
  beforeEach(mockGridLayout);

  it("lists the releasers' queues and opens a specimen's report workspace with Enter", async () => {
    const items = vi.fn(() => [ITEM]);
    const { user, container } = renderPage("/reports/release", items);
    expect(await screen.findByRole("gridcell", { name: "A26000001 urine" })).toBeTruthy();
    expect(items).toHaveBeenCalledWith({
      input: { queue: "release_ready", limit: 200, after: null },
    });
    expect(screen.getByRole("gridcell", { name: "Ready" })).toBeTruthy();
    expect(screen.queryByText(/double-click/)).toBeNull();
    await expectNoA11yViolations(container, { wholePage: true });
    await openRowWithEnter(user, "Ready");
    expect(await screen.findByText(`Navigated to /reports/specimens/${SPECIMEN_ID}`)).toBeTruthy();
  });

  it("opens a workspace with the queue in its breadcrumb, and Previous and Next", async () => {
    const second = { ...ITEM, itemId: "00000000-0000-7000-8000-000000006002", title: "A26000002" };
    mockCommands({
      current_session: () => sessionWith(["release_reports"]),
      list_work_queues: () => QUEUES,
      list_work_queue_items: () => ({ items: [ITEM, second], next: null, scanned: 0 }),
      get_report_workspace: () => WORKSPACE,
    });
    const { user, router } = renderRoutes(
      [
        { path: "/reports/release", element: <ReleaseQueuePage /> },
        { path: "/reports/specimens/:specimenId", element: <ReportWorkspacePage /> },
      ],
      "/reports/release?queue=release_ready",
    );
    await openRow(user, "A26000001 urine");
    const crumbs = await screen.findByRole("navigation", { name: "Breadcrumb" });
    expect(within(crumbs).getByRole("link", { name: "Release queue" }).getAttribute("href")).toBe(
      "/reports/release?queue=release_ready",
    );
    const sequence = screen.getByRole("navigation", { name: "Records in list order" });
    expect(
      within(sequence).getByRole("link", { name: "Previous" }).getAttribute("aria-disabled"),
    ).toBe("true");
    await user.click(within(sequence).getByRole("link", { name: "Next" }));
    expect(router.state.location.pathname).toBe(`/reports/specimens/${second.itemId}`);
    const after = await screen.findByRole("navigation", { name: "Breadcrumb" });
    await user.click(within(after).getByRole("link", { name: "Release queue" }));
    expect(await screen.findByRole("gridcell", { name: "A26000002" })).toBeTruthy();
  });

  it("refreshes the counts and the items on request", async () => {
    const items = vi.fn(() => [ITEM]);
    const queues = vi.fn(() => QUEUES);
    const { user } = renderPage("/reports/release", items, queues);
    await screen.findByRole("gridcell", { name: "A26000001 urine" });
    expect(screen.getByText(/^Updated /)).toBeTruthy();
    await user.click(screen.getByRole("button", { name: "Refresh" }));
    await vi.waitFor(() => expect(items).toHaveBeenCalledTimes(2));
    expect(queues).toHaveBeenCalledTimes(2);
  });

  it("says how many wait when the list stops at the limit", async () => {
    const many = Array.from({ length: 200 }, (_, index) => ({
      ...ITEM,
      itemId: `00000000-0000-7000-8000-${String(index).padStart(12, "0")}`,
    }));
    renderPage(
      "/reports/release",
      () => many,
      () => [{ ...QUEUES[0], count: 500, countCapped: true }],
    );
    expect(await screen.findByText("200 shown · 500 or more items")).toBeTruthy();
  });

  it("loads later release work with a retained tuple cursor across equal timestamps", async () => {
    const head = Array.from({ length: 200 }, (_, index) => ({
      ...ITEM,
      itemId: `synthetic-${String(index).padStart(4, "0")}`,
      title: `Synthetic queue item ${index}`,
      hasMore: true,
    }));
    const tail = {
      ...ITEM,
      itemId: "synthetic-0200",
      title: "Last synthetic release item",
      hasMore: false,
    };
    const items = vi
      .fn()
      .mockReturnValueOnce({
        items: head,
        next: { since: ITEM.since, itemKind: ITEM.itemKind, itemId: "synthetic-0199" },
        scanned: 0,
      })
      .mockReturnValueOnce({ items: [tail], next: null, scanned: 0 });
    const { user } = renderPage("/reports/release", items, () => [
      { ...QUEUES[0], count: 201, countCapped: false },
    ]);
    await user.click(await screen.findByRole("button", { name: "Load more" }));
    expect(await screen.findByText("201 items")).toBeTruthy();
    expect(items).toHaveBeenLastCalledWith({
      input: {
        queue: "release_ready",
        limit: 200,
        after: { since: ITEM.since, itemKind: ITEM.itemKind, itemId: "synthetic-0199" },
      },
    });
    expect(screen.queryByRole("button", { name: "Load more" })).toBeNull();
  });

  it("keeps the chosen queue in the address and offers only release queues", async () => {
    const items = vi.fn(() => []);
    const { user, router } = renderPage("/reports/release", items);
    await screen.findByText("Nothing waiting");
    expect(screen.queryByRole("grid")).toBeNull();
    await user.click(screen.getByRole("button", { name: /Queue$/ }));
    expect(screen.queryByRole("option", { name: /Awaiting review/ })).toBeNull();
    await user.keyboard("{Escape}");
    await chooseWithKeyboard(user, /Queue$/, "Report release blocked");
    expect(router.state.location.search).toBe("?queue=release_blocked");
    expect(items).toHaveBeenLastCalledWith({
      input: { queue: "release_blocked", limit: 200, after: null },
    });
  });

  it("opens a partial report's revision, and marks a capped count", async () => {
    const partial: WorkQueueSummary = {
      queue: "partial_reports",
      label: "Partial reports outstanding",
      ownerCapability: "release_reports",
      count: 500,
      countCapped: true,
      oldestSince: "2026-10-12T10:00:00Z",
      nextAction: "Complete",
    };
    const revision: WorkQueueItem = {
      queue: "partial_reports",
      itemKind: "report_revision",
      itemId: REVISION_ID,
      title: "A26000001-URN",
      state: "partial_reports",
      stateLabel: "Partial report",
      since: "2026-10-12T10:00:00Z",
      nextAction: "Complete",
    };
    const { user } = renderPage(
      "/reports/release?queue=partial_reports",
      () => [revision],
      () => [partial],
    );
    expect(await screen.findByRole("button", { name: /Partial reports outstanding/ })).toBeTruthy();
    expect(await screen.findByText("1 shown · 500 or more items")).toBeTruthy();
    expect(await screen.findByRole("gridcell", { name: "Partial report" })).toBeTruthy();
    await openRow(user, "Partial report");
    expect(await screen.findByText(`Navigated to /reports/revisions/${REVISION_ID}`)).toBeTruthy();
  });

  it("says when the session has no release queue", async () => {
    renderPage(
      "/reports/release",
      () => [],
      () => [QUEUES[2]],
    );
    expect(await screen.findByText("No release queues")).toBeTruthy();
  });

  it("shows the engine's error", async () => {
    renderPage("/reports/release", () =>
      Promise.reject(engineError("internal", "The queue could not be read.")),
    );
    expect(await screen.findByText("The queue could not be read.")).toBeTruthy();
  });
});
